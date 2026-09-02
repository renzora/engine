//! The drawer's root, its rebuild signature, and the body it assembles.

use std::path::PathBuf;

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_editor_framework::MaterialThumbnailRegistry;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{border, placeholder, rgb, text_primary};

use renzora_shader::material::codegen::MaterialParam;
use renzora_shader::material::graph::MaterialGraph;
use renzora_shader::material::instance::{read_master_parameters, MaterialInstance};
use renzora_shader::material::texture_slots::{self, TextureSlot, TEXTURE_SLOTS};

use super::{material_abs, material_path, sig_of, MatCache, MatRoot, TexSlotsExpanded};

pub(super) fn material_drawer_root(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(2.0)), ..default() },
            MatRoot { entity, sig: None },
            Name::new("material-ref-inspector-root"),
        ))
        .id()
}

/// Reload [`MatCache`] from disk for the given entity + path.
///
/// The two `.material` shapes are told apart by which one parses: a derived
/// instance has a `master` field and no nodes, a master graph the reverse, so
/// neither ever deserializes as the other.
fn load_cache(world: &mut World, entity: Entity, path: &str) {
    let mut instance = None;
    let mut params = Vec::new();
    let mut graph = None;
    let mut instance_abs = PathBuf::new();
    if let Some(project) = world.get_resource::<CurrentProject>() {
        if !path.is_empty() {
            instance_abs = project.resolve_path(path);
            if let Ok(content) = std::fs::read_to_string(&instance_abs) {
                if let Ok(inst) = serde_json::from_str::<MaterialInstance>(&content) {
                    if !inst.master.is_empty() {
                        let master_abs = project.resolve_path(&inst.master);
                        params = read_master_parameters(&master_abs).unwrap_or_default();
                    }
                    instance = Some(inst);
                } else {
                    graph = serde_json::from_str::<MaterialGraph>(&content).ok();
                }
            }
        }
    }
    if let Some(mut cache) = world.get_resource_mut::<MatCache>() {
        cache.entity = Some(entity);
        cache.path = path.to_string();
        cache.instance_abs = instance_abs;
        cache.instance = instance;
        cache.params = params;
        cache.graph = graph;
        cache.dirty = false;
    }
}

pub(super) fn rebuild_material(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut q = world.query::<(Entity, &MatRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> = q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
    for (root, entity, old_sig) in roots {
        let path = material_path(&Rx::new(&*world), entity);
        let rev = world.get_resource::<MatCache>().map(|c| c.rev).unwrap_or(0);
        let expanded = world.get_resource::<TexSlotsExpanded>().is_some_and(|e| e.0);
        let sig = sig_of(entity, &path, rev, expanded);
        if old_sig == Some(sig) {
            continue;
        }
        load_cache(world, entity, &path);
        // Request the current material's thumbnail.
        if let Some(abs) = material_abs(&Rx::new(&*world), &path) {
            if let Some(mut reg) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
                reg.request(abs);
            }
        }
        let params = world.get_resource::<MatCache>().map(|c| c.params.clone()).unwrap_or_default();
        let slots = slot_states(world);

        let existing: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            build_body(&mut commands, &fonts, root, entity, &path, &params, &slots, expanded);
        }
        queue.apply(world);
        if let Some(mut mr) = world.get_mut::<MatRoot>(root) {
            mr.sig = Some(sig);
        }
        // No picker poke needed: its keyed list folds `material_path(entity)`
        // into its dirty token, so the new selection re-snapshots on its own.
    }
}

/// One texture-slot row's state, resolved from the cached graph before the
/// build so the row builder needs no world access.
pub(super) struct SlotState {
    pub(super) slot: &'static TextureSlot,
    /// Asset-relative texture path currently wired into the slot.
    pub(super) texture: Option<String>,
    /// Wired, but not applied to the mesh — see
    /// [`renzora_shader::material::texture_slots::set_slot_muted`].
    pub(super) muted: bool,
    /// Preview handle for `texture`. Held by the row's `ImageNode`, so the image
    /// stays loaded for as long as the row is on screen.
    pub(super) thumb: Option<Handle<Image>>,
}

/// Read every slot's current texture off the cached master graph.
///
/// Returns nothing in two cases. A mesh with **no material at all**: six empty
/// channel rows are six things you can't meaningfully do yet, and they buried
/// the one thing you can — the picker and the New-material button at the top.
/// (Dropping images still works: the material *slot* routes a whole texture set
/// by filename and mints a material to hold it, so the affordance those rows
/// used to be is still there, one target up.) And a **derived instance**, whose
/// graph belongs to the master and must not be edited from here.
fn slot_states(world: &World) -> Vec<SlotState> {
    let Some(cache) = world.get_resource::<MatCache>() else { return Vec::new() };
    if cache.instance.is_some() || cache.path.is_empty() {
        return Vec::new();
    }
    let graph = cache.graph.as_ref();
    let assets = world.get_resource::<AssetServer>();
    TEXTURE_SLOTS
        .iter()
        .map(|slot| {
            let texture = graph.and_then(|g| texture_slots::slot_texture(g, slot));
            let muted = graph.is_some_and(|g| texture_slots::slot_muted(g, slot));
            let thumb = texture
                .as_ref()
                .and_then(|p| assets.map(|a| a.load::<Image>(p.clone())));
            SlotState { slot, texture, muted, thumb }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn build_body(
    commands: &mut Commands,
    fonts: &EmberFonts,
    root: Entity,
    entity: Entity,
    path: &str,
    params: &[MaterialParam],
    slots: &[SlotState],
    expanded: bool,
) {
    let mut children: Vec<Entity> = Vec::new();

    // ── Material slot ──
    children.push(super::slot::build_slot(commands, fonts, entity, path));

    // ── Texture slots ──
    //
    // No section header: each row already names its channel, so a heading plus a
    // sentence of instructions above six self-explanatory rows was two lines of
    // chrome saying what the rows say themselves.
    //
    // Collapsed, only the first channel is *built* — hiding the rest with
    // `Display::None` would collide with `set_texture_rows`, which shows every
    // row again when the material picker tray closes.
    let shown = if expanded { slots.len() } else { slots.len().min(1) };
    for (i, state) in slots.iter().take(shown).enumerate() {
        let row = super::textures::texture_slot_row(commands, fonts, entity, state);
        if i == 0 {
            // The only spacing the block needs, now that nothing announces it.
            commands.entity(row).entry::<Node>().and_modify(|mut n| {
                n.margin.top = Val::Px(8.0);
            });
        }
        children.push(row);
    }
    if slots.len() > 1 {
        children.push(super::textures::tex_slots_toggle_row(commands, fonts, entity, slots, expanded));
    }

    // ── Overrides ──
    if !params.is_empty() {
        children.push(section_header(commands, fonts, "Overrides", "Parameters this instance overrides on its master"));
        for (i, param) in params.iter().enumerate() {
            let row = super::overrides::param_row(commands, fonts, param);
            commands.entity(row).insert(BackgroundColor(inspector_stripe(i)));
            children.push(row);
        }
    }

    commands.entity(root).add_children(&children);
}

/// A section divider: a small label over a hairline, with the hint that follows
/// it kept muted so the eye lands on the rows, not the prose.
fn section_header(commands: &mut Commands, fonts: &EmberFonts, title: &str, hint: &str) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                margin: UiRect { top: Val::Px(10.0), bottom: Val::Px(4.0), ..default() },
                padding: UiRect { top: Val::Px(6.0), ..default() },
                border: UiRect { top: Val::Px(1.0), ..default() },
                ..default()
            },
            BorderColor::all(rgb(border())),
            Name::new("material-section-header"),
        ))
        .id();
    let t = commands
        .spawn((Text::new(title), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    let h = commands
        .spawn((Text::new(hint), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
        .id();
    commands.entity(wrap).add_children(&[t, h]);
    wrap
}
