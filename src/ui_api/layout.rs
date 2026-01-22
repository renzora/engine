//! Layout types for UI construction.
//!
//! These types define how widgets are positioned and sized within their containers.

use super::types::{CrossAlign, JustifyContent};

/// Padding specification for containers
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Create uniform padding on all sides
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create symmetric padding (vertical, horizontal)
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Create padding with individual sides
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

/// Margin specification for widgets
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[repr(C)]
pub struct Margin {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Margin {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// Create uniform margin on all sides
    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Create symmetric margin (vertical, horizontal)
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    /// Create margin with individual sides
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

/// Constraints for widget sizing
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SizeConstraints {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
}

impl Default for SizeConstraints {
    fn default() -> Self {
        Self {
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
        }
    }
}

impl SizeConstraints {
    /// No constraints
    pub const NONE: Self = Self {
        min_width: None,
        max_width: None,
        min_height: None,
        max_height: None,
    };

    /// Set minimum width
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Set maximum width
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set minimum height
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height);
        self
    }

    /// Set maximum height
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }
}

/// Layout style for flex containers
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct FlexLayout {
    /// Main axis alignment
    pub justify: JustifyContent,
    /// Cross axis alignment
    pub align: CrossAlign,
    /// Gap between items
    pub gap: f32,
    /// Allow wrapping
    pub wrap: bool,
}

impl FlexLayout {
    /// Create a default horizontal flex layout
    pub const fn row() -> Self {
        Self {
            justify: JustifyContent::Start,
            align: CrossAlign::Center,
            gap: 4.0,
            wrap: false,
        }
    }

    /// Create a default vertical flex layout
    pub const fn column() -> Self {
        Self {
            justify: JustifyContent::Start,
            align: CrossAlign::Stretch,
            gap: 4.0,
            wrap: false,
        }
    }

    /// Set gap between items
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Set main axis alignment
    pub fn justify(mut self, justify: JustifyContent) -> Self {
        self.justify = justify;
        self
    }

    /// Set cross axis alignment
    pub fn align(mut self, align: CrossAlign) -> Self {
        self.align = align;
        self
    }

    /// Enable wrapping
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }
}

/// Anchor point for positioning
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Position specification (for absolute positioning)
#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(C)]
pub struct Position {
    pub anchor: Anchor,
    pub offset_x: f32,
    pub offset_y: f32,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopLeft,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

impl Position {
    /// Create a position at the top-left corner
    pub const fn top_left(x: f32, y: f32) -> Self {
        Self {
            anchor: Anchor::TopLeft,
            offset_x: x,
            offset_y: y,
        }
    }

    /// Create a position at the center
    pub const fn center() -> Self {
        Self {
            anchor: Anchor::Center,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}
