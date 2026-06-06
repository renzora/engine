//! Shared blueprint-graph helpers for the native (ember) graph view.

/// Phosphor icon glyph for a blueprint node category. Used by the native graph
/// editor's "Add Node" category menu.
pub fn category_icon(category: &str) -> &'static str {
    match category {
        "Event" => egui_phosphor::regular::LIGHTNING,
        "Flow" => egui_phosphor::regular::FLOW_ARROW,
        "Math" => egui_phosphor::regular::CALCULATOR,
        "Transform" => egui_phosphor::regular::ARROWS_OUT_CARDINAL,
        "Input" => egui_phosphor::regular::KEYBOARD,
        "Entity" => egui_phosphor::regular::CUBE,
        "Component" => egui_phosphor::regular::PUZZLE_PIECE,
        "Physics" => egui_phosphor::regular::ATOM,
        "Audio" => egui_phosphor::regular::SPEAKER_HIGH,
        "UI" => egui_phosphor::regular::LAYOUT,
        "Scene" => egui_phosphor::regular::FILM_STRIP,
        "Debug" => egui_phosphor::regular::BUG,
        "Variable" => egui_phosphor::regular::DATABASE,
        "Rendering" => egui_phosphor::regular::EYE,
        "Animation" => egui_phosphor::regular::PLAY,
        _ => egui_phosphor::regular::CIRCLE,
    }
}
