//! UI types for plugin UI definitions.
//!
//! This module provides the abstract widget definitions that plugins use
//! to define their UI. The editor's internal renderer translates these
//! to actual egui calls.

/// Unique identifier for UI elements
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct UiId(pub u64);

impl UiId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Semantic text styles
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextStyle {
    #[default]
    Body,
    Heading1,
    Heading2,
    Heading3,
    Caption,
    Code,
    Label,
}

/// Size specification for layout
#[derive(Clone, Copy, Debug)]
pub enum Size {
    Auto,
    Fixed(f32),
    Percent(f32),
    Fill,
    FillPortion(u32),
}

impl Default for Size {
    fn default() -> Self {
        Self::Auto
    }
}

/// Alignment for layout
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

/// All UI widgets plugins can create
#[derive(Clone, Debug)]
pub enum Widget {
    /// Simple text label
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
        max_lines: u32,
    },
    /// Checkbox with label
    Checkbox {
        checked: bool,
        label: String,
        id: UiId,
    },
    /// Float slider
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
    /// Dropdown selection
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
    /// Horizontal layout container
    Row {
        children: Vec<Widget>,
        spacing: f32,
        align: Align,
    },
    /// Vertical layout container
    Column {
        children: Vec<Widget>,
        spacing: f32,
        align: Align,
    },
    /// Collapsible panel section
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
    /// Group of widgets
    Group {
        children: Vec<Widget>,
        frame: bool,
    },
    /// Tree node (collapsible hierarchy item)
    TreeNode {
        label: String,
        id: UiId,
        children: Vec<Widget>,
        expanded: bool,
        leaf: bool,
    },
    /// Table
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
    /// Visual separator
    Separator,
    /// Empty space
    Spacer {
        size: Size,
    },
    /// Image (path-based)
    Image {
        path: String,
        size: Option<[f32; 2]>,
    },
    /// Custom widget (plugin-specific)
    Custom {
        type_id: String,
        data: Vec<u8>,
    },
    /// Empty widget (no-op)
    Empty,
}

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
            style: TextStyle::Heading2,
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

    /// Create a horizontal row layout
    pub fn row(children: Vec<Widget>) -> Self {
        Widget::Row {
            children,
            spacing: 8.0,
            align: Align::Center,
        }
    }

    /// Create a vertical column layout
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

    /// Create a spacer with fixed size
    pub fn spacer(size: f32) -> Self {
        Widget::Spacer {
            size: Size::Fixed(size),
        }
    }
}

/// Tab definition
#[derive(Clone, Debug)]
pub struct Tab {
    pub label: String,
    pub icon: Option<String>,
    pub content: Vec<Widget>,
    pub closable: bool,
}

impl Tab {
    pub fn new(label: impl Into<String>, content: Vec<Widget>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            content,
            closable: false,
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
}

/// Table column definition
#[derive(Clone, Debug)]
pub struct TableColumn {
    pub header: String,
    pub width: Size,
    pub sortable: bool,
    pub resizable: bool,
}

impl TableColumn {
    pub fn new(header: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            width: Size::Auto,
            sortable: false,
            resizable: true,
        }
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }
}

/// Table row definition
#[derive(Clone, Debug)]
pub struct TableRow {
    pub cells: Vec<Widget>,
    pub id: UiId,
}

impl TableRow {
    pub fn new(cells: Vec<Widget>, id: UiId) -> Self {
        Self { cells, id }
    }
}
