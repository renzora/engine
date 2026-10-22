//! The tree pane's header: the name of the list on show, and a small sliding
//! switch that picks between the project tree, Recent and Favourites.
//!
//! It replaced a row of three tabs. The panel's own tab strip (Assets, Console,
//! Timeline...) sits directly above the pane, so a second tab row right under it
//! read as two layers of tabs, and the words took a row of their own. Three
//! icons in a pill sit in the header beside the label instead, and the label
//! says which list you are in. The lists still take turns, one at a time, so a
//! long Recent or Favourites list scrolls by itself rather than pushing the
//! project tree down.

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_text, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};
use renzora_ember::widgets::HoverTooltip;
use renzora_ember::UiInteraction;

use crate::state::{ListSwitchKnob, NativeAssets, TreeTab, TreeTabBtn};

/// One segment of the switch, and the knob behind it.
const SEG_W: f32 = 26.0;
const SEG_H: f32 = 20.0;
/// The track's inner padding: the knob sits this far in from its edge.
const TRACK_PAD: f32 = 2.0;
/// How long the knob takes to slide to a new segment, in seconds.
const SLIDE_SECS: f32 = 0.12;

fn tab_of(w: &Rx) -> TreeTab {
    w.get_resource::<NativeAssets>().map(|s| s.tree_tab).unwrap_or(TreeTab::Folders)
}

fn knob_left(tab: TreeTab) -> f32 {
    TRACK_PAD + tab.index() as f32 * SEG_W
}

/// The header row: list label on the left, the switch on the right.
pub(crate) fn build_list_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::new(Val::Px(12.0), Val::Px(8.0), Val::Px(2.0), Val::Px(2.0)),
                ..default()
            },
            Name::new("assets-list-header"),
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(TreeTab::Folders.label()),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Pickable::IGNORE,
        ))
        .id();
    bind_text(commands, label, |w| tab_of(w).label());

    let track = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                flex_direction: FlexDirection::Row,
                flex_shrink: 0.0,
                padding: UiRect::all(Val::Px(TRACK_PAD - 1.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::faint_bg())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("assets-list-switch"),
        ))
        .id();
    // The knob is absolutely placed behind the segments and eased between them
    // by `slide_list_knob`; it starts on the list already selected.
    let knob = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(knob_left(TreeTab::Folders) - 1.0),
                top: Val::Px(TRACK_PAD - 1.0),
                width: Val::Px(SEG_W),
                height: Val::Px(SEG_H),
                border_radius: BorderRadius::all(Val::Px(SEG_H / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            ListSwitchKnob,
            Pickable::IGNORE,
            Name::new("assets-list-knob"),
        ))
        .id();
    commands.entity(track).add_child(knob);

    for tab in TreeTab::ALL {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(SEG_W),
                    height: Val::Px(SEG_H),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                UiInteraction::default(),
                HoverCursor(SystemCursorIcon::Pointer),
                HoverTooltip::new(tab.label()),
                TreeTabBtn(tab),
                Name::new("assets-list-switch-seg"),
            ))
            .id();
        let glyph = icon_text(commands, &fonts.phosphor, tab.icon(), text_muted(), 13.0);
        commands.entity(glyph).insert(Pickable::IGNORE);
        // White on the knob; muted elsewhere, brightening under the cursor.
        bind_with(
            commands,
            glyph,
            move |w| {
                let active = tab_of(w) == tab;
                let hovered = matches!(
                    w.get::<UiInteraction>(btn),
                    Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
                );
                (active, hovered)
            },
            |w, e, (active, hovered): &(bool, bool)| {
                let c = if *active {
                    rgb(renzora_ember::theme::on_accent())
                } else if *hovered {
                    rgb(text_primary())
                } else {
                    rgb(text_muted())
                };
                if let Some(mut t) = w.get_mut::<TextColor>(e) {
                    t.0 = c;
                }
            },
        );
        commands.entity(btn).add_child(glyph);
        commands.entity(track).add_child(btn);
    }

    commands.entity(header).add_children(&[label, track]);
    header
}

/// Ease the knob toward the selected segment. Does nothing once it is there, so
/// it costs a comparison per frame at rest.
pub(crate) fn slide_list_knob(
    state: Res<NativeAssets>,
    time: Res<Time>,
    mut knobs: Query<&mut Node, With<ListSwitchKnob>>,
) {
    let target = knob_left(state.tree_tab) - 1.0;
    let step = SEG_W * 2.0 / SLIDE_SECS * time.delta_secs();
    for mut node in &mut knobs {
        let Val::Px(left) = node.left else {
            node.left = Val::Px(target);
            continue;
        };
        if (left - target).abs() < 0.01 {
            continue;
        }
        let next = if left < target { (left + step).min(target) } else { (left - step).max(target) };
        node.left = Val::Px(next);
    }
}
