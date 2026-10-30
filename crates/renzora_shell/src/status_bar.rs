//! The bottom status bar, and the two dropups that live in it (theme and
//! language). Also [`apply_chrome_style`], which repaints the top and status
//! bars from the ember `Theme.chrome` — it is here because [`ChromeBar`] is.
//!
//! Plugin-contributed status items are rendered through a reactive keyed list,
//! so a live metric updates without rebuilding the bar around it.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{divider, rgb, text_muted, window_bg};
use renzora_ember::widgets::{menu_item, scroll_area_keyed, Popup};
use renzora_ember::UiInteraction;

/// A status segment that invokes a shell action when clicked; see
/// [`renzora::ShellStatusSegment::action`].
#[derive(Component)]
pub(crate) struct StatusSegmentAction(&'static str);

/// Send a clicked segment's action, exactly as a top-bar button would.
///
/// Through the `Messages` resource rather than a `MessageWriter`: the message
/// type only exists once some plugin has registered a shell action, and a shell
/// without one must not fail to run this system.
pub(crate) fn status_segment_click(
    segments: Query<(&UiInteraction, &StatusSegmentAction), Changed<UiInteraction>>,
    messages: Option<ResMut<bevy::ecs::message::Messages<renzora::ShellActionInvoked>>>,
) {
    let Some(mut messages) = messages else { return };
    for (interaction, action) in &segments {
        if *interaction == UiInteraction::Pressed {
            messages.write(renzora::ShellActionInvoked(action.0));
        }
    }
}

/// Marks the status-bar theme picker's trigger so its open/closed state can be
/// mirrored into [`ThemeMenuOpen`] each frame.
#[derive(Component)]
pub(crate) struct ThemeDropup;

/// Whether the status-bar theme dropup is open, persisted *across* chrome
/// rebuilds. Picking a theme switches `active_theme_name`, which makes
/// `theme_bridge` despawn and respawn the whole chrome — without this the rebuilt
/// dropup would always come back closed, so the menu would vanish the instant you
/// clicked a theme inside it. Holding the open state here lets the rebuilt dropup
/// re-open, so the menu only closes on a real outside click (or toggling the
/// trigger).
#[derive(Resource, Default)]
pub(crate) struct ThemeMenuOpen(pub(crate) bool);

/// Which chrome bar an entity is, so [`apply_chrome_style`] can repaint each from
/// `Theme.chrome` (fill / height / separator edge / rounding / padding).
///
/// There's no `DocTabs` variant: the shell has no document strip (the open
/// documents are in the top bar's scene switcher). `Theme.chrome.doc_tabs`
/// still exists, since themes on disk set it and the dock's own tab strips read
/// it.
#[derive(Component, Clone, Copy)]
pub(crate) enum ChromeBar {
    Top,
    Status,
}

/// Repaint the chrome bars (top bar, status bar) from the ember `Theme.chrome`
/// whenever the theme changes — mirrors the dock's `apply_dock_style` so the
/// bars are theme-driven (and live-editable in the Theme tab) rather than baking
/// in palette colors. The status bar's separator sits on its top edge; the top
/// bar's on the bottom.
pub(crate) fn apply_chrome_style(
    theme: Res<renzora_ember::style::Theme>,
    mut q: Query<(Ref<ChromeBar>, &mut BackgroundColor, &mut BorderColor, &mut Node)>,
) {
    let repaint = theme.is_changed();
    for (kind, mut bg, mut bc, mut node) in &mut q {
        if !repaint && !kind.is_added() {
            continue;
        }
        let (s, edge_top) = match *kind {
            ChromeBar::Top => (&theme.top_bar, false),
            ChromeBar::Status => (&theme.status_bar, true),
        };
        bg.0 = s.bg.color();
        node.height = Val::Px(s.height);
        node.border = if edge_top {
            UiRect::top(Val::Px(s.border_width))
        } else {
            UiRect::bottom(Val::Px(s.border_width))
        };
        node.border_radius = BorderRadius::all(Val::Px(s.radius));
        node.padding = UiRect::axes(Val::Px(s.pad_x), Val::Px(s.pad_y));
        *bc = BorderColor::all(s.border.color());
    }
}

/// The bottom status bar: a "Ready" label + plugin-contributed items from the
/// bevy-native `ShellStatusRegistry`, rendered via a reactive keyed list (so live
/// metrics update without rebuilding the bar).
pub(crate) fn build_status_bar(
    commands: &mut Commands,
    fonts: &EmberFonts,
    themes: &[String],
    active: &str,
    theme_menu_open: bool,
) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(Color::NONE),
            ChromeBar::Status,
            renzora_ember::widgets::ThemeShaderSurface {
                surface: renzora_ember::widgets::ThemeSurface::StatusBar,
            },
            Name::new("status-bar"),
        ))
        .id();

    // Left items (Ready, the scene's saved state, then left-aligned status) fill
    // the bar, pushing the theme picker + right-aligned metrics to the right.
    // Ready and the plugin items are two keyed lists either side of the badge,
    // since a keyed list owns all of its parent's children.
    let row = |commands: &mut Commands, name: &'static str, grow: f32| {
        commands
            .spawn((
                Node {
                    flex_grow: grow,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(14.0),
                    min_width: Val::Px(0.0),
                    ..default()
                },
                Name::new(name),
            ))
            .id()
    };
    let left_content = row(commands, "status-left", 1.0);
    let ready = row(commands, "status-ready", 0.0);
    renzora_ember::reactive::tracked::keyed_list(commands, ready, status_snapshot_ready);
    let saved = crate::scene_switcher::build_saved_badge(commands, &fonts.ui);
    let left_items = row(commands, "status-left-items", 1.0);
    renzora_ember::reactive::tracked::keyed_list(commands, left_items, status_snapshot_left);
    commands.entity(left_content).add_children(&[ready, saved, left_items]);

    // The language + theme dropups — fixed elements on the right, before metrics.
    let lang_picker = language_dropup(commands, fonts);
    let dropup = theme_dropup(commands, fonts, themes, active, theme_menu_open);

    let right_content = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                ..default()
            },
            Name::new("status-right"),
        ))
        .id();
    renzora_ember::reactive::tracked::keyed_list(commands, right_content, status_snapshot_right);

    // Engine name + version, at the far right of the status bar.
    //
    // Somewhere it is *always* on screen, which the About modal is not — the
    // question "which build is this" comes up when reporting a bug or checking
    // whether an update landed, and both are moments where hunting through a
    // menu is friction. The status bar's right side already holds passive facts
    // (frame time, memory, GPU), so it costs no new furniture.
    //
    // `version::display()` rather than the bare constant: it says `r1-alpha7
    // (dev)` for a local build and the plain tag for a release, so a screenshot
    // of the bar answers "is this a build you shipped?" as well.
    //
    // Under the OS frame the window title says it, and that is always on screen
    // too. The web has no title bar, so it keeps this one.
    commands
        .entity(bar)
        .add_children(&[left_content, lang_picker, dropup, right_content]);
    if !renzora::EDITOR_OS_FRAME || cfg!(target_arch = "wasm32") {
        let version = commands
            .spawn((
                Text::new(format!("Renzora {}", renzora::version::display())),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_muted())),
                bevy::text::TextLayout::no_wrap(),
                Node { flex_shrink: 0.0, ..default() },
                renzora_ember::widgets::HoverTooltip::new(renzora::version::display()),
                Name::new("status-version"),
            ))
            .id();
        commands.entity(bar).add_child(version);
    }
    bar
}

/// The theme picker dropup in the status bar: shows the active theme and opens a
/// menu (flipped up — it's at the window bottom) of available themes; picking one
/// calls `ThemeManager::load_theme`, which the theme bridge applies + rebuilds.
fn theme_dropup(
    commands: &mut Commands,
    fonts: &EmberFonts,
    themes: &[String],
    active: &str,
    open: bool,
) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Flip *up* (the bar is at the window bottom) and anchor to the
                // trigger's right edge so the menu opens up-and-left, on-screen.
                bottom: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(160.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                // Start open when a prior chrome instance had it open (a theme
                // switch rebuilds the chrome — see `ThemeMenuOpen`).
                display: if open { Display::Flex } else { Display::None },
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(600),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("theme-menu"),
        ))
        .id();
    let mut rows = Vec::new();
    for name in themes {
        let n = name.clone();
        let icon = if name == active { "check" } else { "palette" };
        rows.push(menu_item(commands, fonts, icon, name, move |w| {
            if let Some(mut tm) = w.get_resource_mut::<renzora_theme::ThemeManager>() {
                tm.load_theme(&n);
            }
        }));
    }
    // Cap the height + scroll so the long theme list doesn't run off-screen.
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    commands.entity(content).add_children(&rows);
    // Keyed so the list keeps its scroll position when picking a theme rebuilds
    // the whole chrome (see `ThemeMenuOpen`).
    let scroll = scroll_area_keyed(commands, content, 260.0, "status-theme-menu");
    commands.entity(panel).add_child(scroll);

    let icon = icon_text(commands, &fonts.phosphor, "palette", text_muted(), 12.0);
    let theme_label = if active.is_empty() {
        renzora::lang::t_or("status.theme", "Theme")
    } else {
        active.to_string()
    };
    let label = commands
        .spawn((
            Text::new(theme_label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-up", text_muted(), 9.0);
    let trigger = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                position_type: PositionType::Relative,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            Popup { panel, open },
            ThemeDropup,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("theme-dropup"),
        ))
        .id();
    commands.entity(trigger).add_children(&[icon, label, caret, panel]);
    trigger
}

/// Marks the status-bar language picker trigger.
#[derive(Component)]
struct LanguageDropup;

/// The language picker dropup in the status bar: shows the active language and
/// opens a menu (flipped up) of every registered language — built-in packs plus
/// any external `languages/*.toml`. Picking one calls `renzora::lang::set_active`
/// and persists it via `renzora::save_language`; the resulting `LanguageChanged`
/// drives whatever live relocalization is wired up. Mirrors `theme_dropup`.
fn language_dropup(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let langs = renzora::lang::available();
    let active = renzora::lang::active_code();

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Flip up (bar is at the window bottom), anchored to the right.
                bottom: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(160.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(600),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("language-menu"),
        ))
        .id();

    let mut rows = Vec::new();
    for m in &langs {
        let code = m.code.clone();
        let label = if m.name.is_empty() {
            m.code.clone()
        } else {
            m.name.clone()
        };
        let icon = if m.code == active { "check" } else { "globe" };
        rows.push(menu_item(commands, fonts, icon, &label, move |_w| {
            renzora::lang::set_active(&code);
            let _ = renzora::save_language(&code);
        }));
    }
    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    commands.entity(content).add_children(&rows);
    let scroll = scroll_area_keyed(commands, content, 260.0, "status-language-menu");
    commands.entity(panel).add_child(scroll);

    // Trigger shows the active language's native name (falls back to its code).
    let active_name = langs
        .iter()
        .find(|m| m.code == active)
        .map(|m| {
            if m.name.is_empty() {
                m.code.clone()
            } else {
                m.name.clone()
            }
        })
        .unwrap_or_else(|| {
            if active.is_empty() {
                renzora::lang::t("settings.row.language")
            } else {
                active.clone()
            }
        });

    let icon = icon_text(commands, &fonts.phosphor, "globe", text_muted(), 12.0);
    let label = commands
        .spawn((
            Text::new(active_name),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-up", text_muted(), 9.0);
    let trigger = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                position_type: PositionType::Relative,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            Popup { panel, open: false },
            LanguageDropup,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("language-dropup"),
        ))
        .id();
    commands.entity(trigger).add_children(&[icon, label, caret, panel]);
    trigger
}

enum StatusRow {
    Label(String, (u8, u8, u8)),
    Seg(renzora::ShellStatusSegment),
}

/// Status segments for one alignment, as keyed rows (each item's `render` is
/// recomputed every frame).
fn status_rows(world: &Rx, align: renzora::ShellStatusAlign) -> Vec<StatusRow> {
    let mut rows: Vec<StatusRow> = Vec::new();
    if let Some(reg) = world.get_resource::<renzora::ShellStatusRegistry>() {
        let mut items: Vec<&renzora::ShellStatusItem> =
            reg.items.iter().filter(|i| i.align == align).collect();
        items.sort_by_key(|i| i.order);
        for it in items {
            rows.extend((it.render)(world.untracked()).into_iter().map(StatusRow::Seg));
        }
    }
    rows
}

/// The current text of segment `index` on one side: what a segment's text node
/// is bound to, so a live metric updates in place.
fn segment_text(world: &Rx, align: renzora::ShellStatusAlign, index: usize) -> String {
    match status_rows(world, align).into_iter().nth(index) {
        Some(StatusRow::Seg(s)) => s.text,
        _ => String::new(),
    }
}

/// Build a keyed snapshot from a row list. `align` is the side the rows come
/// from, for segments; the Ready label passes `None`.
///
/// A segment's text is NOT part of its row hash. An FPS or memory readout
/// changes several times a second, and with the text hashed every change
/// despawned and respawned the whole segment: new nodes, a new font, the bar's
/// children changed. Each of those dirtied the layout up to the window root, and
/// the full re-layout that followed, of every panel, was most of the editor's
/// layout time. The text node is bound to [`segment_text`] instead, so a new
/// number re-measures one piece of text and nothing else.
fn rows_snapshot(
    rows: Vec<StatusRow>,
    align: Option<renzora::ShellStatusAlign>,
) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match r {
                StatusRow::Label(t, c) => (0u8, t, c).hash(&mut h),
                // The bar's *fraction* is quantized to whole percent before it
                // reaches the hash. It changes continuously, and every change
                // rebuilds the row — a raw f32 would despawn and respawn the
                // segment on most frames of a long job. `Busy` is not hashed at
                // all beyond its kind: it animates in place (see
                // `progress_indeterminate`), so it never needs a rebuild.
                StatusRow::Seg(s) => {
                    let bar = match s.bar {
                        None => (0u8, 0u8),
                        Some(renzora::ShellStatusBar::Busy) => (1u8, 0u8),
                        Some(renzora::ShellStatusBar::Fraction(f)) => {
                            (2u8, (f.clamp(0.0, 1.0) * 100.0) as u8)
                        }
                    };
                    (1u8, &s.icon, s.color, bar, s.action).hash(&mut h)
                }
            }
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| status_row(c, f, &rows[i], align.map(|a| (a, i)))),
    }
}

/// The Ready label at the head of the left side. A plugin can swap the
/// "Ready" text via [`renzora::ShellReadyStatus`] (e.g. the auto-save countdown).
fn status_snapshot_ready(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    let (label, color) = world
        .get_resource::<renzora::ShellReadyStatus>()
        .and_then(|r| r.label.clone().map(|t| (t, r.color)))
        .map(|(t, c)| (t, c.map(|c| (c[0], c[1], c[2])).unwrap_or_else(text_muted)))
        .unwrap_or_else(|| (renzora::lang::t_or("status.ready", "Ready"), text_muted()));
    rows_snapshot(vec![StatusRow::Label(label, color)], None)
}

/// Left side after Ready and the saved badge: the left-aligned plugin items.
fn status_snapshot_left(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    rows_snapshot(status_rows(world, renzora::ShellStatusAlign::Left), Some(renzora::ShellStatusAlign::Left))
}

/// Right side: the right-aligned metrics.
fn status_snapshot_right(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    rows_snapshot(status_rows(world, renzora::ShellStatusAlign::Right), Some(renzora::ShellStatusAlign::Right))
}

/// WCAG contrast a status segment's text must reach against the bar.
const MIN_CONTRAST: f32 = 4.5;

/// `color`, pushed toward black (on a light bar) or white (on a dark one) just
/// far enough to read against `bg`.
///
/// A plugin hands its segment a fixed RGB, and nearly every plugin picked it
/// against the dark theme: a pale green FPS counter or a light grey memory
/// readout is unreadable on the light theme's white bar. Plugins cannot see the
/// palette, so the host is the one place this can be fixed for all of them at
/// once. Mixing toward black or white keeps the hue, so a green that means "ok"
/// stays green; a color that already reads is returned untouched.
fn legible(color: (u8, u8, u8), bg: (u8, u8, u8)) -> (u8, u8, u8) {
    fn lum((r, g, b): (u8, u8, u8)) -> f32 {
        let ch = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)
    }
    fn contrast(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
        let (la, lb) = (lum(a), lum(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }
    if contrast(color, bg) >= MIN_CONTRAST {
        return color;
    }
    let target = if lum(bg) > 0.5 { 0.0 } else { 255.0 };
    let mix = |c: u8, t: f32| (c as f32 + (target - c as f32) * t).round() as u8;
    let mut out = color;
    for step in 1..=10 {
        let t = step as f32 / 10.0;
        out = (mix(color.0, t), mix(color.1, t), mix(color.2, t));
        if contrast(out, bg) >= MIN_CONTRAST {
            break;
        }
    }
    out
}

/// One row of the bar. `source` is where a segment's text lives (its side and
/// index), so the text node can be bound to it rather than rebuilt with it.
fn status_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    row: &StatusRow,
    source: Option<(renzora::ShellStatusAlign, usize)>,
) -> Entity {
    match row {
        StatusRow::Label(text, color) => commands
            .spawn((
                Text::new(text.clone()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(*color)),
            ))
            .id(),
        StatusRow::Seg(s) => {
            let r = commands
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .id();
            if let Some(action) = s.action {
                commands.entity(r).insert((
                    UiInteraction::default(),
                    StatusSegmentAction(action),
                    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                ));
            }
            let mut kids = Vec::new();
            let color = legible((s.color[0], s.color[1], s.color[2]), window_bg());
            if !s.icon.is_empty() {
                let glyph = renzora_ember::font::icon_glyph(&s.icon)
                    .unwrap_or_else(|| s.icon.chars().next().unwrap_or(' '));
                kids.push(
                    commands
                        .spawn((
                            Text::new(glyph.to_string()),
                            TextFont {
                                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                                font_size: bevy::text::FontSize::Px(12.0),
                                ..default()
                            },
                            TextColor(rgb(color)),
                        ))
                        .id(),
                );
            }
            let text = commands
                .spawn((
                    Text::new(s.text.clone()),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(color)),
                ))
                .id();
            if let Some((align, index)) = source {
                renzora_ember::reactive::tracked::bind_text(commands, text, move |w| {
                    segment_text(w, align, index)
                });
            }
            kids.push(text);
            // Sized for a 22px-tall bar: wide enough to read as progress,
            // narrow enough that a background job doesn't push the rest of the
            // status bar around.
            match s.bar {
                None => {}
                Some(renzora::ShellStatusBar::Busy) => kids.push(
                    renzora_ember::widgets::progress_indeterminate(commands, 70.0, 4.0),
                ),
                Some(renzora::ShellStatusBar::Fraction(f)) => {
                    kids.push(renzora_ember::widgets::progress_sized(commands, f, 70.0, 4.0))
                }
            }
            commands.entity(r).add_children(&kids);
            r
        }
    }
}
