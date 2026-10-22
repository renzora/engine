//! The Effect Preview panel: the rig's image filling the panel, with its
//! controls floating over it.
//!
//! Top left: compact Backdrop, Speed and Move dropdowns. Top right: Restart,
//! and a "..." menu holding Loop and Restart on Edit. Bottom left: one chip per
//! emitter (its swatch and name; click to edit it). Bottom right: a stats chip
//! (particles alive, spawns dropped because an emitter was full, seconds in,
//! and whether the emitters simulate on the CPU, the GPU or both).
//! Kept to that so the effect, not the controls, fills the panel.
//!
//! There is no View dropdown. An overdraw view needs the particle renderer to
//! draw every quad as a flat additive count, which is a second material per
//! renderer; until that exists a dropdown with one entry would only advertise
//! the gap.
//!
//! The chips sit on header-coloured pills rather than a translucent wash,
//! which reads on any backdrop without greying the effect behind it.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{dropdown_compact, HoverTooltip};
use renzora_ember::UiInteraction;

use crate::preview::{Backdrop, Motion, RestartPreview, VfxPreviewImage, VfxPreviewSettings, VfxPreviewStats, PANEL, SPEEDS};
use crate::state::VfxEditorState;

pub struct VfxPreviewPanel;

impl Plugin for VfxPreviewPanel {
    fn build(&self, app: &mut App) {
        app.register_panel_content(PANEL, false, build);
        app.add_systems(
            Update,
            (restart_click, chip_click, options_click)
                .run_if(in_state(renzora_editor_framework::SplashState::Editor))
                .run_if(renzora_ember::dock::panel_active(PANEL)),
        );
    }
}

#[derive(Component)]
struct RestartBtn;

#[derive(Component)]
struct EmitterChip(usize);

fn open(w: &Rx) -> bool {
    w.get_resource::<VfxEditorState>().is_some_and(|s| s.is_open() && s.load_error.is_none())
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(window_bg())),
            Name::new("vfx-preview-panel"),
        ))
        .id();

    let empty = crate::templates::empty_state(commands, fonts, "Open a .vfx from the Assets panel to preview it here, or start from a template.");
    bind_display(commands, empty, |w| !open(w));
    let failed = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(close_red())),
            Node { position_type: PositionType::Absolute, left: Val::Px(16.0), top: Val::Px(16.0), right: Val::Px(16.0), ..default() },
        ))
        .id();
    bind_text(commands, failed, |w| {
        w.get_resource::<VfxEditorState>()
            .and_then(|s| s.load_error.as_ref().map(|e| format!("This effect could not be opened: {e}")))
            .unwrap_or_default()
    });

    let image = commands
        .spawn((
            ImageNode::default(),
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            UiInteraction::default(),
            crate::preview::PreviewViewport,
            Name::new("vfx-preview-image"),
        ))
        .id();
    bind_display(commands, image, open);
    bind_with(
        commands,
        image,
        |w| w.get_resource::<VfxPreviewImage>().map(|p| p.handle.clone()),
        |w, e, h: &Option<Handle<Image>>| {
            if let (Some(h), Some(mut n)) = (h, w.get_mut::<ImageNode>(e)) {
                if n.image != *h {
                    n.image = h.clone();
                }
            }
        },
    );

    let top = top_bar(commands, fonts);
    bind_display(commands, top, open);
    let bottom = bottom_bar(commands, fonts);
    bind_display(commands, bottom, open);

    commands.entity(root).add_children(&[empty, failed, image, top, bottom]);
    renzora_editor_framework::mark_drop_zone(commands, root);
    root
}

/// A pill the controls sit on: the header colour, rounded, so it reads on a
/// sky as well as on black.
fn pill(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id()
}

fn label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands.spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), PASS_THROUGH)).id()
}

fn top_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                right: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            },
            PASS_THROUGH,
            Name::new("vfx-preview-top"),
        ))
        .id();

    let backdrop_labels: Vec<&str> = Backdrop::ALL.iter().map(|b| b.label()).collect();
    let backdrop = select(commands, fonts, "Backdrop", &backdrop_labels, |s| Backdrop::ALL.iter().position(|b| *b == s.backdrop).unwrap_or(0), |s, v| {
        s.backdrop = Backdrop::ALL.get(v).copied().unwrap_or_default();
    });
    let speed_labels: Vec<String> = SPEEDS.iter().map(|s| format!("{s}x")).collect();
    let speed_refs: Vec<&str> = speed_labels.iter().map(String::as_str).collect();
    let speed = select(commands, fonts, "Speed", &speed_refs, |s| s.speed, |s, v| s.speed = v.min(SPEEDS.len() - 1));
    let motion_labels: Vec<&str> = Motion::ALL.iter().map(|m| m.label()).collect();
    let motion = select(commands, fonts, "Move", &motion_labels, |s| Motion::ALL.iter().position(|m| *m == s.motion).unwrap_or(0), |s, v| {
        s.motion = Motion::ALL.get(v).copied().unwrap_or_default();
    });
    // Restart and the two playback options moved off the bar: as two switches
    // and a labelled pill they made five more chips over the effect, which was
    // most of what read as clutter. Restart is one icon; Loop and Restart on
    // Edit live in the "..." menu beside it.
    let spacer = commands.spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH)).id();
    let restart = icon_pill(commands, fonts, "arrow-counter-clockwise", "Restart the effect from the beginning");
    commands.entity(restart).insert(RestartBtn);
    let more = icon_pill(commands, fonts, "dots-three", "Playback options");
    commands.entity(more).insert(OptionsBtn);

    commands.entity(bar).add_children(&[backdrop, speed, motion, spacer, restart, more]);
    bar
}

/// "..." on the preview: the playback options as a menu.
#[derive(Component)]
struct OptionsBtn;

/// An icon-only control on the preview, the same pill as the dropdown chips.
fn icon_pill(commands: &mut Commands, fonts: &EmberFonts, icon: &str, tip: &str) -> Entity {
    let p = pill(commands);
    commands.entity(p).insert((
        UiInteraction::default(),
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        HoverTooltip::new(tip),
    ));
    commands.entity(p).entry::<Node>().and_modify(|mut n| {
        n.width = Val::Px(28.0);
        n.height = Val::Px(28.0);
        n.padding = UiRect::ZERO;
        n.justify_content = JustifyContent::Center;
    });
    bind_bg(commands, p, move |w| {
        let hot = matches!(w.get::<UiInteraction>(p), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        rgb(if hot { hover_bg() } else { header_bg() })
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    commands.entity(glyph).insert(PASS_THROUGH);
    commands.entity(p).add_child(glyph);
    p
}

/// Open the playback options menu under the "..." button. Each row shows a
/// tick when its option is on and toggles it.
fn options_click(
    q: Query<&UiInteraction, (With<OptionsBtn>, Changed<UiInteraction>)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    settings: Option<Res<VfxPreviewSettings>>,
    mut commands: Commands,
) {
    let (Some(fonts), Some(settings)) = (fonts, settings) else { return };
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    {
        // Opened leftward from the button: it sits at the panel's right edge.
        let menu = renzora_ember::widgets::screen_menu(&mut commands, (cursor.x - 170.0).max(0.0), cursor.y + 14.0);
        let tick = |on: bool| if on { "check-square" } else { "square" };
        let looping = renzora_ember::widgets::menu_item(&mut commands, &fonts, tick(settings.looping), "Loop", |w| {
            if let Some(mut s) = w.get_resource_mut::<VfxPreviewSettings>() {
                s.looping = !s.looping;
            }
        });
        let restart_edit = renzora_ember::widgets::menu_item(&mut commands, &fonts, tick(settings.restart_on_edit), "Restart on Edit", |w| {
            if let Some(mut s) = w.get_resource_mut::<VfxPreviewSettings>() {
                s.restart_on_edit = !s.restart_on_edit;
            }
        });
        commands.entity(menu).add_children(&[looping, restart_edit]);
    }
}

fn select(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    options: &[&str],
    get: fn(&VfxPreviewSettings) -> usize,
    set: fn(&mut VfxPreviewSettings, usize),
) -> Entity {
    let p = pill(commands);
    let l = label(commands, fonts, name);
    // The toolbar size: the default's 140px minimum made each chip a wide
    // slab over the effect.
    let dd = dropdown_compact(commands, fonts, options, 0, 72.0);
    bind_2way(
        commands,
        dd,
        move |w: &Rx| w.get_resource::<VfxPreviewSettings>().map(get).unwrap_or(0),
        move |w: &mut World, v: &usize| {
            if let Some(mut s) = w.get_resource_mut::<VfxPreviewSettings>() {
                if get(&s) != *v {
                    set(&mut s, *v);
                }
            }
        },
    );
    commands.entity(p).add_children(&[l, dd]);
    p
}

fn bottom_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(8.0),
                left: Val::Px(8.0),
                right: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            },
            PASS_THROUGH,
            Name::new("vfx-preview-bottom"),
        ))
        .id();

    let stats = pill(commands);
    let icon = icon_text(commands, &fonts.phosphor, "gauge", accent(), 12.0);
    let text = commands.spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, text, |w| {
        let s = w.get_resource::<VfxPreviewStats>().cloned().unwrap_or_default();
        format!("{} alive \u{00b7} {} dropped \u{00b7} {:.1}s \u{00b7} {}", s.alive, s.dropped, s.time, s.backend_summary())
    });
    bind_with(
        commands,
        stats,
        |w| w.get_resource::<VfxPreviewStats>().is_some_and(|s| s.dropped > 0),
        |w, e, dropping: &bool| {
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(rgb(if *dropping { warn_amber() } else { border() }));
            }
        },
    );
    commands.entity(stats).insert(HoverTooltip::new(
        "Dropped counts spawns refused because an emitter was at its capacity. The last field is where the emitters simulate: CPU, GPU, or both; a GPU emitter's counts are a frame late",
    ));
    commands.entity(stats).add_children(&[icon, text]);

    let chips = commands.spawn((Node { flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), ..default() }, PASS_THROUGH)).id();
    keyed_list(commands, chips, |w| {
        let names: Vec<String> = w
            .get_resource::<VfxEditorState>()
            .map(|s| s.doc.emitters.iter().map(|e| e.name.clone()).collect())
            .unwrap_or_default();
        let items = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                n.hash(&mut h);
                (i as u64, h.finish())
            })
            .collect();
        KeyedSnapshot { items, build: Box::new(move |c, f, i| emitter_chip(c, f, i, &names[i])) }
    });
    // Emitters on the left, the numbers in the bottom-right corner, as the
    // mockup has them: the effect stays the thing in the middle.
    let spacer = commands.spawn((Node { flex_grow: 1.0, ..default() }, PASS_THROUGH)).id();
    commands.entity(bar).add_children(&[chips, spacer, stats]);
    bar
}

fn emitter_chip(commands: &mut Commands, fonts: &EmberFonts, index: usize, name: &str) -> Entity {
    let chip = pill(commands);
    commands.entity(chip).insert((
        UiInteraction::default(),
        RelativeCursorPosition::default(),
        EmitterChip(index),
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        HoverTooltip::new("Click to edit this emitter"),
    ));
    bind_with(
        commands,
        chip,
        move |w| {
            let active = w.get_resource::<VfxEditorState>().and_then(VfxEditorState::emitter_index) == Some(index);
            let hot = matches!(w.get::<UiInteraction>(chip), Some(UiInteraction::Hovered | UiInteraction::Pressed));
            (active, hot)
        },
        |w, e, &(active, hot): &(bool, bool)| {
            if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                bg.0 = rgb(if hot { hover_bg() } else { header_bg() });
            }
            if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                *b = BorderColor::all(rgb(if active { accent() } else { border() }));
            }
        },
    );
    // The emitter's own swatch, the same one its chip in the graph carries, so
    // the two panels name an emitter the same way. It turns red while the
    // compiler is skipping that emitter.
    let swatch = crate::graph::emitter_swatch(index);
    let dot = commands
        .spawn((Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(swatch)), PASS_THROUGH))
        .id();
    bind_with(
        commands,
        dot,
        move |w| w.get_resource::<VfxPreviewStats>().and_then(|s| s.emitters.get(index).map(|e| e.3)).unwrap_or(true),
        move |w, e, ok: &bool| {
            if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                bg.0 = rgb(if *ok { swatch } else { close_red() });
            }
        },
    );
    let name_t = commands.spawn((Text::new(name), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), PASS_THROUGH)).id();
    commands.entity(chip).add_children(&[dot, name_t]);
    chip
}

fn restart_click(q: Query<&UiInteraction, (With<RestartBtn>, Changed<UiInteraction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        commands.insert_resource(RestartPreview);
    }
}

fn chip_click(q: Query<(&UiInteraction, &EmitterChip), Changed<UiInteraction>>, mut state: ResMut<VfxEditorState>) {
    for (i, chip) in &q {
        if *i == UiInteraction::Pressed && state.active_emitter != chip.0 {
            state.active_emitter = chip.0;
            state.selected_node = None;
        }
    }
}
