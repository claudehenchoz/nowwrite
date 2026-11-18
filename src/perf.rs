//! Optional timing of the main loop (cargo feature `perf`). Without the
//! feature every function here is an empty inline no-op.
//!
//! With it, each phase records its time in CPU timestamp-counter cycles,
//! and `report()` summarises count / average / max per phase. The editor
//! writes the report to `.nowwrite/perf.log` when it exits.

/// Parts of one loop iteration that are timed.
#[derive(Debug, Clone, Copy)]
pub enum Phase {
    /// Handling one key (editing, movement, modals).
    Key = 0,
    /// Building the frame: layout and widgets (inside `terminal.draw`).
    Render,
    /// The backend turning changed cells into pixels or console output.
    BackendDraw,
    /// The backend copying changed pixels to the screen.
    Flush,
    /// One whole `terminal.draw` (render + diff + backend draw + flush).
    Frame,
    /// The periodic backup check.
    Backup,
    /// The document area (part of `Render`).
    View,
    /// The status bar (part of `Render`).
    StatusBar,
}

#[cfg(feature = "perf")]
mod imp {
    use super::Phase;
    use alloc::format;
    use alloc::string::String;
    use core::sync::atomic::{AtomicU64, Ordering::Relaxed};

    const PHASES: usize = 8;
    const NAMES: [&str; PHASES] = ["key", "render", "backend draw", "flush", "frame", "backup", " view", " status bar"];
    static TOTAL: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];
    static COUNT: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];
    static MAX: [AtomicU64; PHASES] = [const { AtomicU64::new(0) }; PHASES];
    /// Timestamp-counter ticks per microsecond, if the platform measured it.
    static TICKS_PER_US: AtomicU64 = AtomicU64::new(0);

    #[inline]
    pub fn now() -> u64 {
        #[cfg(target_arch = "x86_64")]
        // SAFETY: rdtsc has no preconditions on x86_64.
        unsafe {
            core::arch::x86_64::_rdtsc()
        }
        #[cfg(not(target_arch = "x86_64"))]
        0
    }

    pub fn record(phase: Phase, start: u64) {
        let ticks = now().wrapping_sub(start);
        let i = phase as usize;
        TOTAL[i].fetch_add(ticks, Relaxed);
        COUNT[i].fetch_add(1, Relaxed);
        MAX[i].fetch_max(ticks, Relaxed);
    }

    pub fn set_ticks_per_us(ticks: u64) {
        TICKS_PER_US.store(ticks, Relaxed);
    }

    pub fn report() -> String {
        let per_us = TICKS_PER_US.load(Relaxed);
        let mut out = format!("tsc ticks/us: {per_us}\n");
        out.push_str("phase          count      avg        max\n");
        for (i, name) in NAMES.iter().enumerate() {
            let count = COUNT[i].load(Relaxed);
            if count == 0 {
                continue;
            }
            let (avg, max) = (TOTAL[i].load(Relaxed) / count, MAX[i].load(Relaxed));
            let fmt = |t: u64| if per_us > 0 { format!("{:.1}us", t as f64 / per_us as f64) } else { format!("{t}cyc") };
            out.push_str(&format!("{name:<14} {count:>5} {:>10} {:>10}\n", fmt(avg), fmt(max)));
        }
        out
    }
}

#[cfg(feature = "perf")]
pub use imp::{now, record, report, set_ticks_per_us};

#[cfg(not(feature = "perf"))]
#[inline(always)]
pub fn now() -> u64 {
    0
}

#[cfg(not(feature = "perf"))]
#[inline(always)]
pub fn record(_phase: Phase, _start: u64) {}

#[cfg(not(feature = "perf"))]
#[inline(always)]
pub fn set_ticks_per_us(_ticks: u64) {}

