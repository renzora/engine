//! Editor integration for HTML templates: make them creatable from the
//! hierarchy's "+ Add Entity" overlay, identifiable in the tree, and editable
//! (template path) in the inspector.
//!
//! Dragging/positioning needs no work here: `spawn_html_template_at` creates the
//! instance as an absolutely-positioned `UiWidget`, which the existing canvas
//! editor selects and drags like any other widget, and `renzora_hui`'s observer
//! keeps the actual markup under a child `HtmlNode` so bevy_hui never resets the
//! instance's position. So this module is just editor registrations.

use bevy::prelude::*;
use bevy_hui::prelude::Tags;
use renzora::{
    AppEditorExt, ComponentIconEntry, FieldDef, FieldType, FieldValue, InspectorEntry,
};
use renzora_ember::game_ui::{UiCanvas, UiWidget};

use renzora_ember::markup::HtmlTemplatePath;

pub struct HuiEditorPlugin;

impl Plugin for HuiEditorPlugin {
    fn build(&self, app: &mut App) {
        register_editor_entries(app);
        app.add_systems(Update, tag_built_nodes);
    }
}

/// As each bevy_hui node is built (`Tags` is inserted on every node, including
/// the markup root that lands on the `HtmlNode` child), tag it as a `UiWidget`
/// so the canvas editor's hit-test finds it. The canvas selects/drags the
/// visible markup, not the transparent instance overlay — clicks land on the
/// real widget, transparent gaps fall through. Hot-reload safe: bevy_hui
/// re-inserts `Tags` on rebuild, re-firing `Added<Tags>`.
///
/// Insertion ordering: bevy_hui sets `ChildOf` before `Tags`, so when game_ui's
/// reparent observers fire there's no `UiWidget` on the node yet (they no-op),
/// and by the time we add `UiWidget` here there's no `Changed<ChildOf>` — so
/// `apply_parent_layout` never overwrites bevy_hui's Node. No explicit
/// exemption marker needed.
fn tag_built_nodes(
    mut commands: Commands,
    built: Query<Entity, Added<Tags>>,
) {
    for entity in &built {
        commands.entity(entity).insert(UiWidget::default());
    }
}

fn register_editor_entries(app: &mut App) {
    // The "HTML Template" preset was here: "+ Add Entity" → UI → HTML Template
    // spawned a `UiWidget` instance under a canvas, carrying its own
    // `HtmlTemplatePath`. It was a second kind of template holder — one that
    // was not a canvas, had no reference resolution or render space, and was
    // wiped by the next rebuild of the canvas it sat under.
    //
    // A template belongs to a UI Canvas and nothing else. The one entity you add
    // is the canvas; the template goes in its slot.
    //
    // World-space UI is no longer a separate entity — it's a `UiCanvas` with its
    // `render_space` set to "world" (see the canvas inspector). So there's no
    // "World UI Panel" preset; you add a UI Canvas and flip it to world space.

    // Distinctive icon + type label in the hierarchy tree.
    app.register_component_icon(ComponentIconEntry {
        type_id: std::any::TypeId::of::<HtmlTemplatePath>(),
        name: "HTML Template",
        icon: "code",
        color: [120, 170, 220],
        priority: 96,
        dynamic_icon_fn: None,
    });

    // Per-markup-node icons. Every node built from `.html` is tagged with
    // `UiWidget::default()` (priority 60, Container icon) by `tag_built_nodes`,
    // so without these the hierarchy is a wall of identical Container icons.
    // Priorities sit *above* UiWidget(60) and *below* HtmlTemplatePath(96) so
    // the template root keeps its CODE icon while children get type-specific
    // ones.
    app.register_component_icon(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Text>(),
        name: "UI Text",
        icon: "text-aa",
        color: [220, 220, 220],
        priority: 80,
        dynamic_icon_fn: None,
    });
    app.register_component_icon(ComponentIconEntry {
        type_id: std::any::TypeId::of::<Button>(),
        name: "UI Button",
        icon: "cursor-click",
        color: [180, 200, 255],
        priority: 82,
        dynamic_icon_fn: None,
    });
    app.register_component_icon(ComponentIconEntry {
        type_id: std::any::TypeId::of::<ImageNode>(),
        name: "UI Image",
        icon: "image",
        color: [180, 220, 130],
        priority: 80,
        dynamic_icon_fn: None,
    });

    // World-space UI settings now live on the `UiCanvas` inspector (render_space
    // + render_mode) — see `renzora_ember_editor::game_ui::register`. No separate
    // World UI Panel inspector.

    // Inspector: the template slot on a UI Canvas. Pick an existing `.html` or
    // create one in place with "+".
    //
    // **A canvas is the only thing that can hold a template**, and it holds it
    // for its whole life. That is the entity's entire purpose in the scene:
    // "this template appears here, at this reference resolution, in this render
    // space". Which is why all three of the usual controls are gone:
    //
    // - `has_fn` answers for any canvas, template or not, so the slot is visible
    //   and pickable on a fresh canvas rather than appearing only once a path
    //   exists — which, with auto-population dropped, would have been never.
    // - `add_fn: None` keeps it out of "Add Component" entirely. It used to seed
    //   `DEFAULT_TEMPLATE` onto *any* entity, which made a second kind of
    //   template holder that nothing else in the editor understood.
    // - `remove_fn: None` removes the trash button. It was already a no-op on a
    //   canvas — it checked and returned — so the button was there and did
    //   nothing, which is worse than not being there.
    app.register_inspector(InspectorEntry {
        type_id: "html_template",
        display_name: "UI Template",
        icon: "browser",
        category: "ui",
        has_fn: |world, entity| {
            world.get::<UiCanvas>(entity).is_some() || world.get::<HtmlTemplatePath>(entity).is_some()
        },
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Template",
            field_type: FieldType::AssetCreatable {
                extensions: vec!["html".into()],
                // "+" (shown only while empty): author a template in place. A world
                // canvas gets a visible starter card; anything else gets a blank
                // template. Then the normal path-binding observer builds it.
                create_fn: |world, entity| {
                    let Some(root) = world
                        .get_resource::<renzora::CurrentProject>()
                        .map(|p| p.path.clone())
                    else {
                        return;
                    };
                    let is_world_canvas = world
                        .get::<UiCanvas>(entity)
                        .is_some_and(|c| c.is_world());
                    // Named after the canvas, so a scene with three of them ends
                    // up with `main_menu.html` and `hud.html` rather than
                    // `template.html` and `template_1.html`.
                    let slug = world
                        .get::<Name>(entity)
                        .map(|n| slug_name(n.as_str()))
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "canvas".to_string());
                    let rel = if is_world_canvas {
                        create_unique_panel_template(&root, &slug)
                    } else {
                        create_unique_template(&root, &slug)
                    };
                    if let Some(rel) = rel {
                        world.entity_mut(entity).insert(HtmlTemplatePath(rel));
                    }
                },
            },
            get_fn: |world, entity| {
                let path = world
                    .get::<HtmlTemplatePath>(entity)
                    .map(|p| if p.0.is_empty() { None } else { Some(p.0.clone()) })
                    .unwrap_or(None);
                Some(FieldValue::Asset(path))
            },
            // Always insert (replace) so the binding observer fires and rebuilds
            // the markup child for the new path.
            set_fn: |world, entity, val| {
                if let FieldValue::Asset(path) = val {
                    world
                        .entity_mut(entity)
                        .insert(HtmlTemplatePath(path.unwrap_or_default()));
                }
            },
        }],
    });
}

// `AutoCanvasTemplate` + `ensure_canvas_template` lived here. Adding a UI Canvas
// wrote `<project>/ui/<slug>.html` on the frame the entity appeared, and linked
// it. Spawning an entity should not put a file in someone's project: you get an
// empty canvas and pick a template, or make one with the slot's "+" — which is
// the same `create_unique_template` this used, on a press instead of a spawn.
//
// The Assets panel's **New → UI Template** is the other way in, and it is the
// one to reach for first: it names the file, puts it where you want it, and the
// canvas is then just the thing that mounts it.

/// Sanitize an entity name into a lowercase, filesystem-safe file stem
/// (`"UI Canvas"` → `ui_canvas`). Non-alphanumerics become `_`; leading/trailing
/// `_` are trimmed.
fn slug_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

/// Pick the first free stem: `slug`, else `slug_1`, `slug_2`, … `exists` reports
/// whether a stem is already taken. Pure, so it can be unit-tested without disk.
fn unique_stem(slug: &str, exists: impl Fn(&str) -> bool) -> String {
    if !exists(slug) {
        return slug.to_string();
    }
    (1..10_000)
        .map(|n| format!("{slug}_{n}"))
        .find(|s| !exists(s))
        .unwrap_or_else(|| slug.to_string())
}

/// Create a fresh `ui/<stem>.html` under the project root (collision-bumped),
/// write the default empty template, and return the project-relative path
/// (`ui/<stem>.html`) for `HtmlTemplatePath`. `None` if it can't be written.
fn create_unique_template(project_root: &std::path::Path, slug: &str) -> Option<String> {
    // The same minimal template the asset browser's "New → HTML Template" writes.
    const DEFAULT_CONTENT: &str = "<template>\n    <node></node>\n</template>\n";
    let ui_dir = project_root.join("ui");
    if let Err(e) = std::fs::create_dir_all(&ui_dir) {
        warn!("could not create project ui/ dir: {e}");
        return None;
    }
    let stem = unique_stem(slug, |s| ui_dir.join(format!("{s}.html")).exists());
    let abs = ui_dir.join(format!("{stem}.html"));
    if let Err(e) = std::fs::write(&abs, DEFAULT_CONTENT) {
        warn!("could not write UI template {}: {e}", abs.display());
        return None;
    }
    Some(format!("ui/{stem}.html"))
}

/// Like [`create_unique_template`], but writes a *visible* starter panel rather
/// than an empty node. A world panel with an empty template renders a
/// transparent quad — nothing to see and nothing to grab in the viewport — so a
/// freshly-added panel gets a filled card the user can immediately see, aim at,
/// and then edit or replace.
fn create_unique_panel_template(project_root: &std::path::Path, slug: &str) -> Option<String> {
    // Self-contained (no `template="..."` includes) so the panel works in any
    // project, even one with no component library. Full-bleed dark card with a
    // heading and one button — enough to confirm the quad renders and that a
    // pointer hit lands where you aimed.
    const PANEL_CONTENT: &str = "\
<template>
    <node width=\"100%\" height=\"100%\" flex_direction=\"column\"
          justify_content=\"center\" align_items=\"center\" row_gap=\"18px\"
          background=\"#12151C\" padding=\"32px\">
        <text font_size=\"40\" font_color=\"#FFFFFF\">World Panel</text>
        <text font_size=\"16\" font_color=\"#8A93A2\">Edit this template to build your panel</text>
        <button padding=\"14px 28px\" border_radius=\"10px\" background=\"#2A2D34\"
                hover:background=\"#4C8BF5\" pressed:background=\"#2E9E5B\" on_press=\"panel_button\">
            <text font_size=\"18\" font_color=\"#FFFFFF\">Click Me</text>
        </button>
    </node>
</template>
";
    let ui_dir = project_root.join("ui");
    if let Err(e) = std::fs::create_dir_all(&ui_dir) {
        warn!("could not create project ui/ dir: {e}");
        return None;
    }
    let stem = unique_stem(slug, |s| ui_dir.join(format!("{s}.html")).exists());
    let abs = ui_dir.join(format!("{stem}.html"));
    if let Err(e) = std::fs::write(&abs, PANEL_CONTENT) {
        warn!("could not write UI panel template {}: {e}", abs.display());
        return None;
    }
    Some(format!("ui/{stem}.html"))
}

#[cfg(test)]
mod tests {
    use super::{slug_name, unique_stem};

    #[test]
    fn slug_name_sanitizes() {
        assert_eq!(slug_name("UI Canvas"), "ui_canvas");
        assert_eq!(slug_name("  Menu!  "), "menu");
        assert_eq!(slug_name("HUD 2"), "hud_2");
        assert_eq!(slug_name("***"), "");
    }

    #[test]
    fn unique_stem_bumps_past_collisions() {
        let taken = ["canvas", "canvas_1", "canvas_2"];
        assert_eq!(unique_stem("canvas", |s| taken.contains(&s)), "canvas_3");
        assert_eq!(unique_stem("fresh", |_| false), "fresh");
    }
}
