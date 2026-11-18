//! A small, fast allocator for single-threaded environments (the UEFI
//! build), layered over a slower backing allocator.
//!
//! UEFI's pool allocator costs about 1.75 µs per allocate/free pair, and a
//! frame makes hundreds of small allocations. Here, blocks up to
//! `MAX_SMALL` bytes come from per-size-class free lists, carved from large
//! chunks taken from the backing allocator, so allocating and freeing is a
//! pointer push or pop. Larger blocks go straight to the backing allocator.
//! Small blocks are reused but never returned to the backing allocator.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;

/// Smallest block (also fits the free-list link).
const MIN_SMALL: usize = 16;
/// Largest block served from the size classes.
pub const MAX_SMALL: usize = 4096;
/// Size classes: 16, 32, ..., 4096 bytes.
const CLASSES: usize = 9;
/// Bytes requested from the backing allocator at a time.
const CHUNK: usize = 256 * 1024;

struct State {
    /// Head of each size class's free list; each free block stores the next.
    free: [*mut u8; CLASSES],
    /// Unused part of the current chunk.
    bump: usize,
    end: usize,
}

pub struct SizeClassHeap<B> {
    backing: B,
    state: UnsafeCell<State>,
}

// SAFETY: only for single-threaded use (UEFI boot services run one thread,
// and NowWrite registers no event callbacks that allocate).
unsafe impl<B> Sync for SizeClassHeap<B> {}

/// Size class index for `layout`, or `None` if it's too large.
fn class(layout: Layout) -> Option<usize> {
    let size = layout.size().max(layout.align()).max(MIN_SMALL);
    (size <= MAX_SMALL).then(|| (size.next_power_of_two().trailing_zeros() - MIN_SMALL.trailing_zeros()) as usize)
}

fn block_size(class: usize) -> usize {
    MIN_SMALL << class
}

impl<B: GlobalAlloc> SizeClassHeap<B> {
    pub const fn new(backing: B) -> Self {
        Self { backing, state: UnsafeCell::new(State { free: [ptr::null_mut(); CLASSES], bump: 0, end: 0 }) }
    }

    /// Carve a block of `class` from the current chunk, taking a new chunk
    /// if needed. Blocks are aligned to their size.
    unsafe fn carve(&self, state: &mut State, class: usize) -> *mut u8 {
        let size = block_size(class);
        let mut start = state.bump.next_multiple_of(size);
        if state.bump == 0 || start + size > state.end {
            // SAFETY: CHUNK and the alignment form a valid, non-zero layout.
            let chunk = unsafe { self.backing.alloc(Layout::from_size_align_unchecked(CHUNK, MAX_SMALL)) };
            if chunk.is_null() {
                return ptr::null_mut();
            }
            (start, state.end) = (chunk as usize, chunk as usize + CHUNK);
        }
        state.bump = start + size;
        start as *mut u8
    }
}

unsafe impl<B: GlobalAlloc> GlobalAlloc for SizeClassHeap<B> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(class) = class(layout) else {
            // SAFETY: forwarded unchanged.
            return unsafe { self.backing.alloc(layout) };
        };
        // SAFETY: single-threaded, and no reference to the state outlives
        // this call.
        let state = unsafe { &mut *self.state.get() };
        let head = state.free[class];
        if head.is_null() {
            // SAFETY: as above.
            unsafe { self.carve(state, class) }
        } else {
            // SAFETY: free blocks store the next free block's address.
            state.free[class] = unsafe { head.cast::<*mut u8>().read() };
            head
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        match class(layout) {
            Some(class) => {
                // SAFETY: single-threaded; the block is ours and at least
                // 16 bytes, aligned to its size.
                let state = unsafe { &mut *self.state.get() };
                unsafe { ptr.cast::<*mut u8>().write(state.free[class]) };
                state.free[class] = ptr;
            }
            // SAFETY: allocated by the backing allocator with this layout.
            None => unsafe { self.backing.dealloc(ptr, layout) },
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller guarantees `new_size` is valid for the alignment.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (class(layout), class(new_layout)) {
            // Still fits the same block.
            (Some(old), Some(new)) if old == new => ptr,
            // Both large: let the backing allocator handle it.
            // SAFETY: forwarded unchanged.
            (None, None) => unsafe { self.backing.realloc(ptr, layout, new_size) },
            _ => {
                // SAFETY: standard allocate-copy-free.
                let new = unsafe { self.alloc(new_layout) };
                if !new.is_null() {
                    unsafe {
                        ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                        self.dealloc(ptr, layout);
                    }
                }
                new
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use core::sync::atomic::{AtomicUsize, Ordering};
    use std::alloc::System;

    /// Counts calls reaching the backing allocator.
    struct Counting(AtomicUsize);
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            self.0.fetch_add(1, Ordering::Relaxed);
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    fn layout(size: usize, align: usize) -> Layout {
        Layout::from_size_align(size, align).unwrap()
    }

    #[test]
    fn size_classes() {
        assert_eq!(class(layout(1, 1)), Some(0));
        assert_eq!(class(layout(16, 8)), Some(0));
        assert_eq!(class(layout(17, 1)), Some(1));
        assert_eq!(class(layout(8, 64)), Some(2)); // alignment raises the class
        assert_eq!(class(layout(4096, 8)), Some(8));
        assert_eq!(class(layout(4097, 8)), None);
    }

    #[test]
    fn blocks_are_aligned_distinct_and_reused() {
        let heap = SizeClassHeap::new(Counting(AtomicUsize::new(0)));
        unsafe {
            let mut blocks = Vec::new();
            for (i, (size, align)) in [(1, 1), (24, 8), (100, 16), (8, 64), (3000, 8), (16, 16)].iter().cycle().take(600).enumerate() {
                let l = layout(*size, *align);
                let p = heap.alloc(l);
                assert!(!p.is_null());
                assert_eq!(p as usize % align, 0);
                p.write_bytes(i as u8, *size);
                blocks.push((p, l, i as u8));
            }
            // No block was handed out twice: every byte still holds its fill.
            for &(p, l, fill) in &blocks {
                assert!(core::slice::from_raw_parts(p, l.size()).iter().all(|&b| b == fill));
            }
            let chunks = heap.backing.0.load(Ordering::Relaxed);
            for &(p, l, _) in &blocks {
                heap.dealloc(p, l);
            }
            // Allocating the same again reuses freed blocks: no new chunks.
            for &(_, l, _) in &blocks {
                assert!(!heap.alloc(l).is_null());
            }
            assert_eq!(heap.backing.0.load(Ordering::Relaxed), chunks);
        }
    }

    #[test]
    fn large_blocks_and_realloc() {
        let heap = SizeClassHeap::new(Counting(AtomicUsize::new(0)));
        unsafe {
            let big = layout(100_000, 8);
            let p = heap.alloc(big);
            assert_eq!(heap.backing.0.load(Ordering::Relaxed), 1);
            heap.dealloc(p, big);

            // Growing within a class keeps the pointer; across classes copies.
            let l = layout(20, 1);
            let p = heap.alloc(l);
            p.copy_from(b"abcdefghijklmnopqrst".as_ptr(), 20);
            assert_eq!(heap.realloc(p, l, 30), p);
            let q = heap.realloc(p, layout(30, 1), 5000);
            assert_eq!(core::slice::from_raw_parts(q, 20), b"abcdefghijklmnopqrst");
            let r = heap.realloc(q, layout(5000, 1), 10);
            assert_eq!(core::slice::from_raw_parts(r, 10), b"abcdefghij");
            heap.dealloc(r, layout(10, 1));
        }
    }
}
