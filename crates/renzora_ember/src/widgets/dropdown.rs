//! Dropdown / combobox — a box that opens a popup option menu.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::Bound;
use crate::theme::*;

/// Max height of an open dropdown list before it scrolls (≈ 8 rows).
const DROPDOWN_MAX_HEIGHT: f32 = 220.0;

/// Where an open menu sits in the global stacking order when nothing around it
/// has claimed a depth of its own — above ordinary panel content, below the
/// popover / menu / modal bands.
const DROPDOWN_Z: i32 = 500;

/// Control height of a [`dropdown_compact`] — matches the 22px button height
/// the editor's toolbar strips use, so a combobox lines up with the icon
/// buttons and snap pills beside it.
const COMPACT_HEIGHT: f32 = 22.0;

#[derive(Component)]
pub(crate) struct EmberDropdown {
    selected: usize,
    open: bool,
    menu: Entity,
    label: Entity,
    options: Vec<String>,
    /// Per-option phosphor icon names (empty when the dropdown has no icons).
    icons: Vec<String>,
    /// The box's leading icon glyph, kept in sync with the selection.
    box_icon: Option<Entity>,
}

/// Phosphor glyph used when an icon name doesn't resolve (matches `icon_text`).
const ICON_FALLBACK: char = '\u{E4C6}';

/// Repoint an existing icon glyph entity to a new phosphor icon by name.
fn set_icon_glyph(texts: &mut Query<&mut Text>, e: Entity, name: &str) {
    let ch = crate::phosphor_map::icon_glyph(name).unwrap_or(ICON_FALLBACK);
    if let Ok(mut t) = texts.get_mut(e) {
        *t = Text::new(ch.to_string());
    }
}

/// One selectable row in a dropdown's menu. Public so a consumer can reach an
/// individual option — the editor's viewport toolbar hides the modes that don't
/// apply to the current 2D/3D view by flipping `Node.display` on the rows whose
/// `dropdown` is its Mode box. `value` is a stable index into the option list
/// the dropdown was built with, so hiding rows never renumbers the rest.
#[derive(Component)]
pub struct EmberDropdownOption {
    pub dropdown: Entity,
    pub value: usize,
}

/// A dropdown / combobox: a box showing the current option; click to open a
/// menu of options below it, click an option to select.
pub fn dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    options: &[&str],
    selected: usize,
) -> Entity {
    build_dropdown(commands, fonts, options, &[], selected, None)
}

/// A [`dropdown`] sized for a toolbar strip: a fixed `width`, tighter padding,
/// and the same 22px control height as the icon buttons it sits next to. Behaves
/// identically otherwise — same `Bound<usize>`, same menu, same dismiss.
///
/// Exists because a toolbar can't spare the default's 140px minimum and 5px
/// vertical padding; without it, callers hand-rolled their own comboboxes and
/// then had to re-implement (and forget) things like overlay pointer-blocking.
pub fn dropdown_compact(
    commands: &mut Commands,
    fonts: &EmberFonts,
    options: &[&str],
    selected: usize,
    width: f32,
) -> Entity {
    build_dropdown(commands, fonts, options, &[], selected, Some(width))
}

/// A dropdown whose options (and the selected box) carry a leading Phosphor
/// icon. `options` is `(icon_name, label)` pairs.
pub fn dropdown_with_icons(
    commands: &mut Commands,
    fonts: &EmberFonts,
    options: &[(&str, &str)],
    selected: usize,
) -> Entity {
    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
    let icons: Vec<&str> = options.iter().map(|(i, _)| *i).collect();
    build_dropdown(commands, fonts, &labels, &icons, selected, None)
}

/// Shared builder for [`dropdown`] / [`dropdown_compact`] /
/// [`dropdown_with_icons`]. `icons` is either empty (no icons) or one Phosphor
/// name per option; `compact` is `Some(width)` for the toolbar size.
fn build_dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    options: &[&str],
    icons: &[&str],
    selected: usize,
    compact: Option<f32>,
) -> Entity {
    let sel = selected.min(options.len().saturating_sub(1));
    let with_icons = !icons.is_empty();
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(compact.unwrap_or(140.0)),
                // Fixed size + no shrink in a toolbar: the strip is a flex row of
                // fixed-size controls, and a combobox that grows with the length
                // of the selected label (or squashes) breaks the line-up. The
                // label clips instead — it already has `no_wrap` + `overflow`.
                width: compact.map_or(Val::Auto, Val::Px),
                height: compact.map_or(Val::Auto, |_| Val::Px(COMPACT_HEIGHT)),
                flex_shrink: compact.map_or(1.0, |_| 0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(if compact.is_some() { 4.0 } else { 8.0 }),
                padding: if compact.is_some() {
                    UiRect::horizontal(Val::Px(6.0))
                } else {
                    UiRect::axes(Val::Px(10.0), Val::Px(5.0))
                },
                border_radius: BorderRadius::all(Val::Px(if compact.is_some() { 3.0 } else { 4.0 })),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("dropdown"),
        ))
        .id();
    // Optional leading icon for the current selection.
    let box_icon = with_icons.then(|| {
        let e = icon_text(
            commands,
            &fonts.phosphor,
            icons.get(sel).copied().unwrap_or(""),
            text_muted(),
            13.0,
        );
        commands.entity(e).insert(Node {
            flex_shrink: 0.0,
            ..default()
        });
        e
    });
    let label = commands
        .spawn((
            Text::new(options.get(sel).copied().unwrap_or("")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            // Clip + truncate a too-long selection instead of wrapping it or
            // pushing the caret off the box.
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 12.0);
    commands.entity(caret).insert(Node {
        flex_shrink: 0.0,
        ..default()
    });
    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                // A compact box can be narrower than its own option labels, so
                // the menu keeps a readable floor of its own.
                min_width: Val::Px(compact.map_or(140.0, |w| w.max(120.0))),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(2.0)),
                margin: UiRect::top(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            // Replaced on open by `dropdown_toggle` with a depth relative to
            // this menu's own ancestors — see `floating_z`.
            GlobalZIndex(DROPDOWN_Z),
            super::popup::OverlaySurface,
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("dropdown-menu"),
        ))
        .id();
    let mut rows = Vec::new();
    for (i, opt) in options.iter().enumerate() {
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                EmberDropdownOption {
                    dropdown: box_e,
                    value: i,
                },
                crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("dropdown-option"),
            ))
            .id();
        if with_icons {
            let glyph = icon_text(
                commands,
                &fonts.phosphor,
                icons.get(i).copied().unwrap_or(""),
                text_muted(),
                13.0,
            );
            commands.entity(row).add_child(glyph);
        }
        let txt = commands
            .spawn((
                Text::new(*opt),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_primary())),
            ))
            .id();
        commands.entity(row).add_child(txt);
        rows.push(row);
    }
    // Wrap the options in a height-capped scroll area so long lists scroll
    // instead of running off-screen.
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    commands.entity(content).add_children(&rows);
    let scroll = super::scroll_area::scroll_area(commands, content, DROPDOWN_MAX_HEIGHT);
    commands.entity(menu).add_child(scroll);
    commands.entity(box_e).insert((
        EmberDropdown {
            selected: sel,
            open: false,
            menu,
            label,
            options: options.iter().map(|s| s.to_string()).collect(),
            icons: icons.iter().map(|s| s.to_string()).collect(),
            box_icon,
        },
        // Carry the selection as `Bound<usize>` so `bind_2way` can drive it both
        // ways (read the model on select, push external changes to the label).
        Bound::<usize>(sel),
    ));
    // [optional icon] · label · caret · menu
    if let Some(bi) = box_icon {
        commands.entity(box_e).add_child(bi);
    }
    commands.entity(box_e).add_children(&[label, caret, menu]);
    box_e
}

/// The depth a floating child of `anchor` needs in order to sit above its own
/// container, never below `base`.
///
/// `GlobalZIndex` is *global*: it pulls a node out of its parent's stacking
/// context entirely, so a fixed depth is only safe while no ancestor claims a
/// higher one. The import window is a full-screen surface at 900 with an opaque
/// background, which put every dropdown inside it at 500 — behind that
/// background. The menu opened, drew under the window, and took no clicks,
/// because Bevy's picking gave the pointer to the background stacked above it.
/// It read as a dropdown that simply refused to open.
///
/// Walking the ancestors keeps the menu one step above whatever context it
/// happens to be in, at any nesting depth, without every container having to
/// know the widget's band.
fn floating_z(
    anchor: Entity,
    base: i32,
    zs: &Query<&GlobalZIndex>,
    parents: &Query<&ChildOf>,
) -> i32 {
    let mut z = base;
    let mut e = anchor;
    loop {
        if let Ok(gz) = zs.get(e) {
            z = z.max(gz.0.saturating_add(1));
        }
        match parents.get(e) {
            Ok(c) => e = c.parent(),
            Err(_) => break,
        }
    }
    z
}

/// Should a menu of `est` px open *upward* from a box spanning `box_top`..
/// `box_bottom`?
///
/// `top_edge` / `bottom_edge` bound the region the menu can actually be seen in
/// — the box's clip rect, or the window when nothing clips it. Everything is in
/// the same physical-px space.
fn flips_up(box_top: f32, box_bottom: f32, est: f32, top_edge: f32, bottom_edge: f32) -> bool {
    let room_below = bottom_edge - box_bottom;
    let room_above = box_top - top_edge;
    room_below < est && room_above > room_below
}

pub(crate) fn dropdown_toggle(
    mut commands: Commands,
    mut dropdowns: Query<
        (
            Entity,
            &Interaction,
            &mut EmberDropdown,
            &bevy::ui::ComputedNode,
            &bevy::ui::UiGlobalTransform,
        ),
        Changed<Interaction>,
    >,
    mut nodes: Query<&mut Node>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    zs: Query<&GlobalZIndex>,
    parents: Query<&ChildOf>,
    clips: Query<&bevy::ui::CalculatedClip>,
) {
    for (box_e, interaction, mut dd, cn, xf) in &mut dropdowns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        dd.open = !dd.open;
        let menu = dd.menu;
        if !dd.open {
            if let Ok(mut n) = nodes.get_mut(menu) {
                n.display = Display::None;
            }
            continue;
        }
        // Resolved on open rather than at spawn: the widget is usually built
        // before it is parented, so the ancestors don't exist yet.
        commands
            .entity(menu)
            .insert(GlobalZIndex(floating_z(box_e, DROPDOWN_Z, &zs, &parents)));
        // Position-aware open: if the estimated menu height doesn't fit below the
        // box and there's more room above, flip it to open upward instead of
        // running out of sight. All measurements are physical px
        // (ComputedNode/UiGlobalTransform/window/CalculatedClip) so they compare
        // directly.
        //
        // The edges that matter are the *clip's*, not the window's. `GlobalZIndex`
        // fixes paint order, not clipping: the menu still gets cut off at the
        // bounds of whatever clipping ancestor the box sits in, and the box's own
        // `CalculatedClip` is exactly that rect (the menu is its child, so it
        // inherits it). Measuring against the window instead is why a dropdown at
        // the bottom of a *scroll area* — Settings' panel being the visible case —
        // opened downwards into the clip and showed a sliver of one row: the
        // window had hundreds of px of room left, the scroll viewport had none.
        let flip_up = windows
            .single()
            .ok()
            .map(|w| {
                let half_h = cn.size().y * 0.5;
                let box_bottom = xf.translation.y + half_h;
                let box_top = xf.translation.y - half_h;
                // ~24 logical px per row + padding, capped at the scroll height.
                let est = (dd.options.len() as f32 * 24.0 + 4.0).min(DROPDOWN_MAX_HEIGHT)
                    * w.scale_factor();
                let win_bottom = w.physical_height() as f32;
                let (top_edge, bottom_edge) = clips.get(box_e).map_or((0.0, win_bottom), |c| {
                    (c.clip.min.y.max(0.0), c.clip.max.y.min(win_bottom))
                });
                flips_up(box_top, box_bottom, est, top_edge, bottom_edge)
            })
            .unwrap_or(false);
        if let Ok(mut n) = nodes.get_mut(menu) {
            n.display = Display::Flex;
            if flip_up {
                n.top = Val::Auto;
                n.bottom = Val::Percent(100.0);
                n.margin = UiRect::bottom(Val::Px(2.0));
            } else {
                n.top = Val::Percent(100.0);
                n.bottom = Val::Auto;
                n.margin = UiRect::top(Val::Px(2.0));
            }
        }
    }
}

pub(crate) fn dropdown_select(
    options: Query<(&Interaction, &EmberDropdownOption), Changed<Interaction>>,
    mut dropdowns: Query<(&mut EmberDropdown, &mut Bound<usize>)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, opt) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok((mut dd, mut bound)) = dropdowns.get_mut(opt.dropdown) {
            dd.selected = opt.value;
            dd.open = false;
            if bound.0 != opt.value {
                bound.0 = opt.value;
            }
            let (menu, label) = (dd.menu, dd.label);
            let text = dd.options.get(opt.value).cloned().unwrap_or_default();
            let box_icon = dd.box_icon;
            let icon = dd.icons.get(opt.value).cloned();
            if let Ok(mut n) = nodes.get_mut(menu) {
                n.display = Display::None;
            }
            if let Ok(mut t) = texts.get_mut(label) {
                *t = Text::new(text);
            }
            if let (Some(bi), Some(name)) = (box_icon, icon) {
                set_icon_glyph(&mut texts, bi, &name);
            }
        }
    }
}

/// Model (`Bound<usize>`) → selection + label, when the value changes externally
/// (e.g. `bind_2way` syncing from a resource). Keeps the dropdown in sync when
/// state is edited elsewhere.
pub(crate) fn dropdown_apply(
    mut dropdowns: Query<(&mut EmberDropdown, &Bound<usize>), Changed<Bound<usize>>>,
    mut texts: Query<&mut Text>,
) {
    for (mut dd, bound) in &mut dropdowns {
        if dd.selected == bound.0 {
            continue;
        }
        dd.selected = bound.0;
        let (label, text) = (dd.label, dd.options.get(bound.0).cloned().unwrap_or_default());
        let box_icon = dd.box_icon;
        let icon = dd.icons.get(bound.0).cloned();
        if let Ok(mut t) = texts.get_mut(label) {
            *t = Text::new(text);
        }
        if let (Some(bi), Some(name)) = (box_icon, icon) {
            set_icon_glyph(&mut texts, bi, &name);
        }
    }
}

/// A left press closes every open dropdown **except** the one the press landed
/// on (its own box or one of its option rows) — that one is left to
/// [`dropdown_toggle`] / [`dropdown_select`].
///
/// The exemption used to be app-wide: a press on *any* box or *any* option made
/// this bail entirely, so opening dropdown B left dropdown A hanging open
/// forever. Invisible while dropdowns sat far apart, glaring the moment two of
/// them share a toolbar strip.
pub(crate) fn dropdown_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    mut dropdowns: Query<(Entity, &Interaction, &mut EmberDropdown)>,
    options: Query<(&Interaction, &EmberDropdownOption)>,
    mut nodes: Query<&mut Node>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    // The dropdown the press belongs to, if any: its box, or an option row
    // (which points back at its box).
    let pressed = dropdowns
        .iter()
        .find(|(_, i, _)| **i != Interaction::None)
        .map(|(e, _, _)| e)
        .or_else(|| {
            options
                .iter()
                .find(|(i, _)| **i != Interaction::None)
                .map(|(_, opt)| opt.dropdown)
        });
    for (e, _, mut dd) in &mut dropdowns {
        if !dd.open || Some(e) == pressed {
            continue;
        }
        dd.open = false;
        if let Ok(mut n) = nodes.get_mut(dd.menu) {
            n.display = Display::None;
        }
    }
}

/// Hover / open tint for the dropdown box itself, and the box's theme tracking.
///
/// The box painted `tab_active()` once at spawn, which left it the only control
/// in a toolbar row that didn't react to the pointer — and left it stale after a
/// theme switch. Repainting from the live theme each frame fixes both.
pub(crate) fn dropdown_box_hover(mut q: Query<(&Interaction, &EmberDropdown, &mut BackgroundColor)>) {
    for (interaction, dd, mut bg) in &mut q {
        let want = if dd.open || *interaction != Interaction::None {
            rgb(tab_hover())
        } else {
            rgb(tab_active())
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

pub(crate) fn dropdown_option_hover(
    mut options: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<EmberDropdownOption>),
    >,
) {
    for (interaction, mut bg) in &mut options {
        bg.0 = match *interaction {
            Interaction::Hovered | Interaction::Pressed => rgb(tab_hover()),
            Interaction::None => Color::NONE,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::EmberFonts;

    fn fonts() -> EmberFonts {
        EmberFonts {
            ui: FontSource::default(),
            phosphor: Handle::default(),
            mono: FontSource::default(),
            default_ui: FontSource::default(),
            default_mono: FontSource::default(),
        }
    }

    /// Picking an option must hide the menu. Regression guard: the viewport
    /// toolbar's comboboxes stayed open after a pick.
    #[test]
    fn selecting_an_option_closes_the_menu() {
        let mut world = World::new();
        let f = fonts();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let box_e = {
            let mut commands = Commands::new(&mut queue, &world);
            dropdown(&mut commands, &f, &["A", "B", "C"], 0)
        };
        queue.apply(&mut world);

        // Open it the way `dropdown_toggle` would.
        let menu = world.get::<EmberDropdown>(box_e).unwrap().menu;
        world.get_mut::<EmberDropdown>(box_e).unwrap().open = true;
        world.get_mut::<Node>(menu).unwrap().display = Display::Flex;

        // Press option index 2.
        let mut q = world.query::<(Entity, &EmberDropdownOption)>();
        let row = q
            .iter(&world)
            .find(|(_, o)| o.dropdown == box_e && o.value == 2)
            .map(|(e, _)| e)
            .expect("option row");
        *world.get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;

        let mut sys = bevy::ecs::system::IntoSystem::into_system(dropdown_select);
        sys.initialize(&mut world);
        let _ = sys.run((), &mut world);

        assert_eq!(world.get::<Node>(menu).unwrap().display, Display::None);
        assert_eq!(world.get::<Bound<usize>>(box_e).unwrap().0, 2);
        assert!(!world.get::<EmberDropdown>(box_e).unwrap().open);
    }

    /// An open menu must stack above any container that claimed a global depth
    /// of its own.
    ///
    /// Regression guard: the full-screen import window sits at 900 with an
    /// opaque background, and a menu pinned to a fixed 500 opened *behind* it —
    /// invisible, and unclickable because picking gave the pointer to the
    /// background stacked above. It read as a dropdown that refused to open.
    #[test]
    fn a_menu_stacks_above_its_own_container() {
        let mut world = World::new();
        let f = fonts();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let box_e = {
            let mut commands = Commands::new(&mut queue, &world);
            dropdown(&mut commands, &f, &["A", "B"], 0)
        };
        queue.apply(&mut world);

        // A container that lifts itself into the global stacking context, the
        // way the import window does.
        let window = world.spawn((Node::default(), GlobalZIndex(900))).id();
        let mid = world.spawn(Node::default()).id();
        world.entity_mut(mid).insert(ChildOf(window));
        world.entity_mut(box_e).insert(ChildOf(mid));

        let mut sys = bevy::ecs::system::IntoSystem::into_system(
            move |zs: Query<&GlobalZIndex>, parents: Query<&ChildOf>| {
                floating_z(box_e, DROPDOWN_Z, &zs, &parents)
            },
        );
        sys.initialize(&mut world);
        let z = sys.run((), &mut world).expect("depth");

        assert_eq!(z, 901, "must clear the container it lives inside");
    }

    /// A dropdown at the bottom of a *clipped* scroll area must open upward,
    /// however much empty window there is below it.
    ///
    /// Regression guard: the room was measured against the window, so the
    /// Document Tabs dropdown at the bottom of the Settings panel's scroll
    /// opened downward — into the clip — and showed a sliver of its first row.
    /// `GlobalZIndex` lifts the menu's paint order out of the scroll, but not
    /// its clip rect.
    #[test]
    fn a_menu_flips_up_at_the_bottom_of_a_clipped_scroll() {
        // A box on the bottom edge of a scroll viewport spanning 100..400, in a
        // 900px window: the window has 500px going spare, the viewport none.
        assert!(flips_up(376.0, 400.0, 120.0, 100.0, 400.0));
        // The same box measured against the window — what it used to do, and
        // why it opened into the clip.
        assert!(!flips_up(376.0, 400.0, 120.0, 0.0, 900.0));
        // Mid-scroll there's room below, so it still opens downward.
        assert!(!flips_up(176.0, 200.0, 120.0, 100.0, 400.0));
        // Cramped both ways: it stays down rather than flipping into *less*
        // room — near the top of a scroll, below is still the better side.
        assert!(!flips_up(140.0, 160.0, 200.0, 100.0, 300.0));
    }

    #[test]
    fn an_unnested_menu_keeps_the_default_depth() {
        let mut world = World::new();
        let f = fonts();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let box_e = {
            let mut commands = Commands::new(&mut queue, &world);
            dropdown(&mut commands, &f, &["A"], 0)
        };
        queue.apply(&mut world);

        let mut sys = bevy::ecs::system::IntoSystem::into_system(
            move |zs: Query<&GlobalZIndex>, parents: Query<&ChildOf>| {
                floating_z(box_e, DROPDOWN_Z, &zs, &parents)
            },
        );
        sys.initialize(&mut world);
        // The box itself carries no depth, so nothing lifts it past the band's
        // floor — every existing dropdown keeps behaving exactly as before.
        assert_eq!(sys.run((), &mut world).expect("depth"), DROPDOWN_Z);
    }

    /// Opening one dropdown must close any other that's already open. The
    /// dismiss check used to exempt a press on *any* box, so two dropdowns in
    /// one toolbar strip both stayed open.
    #[test]
    fn opening_one_dropdown_closes_another() {
        let mut world = World::new();
        world.init_resource::<ButtonInput<MouseButton>>();
        let f = fonts();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let (a, b) = {
            let mut commands = Commands::new(&mut queue, &world);
            (
                dropdown(&mut commands, &f, &["A1", "A2"], 0),
                dropdown(&mut commands, &f, &["B1", "B2"], 0),
            )
        };
        queue.apply(&mut world);

        // A is open; the user presses B's box.
        let menu_a = world.get::<EmberDropdown>(a).unwrap().menu;
        let menu_b = world.get::<EmberDropdown>(b).unwrap().menu;
        world.get_mut::<EmberDropdown>(a).unwrap().open = true;
        world.get_mut::<Node>(menu_a).unwrap().display = Display::Flex;
        *world.get_mut::<Interaction>(b).unwrap() = Interaction::Pressed;
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        let mut sys = bevy::ecs::system::IntoSystem::into_system(dropdown_dismiss);
        sys.initialize(&mut world);
        let _ = sys.run((), &mut world);

        assert_eq!(
            world.get::<Node>(menu_a).unwrap().display,
            Display::None,
            "the already-open dropdown should have closed"
        );
        assert!(!world.get::<EmberDropdown>(a).unwrap().open);
        // B is left alone — `dropdown_toggle` owns the press that landed on it.
        assert_eq!(world.get::<Node>(menu_b).unwrap().display, Display::None);
    }

    /// Pressing a dropdown's own option must not close it here — `dropdown_select`
    /// owns that, and closing early would race it.
    #[test]
    fn pressing_an_option_does_not_dismiss_its_own_dropdown() {
        let mut world = World::new();
        world.init_resource::<ButtonInput<MouseButton>>();
        let f = fonts();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let box_e = {
            let mut commands = Commands::new(&mut queue, &world);
            dropdown(&mut commands, &f, &["A", "B"], 0)
        };
        queue.apply(&mut world);

        let menu = world.get::<EmberDropdown>(box_e).unwrap().menu;
        world.get_mut::<EmberDropdown>(box_e).unwrap().open = true;
        world.get_mut::<Node>(menu).unwrap().display = Display::Flex;
        let mut q = world.query::<(Entity, &EmberDropdownOption)>();
        let row = q.iter(&world).next().map(|(e, _)| e).unwrap();
        *world.get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);

        let mut sys = bevy::ecs::system::IntoSystem::into_system(dropdown_dismiss);
        sys.initialize(&mut world);
        let _ = sys.run((), &mut world);

        assert!(world.get::<EmberDropdown>(box_e).unwrap().open);
    }
}
