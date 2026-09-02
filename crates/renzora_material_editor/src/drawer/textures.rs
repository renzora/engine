//! The PBR channel rows: one per texture slot, each a drop target.
//!
//! The whole row is the target (not just the thumbnail) — the row is what the
//! eye reads as "the Normal slot", and a 34 px square is a small thing to ask
//! someone to hit with a dragged file.

use bevy::prelude::*;
use bevy::ui::widget::ImageNode;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::AssetDragPayload;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{accent, faint_bg, hover_bg, placeholder, rgb, text_muted, text_primary};
use renzora_ember::widgets::HoverTint;

use renzora_shader::material::texture_slots::{self, TextureSlot};

use crate::material_inspector::IMAGE_EXTENSIONS;

use super::build::SlotState;
use super::drop::{asset_relative, slot_edit};
use super::slot::icon_btn;
use super::TexSlotsExpanded;

/// Marks a texture-slot row as a drop target for one PBR channel.
#[derive(Component)]
pub(super) struct TexSlotZone {
    pub(super) entity: Entity,
    pub(super) slot: &'static TextureSlot,
}

#[derive(Component)]
pub(super) struct TexSlotClearBtn {
    entity: Entity,
    slot: &'static TextureSlot,
}

/// The eye on a filled texture row: applies or un-applies that channel without
/// touching the texture. Carries the state it was built in, so the click knows
/// which way to flip — the row is rebuilt from the graph afterwards, so it can't
/// drift.
#[derive(Component)]
pub(super) struct TexSlotMuteBtn {
    entity: Entity,
    slot: &'static TextureSlot,
    muted: bool,
}

/// The footer under the channel rows that shows or hides everything past Base
/// Color. Carries the inspected entity for the same reason the rows do: the
/// picker tray hides this drawer's rows and must hide this drawer's footer with
/// them, not another's.
#[derive(Component)]
pub(super) struct TexSlotsToggle {
    pub(super) entity: Entity,
}

/// Footer row: a caret and a count, sitting where the hidden rows would start.
///
/// It names how many of the hidden channels actually have a texture, because
/// collapsed the drawer is otherwise silent about them — "6 more textures"
/// reads as six empty slots when three of them are bound.
pub(super) fn tex_slots_toggle_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    slots: &[SlotState],
    expanded: bool,
) -> Entity {
    let hidden = &slots[1..];
    let assigned = hidden.iter().filter(|s| s.texture.is_some()).count();
    let label = if expanded {
        "Show fewer textures".to_string()
    } else if assigned > 0 {
        format!("{} more textures · {} assigned", hidden.len(), assigned)
    } else {
        format!("{} more textures", hidden.len())
    };

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            HoverTint::solid(Color::NONE, rgb(hover_bg()), rgb(hover_bg())),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            TexSlotsToggle { entity },
            Name::new("material-texture-slots-toggle"),
        ))
        .id();

    let caret = icon_text(
        commands,
        &fonts.phosphor,
        if expanded { "caret-up" } else { "caret-down" },
        text_muted(),
        11.0,
    );
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(row).add_children(&[caret, text]);
    row
}

/// Flip [`TexSlotsExpanded`]. The rebuild signature reads it, so the click is
/// the whole implementation — the drawer rebuilds with (or without) the rest of
/// the channels on the next frame.
pub(super) fn tex_slots_expand(
    q: Query<&Interaction, (Changed<Interaction>, With<TexSlotsToggle>)>,
    mut expanded: ResMut<TexSlotsExpanded>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        expanded.0 = !expanded.0;
    }
}

/// One channel row: preview · label · texture name · clear.
pub(super) fn texture_slot_row(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, state: &SlotState) -> Entity {
    let filled = state.texture.is_some();
    let name = state
        .texture
        .as_deref()
        .and_then(|p| std::path::Path::new(p).file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Drop texture".to_string());

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            HoverTint::solid(Color::NONE, rgb(hover_bg()), rgb(hover_bg())),
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            RelativeCursorPosition::default(),
            TexSlotZone { entity, slot: state.slot },
            // No tooltip here: the row already spells out its channel in the
            // label, and six rows that each pop a sentence on hover turn a
            // glance down the list into a wall of bubbles.
            Name::new("material-texture-slot"),
        ))
        .id();

    // Preview: the texture when one is bound, the channel's icon when not, so
    // an empty set still reads as six labelled places to drop something.
    let preview = commands
        .spawn((
            Node { width: Val::Px(34.0), height: Val::Px(34.0), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(rgb(faint_bg())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("material-texture-thumb"),
        ))
        .id();
    if let Some(thumb) = &state.thumb {
        let mut image = ImageNode::new(thumb.clone());
        if state.muted {
            // Faded rather than hidden: the texture is still *assigned*, and a
            // row that emptied itself would be indistinguishable from one you'd
            // actually cleared.
            image.color = Color::WHITE.with_alpha(0.25);
        }
        commands.entity(preview).insert(image);
    } else {
        let ic = icon_text(commands, &fonts.phosphor, state.slot.icon, placeholder(), 14.0);
        commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
        commands.entity(preview).add_child(ic);
    }

    let text_col = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: Val::Px(1.0), overflow: Overflow::clip(), ..default() },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(state.slot.label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(if state.muted { placeholder() } else { text_primary() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let value = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(if filled && !state.muted { text_muted() } else { placeholder() })),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(text_col).add_children(&[label, value]);
    commands.entity(row).add_children(&[preview, text_col]);

    if filled {
        // The eye turns the channel off *on the mesh* without giving the texture
        // up; the ✕ beside it is the one that actually unwires it. Two very
        // different answers to "I don't want to see this right now", and only
        // one of them is reversible.
        let mute = icon_btn(
            commands,
            fonts,
            if state.muted { "eye-slash" } else { "eye" },
            if state.muted { "Apply this texture again" } else { "Turn this texture off on the mesh" },
        );
        commands.entity(mute).insert(TexSlotMuteBtn {
            entity,
            slot: state.slot,
            muted: state.muted,
        });
        let clear = icon_btn(commands, fonts, "x", "Clear this texture");
        commands.entity(clear).insert(TexSlotClearBtn { entity, slot: state.slot });
        commands.entity(row).add_children(&[mute, clear]);
    }
    row
}

/// Drop an image on a texture-slot row → wire it into that channel.
pub(super) fn tex_slot_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    zones: Query<(&RelativeCursorPosition, &TexSlotZone)>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload else { return };
    if !payload.is_detached || !payload.matches_extensions(IMAGE_EXTENSIONS) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        // A row is one channel, so a multi-file drag aimed at it takes the
        // drag's primary file only. Dropping a whole set is what the material
        // row above is for — there the names decide where each file lands.
        let dropped = payload.path.clone();
        let (entity, slot) = (zone.entity, zone.slot);
        commands.queue(move |w: &mut World| {
            let rel = asset_relative(w, &dropped);
            slot_edit(w, entity, move |graph| texture_slots::set_slot_texture(graph, slot, &rel));
        });
        break;
    }
}

/// Accent the row being dragged over, so the target channel is unambiguous
/// before the mouse comes up.
pub(super) fn tex_slot_highlight(
    payload: Option<Res<AssetDragPayload>>,
    mut zones: Query<(&RelativeCursorPosition, &mut BorderColor), With<TexSlotZone>>,
) {
    for (rcp, mut bc) in &mut zones {
        let active = payload
            .as_ref()
            .is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(IMAGE_EXTENSIONS));
        let want = BorderColor::all(if active { rgb(accent()) } else { Color::NONE });
        if *bc != want {
            *bc = want;
        }
    }
}

/// Click a row (with nothing being dragged) → pick a file for that channel.
pub(super) fn tex_slot_browse(
    q: Query<(&Interaction, &TexSlotZone), Changed<Interaction>>,
    payload: Option<Res<AssetDragPayload>>,
    mut commands: Commands,
) {
    if payload.as_ref().is_some_and(|p| p.is_detached) {
        return;
    }
    for (interaction, zone) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, slot) = (zone.entity, zone.slot);
        #[cfg(not(target_arch = "wasm32"))]
        commands.queue(move |w: &mut World| {
            let Some(file) = rfd::FileDialog::new().add_filter("Image", IMAGE_EXTENSIONS).pick_file() else {
                return;
            };
            let rel = asset_relative(w, &file);
            slot_edit(w, entity, move |graph| texture_slots::set_slot_texture(graph, slot, &rel));
        });
        #[cfg(target_arch = "wasm32")]
        let _ = (entity, slot, &mut commands);
    }
}

/// The eye on a texture row → apply or un-apply that channel on the mesh.
///
/// Routed through `slot_edit` like every other slot change, so the graph is
/// re-saved, recompiled and re-read: the mesh updates immediately and the row
/// rebuilds with the icon the graph now justifies.
pub(super) fn tex_slot_mute(q: Query<(&Interaction, &TexSlotMuteBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, slot, muted) = (btn.entity, btn.slot, btn.muted);
        commands.queue(move |w: &mut World| {
            slot_edit(w, entity, move |graph| texture_slots::set_slot_muted(graph, slot, !muted));
        });
    }
}

pub(super) fn tex_slot_clear(q: Query<(&Interaction, &TexSlotClearBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, slot) = (btn.entity, btn.slot);
        commands.queue(move |w: &mut World| {
            slot_edit(w, entity, move |graph| texture_slots::clear_slot(graph, slot));
        });
    }
}
