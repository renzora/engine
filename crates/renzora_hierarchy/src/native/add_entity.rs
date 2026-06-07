//! Native "Add Entity" — a header button that opens the shared ember search
//! overlay, populated from the spawn / shape / inspector registries. Selecting an
//! entry spawns it through the same undoable commands as the egui panel.

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

    let mut entries: Vec<SearchEntry> = Vec::new();

    if let Some(reg) = &spawn_reg {
        for p in reg.iter() {
            let id = p.id.to_string();
            entries.push(SearchEntry::new(
                p.icon,
                p.display_name,
                p.category,
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

    if let Some(reg) = &shape_reg {
        for s in reg.iter() {
            let (shape_id, name, color) = (s.id.to_string(), s.name.to_string(), s.default_color);
            entries.push(SearchEntry::new(
                s.icon,
                s.name,
                s.category,
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

    if let Some(reg) = &inspector_reg {
        const CATS: [&str; 4] = ["rendering", "post_process", "effects", "Audio"];
        for e in reg.iter() {
            if e.add_fn.is_some() && CATS.contains(&e.category) {
                let (type_id, display_name) = (e.type_id.to_string(), e.display_name.to_string());
                entries.push(SearchEntry::new(
                    e.icon,
                    e.display_name,
                    e.category,
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

    search_overlay(&mut commands, &fonts, "Add Entity", entries);
}
