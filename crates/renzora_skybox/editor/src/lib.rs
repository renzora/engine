//! Editor-only half of `renzora_skybox` — the Skybox inspector entry plus the
//! native (ember) mode-specific inspector drawer (Color / Procedural / Panorama /
//! Tiled, with color pickers) and the panorama HDR/EXR file browser.
//!
//! `renzora_skybox` compiles lean (no `editor` feature, no egui-phosphor /
//! renzora_ember / renzora_inspector / rfd). This crate holds the inspector
//! (renzora editor contract + Phosphor icon) and the native drawer
//! (renzora_ember, reads/writes `renzora_skybox::SkyboxData`), registered
//! `renzora::add!(SkyboxEditorPlugin, Editor)` and linked only by the editor
//! bundle.

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_skybox::SkyboxData;

// ============================================================================
// Inspector entry
// ============================================================================

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "skybox",
        display_name: "Skybox",
        icon: "sun",
        category: "rendering",
        has_fn: |world, entity| world.get::<SkyboxData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SkyboxData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<SkyboxData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}

// ============================================================================
// Native (ember) drawer
// ============================================================================

mod native_inspector {
    use bevy::ecs::world::CommandQueue;
    use bevy::prelude::*;
    use renzora::FieldValue;
    use renzora_ember::font::EmberFonts;
    use renzora_ember::inspector::{color_field, inspector_row, inspector_stripe};
    use renzora_ember::reactive::bind_2way;
    use renzora_ember::widgets::{drag_value, dropdown, icon_label_button, DragRange};
    use renzora_inspector::asset_drop_field;
    use renzora_skybox::{SkyMode, SkyboxData};

    #[derive(Component)]
    pub(super) struct SkyboxRoot {
        entity: Entity,
        sig: Option<u8>,
    }
    #[derive(Component)]
    pub(super) struct SkyBrowseBtn {
        entity: Entity,
    }

    fn sky_disc(m: &SkyMode) -> u8 {
        match m {
            SkyMode::Color => 0,
            SkyMode::Procedural => 1,
            SkyMode::Panorama => 2,
            SkyMode::Tiled => 3,
        }
    }

    // Color accessors (fn-pointers so bind closures stay Copy).
    fn g_clear(d: &SkyboxData) -> (f32, f32, f32) { d.clear_color }
    fn s_clear(d: &mut SkyboxData, c: (f32, f32, f32)) { d.clear_color = c; }
    fn g_top(d: &SkyboxData) -> (f32, f32, f32) { d.procedural_sky.sky_top_color }
    fn s_top(d: &mut SkyboxData, c: (f32, f32, f32)) { d.procedural_sky.sky_top_color = c; }
    fn g_hor(d: &SkyboxData) -> (f32, f32, f32) { d.procedural_sky.sky_horizon_color }
    fn s_hor(d: &mut SkyboxData, c: (f32, f32, f32)) { d.procedural_sky.sky_horizon_color = c; }
    fn g_ghor(d: &SkyboxData) -> (f32, f32, f32) { d.procedural_sky.ground_horizon_color }
    fn s_ghor(d: &mut SkyboxData, c: (f32, f32, f32)) { d.procedural_sky.ground_horizon_color = c; }
    fn g_gbot(d: &SkyboxData) -> (f32, f32, f32) { d.procedural_sky.ground_bottom_color }
    fn s_gbot(d: &mut SkyboxData, c: (f32, f32, f32)) { d.procedural_sky.ground_bottom_color = c; }
    fn g_ta(d: &SkyboxData) -> (f32, f32, f32) { d.tiled_sky.tile_color_a }
    fn s_ta(d: &mut SkyboxData, c: (f32, f32, f32)) { d.tiled_sky.tile_color_a = c; }
    fn g_tb(d: &SkyboxData) -> (f32, f32, f32) { d.tiled_sky.tile_color_b }
    fn s_tb(d: &mut SkyboxData, c: (f32, f32, f32)) { d.tiled_sky.tile_color_b = c; }
    fn g_line(d: &SkyboxData) -> (f32, f32, f32) { d.tiled_sky.line_color }
    fn s_line(d: &mut SkyboxData, c: (f32, f32, f32)) { d.tiled_sky.line_color = c; }

    // Scalar accessors.
    fn g_sky_curve(d: &SkyboxData) -> f32 { d.procedural_sky.sky_curve }
    fn s_sky_curve(d: &mut SkyboxData, v: f32) { d.procedural_sky.sky_curve = v; }
    fn g_ground_curve(d: &SkyboxData) -> f32 { d.procedural_sky.ground_curve }
    fn s_ground_curve(d: &mut SkyboxData, v: f32) { d.procedural_sky.ground_curve = v; }
    fn g_rotation(d: &SkyboxData) -> f32 { d.panorama_sky.rotation }
    fn s_rotation(d: &mut SkyboxData, v: f32) { d.panorama_sky.rotation = v; }
    fn g_energy(d: &SkyboxData) -> f32 { d.panorama_sky.energy }
    fn s_energy(d: &mut SkyboxData, v: f32) { d.panorama_sky.energy = v; }
    fn g_line_width(d: &SkyboxData) -> f32 { d.tiled_sky.line_width }
    fn s_line_width(d: &mut SkyboxData, v: f32) { d.tiled_sky.line_width = v; }
    fn g_tile_count(d: &SkyboxData) -> f32 { d.tiled_sky.tile_count as f32 }
    fn s_tile_count(d: &mut SkyboxData, v: f32) { d.tiled_sky.tile_count = v.round().clamp(2.0, 32.0) as u32; }

    fn pano_get(w: &World, e: Entity) -> Option<FieldValue> {
        w.get::<SkyboxData>(e).map(|d| {
            let p = &d.panorama_sky.panorama_path;
            FieldValue::Asset(if p.is_empty() { None } else { Some(p.clone()) })
        })
    }
    fn pano_set(w: &mut World, e: Entity, v: FieldValue) {
        if let FieldValue::Asset(p) = v {
            if let Some(mut d) = w.get_mut::<SkyboxData>(e) {
                d.panorama_sky.panorama_path = p.unwrap_or_default();
            }
        }
    }

    fn sky_color_row(
        commands: &mut Commands,
        fonts: &EmberFonts,
        entity: Entity,
        label: &str,
        getf: fn(&SkyboxData) -> (f32, f32, f32),
        setf: fn(&mut SkyboxData, (f32, f32, f32)),
    ) -> Entity {
        let cf = color_field(
            commands,
            move |w| w.get::<SkyboxData>(entity).map(|d| { let c = getf(d); [c.0, c.1, c.2] }).unwrap_or([0.0; 3]),
            move |w, a: [f32; 3]| {
                if let Some(mut d) = w.get_mut::<SkyboxData>(entity) {
                    setf(&mut d, (a[0], a[1], a[2]));
                }
            },
        );
        inspector_row(commands, &fonts.ui, label, cf)
    }

    #[allow(clippy::too_many_arguments)]
    fn sky_drag_row(
        commands: &mut Commands,
        fonts: &EmberFonts,
        entity: Entity,
        label: &str,
        getf: fn(&SkyboxData) -> f32,
        setf: fn(&mut SkyboxData, f32),
        min: f32,
        max: f32,
        step: f32,
    ) -> Entity {
        let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), min, step);
        commands.entity(dv).insert(DragRange { min, max });
        bind_2way(
            commands,
            dv,
            move |w| w.get::<SkyboxData>(entity).map(getf).unwrap_or(min),
            move |w, v: &f32| {
                if let Some(mut d) = w.get_mut::<SkyboxData>(entity) {
                    setf(&mut d, *v);
                }
            },
        );
        inspector_row(commands, &fonts.ui, label, dv)
    }

    pub(super) fn skybox_native(world: &mut World, entity: Entity) -> Entity {
        world
            .spawn((
                Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), padding: UiRect::all(Val::Px(2.0)), ..default() },
                SkyboxRoot { entity, sig: None },
                Name::new("skybox-inspector-root"),
            ))
            .id()
    }

    /// Rebuild the mode-specific rows when the sky mode changes.
    pub(super) fn rebuild_skybox(world: &mut World) {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut q = world.query::<(Entity, &SkyboxRoot)>();
        let roots: Vec<(Entity, Entity, Option<u8>)> = q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
        for (root, entity, old_sig) in roots {
            let Some(data) = world.get::<SkyboxData>(entity).cloned() else { continue };
            let sig = sky_disc(&data.sky_mode);
            if old_sig == Some(sig) {
                continue;
            }
            let existing: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
            let mut queue = CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, world);
                for ch in existing {
                    commands.entity(ch).despawn();
                }
                build_skybox_body(&mut commands, &fonts, root, entity, &data);
            }
            queue.apply(world);
            if let Some(mut sr) = world.get_mut::<SkyboxRoot>(root) {
                sr.sig = Some(sig);
            }
        }
    }

    fn build_skybox_body(commands: &mut Commands, fonts: &EmberFonts, root: Entity, entity: Entity, data: &SkyboxData) {
        let mut rows: Vec<Entity> = Vec::new();

        // Type combo.
        let dd = dropdown(commands, fonts, &["Color", "Procedural", "Panorama", "Tiled"], sky_disc(&data.sky_mode) as usize);
        bind_2way(
            commands,
            dd,
            move |w| w.get::<SkyboxData>(entity).map(|d| sky_disc(&d.sky_mode) as usize).unwrap_or(0),
            move |w, i: &usize| {
                if let Some(mut d) = w.get_mut::<SkyboxData>(entity) {
                    d.sky_mode = match i {
                        1 => SkyMode::Procedural,
                        2 => SkyMode::Panorama,
                        3 => SkyMode::Tiled,
                        _ => SkyMode::Color,
                    };
                }
            },
        );
        rows.push(inspector_row(commands, &fonts.ui, "Type", dd));

        match data.sky_mode {
            SkyMode::Color => {
                rows.push(sky_color_row(commands, fonts, entity, "Background", g_clear, s_clear));
            }
            SkyMode::Procedural => {
                rows.push(sky_color_row(commands, fonts, entity, "Top Color", g_top, s_top));
                rows.push(sky_color_row(commands, fonts, entity, "Horizon Color", g_hor, s_hor));
                rows.push(sky_drag_row(commands, fonts, entity, "Sky Curve", g_sky_curve, s_sky_curve, 0.01, 1.0, 0.01));
                rows.push(sky_color_row(commands, fonts, entity, "Ground Horizon", g_ghor, s_ghor));
                rows.push(sky_color_row(commands, fonts, entity, "Ground Bottom", g_gbot, s_gbot));
                rows.push(sky_drag_row(commands, fonts, entity, "Ground Curve", g_ground_curve, s_ground_curve, 0.01, 1.0, 0.01));
            }
            SkyMode::Panorama => {
                let img = asset_drop_field(commands, fonts, entity, pano_get, pano_set, vec!["hdr".into(), "exr".into()]);
                rows.push(inspector_row(commands, &fonts.ui, "Image", img));
                let browse = icon_label_button(commands, fonts, "folder-open", "Browse");
                commands.entity(browse).insert(SkyBrowseBtn { entity });
                rows.push(inspector_row(commands, &fonts.ui, "", browse));
                rows.push(sky_drag_row(commands, fonts, entity, "Rotation", g_rotation, s_rotation, 0.0, 360.0, 1.0));
                rows.push(sky_drag_row(commands, fonts, entity, "Energy", g_energy, s_energy, 0.0, 10.0, 0.1));
            }
            SkyMode::Tiled => {
                rows.push(sky_color_row(commands, fonts, entity, "Tile Color A", g_ta, s_ta));
                rows.push(sky_color_row(commands, fonts, entity, "Tile Color B", g_tb, s_tb));
                rows.push(sky_color_row(commands, fonts, entity, "Line Color", g_line, s_line));
                rows.push(sky_drag_row(commands, fonts, entity, "Tile Count", g_tile_count, s_tile_count, 2.0, 32.0, 1.0));
                rows.push(sky_drag_row(commands, fonts, entity, "Line Width", g_line_width, s_line_width, 0.005, 0.15, 0.005));
            }
        }

        for (i, r) in rows.iter().enumerate() {
            commands.entity(*r).insert(BackgroundColor(inspector_stripe(i)));
        }
        commands.entity(root).add_children(&rows);
    }

    pub(super) fn sky_browse_click(q: Query<(&Interaction, &SkyBrowseBtn), Changed<Interaction>>, mut commands: Commands) {
        for (interaction, b) in &q {
            if *interaction != Interaction::Pressed {
                continue;
            }
            let e = b.entity;
            commands.queue(move |w: &mut World| {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("HDR Images", &["hdr", "exr"])
                    .set_title("Select Sky Texture")
                    .pick_file()
                {
                    let rel = w
                        .get_resource::<renzora::CurrentProject>()
                        .map(|p| p.make_asset_relative(&path))
                        .unwrap_or_else(|| path.to_string_lossy().to_string());
                    if let Some(mut d) = w.get_mut::<SkyboxData>(e) {
                        d.panorama_sky.panorama_path = rel;
                    }
                }
                #[cfg(target_arch = "wasm32")]
                let _ = e;
            });
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

/// Editor-scope companion to `renzora_skybox::SkyboxPlugin`.
#[derive(Default)]
pub struct SkyboxEditorPlugin;

impl Plugin for SkyboxEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SkyboxEditorPlugin");
        app.register_inspector(inspector_entry());
        app.register_native_inspector_ui("skybox", native_inspector::skybox_native);
        app.add_systems(
            Update,
            (native_inspector::rebuild_skybox, native_inspector::sky_browse_click)
                .run_if(in_state(renzora::SplashState::Editor)),
        );
    }
}

renzora::add!(SkyboxEditorPlugin, Editor);
