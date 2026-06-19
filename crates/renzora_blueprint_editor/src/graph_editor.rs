//! Shared blueprint-graph helpers for the native (ember) graph view.

/// Phosphor icon *name* (kebab-case) for a blueprint node category. Used by the
/// native graph editor's "Add Node" category menu, where a downstream
/// `renzora_ember` name resolver (`icon_glyph`) turns it into a glyph.
pub fn category_icon(category: &str) -> &'static str {
    match category {
        "Event" => "lightning",
        "Flow" => "flow-arrow",
        "Math" => "calculator",
        "Transform" => "arrows-out-cardinal",
        "Input" => "keyboard",
        "Entity" => "cube",
        "Component" => "puzzle-piece",
        "Physics" => "atom",
        "Audio" => "speaker-high",
        "UI" => "layout",
        "Scene" => "film-slate",
        "Debug" => "bug",
        "Variable" => "database",
        "Rendering" => "eye",
        "Animation" => "play",
        _ => "circle",
    }
}
