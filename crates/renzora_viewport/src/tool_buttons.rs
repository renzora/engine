//! Registry-driven tool buttons, shared by the viewport's two tool surfaces.
//!
//! A [`ToolEntry`](renzora_editor_framework::ToolEntry) is the same thing
//! wherever it renders: an icon, a tooltip, and three closures deciding whether
//! it shows, whether it's the active one, and what clicking it does. Only the
//! *layout* differs — the horizontal strip across the viewport's top edge
//! ([`crate::toolbar::build_side_toolbar`]) versus the two-column shelf
//! down its left edge ([`crate::native_tool_shelf`]).
//!
//! So the button widget, the separator, and the two driver systems live here and
//! both surfaces spawn from them. That's not just deduplication: the predicates
//! take `&World` and therefore have to be evaluated from an exclusive system, and
//! a second copy of that system would double the per-frame archetype scan for no
//! benefit. [`update_tool_buttons`] handles every button on screen in one pass.

use bevy::color::Hsla;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;
use std::sync::Arc;

use renzora_editor_framework::{EditorCommands, ToolActiveStyle, ToolEntry};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_glyph, EmberFonts};
use renzora_ember::theme::{border, rgb, text_primary};
use renzora_theme::ThemeManager;

/// Toolbar button metrics. Aliases rather than their own numbers: the UI editor
/// builds the same strip, and when both crates held their own copy they drifted
/// into two visibly different toolbars. One definition, in
/// `renzora_ember::widgets::toolbar`.
pub(crate) use renzora_ember::widgets::TOOLBAR_BTN as SIDE_BTN;
pub(crate) use renzora_ember::widgets::TOOLBAR_ICON as SIDE_ICON;

/// A registry-backed tool button: carries the predicates/activator so the
/// per-frame systems can highlight, show/hide, and fire it.
#[derive(Component, Clone)]
pub(crate) struct ToolButton {
    pub glyph: Entity,
    pub visible: Arc<dyn Fn(&World) -> bool + Send + Sync>,
    pub is_active: Arc<dyn Fn(&World) -> bool + Send + Sync>,
    pub active_style: ToolActiveStyle,
    pub activate: Arc<dyn Fn(&mut World) + Send + Sync>,
}

/// A separator between tool groups. Tools hide per-mode via their `visible`
/// predicates, and a whole group can vanish (e.g. the terrain brushes outside a
/// terrain tool) — which used to leave its separators stacked up as dangling
/// lines at the strip's end. The separator shows only while at least one tool on
/// EACH side of it is visible.
///
/// The two sides are deliberately asymmetric: `before` is every tool in *all*
/// the groups ahead of the rule, `after` is only the one group immediately
/// behind it. That is what makes exactly one rule appear between each pair of
/// adjacent visible groups. With `after` spanning the rest of the list instead,
/// a shelf whose only visible groups are #0 and #6 draws all six rules between
/// them — each one has a visible group somewhere ahead and somewhere behind —
/// and 40px of stacked lines opens up between two buttons.
#[derive(Component)]
pub(crate) struct ToolSepVis {
    /// Every tool in the groups ahead of this rule.
    pub before: Vec<Entity>,
    /// The tools of the single group immediately behind this rule.
    pub after: Vec<Entity>,
}

/// Marks a container that's already been populated (so we don't refill it, but a
/// freshly re-created panel still gets filled).
#[derive(Component)]
pub(crate) struct ToolsPopulated;

/// Build one button from a registry entry.
pub(crate) fn tool_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entry: &ToolEntry,
) -> Entity {
    // `entry.icon` is either a kebab-case Phosphor name (resolved via `icon_glyph`)
    // or, for entries that still pass a raw glyph constant, the glyph char itself —
    // fall back to rendering it verbatim when it isn't a known name.
    let glyph_str = icon_glyph(entry.icon)
        .map(|c| c.to_string())
        .unwrap_or_else(|| entry.icon.to_string());
    let glyph = commands
        .spawn((
            Text::new(glyph_str),
            TextFont {
                // 0.19 Parley: font -> FontSource, font_size -> FontSize.
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(SIDE_ICON),
                ..default()
            },
            TextColor(rgb(text_primary())),
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(SIDE_BTN),
                height: Val::Px(SIDE_BTN),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ToolButton {
                glyph,
                visible: entry.visible.clone(),
                is_active: entry.is_active.clone(),
                active_style: entry.active_style,
                activate: entry.activate.clone(),
            },
            HoverCursor(SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(entry.tooltip),
            Name::new(format!("vp-tool:{}", entry.id)),
        ))
        .id();
    commands.entity(btn).add_child(glyph);
    btn
}

/// A vertical rule between sections of the horizontal tool strip.
pub(crate) fn tool_separator(commands: &mut Commands) -> Entity {
    let sep = renzora_ember::widgets::toolbar_separator(commands);
    commands.entity(sep).insert(Name::new("vp-tool-sep"));
    sep
}

/// A horizontal rule between groups of the vertical shelf. Full-width so it
/// reads as a division of the column rather than a stray tick beside it, and so
/// the group after it starts on a fresh row.
pub(crate) fn shelf_separator(commands: &mut Commands, width: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("vp-shelf-sep"),
        ))
        .id()
}

/// Per-frame: evaluate each tool's `visible` (show/hide) and `is_active`
/// (accent highlight). Exclusive because the predicates take `&World`.
pub(crate) fn update_tool_buttons(world: &mut World) {
    let mut q = world.query::<(Entity, &ToolButton, &Interaction)>();
    let collected: Vec<(Entity, ToolButton, Interaction)> = q
        .iter(world)
        .map(|(e, b, i)| (e, b.clone(), *i))
        .collect();
    if collected.is_empty() {
        return;
    }
    let (accent, accent_glyph, hovered, icon_active, icon_inactive) = {
        let Some(tm) = world.get_resource::<ThemeManager>() else {
            return;
        };
        let accent = col(tm.active_theme.semantic.accent);
        (
            accent,
            // A step lighter than the fill uses. The accent is chosen to carry
            // white text on top of it, which makes it dark enough that a glyph
            // *drawn in* it reads as dim rather than lit — the opposite of what
            // an on-toggle should say. Lightening it is what puts the tinted
            // icon at the same visual weight as the filled button beside it.
            //
            // In **HSL**, not `Luminance::lighter`: that works in Lab, where
            // raising lightness pulls the colour toward white and takes the
            // chroma with it. The result was a pale grey-blue that read as
            // washed out rather than lit — brighter, and less blue, which is
            // the one thing this colour has to be. HSL lifts the lightness and
            // leaves the saturation where the theme put it.
            lighten(accent, 0.1),
            col(tm.active_theme.widgets.hovered_bg),
            // White-ish on the accent fill when active; a clear neutral otherwise
            // (so tool icons stay legible on light themes).
            col(tm.active_theme.widgets.active_fg),
            col(tm.active_theme.text.secondary),
        )
    };
    let results: Vec<(Entity, bool, Color, Entity, Color)> = collected
        .iter()
        .map(|(e, b, inter)| {
            let visible = (b.visible)(world);
            let active = (b.is_active)(world);
            let hover = *inter == Interaction::Hovered;
            // A tool takes the accent as a *fill*: it is the one thing the
            // viewport is set to do, and the fill is what says only one of a
            // group can be lit. A toggle tints its glyph instead — see
            // `ToolActiveStyle`.
            let tint = b.active_style == ToolActiveStyle::Tint;
            let bg = match (active && !tint, hover) {
                (true, _) => accent,
                (false, true) => hovered,
                (false, false) => Color::NONE,
            };
            let icol = match (active, tint) {
                (true, true) => accent_glyph,
                (true, false) => icon_active,
                (false, _) => icon_inactive,
            };
            (*e, visible, bg, b.glyph, icol)
        })
        .collect();
    for (e, visible, bg, glyph, icol) in &results {
        if let Some(mut node) = world.get_mut::<Node>(*e) {
            let want = if *visible { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
        if let Some(mut bgc) = world.get_mut::<BackgroundColor>(*e) {
            if bgc.0 != *bg {
                bgc.0 = *bg;
            }
        }
        if let Some(mut tc) = world.get_mut::<TextColor>(*glyph) {
            if tc.0 != *icol {
                tc.0 = *icol;
            }
        }
    }

    // Section separators: visible only while a tool on each side of them is
    // visible, so hidden sections never leave dangling divider lines.
    let vis: std::collections::HashMap<Entity, bool> =
        results.iter().map(|(e, v, ..)| (*e, *v)).collect();
    let any_visible =
        |ents: &[Entity]| ents.iter().any(|e| vis.get(e).copied().unwrap_or(false));
    let mut sq = world.query::<(Entity, &ToolSepVis)>();
    let seps: Vec<(Entity, bool)> = sq
        .iter(world)
        .map(|(e, s)| (e, any_visible(&s.before) && any_visible(&s.after)))
        .collect();
    for (e, show) in seps {
        if let Some(mut node) = world.get_mut::<Node>(e) {
            let want = if show { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
    }

    // The shelf is an overlay floating over the scene: with no visible tool in
    // it, it would sit there as an empty tinted rectangle. Collapse it whole.
    let mut rq = world.query::<(Entity, &ShelfRoot)>();
    let roots: Vec<(Entity, bool)> = rq
        .iter(world)
        .map(|(e, r)| (e, any_visible(&r.buttons)))
        .collect();
    for (e, show) in roots {
        if let Some(mut node) = world.get_mut::<Node>(e) {
            let want = if show { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
    }
}

/// The shelf's outer node, carrying every button in it so the whole overlay can
/// collapse when none of them apply. Lives here rather than in the shelf module
/// so [`update_tool_buttons`] can resolve it in the same pass that computed the
/// per-button visibility.
#[derive(Component)]
pub(crate) struct ShelfRoot {
    pub buttons: Vec<Entity>,
}

pub(crate) fn tool_button_click(
    q: Query<(&Interaction, &ToolButton), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let activate = btn.activate.clone();
        cmds.push(move |w: &mut World| (activate)(w));
    }
}

fn col(c: renzora_theme::ThemeColor) -> Color {
    let [r, g, b, _a] = c.to_array();
    Color::srgb_u8(r, g, b)
}

/// Raise a colour's HSL lightness by `amount`, keeping its hue and saturation.
///
/// The cap matters as much as the lift: past about 0.75 an HSL colour is on its
/// way to white whatever its saturation says, so a theme whose accent is already
/// pale gets no lift rather than a white glyph that reads as "off".
fn lighten(c: Color, amount: f32) -> Color {
    let mut hsl = Hsla::from(c);
    hsl.lightness = (hsl.lightness + amount).min(0.75);
    Color::from(hsl)
}
