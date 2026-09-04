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

/// [`overlay`] with an explicit fixed `width`×`height` in px, and optional
/// border.
pub fn overlay_sized(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    width: f32,
    height: f32,
    bordered: bool,
) -> (Entity, Entity) {
    overlay_val(commands, fonts, title, Val::Px(width), Val::Px(height), bordered)
}

/// [`overlay_sized`] with the card sized in whatever [`Val`] suits — for an
/// overlay that is a *workspace* rather than a dialog and should take a share of
/// the window rather than a fixed number of pixels.
pub fn overlay_val(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    width: Val,
    height: Val,
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
                width,
                height,
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
/// Dismiss the **topmost** overlay on Escape, its X, or a click outside it.
///
/// Topmost, not all of them: overlays stack, and one raised from another has to
/// be closable without taking its opener with it. The marketplace stacks three
/// deep — the store at 9400, an item at 9600, the install dialog at 9700 — and
/// closing the install dialog used to despawn every `Overlay` in the world, so
/// pressing its X dropped the user back to the editor instead of to the store
/// they opened it from.
///
/// `GlobalZIndex` is the ordering because it is the same thing that decides
/// what is drawn on top, so what the user sees in front is what closes. An
/// overlay that never set one keeps `overlay_val`'s default, and overlays
/// sharing the top index close together — they are siblings, not a stack.
pub(crate) fn overlay_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    closes: Query<&Interaction, (Changed<Interaction>, With<OverlayClose>)>,
    overlays: Query<(Entity, Option<&GlobalZIndex>), With<Overlay>>,
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
    if !(esc || x || click_outside) {
        return;
    }
    let top = overlays
        .iter()
        .map(|(_, z)| z.map(|z| z.0).unwrap_or(0))
        .max()
        .unwrap_or(0);
    for (entity, z) in &overlays {
        if z.map(|z| z.0).unwrap_or(0) == top {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod dismiss_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A bare overlay: the marker plus a z-index, which is all `overlay_dismiss`
    /// reads.
    fn overlay(world: &mut World, z: i32) -> Entity {
        world.spawn((Overlay, GlobalZIndex(z))).id()
    }

    /// Escape with no cursor over any card, which is also the click-outside path.
    fn press_escape(world: &mut World) {
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::Escape);
        world.insert_resource(keys);
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.run_system_once(overlay_dismiss).unwrap();
    }

    #[test]
    fn a_lone_overlay_closes() {
        let mut world = World::new();
        let only = overlay(&mut world, 8000);
        press_escape(&mut world);
        assert!(world.get_entity(only).is_err(), "the only overlay should close");
    }

    /// The bug: the marketplace stacks the store (9400) under the install
    /// dialog (9700), and dismissing the dialog took the store with it.
    #[test]
    fn only_the_topmost_of_a_stack_closes() {
        let mut world = World::new();
        let store = overlay(&mut world, 9400);
        let install = overlay(&mut world, 9700);

        press_escape(&mut world);

        assert!(world.get_entity(install).is_err(), "the dialog should close");
        assert!(
            world.get_entity(store).is_ok(),
            "the overlay it was opened from must survive"
        );
    }

    /// Closing the top one twice walks back down the stack, rather than the
    /// second dismissal doing nothing.
    #[test]
    fn dismissing_again_closes_the_next_one_down() {
        let mut world = World::new();
        let store = overlay(&mut world, 9400);
        overlay(&mut world, 9700);

        press_escape(&mut world);
        press_escape(&mut world);

        assert!(world.get_entity(store).is_err(), "the second dismissal closes the store");
    }

    /// Same index means siblings, not a stack — they close together.
    #[test]
    fn overlays_sharing_the_top_index_close_together() {
        let mut world = World::new();
        let a = overlay(&mut world, 9000);
        let b = overlay(&mut world, 9000);
        press_escape(&mut world);
        assert!(world.get_entity(a).is_err() && world.get_entity(b).is_err());
    }

    /// An overlay that never set an index still has `overlay_val`'s default, so
    /// a missing component must not read as "on top of everything".
    #[test]
    fn an_overlay_without_a_z_index_is_treated_as_the_bottom() {
        let mut world = World::new();
        let bare = world.spawn(Overlay).id();
        let top = overlay(&mut world, 9700);
        press_escape(&mut world);
        assert!(world.get_entity(top).is_err(), "the indexed overlay is on top");
        assert!(world.get_entity(bare).is_ok(), "the un-indexed one is below it");
    }

    #[test]
    fn nothing_happens_without_a_dismissal() {
        let mut world = World::new();
        let only = overlay(&mut world, 8000);
        world.insert_resource(ButtonInput::<KeyCode>::default());
        world.insert_resource(ButtonInput::<MouseButton>::default());
        world.run_system_once(overlay_dismiss).unwrap();
        assert!(world.get_entity(only).is_ok());
    }
}
