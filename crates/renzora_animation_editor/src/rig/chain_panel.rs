//! Chains: a row of bones along anything that bends. Rig mode's Chain
//! template, and the shape library's Bendable option.
//!
//! [`chain_builder`] rigs the model on the stage with `renzora_rig::chain`:
//! a number of bones from one end to the other, along the two points placed
//! or, with none placed, along the mesh's longest axis.
//!
//! A library shape (a capsule, a cylinder) is not a model: it has no file
//! for a rig to be written into, only a shape id the engine rebuilds its mesh
//! from. [`make_bendable`] gives it one. The mesh is written to
//! `models/shapes/<name>.glb` and rigged there, and the same entity becomes
//! an instance of that model, keeping its name, place and every other
//! component (a collider, a script). It is one undoable step: undoing puts
//! the shape back exactly as it was, and the model file stays, like any
//! imported model does.

use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;

use renzora::core::{EditedMesh, MakeBendable, MeshColor, MeshInstanceData, MeshPrimitive};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, keyed_list};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, spin_slider};
use renzora_rig::chain::{self, ChainSpec};
use renzora_rig::glb::{Geometry, Glb};

use super::markers::{MarkerSet, Markers};
use super::rig_panel::{marker_snapshot, markers_in_model, unrig_row};
use super::{joints, model_file, on_click, subject, tell};

/// How many bones the next chain gets.
#[derive(Resource)]
pub(crate) struct ChainSettings {
    pub(crate) segments: f32,
}

impl Default for ChainSettings {
    fn default() -> Self {
        Self { segments: 4.0 }
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<ChainSettings>().add_message::<MakeBendable>();
    app.add_systems(Update, carry_out_make_bendable.run_if(in_state(renzora_editor_framework::SplashState::Editor)));
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() }))
        .id()
}

fn column(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id()
}

/// A library shape in the scene: a mesh with no model file behind it.
fn is_shape(world: &World, e: Entity) -> bool {
    world.get::<Mesh3d>(e).is_some() && world.get::<MeshInstanceData>(e).is_none()
}

fn rigged(world: &World) -> bool {
    subject(world).is_some_and(|r| !joints(world, r).is_empty())
}

/// Rig mode's Chain template.
pub(crate) fn chain_builder(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let has = note(commands, fonts, "This model has a skeleton. Its bones are listed on the right; Skin and Animate work on it.");
    bind_display(commands, has, |w| rigged(w.untracked()));
    let body = column(commands);
    bind_display(commands, body, |w| subject(w.untracked()).is_some() && !rigged(w.untracked()));
    let hint = note(
        commands,
        fonts,
        "A row of bones along anything that bends: a tail, a rope, a tentacle, a capsule. Place the root end and the tip, or place none and the chain runs along the model's longest side. The original file is kept.",
    );
    let shape = note(
        commands,
        fonts,
        "This is a library shape. Building writes its mesh to a model file in models/shapes/ first; Undo puts the shape back.",
    );
    bind_display(commands, shape, |w| subject(w.untracked()).is_some_and(|e| is_shape(w.untracked(), e)));
    let bones = spin_slider(commands, &fonts.ui, "Bones", 4.0, 1.0, 16.0);
    commands.entity(bones).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        bones,
        |w: &Rx| w.get_resource::<ChainSettings>().map_or(4.0, |s| s.segments),
        |w: &mut World, v: &f32| w.resource_mut::<ChainSettings>().segments = v.round().clamp(1.0, 16.0),
    );
    let points = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, points, |w| marker_snapshot(w, MarkerSet::Chain));
    // A shape has no copy on the stage to click points on until it is a model.
    bind_display(commands, points, |w| subject(w.untracked()).is_some_and(|e| !is_shape(w.untracked(), e)));
    let r = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(4.0), row_gap: Val::Px(4.0), ..default() })
        .id();
    let place = button(commands, &fonts.ui, "Place Points");
    on_click(commands, place, |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.start(MarkerSet::Chain, 0);
            let next = m.points(MarkerSet::Chain).iter().position(Option::is_none).unwrap_or(0);
            m.start(MarkerSet::Chain, next);
        }
    });
    let clear = button(commands, &fonts.ui, "Clear Points");
    on_click(commands, clear, |w| {
        if let Some(mut m) = w.get_resource_mut::<Markers>() {
            m.clear(MarkerSet::Chain);
        }
    });
    commands.entity(r).add_children(&[place, clear]);
    bind_display(commands, r, |w| subject(w.untracked()).is_some_and(|e| !is_shape(w.untracked(), e)));
    let build = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Build Chain");
    commands.entity(build).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    on_click(commands, build, build_chain);
    commands.entity(body).add_children(&[hint, shape, bones, points, r, build]);
    let unrig = unrig_row(commands, fonts);
    commands.entity(col).add_children(&[has, body, unrig]);
    col
}

fn build_chain(world: &mut World) {
    let Some(root) = subject(world) else { return };
    if !joints(world, root).is_empty() {
        tell(world, false, "This model already has a skeleton");
        return;
    }
    let segments = world.get_resource::<ChainSettings>().map_or(4.0, |s| s.segments).round().clamp(1.0, 16.0) as usize;
    if is_shape(world, root) {
        match make_bendable(world, root, segments) {
            Ok(rel) => tell(world, true, format!("Made the shape bendable: {segments} bones, saved as {rel}")),
            Err(e) => tell(world, false, format!("Could not make the shape bendable: {e}")),
        }
        return;
    }
    let Some((abs, rel)) = model_file(world, root) else {
        tell(world, false, "This entity has no model file to rig");
        return;
    };
    let ends = markers_in_model(world, MarkerSet::Chain).map(|p| (p[0], p[1]));
    let result = renzora_rig::rig_file(&abs, |glb| {
        let (start, end) = match ends {
            Some(ends) => ends,
            None => chain::long_axis(glb)?,
        };
        chain::build(glb, &ChainSpec { start, end, segments })
    });
    match result {
        Ok(applied) => {
            tell(world, true, format!("Rigged {rel} with a chain: {} bones over {} meshes. The original is kept beside it.", applied.bones, applied.meshes));
            if let Some(mut m) = world.get_resource_mut::<Markers>() {
                m.clear(MarkerSet::Chain);
            }
            super::respawn::request(world, &rel);
        }
        Err(e) => tell(world, false, format!("Could not rig {rel}: {e}")),
    }
}

/// The shape library's Bendable option, sent as a message so that crate
/// need not depend on this one.
fn carry_out_make_bendable(world: &mut World) {
    let asks: Vec<MakeBendable> = match world.get_resource_mut::<Messages<MakeBendable>>() {
        Some(mut m) => m.drain().collect(),
        None => return,
    };
    for ask in asks {
        match make_bendable(world, ask.entity, ask.segments.clamp(1, 16) as usize) {
            Ok(rel) => tell(world, true, format!("Added a bendable shape ({} bones), saved as {rel}", ask.segments)),
            Err(e) => tell(world, false, format!("Could not make the shape bendable: {e}")),
        }
    }
}

/// Write a shape's mesh to a model file, rig it with a chain, and turn the
/// shape into an instance of it. Returns the model's project-relative path.
pub(crate) fn make_bendable(world: &mut World, entity: Entity, segments: usize) -> Result<String, String> {
    if !is_shape(world, entity) {
        return Err("only a library shape can be made bendable this way".into());
    }
    let mesh = world.get::<Mesh3d>(entity).map(|m| m.0.clone()).ok_or("the shape has no mesh")?;
    let mesh = world.resource::<Assets<Mesh>>().get(&mesh).cloned().ok_or("the shape's mesh is not loaded")?;
    let color = world.get::<MeshColor>(entity).map_or(Color::WHITE, |c| c.0).to_linear();
    let name = world.get::<Name>(entity).map(|n| n.as_str().to_string()).unwrap_or_else(|| "shape".into());
    let project = world.get_resource::<renzora::core::CurrentProject>().ok_or("no project is open")?.path.clone();

    let positions: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| Vec3::from_array(*p)).collect(),
        _ => return Err("the shape's mesh has no positions".into()),
    };
    let normals: Vec<Vec3> = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| Vec3::from_array(*p)).collect(),
        _ => Vec::new(),
    };
    let uvs: Vec<[f32; 2]> = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(v)) => v.clone(),
        _ => Vec::new(),
    };
    let mut indices: Vec<u32> = match mesh.indices() {
        Some(bevy::mesh::Indices::U16(v)) => v.iter().map(|&i| i as u32).collect(),
        Some(bevy::mesh::Indices::U32(v)) => v.clone(),
        None => (0..positions.len() as u32).collect(),
    };
    // Rows to bend at: a library shape's straight stretches are single
    // spans, with nothing for a joint in the middle of one to curve.
    let (mut positions, mut normals, mut uvs) = (positions, normals, uvs);
    if let Some((start, end)) = chain::axis_of(&positions) {
        let axis = (end - start).normalize_or(Vec3::Y);
        let max = start.distance(end) / (segments as f32 * 6.0);
        chain::tessellate_along(chain::Tessellate { positions: &mut positions, normals: &mut normals, uvs: &mut uvs, indices: &mut indices }, axis, max);
    }
    let stem = file_stem(&name);
    let dir = project.join("models").join("shapes");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Never over an existing model: another bendable shape of the same name
    // may already be using that file.
    let mut file = dir.join(format!("{stem}.glb"));
    let mut n = 2;
    while file.exists() {
        file = dir.join(format!("{stem}_{n}.glb"));
        n += 1;
    }
    let geometry = Geometry {
        name: &stem,
        positions: &positions,
        normals: &normals,
        uvs: &uvs,
        indices: &indices,
        base_color: [color.red, color.green, color.blue, color.alpha],
    };
    let mut glb = Glb::from_geometry(&geometry).map_err(|e| e.to_string())?;
    std::fs::write(&file, glb.to_bytes()).map_err(|e| e.to_string())?;
    renzora_rig::rig_file(&file, |glb| {
        let (start, end) = chain::long_axis(glb)?;
        chain::build(glb, &ChainSpec { start, end, segments })
    })
    .map_err(|e| e.to_string())?;
    let rel = file.strip_prefix(&project).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");

    let mut cmd = ConvertToModel { entity, rel: rel.clone(), stash: None };
    renzora_undo::UndoCommand::execute(&mut cmd, world);
    renzora_undo::record(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
    Ok(rel)
}

/// A file name from an entity name: letters, digits and underscores.
fn file_stem(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "shape".into()
    } else {
        s
    }
}

/// What a shape had that a model does not, kept so Undo can put it back.
struct Stash {
    mesh: Option<Mesh3d>,
    material: Option<MeshMaterial3d<StandardMaterial>>,
    primitive: Option<MeshPrimitive>,
    color: Option<MeshColor>,
    edited: Option<EditedMesh>,
}

/// A shape becoming an instance of a model file, in place.
struct ConvertToModel {
    entity: Entity,
    rel: String,
    stash: Option<Stash>,
}

impl renzora_undo::UndoCommand for ConvertToModel {
    fn label(&self) -> &str {
        "Make Bendable"
    }
    fn execute(&mut self, world: &mut World) {
        let Ok(mut e) = world.get_entity_mut(self.entity) else { return };
        self.stash = Some(Stash {
            mesh: e.take::<Mesh3d>(),
            material: e.take::<MeshMaterial3d<StandardMaterial>>(),
            primitive: e.take::<MeshPrimitive>(),
            color: e.take::<MeshColor>(),
            edited: e.take::<EditedMesh>(),
        });
        // A bare instance, as a saved scene loads one: the engine's
        // rehydrate step spawns the model under it.
        e.insert(MeshInstanceData { model_path: Some(self.rel.clone()) });
        restage(world);
    }
    fn undo(&mut self, world: &mut World) {
        let children: Vec<Entity> = world.get::<Children>(self.entity).map(|c| c.iter().collect()).unwrap_or_default();
        for c in children {
            if let Ok(c) = world.get_entity_mut(c) {
                c.despawn();
            }
        }
        let Ok(mut e) = world.get_entity_mut(self.entity) else { return };
        e.remove::<MeshInstanceData>();
        if let Some(s) = self.stash.take() {
            if let Some(c) = s.mesh {
                e.insert(c);
            }
            if let Some(c) = s.material {
                e.insert(c);
            }
            if let Some(c) = s.primitive {
                e.insert(c);
            }
            if let Some(c) = s.color {
                e.insert(c);
            }
            if let Some(c) = s.edited {
                e.insert(c);
            }
        }
        restage(world);
    }
}

/// Have the stage load its copy again. It reloads only when the selection
/// changes, and a shape turned into a model in place is still the same
/// selected entity: it was a shape (nothing to show) when it was picked, so
/// without this the stage stayed empty with the model right there.
fn restage(world: &mut World) {
    if let Some(mut tracker) = world.get_resource_mut::<crate::studio_preview::StudioPreviewTracker>() {
        tracker.source_entity = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_stems_are_plain() {
        assert_eq!(file_stem("Capsule (2)"), "capsule__2");
        assert_eq!(file_stem("***"), "shape");
    }
}
