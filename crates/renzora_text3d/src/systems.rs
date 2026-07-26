//! Turn a [`Text3d`] into geometry — flat SDF quads or true outline mesh — and
//! keep it in sync.

use bevy::prelude::*;
use bevy::text::{Font, FontAtlasSet, FontCx, FontSource, LayoutCx, RemSize, ScaleCx, TextPipeline};

use crate::outline::build_outline_mesh;
use crate::Text3d;
use renzora_text_mesh::{build_text_mesh, SdfTextMaterial};

/// Marks a `Text3d` whose font hasn't finished loading yet, so the rebuild
/// system keeps retrying every frame until the glyphs are available. `Changed`
/// alone can't cover this: the font asset finishing load doesn't touch the
/// `Text3d` component, so nothing would re-trigger the build.
#[derive(Component)]
pub struct Text3dPending;

/// (Re)build a `Text3d` that changed, or that is still waiting on its font.
///
/// `mode == "mesh"` → real extruded outline geometry with a lit `StandardMaterial`
/// (needs a font file to read outlines from). Otherwise → flat SDF quads with
/// [`SdfTextMaterial`]. Switching modes swaps the material component.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_text3d(
    mut commands: Commands,
    mut pipeline: ResMut<TextPipeline>,
    fonts: Res<Assets<Font>>,
    mut atlas_set: ResMut<FontAtlasSet>,
    mut images: ResMut<Assets<Image>>,
    mut font_cx: ResMut<FontCx>,
    mut layout_cx: ResMut<LayoutCx>,
    mut scale_cx: ResMut<ScaleCx>,
    rem: Res<RemSize>,
    asset_server: Res<AssetServer>,
    default_font: Option<Res<crate::DefaultFont>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut sdf_materials: ResMut<Assets<SdfTextMaterial>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    text3d: Query<(Entity, &Text3d), Or<(Changed<Text3d>, With<Text3dPending>)>>,
) {
    for (entity, t3d) in &text3d {
        if t3d.text.trim().is_empty() {
            commands.entity(entity).remove::<(
                Mesh3d,
                MeshMaterial3d<SdfTextMaterial>,
                MeshMaterial3d<StandardMaterial>,
                Text3dPending,
            )>();
            continue;
        }

        if t3d.mode.trim().eq_ignore_ascii_case("mesh") {
            // ── True geometry: outline mesh, extruded, lit. ──────────────────
            // Mesh mode reads glyph outlines from real font bytes: the embedded
            // default when the field is empty, else the project font (retrying
            // while it loads).
            let mesh = if t3d.font.trim().is_empty() {
                build_outline_mesh(crate::DEFAULT_FONT, &t3d.text, t3d.size.max(1.0), t3d.depth)
            } else {
                let handle: Handle<Font> = asset_server.load(t3d.font.trim().to_string());
                match fonts.get(&handle) {
                    Some(font_asset) => build_outline_mesh(
                        font_asset.data.as_ref(),
                        &t3d.text,
                        t3d.size.max(1.0),
                        t3d.depth,
                    ),
                    None => {
                        commands.entity(entity).insert(Text3dPending); // still loading
                        continue;
                    }
                }
            };
            let Some(mesh) = mesh else {
                commands.entity(entity).remove::<Text3dPending>();
                continue;
            };
            let color = Color::srgb(t3d.color[0], t3d.color[1], t3d.color[2]);
            let mesh_h = meshes.add(mesh);
            let mat = std_materials.add(StandardMaterial {
                base_color: color,
                // Double-sided so a flat (depth 0) letter is legible from behind.
                cull_mode: None,
                ..default()
            });
            commands
                .entity(entity)
                .insert((Mesh3d(mesh_h), MeshMaterial3d(mat)))
                .remove::<(MeshMaterial3d<SdfTextMaterial>, Text3dPending)>();
        } else {
            // ── Flat SDF quads (default). ────────────────────────────────────
            // Empty field → the same embedded default font mesh mode uses (falls
            // back to the OS sans-serif if the default isn't registered yet).
            let font = if t3d.font.trim().is_empty() {
                match &default_font {
                    Some(df) => FontSource::Handle(df.0.clone()),
                    None => FontSource::SansSerif,
                }
            } else {
                FontSource::Handle(asset_server.load(t3d.font.trim().to_string()))
            };
            let built = build_text_mesh(
                &mut pipeline,
                &fonts,
                &mut atlas_set,
                &mut images,
                &mut font_cx,
                &mut layout_cx,
                &mut scale_cx,
                rem.0,
                font,
                &t3d.text,
                t3d.size.max(1.0),
            );
            match built {
                Some((mesh, atlas)) => {
                    let color =
                        Color::srgb(t3d.color[0], t3d.color[1], t3d.color[2]).to_linear();
                    let mesh_h = meshes.add(mesh);
                    let mat = sdf_materials.add(SdfTextMaterial { color, atlas });
                    commands
                        .entity(entity)
                        .insert((Mesh3d(mesh_h), MeshMaterial3d(mat)))
                        .remove::<(MeshMaterial3d<StandardMaterial>, Text3dPending)>();
                }
                None => {
                    commands.entity(entity).insert(Text3dPending);
                }
            }
        }
    }
}

/// Strip the generated mesh/materials when a `Text3d` is removed.
pub(crate) fn cleanup_text3d(
    mut commands: Commands,
    mut removed: RemovedComponents<Text3d>,
    still_mesh: Query<(), With<Mesh3d>>,
) {
    for entity in removed.read() {
        if still_mesh.get(entity).is_ok() {
            commands.entity(entity).remove::<(
                Mesh3d,
                MeshMaterial3d<SdfTextMaterial>,
                MeshMaterial3d<StandardMaterial>,
                Text3dPending,
            )>();
        }
    }
}
