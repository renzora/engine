//! Shared chrome for the editor's panel toolbars — the strip itself, the groups
//! inside it, the separators between them, and a plain icon button.
//!
//! This exists because the viewport toolbar and the UI editor toolbar had each
//! grown their own copy of the same design, and the copies had drifted: one was
//! a fixed-height non-wrapping row on `header_bg` with 22×20 buttons, the other
//! a wrapping row on `panel_bg` with 28×28 buttons and a different bottom rule.
//! Both were reasonable, which is exactly the problem — nothing said which was
//! the toolbar design, so they read as two different pieces of software.
//!
//! The numbers live here now, once. A panel that wants a toolbar composes
//! [`toolbar_bar`] + [`toolbar_group`] + [`toolbar_separator`] and gets the
//! house style without deciding anything.
//!
//! What it deliberately does *not* cover is behaviour: the viewport's tool
//! buttons are registry-backed, carry predicates, and highlight from world
//! state, while the UI editor's are plain click handlers. Those stay with their
//! owners. Only the look is shared.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use crate::cursor_icon::HoverCursor;
use crate::font::{icon_text, EmberFonts};
use crate::theme::{border, divider, hover_bg, panel_bg, rgb, text_primary, value_text};
use crate::widgets::{arrange_row, OverlaySurface};

/// Edge length of a square toolbar button.
pub const TOOLBAR_BTN: f32 = 28.0;
/// Icon glyph size inside a [`TOOLBAR_BTN`] square.
pub const TOOLBAR_ICON: f32 = 15.0;
/// Corner rounding on a toolbar button.
pub const TOOLBAR_RADIUS: f32 = 3.0;

/// The toolbar strip: a wrapping row that sits above a panel's content.
///
/// Wrapping rather than clipping, and grouped rather than flat, so a strip with
/// more controls than fit becomes two lines instead of hiding the overflow
/// behind a menu — and a group that does not fit moves down whole rather than
/// being squeezed.
///
/// The bottom rule is not decoration. Without it the strip and whatever follows
/// it meet with nothing between them, and a two-line toolbar in particular reads
/// as an indeterminate slab of chrome rather than as one band that ends here.
pub fn toolbar_bar(commands: &mut Commands, name: &str) -> Entity {
    let bar = arrange_row(commands, name);
    commands.entity(bar).insert((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            align_content: AlignContent::FlexStart,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(2.0),
            row_gap: Val::Px(2.0),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
            flex_shrink: 0.0,
            border: UiRect::bottom(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(rgb(panel_bg())),
        BorderColor::all(rgb(divider())),
        bevy::ui::RelativeCursorPosition::default(),
        OverlaySurface,
    ));
    // Track the live theme — the colours set above only cover the first frame,
    // so without this a toolbar keeps the palette it was born under and a theme
    // switch leaves it behind. The viewport's strip already did this; the UI
    // editor's did not, which is one of the ways the two had drifted. There is
    // no `bind_border`, so the closing rule goes through the generic binding.
    crate::reactive::tracked::bind_bg(commands, bar, |_| rgb(panel_bg()));
    crate::reactive::tracked::bind_with(
        commands,
        bar,
        |_: &crate::reactive::Rx| rgb(divider()),
        |world, target, c: &Color| {
            if let Some(mut b) = world.get_mut::<BorderColor>(target) {
                *b = BorderColor::all(*c);
            }
        },
    );
    bar
}

/// A run of related buttons inside a [`toolbar_bar`].
///
/// Tighter internally (1px) than the bar's gap between groups (2px), which is
/// what makes "these six align buttons are one control" legible without drawing
/// a box around them. Grouping is also what keeps a wrap from splitting a
/// cluster down the middle.
pub fn toolbar_group(commands: &mut Commands, name: &str) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                flex_shrink: 0.0,
                ..default()
            },
            FocusPolicy::Pass,
            Name::new(name.to_string()),
        ))
        .id()
}

/// A hairline between two groups, for the harder divisions that spacing alone
/// does not carry.
pub fn toolbar_separator(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(20.0),
                margin: UiRect::horizontal(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("toolbar-sep"),
        ))
        .id()
}

/// Height of the inline controls that sit in a toolbar next to the square
/// buttons — pills, dropdowns, scrub fields. Shorter than [`TOOLBAR_BTN`] so a
/// row of them reads as one band rather than as stacked boxes.
pub const TOOLBAR_INLINE_H: f32 = 22.0;

/// The parts of a [`toolbar_pill`], so the caller can attach its own markers and
/// bindings without the widget having to know what the pill controls.
pub struct ToolbarPill {
    /// The pill itself. Bind its background to reflect the on/off state.
    pub root: Entity,
    /// The icon half — the clickable toggle. Put the marker component here.
    pub toggle: Entity,
    /// The glyph inside the toggle. Bind its colour to the on/off state.
    pub icon: Entity,
    /// The number half, a flat `drag_value`. Bind it two-way to the setting.
    pub value: Entity,
}

/// A toggle fused to the number it governs: `[icon│value]` in one pill.
///
/// Two halves in one control because they are one idea — "snap, by this much".
/// Split into a separate icon button and a separate boxed field they read as two
/// unrelated widgets that happen to be adjacent, which is what the UI editor's
/// toolbar looked like before it used this.
///
/// The divider is what keeps the two hit areas legible; without it the pill
/// looks like a single button that mysteriously scrubs on one end.
pub fn toolbar_pill(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    min: f32,
    max: f32,
    step: f32,
) -> ToolbarPill {
    let glyph = icon_text(commands, &fonts.phosphor, icon, value_text(), 13.0);
    let toggle = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(TOOLBAR_INLINE_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(TOOLBAR_RADIUS)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("toolbar-pill-toggle"),
        ))
        .id();
    commands.entity(toggle).add_child(glyph);

    let divider_bar = commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(14.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("toolbar-pill-divider"),
        ))
        .id();

    let value = crate::widgets::drag_value_flat(commands, &fonts.ui, "", value_text(), min, step);
    commands
        .entity(value)
        .insert(crate::widgets::DragRange { min, max });
    // Narrower than the widget's 44px default: toolbar width is precious and
    // these values are one to four characters.
    commands
        .entity(value)
        .entry::<Node>()
        .and_modify(|mut n| n.min_width = Val::Px(32.0));

    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(TOOLBAR_RADIUS)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            Name::new("toolbar-pill"),
        ))
        .id();
    commands
        .entity(root)
        .add_children(&[toggle, divider_bar, value]);
    ToolbarPill {
        root,
        toggle,
        icon: glyph,
        value,
    }
}

/// A plain icon button at toolbar size. Returns `(button, icon)` — the caller
/// keeps the icon entity so it can bind the glyph's colour to whatever state the
/// button reflects.
///
/// No marker component and no click handler: what the button *does* belongs to
/// the panel that built it.
pub fn toolbar_icon_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
) -> (Entity, Entity) {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(TOOLBAR_BTN),
                height: Val::Px(TOOLBAR_BTN),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(TOOLBAR_RADIUS)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), TOOLBAR_ICON);
    commands.entity(btn).add_child(ic);
    (btn, ic)
}
