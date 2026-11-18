//! Box drawing (`─│┌┐└┘├┤┬┴┼═║╔╗╚╝╭╮╰╯`) and block elements (`█▀▄▌▐░▒▓`)
//! drawn procedurally to fit the cell, so lines join across cells even when
//! the cell is taller than the font's own glyphs.

/// Arms of a line character: how far each side extends (0 none, 1 single,
/// 2 double), in the order up, down, left, right.
fn arms(ch: char) -> Option<[u8; 4]> {
    Some(match ch {
        '─' => [0, 0, 1, 1],
        '│' => [1, 1, 0, 0],
        '┌' => [0, 1, 0, 1],
        '┐' => [0, 1, 1, 0],
        '└' => [1, 0, 0, 1],
        '┘' => [1, 0, 1, 0],
        '├' => [1, 1, 0, 1],
        '┤' => [1, 1, 1, 0],
        '┬' => [0, 1, 1, 1],
        '┴' => [1, 0, 1, 1],
        '┼' => [1, 1, 1, 1],
        '═' => [0, 0, 2, 2],
        '║' => [2, 2, 0, 0],
        '╔' => [0, 2, 0, 2],
        '╗' => [0, 2, 2, 0],
        '╚' => [2, 0, 0, 2],
        '╝' => [2, 0, 2, 0],
        _ => return None,
    })
}

/// Line thickness in pixels for a font of `px` pixels.
pub fn thickness(px: u16) -> usize {
    if px >= 24 { 2 } else { 1 }
}

/// A `w` x `h` coverage canvas (0 = background, 255 = ink).
struct Canvas<'a> {
    out: &'a mut [u8],
    w: usize,
    h: usize,
}

impl Canvas<'_> {
    /// Fill `x0..x1` x `y0..y1`, clipped to the cell.
    fn rect(&mut self, x0: isize, y0: isize, x1: isize, y1: isize, value: u8) {
        let clamp = |v: isize, max: usize| v.clamp(0, max as isize) as usize;
        let (x0, x1) = (clamp(x0, self.w), clamp(x1, self.w));
        let (y0, y1) = (clamp(y0, self.h), clamp(y1, self.h));
        for y in y0..y1 {
            for v in &mut self.out[y * self.w + x0..y * self.w + x1] {
                *v = (*v).max(value);
            }
        }
    }
}

/// Render `ch` into `out` (`w * h` bytes, cleared first). Returns false if
/// `ch` isn't drawn procedurally; `out` is then left cleared.
pub fn render(ch: char, w: usize, h: usize, thick: usize, out: &mut [u8]) -> bool {
    let out = &mut out[..w * h];
    out.fill(0);
    let mut c = Canvas { out, w, h };
    let (wi, hi) = (w as isize, h as isize);
    let t = thick as isize;
    // Top-left of the centre stroke, shared by every line character so
    // neighbouring cells meet.
    let cx = (wi - t) / 2;
    let cy = (hi - t) / 2;

    match ch {
        '█' => c.rect(0, 0, wi, hi, 255),
        '▀' => c.rect(0, 0, wi, hi / 2, 255),
        '▄' => c.rect(0, hi / 2, wi, hi, 255),
        '▌' => c.rect(0, 0, wi / 2, hi, 255),
        '▐' => c.rect(wi / 2, 0, wi, hi, 255),
        '░' => c.rect(0, 0, wi, hi, 64),
        '▒' => c.rect(0, 0, wi, hi, 128),
        '▓' => c.rect(0, 0, wi, hi, 192),
        '╭' | '╮' | '╰' | '╯' => {
            let hd = if matches!(ch, '╭' | '╰') { 1 } else { -1 };
            let vd = if matches!(ch, '╭' | '╮') { 1 } else { -1 };
            arc(&mut c, cx, cy, t, hd, vd);
        }
        _ => {
            let Some([up, down, left, right]) = arms(ch) else { return false };
            if up.max(down).max(left).max(right) == 2 {
                double(&mut c, cx, cy, t, [up, down, left, right]);
            } else {
                // Each arm runs from the edge through the centre square.
                if up > 0 {
                    c.rect(cx, 0, cx + t, cy + t, 255);
                }
                if down > 0 {
                    c.rect(cx, cy, cx + t, hi, 255);
                }
                if left > 0 {
                    c.rect(0, cy, cx + t, cy + t, 255);
                }
                if right > 0 {
                    c.rect(cx, cy, wi, cy + t, 255);
                }
            }
        }
    }
    true
}

/// Double lines: two strokes `s` pixels either side of the single-line
/// position. Corners join the outer strokes with each other, and the inner
/// strokes with each other.
fn double(c: &mut Canvas, cx: isize, cy: isize, t: isize, [up, down, left, right]: [u8; 4]) {
    let (wi, hi) = (c.w as isize, c.h as isize);
    let s = t * 2;
    let horizontal = left > 0 || right > 0;
    let vertical = up > 0 || down > 0;
    if horizontal && !vertical {
        c.rect(0, cy - s, wi, cy - s + t, 255);
        c.rect(0, cy + s, wi, cy + s + t, 255);
        return;
    }
    if vertical && !horizontal {
        c.rect(cx - s, 0, cx - s + t, hi, 255);
        c.rect(cx + s, 0, cx + s + t, hi, 255);
        return;
    }
    // A corner: hd/vd point along its arms.
    let hd = if right > 0 { 1 } else { -1 };
    let vd = if down > 0 { 1 } else { -1 };
    for (offset, sign) in [(s, -1), (s, 1)] {
        // sign -1: outer pair, +1: inner pair.
        let y = cy + sign * offset * vd;
        let x = cx + sign * offset * hd;
        // Horizontal stroke from the joint to the edge in direction hd.
        let (hx0, hx1) = if hd > 0 { (x, wi) } else { (0, x + t) };
        c.rect(hx0, y, hx1, y + t, 255);
        // Vertical stroke from the joint to the edge in direction vd.
        let (vy0, vy1) = if vd > 0 { (y, hi) } else { (0, y + t) };
        c.rect(x, vy0, x + t, vy1, 255);
    }
}

/// A rounded corner: a quarter-circle arc joining the centre lines, with
/// straight stubs to the cell edges. Anti-aliased by 4x4 supersampling.
fn arc(c: &mut Canvas, cx: isize, cy: isize, t: isize, hd: isize, vd: isize) {
    let (wi, hi) = (c.w as isize, c.h as isize);
    // Centre line of the strokes, in pixels (as f32 for the arc).
    let lx = cx as f32 + t as f32 / 2.0;
    let ly = cy as f32 + t as f32 / 2.0;
    let room_x = if hd > 0 { wi as f32 - lx } else { lx };
    let room_y = if vd > 0 { hi as f32 - ly } else { ly };
    let r = room_x.min(room_y).max(1.0);
    let (ax, ay) = (lx + hd as f32 * r, ly + vd as f32 * r); // arc centre

    // Straight stubs beyond the arc's ends.
    let end_x = ax as isize;
    let end_y = ay as isize;
    if hd > 0 { c.rect(end_x, cy, wi, cy + t, 255) } else { c.rect(0, cy, end_x, cy + t, 255) }
    if vd > 0 { c.rect(cx, end_y, cx + t, hi, 255) } else { c.rect(cx, 0, cx + t, end_y, 255) }

    let half = t as f32 / 2.0;
    let (inner, outer) = ((r - half).max(0.0), r + half);
    let (inner2, outer2) = (inner * inner, outer * outer);
    for y in 0..c.h {
        for x in 0..c.w {
            let mut hits = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let px = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let py = y as f32 + (sy as f32 + 0.5) / 4.0;
                    // Only the quadrant facing the corner.
                    if (px - ax) * hd as f32 > 0.0 || (py - ay) * vd as f32 > 0.0 {
                        continue;
                    }
                    let d2 = (px - ax) * (px - ax) + (py - ay) * (py - ay);
                    if d2 >= inner2 && d2 <= outer2 {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let v = &mut c.out[y * c.w + x];
                *v = (*v).max((hits * 255 / 16) as u8);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    const W: usize = 10;
    const H: usize = 22;

    fn draw(ch: char, thick: usize) -> Vec<u8> {
        let mut out = vec![0; W * H];
        assert!(render(ch, W, H, thick, &mut out), "{ch}");
        out
    }

    /// Inked positions along one edge of the cell.
    fn edge(cell: &[u8], side: char) -> Vec<usize> {
        let ink = |x: usize, y: usize| cell[y * W + x] > 127;
        match side {
            't' => (0..W).filter(|&x| ink(x, 0)).collect(),
            'b' => (0..W).filter(|&x| ink(x, H - 1)).collect(),
            'l' => (0..H).filter(|&y| ink(0, y)).collect(),
            'r' => (0..H).filter(|&y| ink(W - 1, y)).collect(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn single_lines_meet_on_cell_edges() {
        for thick in [1, 2] {
            let h = draw('─', thick);
            let v = draw('│', thick);
            let row = edge(&h, 'l');
            let col = edge(&v, 't');
            assert_eq!(row.len(), thick);
            assert_eq!(col.len(), thick);
            assert_eq!(edge(&h, 'r'), row);
            assert_eq!(edge(&v, 'b'), col);
            // Every piece with an arm on a side touches that side exactly
            // where the straight lines do.
            for (ch, sides) in [
                ('┌', "rb"), ('┐', "lb"), ('└', "rt"), ('┘', "lt"),
                ('├', "tbr"), ('┤', "tbl"), ('┬', "lrb"), ('┴', "lrt"), ('┼', "tblr"),
                ('╭', "rb"), ('╮', "lb"), ('╰', "rt"), ('╯', "lt"),
            ] {
                let cell = draw(ch, thick);
                for side in ['t', 'b', 'l', 'r'] {
                    let expected = match side {
                        _ if !sides.contains(side) => vec![],
                        't' | 'b' => col.clone(),
                        _ => row.clone(),
                    };
                    assert_eq!(edge(&cell, side), expected, "{ch} side {side} thick {thick}");
                }
            }
        }
    }

    #[test]
    fn double_lines_meet_on_cell_edges() {
        let h = draw('═', 1);
        let v = draw('║', 1);
        assert_eq!(edge(&h, 'l').len(), 2);
        assert_eq!(edge(&v, 't').len(), 2);
        for (ch, sides) in [('╔', "rb"), ('╗', "lb"), ('╚', "rt"), ('╝', "lt")] {
            let cell = draw(ch, 1);
            for side in sides.chars() {
                let straight = if matches!(side, 't' | 'b') { edge(&v, 't') } else { edge(&h, 'l') };
                assert_eq!(edge(&cell, side), straight, "{ch} side {side}");
            }
        }
    }

    #[test]
    fn blocks_and_shades() {
        assert!(draw('█', 1).iter().all(|&v| v == 255));
        let top = draw('▀', 1);
        assert_eq!(top[0], 255);
        assert_eq!(top[(H - 1) * W], 0);
        let right = draw('▐', 1);
        assert_eq!((right[0], right[W - 1]), (0, 255));
        assert!(draw('▒', 1).iter().all(|&v| v == 128));
    }

    #[test]
    fn other_chars_are_not_drawn() {
        let mut out = vec![7; W * H];
        assert!(!render('a', W, H, 1, &mut out));
        assert!(out.iter().all(|&v| v == 0));
    }
}
