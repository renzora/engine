//! Native "Add Entity" — a header button that opens the shared ember search
//! overlay, populated from the spawn / shape / inspector registries. Selecting an
//! entry spawns it through the same undoable commands as the egui panel.
//!
//! [`spawn_entries`] is the one place that list is built: the header button
//! feeds it to the search overlay, and the right-click menu (`context_menu`)
//! groups the same entries into category submenus, so the two can't drift apart.

use bevy::prelude::*;

use renzora::core::ShapeRegistry;
use renzora_editor_framework::{InspectorRegistry, SpawnRegistry};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{search_overlay, SearchEntry};
use renzora_undo::{execute, SpawnEntityCmd, SpawnEntityKind, SpawnShapeCmd, UndoContext};

/// Marker on the hierarchy header's "Add Entity" button.
#[derive(Component)]
pub(crate) struct HierAddEntity;

/// Click "Add Entity" → open the search overlay with every spawnable preset /
/// shape / component.
pub(crate) fn hier_add_entity_open(
    q: Query<&Interaction, (With<HierAddEntity>, Changed<Interaction>)>,
    fonts: Option<Res<EmberFonts>>,
    spawn_reg: Option<Res<SpawnRegistry>>,
    shape_reg: Option<Res<ShapeRegistry>>,
    inspector_reg: Option<Res<InspectorRegistry>>,
    mut commands: Commands,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };

    let entries = spawn_entries(
        spawn_reg.as_deref(),
        shape_reg.as_deref(),
        inspector_reg.as_deref(),
    );
    search_overlay(&mut commands, &fonts, &renzora::lang::t("hierarchy.add.title"), entries);
}

/// Every spawnable preset / shape / component, as search entries whose actions
/// spawn undoably at the scene root.
pub(crate) fn spawn_entries(
    spawn_reg: Option<&SpawnRegistry>,
    shape_reg: Option<&ShapeRegistry>,
    inspector_reg: Option<&InspectorRegistry>,
) -> Vec<SearchEntry> {
    let mut entries: Vec<SearchEntry> = Vec::new();

    if let Some(reg) = spawn_reg {
        for p in reg.iter() {
            let id = p.id.to_string();
            // Localize the preset name + category for display only — the spawn
            // still keys off `p.id`, and the category is consistent per English
            // string so grouping/sidebar matching stays intact.
            let label =
                renzora::lang::t_or(&format!("entity.preset.{}", slug(p.display_name)), p.display_name);
            let category = renzora::lang::t_or(&format!("entity.cat.{}", slug(p.category)), p.category);
            entries.push(SearchEntry::new(
                p.icon,
                label,
                category,
                move |w: &mut World| {
                    execute(
                        w,
                        UndoContext::Scene,
                        Box::new(SpawnEntityCmd {
                            entity: Entity::PLACEHOLDER,
                            kind: SpawnEntityKind::Preset { id: id.clone() },
                        }),
                    );
                },
            ));
        }
    }

    if let Some(reg) = shape_reg {
        for s in reg.iter() {
            let (shape_id, name, color) = (s.id.to_string(), s.name.to_string(), s.default_color);
            let category = renzora::lang::t_or(&format!("entity.cat.{}", slug(s.category)), s.category);
            entries.push(SearchEntry::new(
                s.icon,
                s.name,
                category,
                move |w: &mut World| {
                    execute(
                        w,
                        UndoContext::Scene,
                        Box::new(SpawnShapeCmd {
                            entity: Entity::PLACEHOLDER,
                            shape_id: shape_id.clone(),
                            name: name.clone(),
                            position: Vec3::ZERO,
                            color,
                        }),
                    );
                },
            ));
        }
    }

    if let Some(reg) = inspector_reg {
        const CATS: [&str; 4] = ["rendering", "post_process", "effects", "Audio"];
        for e in reg.iter() {
            if e.add_fn.is_some() && CATS.contains(&e.category) {
                let (type_id, display_name) = (e.type_id.to_string(), e.display_name.to_string());
                let category = renzora::lang::t_or(&format!("entity.cat.{}", slug(e.category)), e.category);
                entries.push(SearchEntry::new(
                    e.icon,
                    e.display_name,
                    category,
                    move |w: &mut World| {
                        execute(
                            w,
                            UndoContext::Scene,
                            Box::new(SpawnEntityCmd {
                                entity: Entity::PLACEHOLDER,
                                kind: SpawnEntityKind::Component {
                                    type_id: type_id.clone(),
                                    display_name: display_name.clone(),
                                },
                            }),
                        );
                    },
                ));
            }
        }
    }

    entries
}

/// Slugify a preset/category name into a localization-key segment: lowercased,
/// every non-alphanumeric run collapsed to `_`. Keeps `entity.preset.<slug>` /
/// `entity.cat.<slug>` stable regardless of the human-readable casing.
fn slug(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}
