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
use bevy_hui::prelude::{HtmlNode, Tags};
use renzora::{
    AppEditorExt, ComponentIconEntry, EntityPreset, FieldDef, FieldType, FieldValue, InspectorEntry,
};
use renzora_ember::game_ui::{UiCanvas, UiWidget};

use renzora_ember::markup::HtmlTemplatePath;

/// Default template a freshly-created HTML entity points at, so it shows
/// something immediately instead of an empty node.
const DEFAULT_TEMPLATE: &str = "ui/example_menu.html";

pub struct HuiEditorPlugin;

impl Plugin for HuiEditorPlugin {
    fn build(&self, app: &mut App) {
        register_editor_entries(app);
        app.add_systems(Update, (tag_built_nodes, ensure_canvas_template));
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
    // "+ Add Entity" → UI → "HTML Template". Spawns a draggable, absolutely-
    // positioned instance under a UI Canvas; the runtime observer builds the
    // markup beneath it.
    app.register_entity_preset(EntityPreset {
        id: "html_template",
        display_name: "HTML Template",
        icon: "code",
        category: "ui",
        spawn_fn: |world| {
            renzora_ember::game_ui::spawn::spawn_html_template_at(
                world,
                std::path::Path::new(DEFAULT_TEMPLATE),
                None,
            )
        },
    });

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

    // Inspector: pick/replace the .html the instance displays. Adding the
    // component (also via "Add Component") seeds the default template.
    app.register_inspector(InspectorEntry {
        type_id: "html_template",
        display_name: "HTML Template",
        icon: "code",
        category: "ui",
        has_fn: |world, entity| world.get::<HtmlTemplatePath>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(HtmlTemplatePath(DEFAULT_TEMPLATE.to_string()));
        }),
        remove_fn: Some(|world, entity| {
            // A UiCanvas's template is its backbone — mandatory, not removable.
            // (Standalone `html_template` instance entities can still drop it.)
            if world.get::<UiCanvas>(entity).is_some() {
                return;
            }
            // Drop the path and any markup child it built.
            let children: Vec<Entity> = world
                .get::<Children>(entity)
                .map(|c| c.iter().collect())
                .unwrap_or_default();
            for child in children {
                if world.get::<HtmlNode>(child).is_some() {
                    world.entity_mut(child).despawn();
                }
            }
            world.entity_mut(entity).remove::<HtmlTemplatePath>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![FieldDef {
            name: "Template",
            field_type: FieldType::Asset {
                extensions: vec!["html".into()],
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

/// A canvas the user asked for *directly* — "+ Add Entity → UI Canvas" or the
/// "New UI" scene starter. Only these get a blank template written for them.
///
/// Canvases also appear implicitly, as a host for something the user dropped
/// when the scene had no canvas yet (`spawn_html_template_at` for a dropped
/// `.html`, `spawn_widget`, `spawn_image_at`). Those already carry the content
/// the user chose, so writing a fresh `ui/<name>.html` for the host would leave
/// a stray blank template in the project next to the real one.
#[derive(Component)]
pub(crate) struct AutoCanvasTemplate;

/// A `UiCanvas` the user created deliberately is backed by an `.html` template —
/// its contents are authored as markup, so the template is its backbone, not an
/// optional add-on. Such a canvas starts with none, so we create one under the
/// project's `ui/` folder and link it here.
///
/// Filtered to `Added<UiCanvas>` *without* a path, so scene-loaded canvases
/// (which already carry their template) are left alone, and to
/// `AutoCanvasTemplate` so implicitly-spawned host canvases don't get a blank
/// template they never asked for. Creating it eagerly on spawn — rather than
/// lazily on the first widget — means a canvas always has its template *before*
/// any widget is added, so the markup loader's build-on-insert runs against an
/// empty canvas and never wipes authored children.
fn ensure_canvas_template(
    mut commands: Commands,
    project: Option<Res<renzora::CurrentProject>>,
    canvases: Query<
        (Entity, Option<&Name>),
        (Added<UiCanvas>, With<AutoCanvasTemplate>, Without<HtmlTemplatePath>),
    >,
) {
    let Some(project) = project else {
        return; // No project open — nowhere to write the file.
    };
    for (entity, name) in &canvases {
        let slug = name
            .map(|n| slug_name(n.as_str()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "canvas".to_string());
        if let Some(rel) = create_unique_template(&project.path, &slug) {
            commands.entity(entity).insert(HtmlTemplatePath(rel));
        }
    }
}

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
