//! The top bar: the hamburger, session actions, Settings and Play on the left;
//! the workspace ribbon centred; the update chip, plugin-contributed buttons and
//! the window controls on the right.
//!
//! Everything in the left zone acts on the *session* rather than on a panel. All
//! of it used to live somewhere in the viewport's tool strip, which meant it was
//! missing from any workspace without a viewport — and none of it is a viewport
//! action. This bar is on screen in every workspace.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{glyph, ui_font, EmberFonts};
use renzora_ember::theme::{rgb, text_muted, text_primary, window_bg};

use renzora_ui::window_chrome::WindowAction;

use crate::doc_tabs::build_doc_tab_menu_group;
use crate::play_controls::build_play_group;
use crate::ribbon::{ribbon_snapshot, WorkspaceAddBtn, WorkspaceDropZone, RIBBON_W};
use crate::status_bar::ChromeBar;
use crate::top_menu::{build_update_chip, hamburger_menu_item};
use crate::window_chrome::{MaximizeIcon, WindowDragHandle};

#[cfg(not(target_arch = "wasm32"))]
use crate::window_chrome::WindowBtn;
#[cfg(target_arch = "wasm32")]
use crate::window_chrome::WebFullscreenBtn;

/// The top-bar magnifier — toggles the command palette.
#[derive(Component)]
pub(crate) struct CommandPaletteBtn;

/// The top-bar gear — toggles the Settings panel.
#[derive(Component)]
pub(crate) struct SettingsBtn;

/// Marks a plugin-contributed top-bar button with the id it reports when
/// pressed.
#[derive(Component)]
pub(crate) struct ShellActionBtn(&'static str);

/// The top bar: File/Edit/View/Help on the left, the layout ribbon centered,
/// action buttons on the right.
pub(crate) fn build_top_bar(commands: &mut Commands, font: &bevy::text::FontSource, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(Color::NONE),
            ChromeBar::Top,
            // Host for a themeable shader effect (matrix rain, …). The driver in
            // ember paints it as this node's background when the active theme sets
            // `effects.top_bar`; the menus/buttons render on top.
            renzora_ember::widgets::ThemeShaderSurface {
                surface: renzora_ember::widgets::ThemeSurface::TopBar,
            },
            // The bar is the window drag handle; empty areas (zones pass through)
            // reach it, while interactive children (menus/buttons) block it.
            Interaction::default(),
            WindowDragHandle,
            Name::new("top-bar"),
        ))
        .id();

    // `clip: false` — the Play button's target dropdown is a child of its caret,
    // absolutely positioned below the bar, and bevy_ui clips absolutely
    // positioned descendants like everything else (the trap that eats tooltips
    // and submenu panels). A growing zone clips by default so its contents can't
    // spill over the centered ribbon, which mattered while the document tabs
    // lived here and could be arbitrarily wide; what's left is a fixed handful
    // of buttons that will never reach half the window.
    let left = zone(commands, "top-left", JustifyContent::FlexStart, 2.0, 1.0, false);
    // Everything that acts on the *session* rather than on a panel: the
    // hamburger, Settings, undo / redo / save, and Play. All of them used to be
    // somewhere in the viewport's tool strip or its menus, which meant they were
    // missing from any workspace without a viewport — and none of them is a
    // viewport action. This bar is on screen in every workspace. The document
    // tabs used to fill the rest of this zone; they now sit at the top of the
    // viewport panel (see `build_doc_tabs`).
    let hamburger = hamburger_menu_item(commands, font);
    let session = renzora_viewport::native_header::build_session_actions(commands, fonts);
    let settings = settings_button(commands);
    let play = build_play_group(commands, font);
    // The document tabs, for anyone who'd rather not spend a row of the window
    // on them — hidden unless Settings has them set to Dropdown, in which case
    // the strip under this bar is the one that's hidden instead.
    let docs = build_doc_tab_menu_group(commands, fonts, font);
    commands
        .entity(left)
        .add_children(&[hamburger, session, settings, play, docs]);

    let center = zone(commands, "top-center", JustifyContent::Center, 2.0, 0.0, false);
    let magnifier = glyph(commands, "magnifying-glass", text_muted(), 14.0);
    // Search button — toggles the global command palette (Ctrl+P).
    commands.entity(magnifier).insert((
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        Interaction::default(),
        CommandPaletteBtn,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));
    // Reactive ribbon — one button per workspace in `ShellLayouts`. Capped the
    // same way the document tabs are, so a project with a dozen workspaces folds
    // the tail into a `»` menu instead of crowding out the bar's two ends.
    let (ribbon_strip, ribbon) = renzora_ember::widgets::overflow_strip(
        commands,
        renzora_ember::widgets::OverflowBudget::Fixed(RIBBON_W),
        "ribbon",
    );
    commands
        .entity(ribbon)
        .insert((WorkspaceDropZone, RelativeCursorPosition::default()));
    renzora_ember::reactive::tracked::keyed_list(commands, ribbon, ribbon_snapshot);
    let add = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            WorkspaceAddBtn,
            WorkspaceDropZone,
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("workspace-add"),
        ))
        .id();
    let add_label = commands
        .spawn((Text::new("+"), ui_font(font, 12.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(add).add_child(add_label);
    commands.entity(center).add_children(&[magnifier, ribbon_strip, add]);

    // The right zone is window controls only now: Play moved to the toolbar
    // strip's trailing edge (see `build_play_group`) and the gear moved into
    // the hamburger menu as its own top-level Settings row.
    let right = zone(commands, "top-right", JustifyContent::FlexEnd, 8.0, 1.0, true);

    // Window controls: a fixed-size button with the glyph as a *child* so
    // `align_items`/`justify_content: Center` truly center it (text placed
    // directly on a node is NOT vertically centered by Bevy — it rides the top
    // of the box). The buttons are shorter than the bar and centered in it, so
    // their glyphs line up with the play/code/gear icons to their left.
    let window = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            Name::new("window-buttons"),
        ))
        .id();
    #[allow(unused_mut)]
    let mut kids: Vec<Entity> = Vec::new();

    // Web: one fullscreen toggle instead of the three window controls.
    //
    // A browser tab has no OS window to minimize, maximize or close —
    // `set_minimized` is a no-op and a tab cannot close itself unless a script
    // opened it. Fullscreen is the one window state a page CAN change, and it
    // is the one worth having: it hides the tab strip and address bar and gives
    // the editor the whole display.
    #[cfg(target_arch = "wasm32")]
    {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(24.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                WebFullscreenBtn,
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        let g = glyph(commands, "corners-out", text_muted(), 14.0);
        commands.entity(g).insert(bevy::ui::FocusPolicy::Pass);
        commands.entity(btn).add_child(g);
        // Same hover treatment as the desktop minimize/maximize buttons — a
        // faint wash, no red (nothing here is destructive).
        renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
            match w.get::<Interaction>(btn) {
                Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                    Color::srgba(1.0, 1.0, 1.0, 0.09)
                }
                _ => Color::NONE,
            }
        });
        renzora_ember::reactive::tracked::bind_text_color(commands, g, move |w| {
            match w.get::<Interaction>(btn) {
                Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(text_primary()),
                _ => rgb(text_muted()),
            }
        });
        kids.push(btn);
    }

    #[cfg(not(target_arch = "wasm32"))]
    for (name, action, is_close) in [
        ("minus", WindowAction::Minimize, false),
        ("square", WindowAction::ToggleMaximize, false),
        ("x", WindowAction::Close, true),
    ] {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(24.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                WindowBtn(action),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        // The glyph is a child; `FocusPolicy::Pass` lets the hover/click land on
        // the button (so the bindings below see the parent's `Interaction`).
        let g = glyph(commands, name, text_muted(), 14.0);
        commands.entity(g).insert(bevy::ui::FocusPolicy::Pass);
        if matches!(action, WindowAction::ToggleMaximize) {
            // The maximize glyph reflects window state (square ↔ restore).
            commands.entity(g).insert(MaximizeIcon);
        }
        commands.entity(btn).add_child(g);

        // Hover fill on the button: minimize/maximize get a faint wash; close
        // goes the standard Windows close-red.
        renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| match w.get::<Interaction>(btn) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                if is_close {
                    Color::srgb_u8(232, 17, 35)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.09)
                }
            }
            _ => Color::NONE,
        });
        // Glyph color tracks the parent button's hover: the close × turns white
        // on its red fill; the other two brighten from muted to primary.
        renzora_ember::reactive::tracked::bind_text_color(commands, g, move |w| {
            match w.get::<Interaction>(btn) {
                Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                    if is_close {
                        Color::WHITE
                    } else {
                        rgb(text_primary())
                    }
                }
                _ => rgb(text_muted()),
            }
        });
        kids.push(btn);
    }
    commands.entity(window).add_children(&kids);

    // ── "Update available" chip ──────────────────────────────────────────────
    // Present only while `renzora_update`'s background check has something to
    // offer; the resource is removed again when a later check disagrees, and the
    // chip goes with it. Clicking opens the same overlay as Help ▸ Check for
    // Updates — this is a shortcut to it, not a second way of doing it.
    let update_chip = build_update_chip(commands, font);

    // Plugin-contributed buttons — the Marketplace's is the first. It lived by
    // the gear, as one more small grey glyph among the session controls, which
    // is the last place anyone looks for somewhere to *go*. Over here it is a
    // labelled, tinted chip beside the update chip: the two of them are the
    // bar's "things that are waiting for you" corner.
    let actions = build_shell_actions(commands);

    commands.entity(right).add_children(&[actions, update_chip, window]);

    commands.entity(bar).add_children(&[left, center, right]);
    bar
}

/// The top bar's gear — opens (or closes) the Settings panel.
///
/// The bar carried a gear button once before; it was dropped when the menus were
/// folded into the hamburger, which left the hamburger's own **Settings** row as
/// the only way in. That row stays — this is the one-click path back, for the
/// thing the menu's own comment admits is "reached far too often".
fn settings_button(commands: &mut Commands) -> Entity {
    let gear = glyph(commands, "gear", text_muted(), 14.0);
    commands.entity(gear).insert((
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        Interaction::default(),
        SettingsBtn,
        renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("common.settings")),
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));
    gear
}

/// The row of plugin-contributed top-bar buttons.
///
/// Built once with the chrome, from whatever is in
/// [`renzora::ShellActionRegistry`] at that moment — which is every plugin's
/// registration, since plugins are added during `App` assembly and the chrome is
/// built after the splash. A plugin that registers later gets its button on the
/// next chrome rebuild (a theme or layout change), which is the same deal
/// status-bar items and panels get.
fn build_shell_actions(commands: &mut Commands) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("top-bar-actions"),
        ))
        .id();
    commands.queue(move |world: &mut World| {
        struct Item {
            id: &'static str,
            icon: &'static str,
            label: Option<String>,
            color: Option<[u8; 3]>,
            tooltip: String,
        }
        let items: Vec<Item> = world
            .get_resource::<renzora::ShellActionRegistry>()
            .map(|reg| {
                let mut v: Vec<(i32, Item)> = reg
                    .items
                    .iter()
                    .map(|i| {
                        (
                            i.order,
                            Item {
                                id: i.id,
                                icon: i.icon,
                                label: i.label.map(|f| f()),
                                color: i.color,
                                tooltip: (i.tooltip)(),
                            },
                        )
                    })
                    .collect();
                v.sort_by_key(|(order, _)| *order);
                v.into_iter().map(|(_, item)| item).collect()
            })
            .unwrap_or_default();
        if items.is_empty() {
            return;
        }
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
            return;
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            let kids: Vec<Entity> = items
                .into_iter()
                .map(|item| shell_action_button(&mut commands, &fonts, item.id, item.icon, item.label, item.color, item.tooltip))
                .collect();
            commands.entity(row).add_children(&kids);
        }
        queue.apply(world);
    });
    row
}

/// One plugin-contributed top-bar button.
///
/// A tinted pill with a coloured glyph and a label when the item asks for them,
/// and the same quiet icon-only treatment as the gear when it does not. The
/// tint is what makes a *destination* legible in a bar of toggles — an
/// unlabelled grey glyph beside four other grey glyphs is not somewhere anyone
/// finds by looking.
#[allow(clippy::too_many_arguments)]
fn shell_action_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: &'static str,
    icon: &'static str,
    label: Option<String>,
    color: Option<[u8; 3]>,
    tooltip: String,
) -> Entity {
    let hue = color.map(|c| (c[0], c[1], c[2]));
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(if label.is_some() { 8.0 } else { 5.0 }), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ShellActionBtn(id),
            renzora_ember::widgets::HoverTooltip::new(tooltip),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("top-action:{id}")),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
        let hot = matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        match hue {
            Some((r, g, b)) => Color::srgba(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                if hot { 0.34 } else { 0.20 },
            ),
            None if hot => rgb(renzora_ember::theme::hover_bg()),
            None => Color::NONE,
        }
    });

    let ic = renzora_ember::font::icon_text(
        commands,
        &fonts.phosphor,
        icon,
        hue.unwrap_or_else(text_muted),
        14.0,
    );
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    let mut kids = vec![ic];
    if let Some(label) = label {
        kids.push(
            commands
                .spawn((
                    Text::new(label),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(text_primary())),
                    bevy::ui::FocusPolicy::Pass,
                    bevy::text::TextLayout::no_wrap(),
                ))
                .id(),
        );
    }
    commands.entity(btn).add_children(&kids);
    btn
}

/// Turn a press into a [`renzora::ShellActionInvoked`] for whoever registered
/// the id.
pub(crate) fn shell_action_press(
    q: Query<(&Interaction, &ShellActionBtn), Changed<Interaction>>,
    mut invoked: MessageWriter<renzora::ShellActionInvoked>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            invoked.write(renzora::ShellActionInvoked(btn.0));
        }
    }
}

/// Gear → toggle the Settings panel. Same toggle the hamburger's Settings row
/// runs, so clicking either while it's open closes it.
pub(crate) fn settings_btn_click(
    q: Query<&Interaction, (With<SettingsBtn>, Changed<Interaction>)>,
    settings: Option<ResMut<renzora_editor_framework::EditorSettings>>,
) {
    let Some(mut settings) = settings else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        settings.show_settings = !settings.show_settings;
    }
}

/// The top-bar magnifier → toggle the command palette (consumed by
/// `renzora_command_palette`).
pub(crate) fn palette_btn_click(
    q: Query<&Interaction, (With<CommandPaletteBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(renzora::core::ToggleCommandPaletteRequested);
    }
}

/// A full-height flex row used as a top-bar zone (left / center / right).
///
/// A growing zone gets `flex_basis: 0` — without it flexbox hands out only the
/// *leftover* space equally, so the two side zones end up as wide as their own
/// content plus a share, and the "centered" middle zone sits wherever the
/// heavier side pushes it. From a zero basis both sides are dealt identical
/// widths whatever they hold, which is what actually centers the ribbon in the
/// window. They shrink rather than grow past that half.
///
/// `clip` is separate from `grow` because the two wants can conflict: clipping
/// is what stops a zone's contents spilling over the ribbon, but it also cuts
/// off anything a child hangs *outside* the bar — a dropdown panel, a tooltip.
/// A zone holding a fixed, small set of buttons has nothing to contain and
/// should not clip.
fn zone(
    commands: &mut Commands,
    name: &str,
    justify: JustifyContent,
    gap: f32,
    grow: f32,
    clip: bool,
) -> Entity {
    let growing = grow > 0.0;
    commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: justify,
                column_gap: Val::Px(gap),
                flex_grow: grow,
                flex_basis: if growing { Val::Px(0.0) } else { Val::Auto },
                min_width: Val::Px(0.0),
                overflow: if clip { Overflow::clip() } else { Overflow::visible() },
                ..default()
            },
            // Structural container — let clicks fall through to the bar's drag
            // handle behind it (interactive children still block on their own).
            bevy::ui::FocusPolicy::Pass,
            Name::new(name.to_string()),
        ))
        .id()
}
