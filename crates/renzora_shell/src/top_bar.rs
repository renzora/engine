//! The top bar: the hamburger, session actions, Settings and Play on the left;
//! the workspace ribbon centred; the update chip, plugin-contributed buttons and
//! the window controls on the right.
//!
//! Everything in the left zone acts on the *session* rather than on a panel. All
//! of it used to live somewhere in the viewport's tool strip, which meant it was
//! missing from any workspace without a viewport — and none of it is a viewport
//! action. This bar is on screen in every workspace.

use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, RelativeCursorPosition};

use renzora_ember::font::{glyph, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::theme::{rgb, text_muted, text_primary, window_bg};
use renzora_ember::widgets::{chip_border, chip_gradient, CHIP_RADIUS};
use renzora_ember::UiInteraction;

use renzora_ui::window_chrome::WindowAction;

use crate::play_controls::build_play_group;
use crate::ribbon::{ribbon_snapshot, WorkspaceAddBtn, WorkspaceDropZone, RIBBON_W};
use crate::status_bar::ChromeBar;
use crate::top_menu::{brand_mark, build_update_chip, hamburger_menu_item};
use crate::window_chrome::WindowDragHandle;
// Only the trailing glyph buttons carry it — the macOS zoom dot keeps one glyph
// whatever the window state, the way the real one does.
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
use crate::window_chrome::MaximizeIcon;

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
            UiInteraction::default(),
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
    // hamburger, Settings, save, and Play. All of them used to be
    // somewhere in the viewport's tool strip or its menus, which meant they were
    // missing from any workspace without a viewport — and none of them is a
    // viewport action. This bar is on screen in every workspace. The open
    // documents are listed by the scene switcher at the end of this zone.
    // The Renzora mark leads the bar, ahead of the menu: the spot every desktop
    // application puts its identity, and a click on it opens About. Under the
    // OS frame the mark is the window icon in the system title bar instead
    // (About stays in Help); the web has no title bar, so it keeps ours.
    let os_title_bar = renzora::EDITOR_OS_FRAME && !cfg!(target_arch = "wasm32");
    let mut lead = Vec::new();
    #[cfg(all(target_os = "macos", not(target_arch = "wasm32")))]
    if !renzora::EDITOR_OS_FRAME {
        lead.push(build_traffic_lights(commands));
    }
    if !os_title_bar {
        lead.push(brand_mark(commands, fonts));
    }
    let hamburger = hamburger_menu_item(commands, font);
    let session = renzora_viewport::toolbar::build_session_actions(commands, fonts);
    let settings = settings_button(commands);
    // Plugin tools (the companion's spanner) sit right after the gear, as more
    // of the same small glyphs.
    let tools = build_shell_actions(commands, true);
    // The document you are in, where Play used to be. Play moved to the right
    // zone beside the Marketplace: the left end now says what you are
    // editing, and the right end holds the things that run or ship it.
    let scene = crate::scene_switcher::build_scene_switcher(commands, fonts, font);
    commands.entity(scene).entry::<Node>().and_modify(|mut n| {
        n.margin = UiRect::left(Val::Px(8.0));
    });
    // macOS puts the window buttons at the LEADING edge, so on that platform
    // they come before the mark (see `lead` above); everywhere else they stay in
    // the trailing zone where Windows and most Linux desktops put them. One bar,
    // two conventions, chosen by the platform rather than by a setting: nobody
    // wants Mac buttons on Windows.
    lead.extend([hamburger, session, settings, tools, scene]);
    commands.entity(left).add_children(&lead);

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
        UiInteraction::default(),
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
            UiInteraction::default(),
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

    // The right zone: Play, the plugin buttons (the Marketplace first), the
    // update chip and the window controls.
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
                UiInteraction::default(),
                WebFullscreenBtn,
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        let g = glyph(commands, "corners-out", text_muted(), 14.0);
        commands.entity(g).insert(PASS_THROUGH);
        commands.entity(btn).add_child(g);
        // Same hover treatment as the desktop minimize/maximize buttons — a
        // faint wash, no red (nothing here is destructive).
        renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
            match w.get::<UiInteraction>(btn) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => {
                    Color::srgba(1.0, 1.0, 1.0, 0.09)
                }
                _ => Color::NONE,
            }
        });
        renzora_ember::reactive::tracked::bind_text_color(commands, g, move |w| {
            match w.get::<UiInteraction>(btn) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(text_primary()),
                _ => rgb(text_muted()),
            }
        });
        kids.push(btn);
    }

    // Not on macOS: the same three actions are already in the leading zone as
    // traffic lights, and a window with controls at both ends is nobody's
    // convention. Nor under the OS frame, which has its own.
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "macos")))]
    for (name, action, is_close) in [
        ("minus", WindowAction::Minimize, false),
        ("square", WindowAction::ToggleMaximize, false),
        ("x", WindowAction::Close, true),
    ]
    .into_iter()
    .filter(|_| !renzora::EDITOR_OS_FRAME)
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
                UiInteraction::default(),
                WindowBtn(action),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        // The glyph is a child; `PASS_THROUGH` lets the hover/click land on
        // the button (so the bindings below see the parent's `UiInteraction`).
        let g = glyph(commands, name, text_muted(), 14.0);
        commands.entity(g).insert(PASS_THROUGH);
        if matches!(action, WindowAction::ToggleMaximize) {
            // The maximize glyph reflects window state (square ↔ restore).
            commands.entity(g).insert(MaximizeIcon);
        }
        commands.entity(btn).add_child(g);

        // Hover fill on the button: minimize/maximize get a faint wash; close
        // goes the standard Windows close-red.
        renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => {
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
            match w.get::<UiInteraction>(btn) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => {
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
    let actions = build_shell_actions(commands, false);
    let play = build_play_group(commands, font);

    commands.entity(right).add_children(&[play, actions, update_chip, window]);

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
        UiInteraction::default(),
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
///
/// With `tools`, the registry's tools instead: the icon-only row beside the
/// gear, drawn in the gear's quiet style whatever the item asks for, so a tool
/// reads as one more of the editor's own controls.
fn build_shell_actions(commands: &mut Commands, tools: bool) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                ..default()
            },
            Name::new(if tools { "top-bar-tools" } else { "top-bar-actions" }),
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
                let list = if tools { &reg.tools } else { &reg.items };
                let mut v: Vec<(i32, Item)> = list
                    .iter()
                    .map(|i| {
                        (
                            i.order,
                            Item {
                                id: i.id,
                                icon: i.icon,
                                label: if tools { None } else { i.label.map(|f| f()) },
                                color: if tools { None } else { i.color },
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
                border: UiRect::all(Val::Px(if hue.is_some() { 1.0 } else { 0.0 })),
                border_radius: BorderRadius::all(Val::Px(CHIP_RADIUS)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            ShellActionBtn(id),
            renzora_ember::widgets::HoverTooltip::new(tooltip),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("top-action:{id}")),
        ))
        .id();
    match hue {
        // A tinted action is a chip like Play: gradient + hairline, driven by
        // `shell_action_chip` on hover.
        Some(h) => {
            let c = rgb(h);
            commands
                .entity(btn)
                .insert((ShellActionHue(c), chip_gradient(c, false), chip_border(c, false)));
        }
        None => renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
            let hot = matches!(
                w.get::<UiInteraction>(btn),
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
            );
            if hot {
                rgb(renzora_ember::theme::hover_bg())
            } else {
                Color::NONE
            }
        }),
    }

    let ic = renzora_ember::font::icon_text(
        commands,
        &fonts.phosphor,
        icon,
        hue.unwrap_or_else(text_muted),
        14.0,
    );
    commands.entity(ic).insert(PASS_THROUGH);
    let mut kids = vec![ic];
    if let Some(label) = label {
        kids.push(
            commands
                .spawn((
                    Text::new(label),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(text_primary())),
                    PASS_THROUGH,
                    bevy::text::TextLayout::no_wrap(),
                ))
                .id(),
        );
    }
    commands.entity(btn).add_children(&kids);
    btn
}

/// The hue a tinted plugin action paints its chip with.
#[derive(Component)]
pub(crate) struct ShellActionHue(Color);

/// Strengthen a tinted action chip's gradient and edge while hovered.
pub(crate) fn shell_action_chip(
    mut q: Query<
        (&UiInteraction, &ShellActionHue, &mut BackgroundGradient, &mut BorderColor),
        Changed<UiInteraction>,
    >,
) {
    for (interaction, hue, mut bg, mut edge) in &mut q {
        let hot = *interaction != UiInteraction::None;
        *bg = chip_gradient(hue.0, hot);
        *edge = chip_border(hue.0, hot);
    }
}

/// Turn a press into a [`renzora::ShellActionInvoked`] for whoever registered
/// the id.
pub(crate) fn shell_action_press(
    q: Query<(&UiInteraction, &ShellActionBtn), Changed<UiInteraction>>,
    mut invoked: MessageWriter<renzora::ShellActionInvoked>,
) {
    for (interaction, btn) in &q {
        if *interaction == UiInteraction::Pressed {
            invoked.write(renzora::ShellActionInvoked(btn.0));
        }
    }
}

/// Gear → toggle the Settings panel. Same toggle the hamburger's Settings row
/// runs, so clicking either while it's open closes it.
pub(crate) fn settings_btn_click(
    q: Query<&UiInteraction, (With<SettingsBtn>, Changed<UiInteraction>)>,
    settings: Option<ResMut<renzora_editor_framework::EditorSettings>>,
) {
    let Some(mut settings) = settings else { return };
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
        settings.show_settings = !settings.show_settings;
    }
}

/// The top-bar magnifier → toggle the command palette (consumed by
/// `renzora_command_palette`).
pub(crate) fn palette_btn_click(
    q: Query<&UiInteraction, (With<CommandPaletteBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == UiInteraction::Pressed) {
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
/// The three round window buttons macOS puts at the LEADING edge of a window.
///
/// Custom-drawn rather than the real `NSWindow` ones. The editor runs
/// `decorations: false` and owns its whole title bar — drag handling, eight
/// resize zones, the lot — so asking macOS to draw genuine traffic lights would
/// mean a transparent titlebar and a fullsize content view underneath chrome
/// that already does that job, and a drag region negotiated between the two.
/// These are ours: same colours, same order, same hover behaviour, and they work
/// identically in the editor and the splash without a second code path.
///
/// Order is close, minimize, zoom — reading order, and the opposite end of the
/// window from every other platform, which is the whole point.
///
/// # Why the glyphs only appear on hover
///
/// That is what macOS does, and it is not decoration: three unlabelled dots are
/// unambiguous by position and colour, and the glyphs would otherwise be visual
/// noise on a bar that already carries a mark, a menu and a row of actions. They
/// appear together, on hover of the GROUP rather than of each button, because
/// that is also what macOS does — reaching for one reveals all three, so you can
/// see what you are about to hit before you commit to which.
#[cfg(all(target_os = "macos", not(target_arch = "wasm32")))]
fn build_traffic_lights(commands: &mut Commands) -> Entity {
    // Apple's own values, so the window reads as a Mac window rather than as an
    // approximation of one.
    const CLOSE: (u8, u8, u8) = (255, 95, 87);
    const MINIMIZE: (u8, u8, u8) = (254, 188, 46);
    const ZOOM: (u8, u8, u8) = (40, 200, 64);
    /// 12pt is the system size; the glyph inside is deliberately smaller than the
    /// dot so it reads as engraved rather than as an icon in a coloured box.
    const DOT: f32 = 12.0;
    const GLYPH: f32 = 8.0;

    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                margin: UiRect::axes(Val::Px(6.0), Val::Px(0.0)),
                ..default()
            },
            // The group is what hover is read from, so all three glyphs appear
            // together. Without an `UiInteraction` here the children would each
            // answer only for themselves.
            UiInteraction::default(),
            Name::new("traffic-lights"),
        ))
        .id();

    let mut dots = Vec::new();
    for (name, action, color) in [
        ("x", WindowAction::Close, CLOSE),
        ("minus", WindowAction::Minimize, MINIMIZE),
        ("plus", WindowAction::ToggleMaximize, ZOOM),
    ] {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(DOT),
                    height: Val::Px(DOT),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    // Half the box: bevy_ui has no `50%` shorthand that survives
                    // a non-square node, and these are square by construction.
                    border_radius: BorderRadius::all(Val::Px(DOT / 2.0)),
                    ..default()
                },
                BackgroundColor(rgb(color)),
                UiInteraction::default(),
                WindowBtn(action),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();

        // Darkened rather than black: a black glyph on the red dot reads as a
        // hole punched through it, where a dark tint of the dot's own hue looks
        // engraved the way Apple's do.
        let ink = (
            (color.0 as f32 * 0.35) as u8,
            (color.1 as f32 * 0.35) as u8,
            (color.2 as f32 * 0.35) as u8,
        );
        let g = glyph(commands, name, ink, GLYPH);
        // Pass, or the glyph would eat the hover the button needs to answer.
        commands.entity(g).insert(PASS_THROUGH);
        renzora_ember::reactive::tracked::bind_text_color(commands, g, move |w| {
            let showing = matches!(
                w.get::<UiInteraction>(group),
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
            );
            if showing {
                rgb(ink)
            } else {
                Color::NONE
            }
        });
        commands.entity(btn).add_child(g);
        dots.push(btn);
    }
    commands.entity(group).add_children(&dots);
    group
}

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
            PASS_THROUGH,
            Name::new(name.to_string()),
        ))
        .id()
}
