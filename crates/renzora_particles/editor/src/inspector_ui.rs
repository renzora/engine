//! The Particle System's inspector drawer.
//!
//! The component was drawn from declarative fields, which give every field a
//! full-width row: Edit Effect, Play, Pause, Stop and Restart came out as five
//! stacked slabs of button, taller than the rest of the component together.
//! Here they are what they are, one transport control: four icon buttons in a
//! group, the current state lit, and its name beside them. The effect slot
//! gets its edit button alongside it rather than a row of its own.

use bevy::prelude::*;

use renzora::FieldValue;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{inspector_body, inspector_row};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, dropdown, DragRange, HoverTooltip};
use renzora_ember::UiInteraction;
use renzora_particles::{OneShot, ParticlePlayback, ParticleSystem, PlayState};

/// One transport button: which entity it drives and what it does.
#[derive(Component, Clone, Copy)]
pub(crate) struct TransportBtn {
    entity: Entity,
    action: Transport,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Transport {
    Play,
    Pause,
    Stop,
    Restart,
}

/// The effect slot's edit button.
#[derive(Component, Clone, Copy)]
pub(crate) struct EditEffectBtn(Entity);

const ONE_SHOT: [&str; 3] = ["Keep", "Deactivate", "Despawn"];

pub(crate) fn particle_system_ui(world: &mut World, entity: Entity) -> Entity {
    if world.get::<ParticleSystem>(entity).is_none() {
        return world.spawn(Node::default()).id();
    }
    inspector_body(world, move |commands, fonts| {
        let col = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(2.0)), ..default() })
            .id();

        // Effect: the asset slot (drop a .vfx, or a .particle to convert),
        // with the editor one click away beside it.
        let slot = renzora_inspector::asset_drop_field(
            commands,
            fonts,
            entity,
            |w, e| w.get::<ParticleSystem>(e).map(|s| FieldValue::Asset((!s.path.is_empty()).then(|| s.path.clone()))),
            crate::set_effect_path,
            vec!["vfx".to_string(), "particle".to_string()],
        );
        commands.entity(slot).entry::<Node>().and_modify(|mut n| {
            n.flex_grow = 1.0;
            n.min_width = Val::Px(0.0);
        });
        let edit = icon_button(commands, fonts, "pencil-simple", "Edit this effect in the Particles workspace");
        commands.entity(edit).insert(EditEffectBtn(entity));
        let effect = commands
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), flex_grow: 1.0, min_width: Val::Px(0.0), ..default() })
            .id();
        commands.entity(effect).add_children(&[slot, edit]);
        let effect_row = inspector_row(commands, &fonts.ui, "Effect", effect);

        // Playback: the transport group and the state it is in.
        let group = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    padding: UiRect::all(Val::Px(2.0)),
                    column_gap: Val::Px(2.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(rgb(faint_bg())),
                BorderColor::all(rgb(border())),
            ))
            .id();
        let buttons: Vec<Entity> = [
            (Transport::Play, "play", "Play"),
            (Transport::Pause, "pause", "Pause"),
            (Transport::Stop, "stop", "Stop and clear every particle"),
            (Transport::Restart, "arrow-counter-clockwise", "Restart from the beginning"),
        ]
        .into_iter()
        .map(|(action, icon, tip)| transport_button(commands, fonts, entity, action, icon, tip))
        .collect();
        commands.entity(group).add_children(&buttons);
        let state = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { margin: UiRect::left(Val::Px(8.0)), ..default() })).id();
        bind_text(commands, state, move |w| {
            match w.get::<ParticlePlayback>(entity).map(|p| p.state) {
                Some(PlayState::Playing) => "Playing",
                Some(PlayState::Paused) => "Paused",
                Some(PlayState::Stopped) | None => "Stopped",
            }
            .to_string()
        });
        let playback = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() }).id();
        commands.entity(playback).add_children(&[group, state]);
        let playback_row = inspector_row(commands, &fonts.ui, "Playback", playback);

        // Speed: the time scale, as a scrubbable value.
        let speed = drag_value(commands, &fonts.ui, "", text_primary(), 1.0, 0.01);
        commands.entity(speed).insert(DragRange { min: 0.0, max: 10.0 });
        bind_2way(
            commands,
            speed,
            move |w| w.get::<ParticlePlayback>(entity).map(|p| p.time_scale).unwrap_or(1.0),
            move |w, v: &f32| {
                if let Some(mut p) = w.get_mut::<ParticlePlayback>(entity) {
                    if p.time_scale != *v {
                        p.time_scale = v.max(0.0);
                    }
                }
            },
        );
        let speed_row = inspector_row(commands, &fonts.ui, "Speed", speed);

        // When Finished: what a non-looping effect does once it is over.
        let finished = dropdown(commands, fonts, &ONE_SHOT, 0);
        bind_2way(
            commands,
            finished,
            move |w| match w.get::<ParticlePlayback>(entity).map(|p| p.one_shot) {
                Some(OneShot::Deactivate) => 1,
                Some(OneShot::Despawn) => 2,
                _ => 0,
            },
            move |w, i: &usize| {
                let want = match i {
                    1 => OneShot::Deactivate,
                    2 => OneShot::Despawn,
                    _ => OneShot::Keep,
                };
                if let Some(mut p) = w.get_mut::<ParticlePlayback>(entity) {
                    if p.one_shot != want {
                        p.one_shot = want;
                    }
                }
            },
        );
        commands.entity(finished).insert(HoverTooltip::new("What a non-looping effect does once it is over"));
        let finished_row = inspector_row(commands, &fonts.ui, "When Finished", finished);

        commands.entity(col).add_children(&[effect_row, playback_row, speed_row, finished_row]);
        col
    })
}

fn icon_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, tip: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(24.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            HoverTooltip::new(tip),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered | UiInteraction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    commands.entity(glyph).insert(PASS_THROUGH);
    commands.entity(btn).add_child(glyph);
    btn
}

/// A transport button, lit in the accent while the effect is in its state
/// (Restart has no state of its own, so it is never lit).
fn transport_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, action: Transport, icon: &str, tip: &str) -> Entity {
    let btn = icon_button(commands, fonts, icon, tip);
    commands.entity(btn).insert(TransportBtn { entity, action });
    let lit = move |w: &renzora_ember::reactive::Rx| {
        let state = w.get::<ParticlePlayback>(entity).map(|p| p.state);
        matches!(
            (action, state),
            (Transport::Play, Some(PlayState::Playing)) | (Transport::Pause, Some(PlayState::Paused)) | (Transport::Stop, Some(PlayState::Stopped))
        )
    };
    bind_bg(commands, btn, move |w| {
        let hot = matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        if lit(w) {
            mix(card_bg(), accent(), 0.35)
        } else if hot {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    btn
}

pub(crate) fn transport_click(q: Query<(&UiInteraction, &TransportBtn), Changed<UiInteraction>>, mut playback: Query<&mut ParticlePlayback>) {
    for (i, btn) in &q {
        if *i != UiInteraction::Pressed {
            continue;
        }
        let Ok(mut p) = playback.get_mut(btn.entity) else { continue };
        match btn.action {
            Transport::Play => p.play(),
            Transport::Pause => p.pause(),
            Transport::Stop => p.stop(),
            Transport::Restart => p.restart(),
        }
    }
}

pub(crate) fn edit_effect_click(q: Query<(&UiInteraction, &EditEffectBtn), Changed<UiInteraction>>, mut commands: Commands) {
    for (i, btn) in &q {
        if *i == UiInteraction::Pressed {
            let entity = btn.0;
            commands.queue(move |w: &mut World| crate::edit_effect(w, entity));
        }
    }
}
