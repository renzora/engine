//! Editor events that plugins can receive.

use crate::abi::EntityId;
use crate::ui::UiId;

/// UI events from plugin-registered widgets
#[derive(Clone, Debug)]
pub enum UiEvent {
    /// Button was clicked
    ButtonClicked(UiId),
    /// Checkbox was toggled
    CheckboxToggled { id: UiId, checked: bool },
    /// Slider value changed
    SliderChanged { id: UiId, value: f32 },
    /// Integer slider value changed
    SliderIntChanged { id: UiId, value: i32 },
    /// Text input changed
    TextInputChanged { id: UiId, value: String },
    /// Text input submitted (Enter pressed)
    TextInputSubmitted { id: UiId, value: String },
    /// Dropdown selection changed
    DropdownSelected { id: UiId, index: u32 },
    /// Color picker value changed
    ColorChanged { id: UiId, color: [f32; 4] },
    /// Tree node was toggled (expanded/collapsed)
    TreeNodeToggled { id: UiId, expanded: bool },
    /// Tree node was selected
    TreeNodeSelected(UiId),
    /// Tab was selected
    TabSelected { id: UiId, index: u32 },
    /// Tab was closed
    TabClosed { id: UiId, index: u32 },
    /// Table row was selected
    TableRowSelected { id: UiId, row: u32 },
    /// Table sort changed
    TableSortChanged { id: UiId, column: u32, ascending: bool },
    /// Custom event
    CustomEvent { type_id: String, data: Vec<u8> },
}

/// Editor events that plugins can receive
#[derive(Clone, Debug)]
pub enum EditorEvent {
    /// Entity was selected
    EntitySelected(EntityId),
    /// Entity was deselected
    EntityDeselected(EntityId),
    /// Scene was loaded
    SceneLoaded { path: String },
    /// Scene was saved
    SceneSaved { path: String },
    /// Play mode started
    PlayStarted,
    /// Play mode stopped
    PlayStopped,
    /// Project was opened
    ProjectOpened { path: String },
    /// Project was closed
    ProjectClosed,
    /// UI event from a plugin-registered widget
    UiEvent(UiEvent),
    /// Custom event from another plugin
    CustomEvent { plugin_id: String, event_type: String, data: Vec<u8> },
}

/// Event types for subscription
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EditorEventType {
    EntitySelected,
    EntityDeselected,
    SceneLoaded,
    SceneSaved,
    PlayStarted,
    PlayStopped,
    ProjectOpened,
    ProjectClosed,
    UiEvent,
    CustomEvent,
    All,
}
