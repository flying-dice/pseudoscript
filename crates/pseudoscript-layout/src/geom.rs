//! Projection-agnostic geometry primitives shared by every layout engine.

use serde::{Deserialize, Serialize};

/// A point in renderer coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// A width/height extent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Size {
    pub w: i32,
    pub h: i32,
}

/// An axis-aligned rectangle in renderer coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    #[must_use]
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    /// The right edge (`x + w`).
    #[must_use]
    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    /// The bottom edge (`y + h`).
    #[must_use]
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    /// Whether `p` lies within the rectangle (edges inclusive).
    #[must_use]
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }
}

/// A min/max accumulator that grows to enclose the points and rectangles fed to
/// it — the standard "bounds" helper a layout engine uses to size a container
/// (a sequence fragment, a C4 boundary, a flowchart subgraph) around its
/// contents. Empty until the first insertion.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bounds {
    extent: Option<(i32, i32, i32, i32)>, // (min_x, min_y, max_x, max_y)
}

impl Bounds {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether nothing has been inserted yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.extent.is_none()
    }

    /// Grow to include the point `(x, y)`.
    pub fn include(&mut self, x: i32, y: i32) {
        self.extent = Some(match self.extent {
            None => (x, y, x, y),
            Some((min_x, min_y, max_x, max_y)) => {
                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
            }
        });
    }

    /// Grow to include every corner of `r`.
    pub fn include_rect(&mut self, r: Rect) {
        self.include(r.x, r.y);
        self.include(r.right(), r.bottom());
    }

    /// The enclosing rectangle, or `None` if empty.
    #[must_use]
    pub fn rect(&self) -> Option<Rect> {
        self.extent.map(|(min_x, min_y, max_x, max_y)| {
            Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
        })
    }

    /// The enclosing rectangle grown by `pad` on every side, or `None` if empty.
    #[must_use]
    pub fn padded(&self, pad: i32) -> Option<Rect> {
        self.rect()
            .map(|r| Rect::new(r.x - pad, r.y - pad, r.w + 2 * pad, r.h + 2 * pad))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_grows_to_enclose() {
        let mut b = Bounds::new();
        assert!(b.is_empty());
        b.include(10, 20);
        b.include(30, 5);
        assert_eq!(b.rect(), Some(Rect::new(10, 5, 20, 15)));
        assert_eq!(b.padded(2), Some(Rect::new(8, 3, 24, 19)));
    }

    #[test]
    fn rect_edges_and_contains() {
        let r = Rect::new(0, 0, 10, 10);
        assert_eq!(r.right(), 10);
        assert_eq!(r.bottom(), 10);
        assert!(r.contains(Point { x: 5, y: 5 }));
        assert!(!r.contains(Point { x: 11, y: 5 }));
    }
}
