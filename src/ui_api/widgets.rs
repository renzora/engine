//! Widget definitions for the UI abstraction layer.
//!
//! Plugins define their UIs using these widget types. The editor's internal
//! renderer translates these to actual egui widgets.

use serde::{Deserialize, Serialize};

use super::types::{Align, Size, TextStyle, UiId};

/// All UI widgets plugins can create.
/// This enum represents the complete set of widgets available to plugins.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Widget {
    // === Basic Widgets ===
    /// Text label
    Label {
        text: String,
        style: TextStyle,
    },

    /// Clickable button
    Button {
        label: String,
        id: UiId,
        enabled: bool,
    },

    /// Icon button with tooltip
    IconButton {
        icon: String,
        tooltip: String,
        id: UiId,
        enabled: bool,
    },

    /// Single-line text input
    TextInput {
        value: String,
        placeholder: String,
        id: UiId,
    },

    /// Multi-line text editor
    TextEdit {
        value: String,
        id: UiId,
        min_lines: u32,
        max_lines: Option<u32>,
    },

    /// Boolean checkbox
    Checkbox {
        checked: bool,
        label: String,
        id: UiId,
    },

    /// Numeric slider
    Slider {
        value: f32,
        min: f32,
        max: f32,
        id: UiId,
        label: Option<String>,
    },

    /// Integer slider
    SliderInt {
        value: i32,
        min: i32,
        max: i32,
        id: UiId,
        label: Option<String>,
    },

    /// Dropdown selector
    Dropdown {
        selected: u32,
        options: Vec<String>,
        id: UiId,
    },

    /// Color picker
    ColorPicker {
        color: [f32; 4],
        id: UiId,
        alpha: bool,
    },

    /// Progress bar
    ProgressBar {
        progress: f32,
        label: Option<String>,
    },

    // === Layout Containers ===
    /// Horizontal row of widgets
    Row {
        children: Vec<Widget>,
        spacing: f32,
        align: Align,
    },

    /// Vertical column of widgets
    Column {
        children: Vec<Widget>,
        spacing: f32,
        align: Align,
    },

    /// Collapsible panel with title
    Panel {
        title: String,
        children: Vec<Widget>,
        collapsible: bool,
        default_open: bool,
    },

    /// Scrollable area
    ScrollArea {
        child: Box<Widget>,
        max_height: Option<f32>,
        horizontal: bool,
    },

    /// Group with optional frame
    Group {
        children: Vec<Widget>,
        frame: bool,
    },

    // === Complex Widgets ===
    /// Tree node with expandable children
    TreeNode {
        label: String,
        id: UiId,
        children: Vec<Widget>,
        expanded: bool,
        leaf: bool,
    },

    /// Data table
    Table {
        columns: Vec<TableColumn>,
        rows: Vec<TableRow>,
        id: UiId,
        striped: bool,
    },

    /// Tab container
    Tabs {
        tabs: Vec<Tab>,
        active: u32,
        id: UiId,
    },

    /// Horizontal separator line
    Separator,

    /// Spacing element
    Spacer {
        size: Size,
    },

    /// Image display
    Image {
        path: String,
        size: Option<[f32; 2]>,
    },

    // === Special ===
    /// Custom widget for plugin-specific rendering
    Custom {
        type_id: String,
        data: Vec<u8>,
    },

    /// Empty placeholder
    Empty,
}

impl Default for Widget {
    fn default() -> Self {
        Widget::Empty
    }
}

/// Tab definition for tab containers
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tab {
    pub label: String,
    pub icon: Option<String>,
    pub content: Vec<Widget>,
    pub closable: bool,
}

/// Table column definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TableColumn {
    pub header: String,
    pub width: Size,
    pub sortable: bool,
    pub resizable: bool,
}

impl Default for TableColumn {
    fn default() -> Self {
        Self {
            header: String::new(),
            width: Size::Auto,
            sortable: false,
            resizable: true,
        }
    }
}

/// Table row definition
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<Widget>,
    pub id: UiId,
}

// === Builder patterns for easier widget construction ===

impl Widget {
    /// Create a label widget
    pub fn label(text: impl Into<String>) -> Self {
        Widget::Label {
            text: text.into(),
            style: TextStyle::Body,
        }
    }

    /// Create a heading widget
    pub fn heading(text: impl Into<String>) -> Self {
        Widget::Label {
            text: text.into(),
            style: TextStyle::Heading1,
        }
    }

    /// Create a button widget
    pub fn button(label: impl Into<String>, id: UiId) -> Self {
        Widget::Button {
            label: label.into(),
            id,
            enabled: true,
        }
    }

    /// Create a text input widget
    pub fn text_input(value: impl Into<String>, id: UiId) -> Self {
        Widget::TextInput {
            value: value.into(),
            placeholder: String::new(),
            id,
        }
    }

    /// Create a checkbox widget
    pub fn checkbox(label: impl Into<String>, checked: bool, id: UiId) -> Self {
        Widget::Checkbox {
            checked,
            label: label.into(),
            id,
        }
    }

    /// Create a slider widget
    pub fn slider(value: f32, min: f32, max: f32, id: UiId) -> Self {
        Widget::Slider {
            value,
            min,
            max,
            id,
            label: None,
        }
    }

    /// Create a horizontal row
    pub fn row(children: Vec<Widget>) -> Self {
        Widget::Row {
            children,
            spacing: 4.0,
            align: Align::Start,
        }
    }

    /// Create a vertical column
    pub fn column(children: Vec<Widget>) -> Self {
        Widget::Column {
            children,
            spacing: 4.0,
            align: Align::Start,
        }
    }

    /// Create a collapsible panel
    pub fn panel(title: impl Into<String>, children: Vec<Widget>) -> Self {
        Widget::Panel {
            title: title.into(),
            children,
            collapsible: true,
            default_open: true,
        }
    }

    /// Create a separator
    pub fn separator() -> Self {
        Widget::Separator
    }

    /// Create a spacer
    pub fn spacer(size: Size) -> Self {
        Widget::Spacer { size }
    }
}
