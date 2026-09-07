//! Edit chrome. Every control is one rectangle: the pixels we draw are the hit.

use smithay::utils::{Logical, Point, Rectangle, Size};

pub const HANDLE_W: i32 = 24;
pub const HANDLE_H: i32 = 16;
pub const CLOSE: i32 = 20;
pub const BORDER: i32 = 2;

const CLOSE_FILL: [f32; 4] = [0.90, 0.16, 0.14, 1.0];
const HANDLE_FILL: [f32; 4] = [0.20, 0.72, 0.95, 1.0];
const KNOB_EDGE: [f32; 4] = [0.04, 0.04, 0.04, 1.0];
const BORDER_FILL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const HOLE_BORDER: [f32; 4] = [1.0, 1.0, 1.0, 0.70];
const SHADOW_FILL: [f32; 4] = [0.0, 0.0, 0.0, 0.40];
const HOLE_BORDER_W: i32 = 2;
const IDLE_BORDER: i32 = 2;
const IDLE_BORDER_FILL: [f32; 4] = [1.0, 1.0, 1.0, 0.40];
const DOT: i32 = 5;
const DOT_FILL: [f32; 4] = [1.0, 1.0, 1.0, 0.45];

/// Axis-aligned fill. Submitted in painter order (later draws on top).
#[derive(Clone, Copy, Debug)]
pub struct Fill {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
pub enum Handle {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Handle {
    pub const ALL: [Self; 8] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
    ];

    pub fn west(self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    pub fn east(self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    pub fn north(self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }

    pub fn south(self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }
}

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
    Rectangle::new(Point::from((x, y)), Size::from((w, h)))
}

pub(crate) fn fill(r: Rectangle<i32, Logical>, color: [f32; 4]) -> Fill {
    Fill {
        x: r.loc.x as f32,
        y: r.loc.y as f32,
        w: r.size.w as f32,
        h: r.size.h as f32,
        color,
    }
}

fn contains(r: Rectangle<i32, Logical>, pos: Point<f64, Logical>) -> bool {
    let x0 = r.loc.x as f64;
    let y0 = r.loc.y as f64;
    pos.x >= x0
        && pos.x < x0 + r.size.w as f64
        && pos.y >= y0
        && pos.y < y0 + r.size.h as f64
}

/// Point on the frame that this handle owns. Resize keeps `pointer - anchor`
/// constant so the grabbed pixel of the handle stays under the cursor.
pub fn handle_anchor(tile: Rectangle<i32, Logical>, handle: Handle) -> Point<f64, Logical> {
    let x = tile.loc.x as f64;
    let y = tile.loc.y as f64;
    let w = tile.size.w as f64;
    let h = tile.size.h as f64;
    match handle {
        Handle::Left => (x, y + h * 0.5).into(),
        Handle::Right => (x + w, y + h * 0.5).into(),
        Handle::Top => (x + w * 0.5, y).into(),
        Handle::Bottom => (x + w * 0.5, y + h).into(),
        Handle::TopLeft => (x, y).into(),
        Handle::TopRight => (x + w, y).into(),
        Handle::BottomLeft => (x, y + h).into(),
        Handle::BottomRight => (x + w, y + h).into(),
    }
}

fn handle_size(handle: Handle) -> (i32, i32) {
    match handle {
        Handle::Left | Handle::Right => (HANDLE_H, HANDLE_W),
        Handle::Top | Handle::Bottom => (HANDLE_W, HANDLE_H),
        _ => (HANDLE_W, HANDLE_W),
    }
}

pub fn handle_rect(tile: Rectangle<i32, Logical>, handle: Handle) -> Rectangle<i32, Logical> {
    let a = handle_anchor(tile, handle);
    let (w, h) = handle_size(handle);
    rect(
        a.x.round() as i32 - w / 2,
        a.y.round() as i32 - h / 2,
        w,
        h,
    )
}

pub fn close_rect(tile: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
    rect(
        tile.loc.x + tile.size.w - CLOSE - BORDER - 6,
        tile.loc.y + BORDER + 6,
        CLOSE,
        CLOSE,
    )
}

pub fn hit_close(tile: Rectangle<i32, Logical>, pos: Point<f64, Logical>) -> bool {
    contains(close_rect(tile), pos)
}

pub fn hit_handle(tile: Rectangle<i32, Logical>, pos: Point<f64, Logical>) -> Option<Handle> {
    Handle::ALL
        .into_iter()
        .find(|&handle| contains(handle_rect(tile, handle), pos))
}

/// Dark 1px ring inside `r`, fill on top. Hit target is `r` itself.
fn knob(r: Rectangle<i32, Logical>, color: [f32; 4]) -> [Fill; 2] {
    let inner = rect(r.loc.x + 1, r.loc.y + 1, r.size.w - 2, r.size.h - 2);
    [fill(r, KNOB_EDGE), fill(inner, color)]
}

pub fn grid_dot(center: Point<i32, Logical>) -> Fill {
    let half = DOT / 2;
    fill(
        rect(center.x - half, center.y - half, DOT, DOT),
        DOT_FILL,
    )
}

pub fn hole_overlay(tile: Rectangle<i32, Logical>) -> Vec<Fill> {
    outside_strips(tile, HOLE_BORDER_W)
        .into_iter()
        .map(|strip| fill(strip, HOLE_BORDER))
        .collect()
}

pub fn lift_overlay(tile: Rectangle<i32, Logical>) -> Vec<Fill> {
    let mut out = Vec::new();
    out.push(fill(
        rect(tile.loc.x + 6, tile.loc.y + 10, tile.size.w, tile.size.h),
        SHADOW_FILL,
    ));
    out.extend(frame(tile, false, false));
    out
}

pub fn idle_border(tile: Rectangle<i32, Logical>) -> Vec<Fill> {
    outside_strips(tile, IDLE_BORDER)
        .into_iter()
        .map(|strip| fill(strip, IDLE_BORDER_FILL))
        .collect()
}

pub fn edit_overlay(tile: Rectangle<i32, Logical>, with_handles: bool) -> Vec<Fill> {
    frame(tile, with_handles, true)
}

fn frame(tile: Rectangle<i32, Logical>, with_handles: bool, with_close: bool) -> Vec<Fill> {
    let mut out = Vec::new();
    for strip in outside_strips(tile, BORDER) {
        out.push(fill(strip, BORDER_FILL));
    }
    if with_handles {
        for handle in Handle::ALL {
            out.extend(knob(handle_rect(tile, handle), HANDLE_FILL));
        }
    }
    if with_close {
        out.extend(knob(close_rect(tile), CLOSE_FILL));
    }
    out
}

fn outside_strips(r: Rectangle<i32, Logical>, t: i32) -> [Rectangle<i32, Logical>; 4] {
    [
        rect(r.loc.x - t, r.loc.y - t, r.size.w + t * 2, t),
        rect(r.loc.x - t, r.loc.y + r.size.h, r.size.w + t * 2, t),
        rect(r.loc.x - t, r.loc.y, t, r.size.h),
        rect(r.loc.x + r.size.w, r.loc.y, t, r.size.h),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile() -> Rectangle<i32, Logical> {
        rect(16, 536, 300, 114)
    }

    #[test]
    fn close_is_a_small_square_on_the_top_right() {
        let c = close_rect(tile());
        assert_eq!(c.size.w, CLOSE);
        assert_eq!(c.size.h, CLOSE);
        assert!(c.loc.x > tile().loc.x + tile().size.w / 2);
        assert!(c.loc.y < tile().loc.y + tile().size.h / 2);
        assert!(c.loc.x + c.size.w <= tile().loc.x + tile().size.w);
    }

    #[test]
    fn handle_rect_is_centered_on_the_anchor() {
        let t = tile();
        for handle in Handle::ALL {
            let a = handle_anchor(t, handle);
            let r = handle_rect(t, handle);
            let (w, h) = handle_size(handle);
            assert_eq!(r.size.w, w);
            assert_eq!(r.size.h, h);
            let cx = r.loc.x as f64 + r.size.w as f64 / 2.0;
            let cy = r.loc.y as f64 + r.size.h as f64 / 2.0;
            assert!((cx - a.x).abs() < 1.0);
            assert!((cy - a.y).abs() < 1.0);
            assert!(hit_handle(t, a).is_some());
        }
    }
}
