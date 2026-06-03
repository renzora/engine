//! Overlay — a reusable centered modal: a dimmed full-screen backdrop + a card
//! with a title bar (title + X close). [`overlay`] returns `(root, content)` —
//! fill `content` with anything (a [`super::search::search_list`], a form, an
//! about box…). Ember dismisses it on Escape, a backdrop click, or the X.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::*;


/// The backdrop root of an open overlay (despawn it to close).
#[derive(Component)]
pub struct Overlay;

/// Marks a full-screen modal surface that should capture wheel/scroll so it
/// never bleeds through to panels behind it — *without* the click-outside
/// dismiss behavior of [`Overlay`]. [`overlay`] roots carry both; a custom modal
/// (e.g. the Settings overlay, which closes via its own button) adds just this.
#[derive(Component)]
pub struct ModalSurface;

#[derive(Component)]
pub(crate) struct OverlayCard;

#[derive(Component)]
pub(crate) struct OverlayClose;

/// Spawn a centered modal (default size, bordered). Returns
/// `(backdrop_root, content)`.
pub fn overlay(commands: &mut Commands, fonts: &EmberFonts, title: &str) -> (Entity, Entity) {
    overlay_sized(commands, fonts, title, 540.0, 520.0, true)
}

/// [`overlay`] with an explicit fixed `width`×`height` and optional border.
pub fn overlay_sized(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    width: f32,
    height: f32,
    bordered: bool,
) -> (Entity, Entity) {
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(8000),
            FocusPolicy::Block,
            Overlay,
            ModalSurface,
            Name::new("overlay"),
        ))
        .id();

    let card = commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                border: UiRect::all(Val::Px(if bordered { 1.0 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            RelativeCursorPosition::default(),
            OverlayCard,
            Name::new("overlay-card"),
        ))
        .id();

    let titlebar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(9.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
        ))
        .id();
    let title_text = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let close = commands
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            OverlayClose,
            Name::new("overlay-close"),
        ))
        .id();
    let close_icon = icon_text(commands, &fonts.phosphor, "x", text_muted(), 13.0);
    commands.entity(close).add_child(close_icon);
    commands
        .entity(titlebar)
        .add_children(&[title_text, spacer, close]);

    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                ..default()
            },
            Name::new("overlay-content"),
        ))
        .id();

    commands.entity(card).add_children(&[titlebar, content]);
    commands.entity(root).add_child(card);
    (root, content)
}

/// Escape, a backdrop click (outside the card), or the X closes the overlay.
pub(crate) fn overlay_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    closes: Query<&Interaction, (Changed<Interaction>, With<OverlayClose>)>,
    overlays: Query<Entity, With<Overlay>>,
    cards: Query<&RelativeCursorPosition, With<OverlayCard>>,
    mut commands: Commands,
) {
    if overlays.is_empty() {
        return;
    }
    let esc = keys.just_pressed(KeyCode::Escape);
    let x = closes.iter().any(|i| *i == Interaction::Pressed);
    let click_outside =
        mouse.just_pressed(MouseButton::Left) && !cards.iter().any(|r| r.cursor_over);
    if esc || x || click_outside {
        for o in &overlays {
            commands.entity(o).despawn();
        }
    }
}
