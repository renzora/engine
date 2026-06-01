//! `renzora_shell` — the bevy_ui-native editor shell.
//!
//! The editor's layout (menu bar, ribbon, document tabs, dock splits, panel
//! tab-bars, status bar) drawn with **`bevy_ui`** instead of egui. This is the
//! host half of the egui → bevy_ui/HUI migration: the [`dock`] data model is
//! reused 1:1 from the egui editor; only the renderer changes from egui
//! immediate-mode to a bevy_ui reconcile.
//!
//! ## Coexistence during the migration
//! The shell is additive. [`renzora::EditorUiBackend`] selects which editor
//! renders — the legacy egui editor (`Egui`, default) or this shell (`BevyUi`)
//! — and the two are mutually exclusive (the egui `editor_ui_system` is gated
//! off when the backend is `BevyUi`). Press **F10** in the editor to toggle, so
//! the editor stays fully usable while the shell is built out panel by panel.
//!
//! ## Status
//! Phase 1: static render of the Scene layout + chrome, with **placeholder**
//! content per panel (panel title centered). No resize/tab-drag yet, no real
//! panel content. Colors are a local dark palette; theme unification with a
//! bevy-native theme comes later (the current `renzora_theme` is egui-coupled).

use bevy::prelude::*;

use renzora::EditorUiBackend;

pub mod dock;

use dock::{DockTree, SplitDirection};

#[derive(Default)]
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShellPlugin (bevy_ui editor shell)");
        app.init_resource::<EditorUiBackend>();
        app.add_systems(Startup, load_shell_assets);
        app.add_systems(Update, (toggle_backend, manage_shell_root));
    }
}

renzora::add!(ShellPlugin, Editor);

/// Fonts/handles the shell renders with. Loaded once at startup.
#[derive(Resource)]
struct ShellAssets {
    /// Proportional UI font — matches the egui editor's default (Noto Sans).
    ui_font: Handle<Font>,
}

/// Noto Sans, embedded so the font is available regardless of the running
/// editor's asset-root (which is the open *project's* folder, not the engine's
/// `assets/`). Mirrors how the egui editor and `renzora_hui` embed their fonts.
const NOTO_SANS: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

fn load_shell_assets(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let ui_font = match bevy::text::Font::try_from_bytes(NOTO_SANS.to_vec()) {
        Ok(font) => fonts.add(font),
        Err(e) => {
            error!("[shell] failed to load embedded Noto Sans: {e:?}");
            Handle::default()
        }
    };
    commands.insert_resource(ShellAssets { ui_font });
}

/// Global nudge so shell text matches the egui editor's slightly smaller text.
const TEXT_SCALE: f32 = 0.92;

/// A `TextFont` in the shell's UI font at the given (pre-scale) size.
fn ui_font(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: size * TEXT_SCALE,
        ..default()
    }
}

// ── Palette — the default theme colors from `renzora_theme` (`*::default()`).
// Hardcoded as bevy Colors so the shell stays egui-free; a bevy-native theme
// resource will replace these constants later, but the VALUES are the source of
// truth from `renzora_theme::{SurfaceColors, PanelColors, TextColors, ...}`.

// Calibrated to the egui editor's *appearance* (eyedropped against the swatch
// ladder). egui lifts the dark end, so these run a touch brighter than the raw
// `renzora_theme` bytes (window 11 / panel 26 / …); the blue undertone is kept.
const WINDOW_BG: (u8, u8, u8) = (24, 24, 30); // chrome (top bar/doc tabs/status/dock gaps)
const PANEL_BG: (u8, u8, u8) = (33, 33, 39); // leaf content (lighter than chrome)
const HEADER_BG: (u8, u8, u8) = (37, 37, 44); // doc tabs + panel tab headers
const TAB_ACTIVE_BG: (u8, u8, u8) = (50, 50, 62); // active tab
const DIVIDER: (u8, u8, u8) = (14, 14, 20); // split dividers (not black)
const TEXT_PRIMARY: (u8, u8, u8) = (230, 230, 240); // text.primary
const TEXT_MUTED: (u8, u8, u8) = (148, 148, 160); // text.muted
const PLACEHOLDER: (u8, u8, u8) = (110, 110, 122); // dim placeholder
const PLAY_GREEN: (u8, u8, u8) = (89, 191, 115); // semantic.success
const ACCENT_BLUE: (u8, u8, u8) = (80, 140, 255); // active ribbon underline

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::srgb_u8(r, g, b)
}

/// Marks the shell's root UI entity so it can be despawned when the backend
/// switches back to egui.
#[derive(Component)]
struct ShellRoot;

// ── Systems ─────────────────────────────────────────────────────────────────

/// F10 flips the active editor UI backend between the legacy egui editor and
/// the bevy_ui shell.
fn toggle_backend(keys: Res<ButtonInput<KeyCode>>, mut backend: ResMut<EditorUiBackend>) {
    if keys.just_pressed(KeyCode::F10) {
        *backend = match *backend {
            EditorUiBackend::Egui => EditorUiBackend::BevyUi,
            EditorUiBackend::BevyUi => EditorUiBackend::Egui,
        };
        info!("[shell] editor UI backend -> {:?}", *backend);
    }
}

/// Spawn the shell when the backend is `BevyUi`; tear it down when it isn't.
fn manage_shell_root(
    mut commands: Commands,
    backend: Res<EditorUiBackend>,
    assets: Option<Res<ShellAssets>>,
    roots: Query<Entity, With<ShellRoot>>,
) {
    let want = backend.is_bevy_ui();
    let have = !roots.is_empty();
    if want && !have {
        // Assets load at startup; wait for the font handle before building so
        // text renders in Noto Sans from the first frame rather than flashing
        // the fallback font.
        let Some(assets) = assets else {
            return;
        };
        spawn_shell(&mut commands, &assets.ui_font);
    } else if !want && have {
        for e in &roots {
            commands.entity(e).despawn();
        }
    }
}

// ── Build the shell UI tree ─────────────────────────────────────────────────

fn spawn_shell(commands: &mut Commands, font: &Handle<Font>) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(WINDOW_BG)),
            ShellRoot,
            renzora::HideInHierarchy,
            Name::new("Renzora Shell"),
        ))
        .id();

    let top_bar = build_top_bar(commands, font);
    let doctabs = build_doc_tabs(commands, font);

    // Dock area fills the remaining vertical space.
    let dock_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("dock-area"),
        ))
        .id();
    let tree = build_tree(commands, font, &dock::scene_layout());
    commands.entity(dock_area).add_child(tree);

    let statusbar = chrome_row(
        commands,
        font,
        "status-bar",
        22.0,
        rgb(WINDOW_BG),
        16.0,
        10.0,
        &[
            ("Ready", false),
            ("Dark", false),
            ("Vulkan", false),
            ("60 FPS", false),
        ],
    );

    commands
        .entity(root)
        .add_children(&[top_bar, doctabs, dock_area, statusbar]);
}

/// The top bar: File/Edit/View/Help on the left, the layout ribbon centered,
/// and action buttons (play, code, settings, sign-in, window controls) on the
/// right. The center group stays centered because the left and right zones
/// both grow equally.
fn build_top_bar(commands: &mut Commands, font: &Handle<Font>) -> Entity {
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
            BackgroundColor(rgb(WINDOW_BG)),
            Name::new("top-bar"),
        ))
        .id();

    // Left: application menus. Small gap so File/Edit/View/Help sit close
    // together like the egui menu bar.
    let left = zone(commands, "top-left", JustifyContent::FlexStart, 2.0, 1.0);
    let left_kids = text_items(
        commands,
        font,
        &[("File", false), ("Edit", false), ("View", false), ("Help", false)],
        14.0,
    );
    commands.entity(left).add_children(&left_kids);

    // Center: the layout ribbon (workspace switcher). `grow: 0` keeps it
    // content-sized so it sits centered between the two growing side zones.
    let center = zone(commands, "top-center", JustifyContent::Center, 2.0, 0.0);
    let mut center_kids = vec![glyph(commands, "magnifying-glass", TEXT_MUTED, 14.0)];
    for (label, active) in [
        ("Scene", true),
        ("Blueprints", false),
        ("Scripting", false),
        ("Animation", false),
        ("Materials", false),
        ("Particles", false),
        ("Video", false),
        ("Audio", false),
        ("Debug", false),
        ("+", false),
    ] {
        center_kids.push(ribbon_item(commands, font, label, active));
    }
    commands.entity(center).add_children(&center_kids);

    // Right: action buttons, account group, then window controls.
    let right = zone(commands, "top-right", JustifyContent::FlexEnd, 8.0, 1.0);
    let play = icon_item(commands, "play", PLAY_GREEN, 16.0);
    let code = icon_item(commands, "code", TEXT_MUTED, 16.0);
    let settings = icon_item(commands, "gear", TEXT_MUTED, 16.0);

    // Account: user icon + "Sign In", kept tight together.
    let account = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("account"),
        ))
        .id();
    let user = glyph(commands, "user", TEXT_MUTED, 14.0);
    let sign_in = commands
        .spawn((
            Text::new("Sign In"),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id();
    commands.entity(account).add_children(&[user, sign_in]);

    // Window controls, spaced away from the account group.
    let window = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
            Name::new("window-buttons"),
        ))
        .id();
    let min = icon_item(commands, "minus", TEXT_MUTED, 14.0);
    let max = icon_item(commands, "square", TEXT_MUTED, 13.0);
    let close = icon_item(commands, "x", TEXT_MUTED, 14.0);
    commands.entity(window).add_children(&[min, max, close]);

    commands
        .entity(right)
        .add_children(&[play, code, settings, account, window]);

    commands.entity(bar).add_children(&[left, center, right]);
    bar
}

/// A top-bar ribbon entry (workspace switcher). Full height so the active
/// item's blue underline pins to the bottom edge of the top bar.
fn ribbon_item(commands: &mut Commands, font: &Handle<Font>, label: &str, active: bool) -> Entity {
    let item = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new(format!("ribbon:{label}")),
        ))
        .id();
    let text_wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(7.0)),
                ..default()
            },
            Name::new("ribbon-label"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(label),
                ui_font(font, 12.0),
                TextColor(rgb(if active { TEXT_PRIMARY } else { TEXT_MUTED })),
            ));
        })
        .id();
    let underline = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(if active { rgb(ACCENT_BLUE) } else { Color::NONE }),
            Name::new("ribbon-underline"),
        ))
        .id();
    commands.entity(item).add_children(&[text_wrap, underline]);
    item
}

/// The document tab strip: a button-styled active document tab (file icon +
/// name + close) and an add-tab button, with a bottom border separating it
/// from the dock below.
fn build_doc_tabs(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            BorderColor::all(rgb(DIVIDER)),
            Name::new("doc-tabs"),
        ))
        .id();
    let tab = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                // Blue accent on the top edge of the active document tab.
                border: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            BorderColor::all(rgb(ACCENT_BLUE)),
            Name::new("doc:sponza"),
        ))
        .id();
    let ic = glyph(commands, "file", TEXT_PRIMARY, 13.0);
    let lbl = commands
        .spawn((
            Text::new("sponza"),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    let cl = glyph(commands, "x", TEXT_MUTED, 11.0);
    commands.entity(tab).add_children(&[ic, lbl, cl]);

    // Add-tab button (a small box, not a bare glyph).
    let plus = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Name::new("doc-add"),
        ))
        .id();
    let plus_icon = glyph(commands, "plus", TEXT_MUTED, 13.0);
    commands.entity(plus).add_child(plus_icon);
    commands.entity(bar).add_children(&[tab, plus]);
    bar
}

/// A full-height flex row used as a top-bar zone (left / center / right).
fn zone(
    commands: &mut Commands,
    name: &str,
    justify: JustifyContent,
    gap: f32,
    grow: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: justify,
                column_gap: Val::Px(gap),
                flex_grow: grow,
                ..default()
            },
            Name::new(name.to_string()),
        ))
        .id()
}

/// A padded text item (menu entry, ribbon tab). `active` → primary, else muted.
fn text_items(
    commands: &mut Commands,
    font: &Handle<Font>,
    items: &[(&str, bool)],
    size: f32,
) -> Vec<Entity> {
    items
        .iter()
        .map(|(label, active)| {
            let color = if *active { TEXT_PRIMARY } else { TEXT_MUTED };
            text_item(commands, font, label, color, size)
        })
        .collect()
}

fn text_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new(format!("item:{label}")),
        ))
        .with_children(|p| {
            p.spawn((Text::new(label), ui_font(font, size), TextColor(rgb(color))));
        })
        .id()
}

/// A Phosphor icon button. The HUI `Icon` component is resolved into a Text +
/// Phosphor-font glyph by `renzora_hui`'s `apply_icons` system (run globally by
/// HuiPlugin), so the editor's icon set renders straight into bevy_ui.
fn icon_item(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            renzora_hui::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}

/// An inline Phosphor glyph with no padding (for tab icons, close/add buttons).
fn glyph(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            renzora_hui::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("glyph:{name}")),
        ))
        .id()
}

/// Display title + Phosphor icon for a panel id. Mirrors each panel's
/// `EditorPanel::title()`/`icon()` (which the shell can't reach yet — those are
/// egui trait objects). A bevy-native panel registry will replace this map.
fn panel_meta(id: &str) -> (String, &'static str) {
    let (title, icon): (&str, &str) = match id {
        "viewport" => ("Viewport", "monitor"),
        "render_pipeline" => ("Render Pipeline", "git-fork"),
        "code_editor" => ("Code Editor", "code"),
        "assets" => ("Assets", "folder"),
        "hub_store" => ("Hub Store", "storefront"),
        "console" => ("Console", "terminal"),
        "mixer" => ("Mixer", "faders"),
        "sequencer" => ("Sequencer", "film-strip"),
        "timeline" => ("Timeline", "clock"),
        "record" => ("Record", "record"),
        "hierarchy" => ("Hierarchy", "tree-structure"),
        "scenes" => ("Scenes", "stack"),
        "shape_library" => ("Shapes", "shapes"),
        "inspector" => ("Inspector", "sliders-horizontal"),
        "gamepad" => ("Gamepad", "game-controller"),
        "history" => ("History", "clock-counter-clockwise"),
        _ => return (humanize(id), "circle"),
    };
    (title.to_string(), icon)
}

/// A horizontal strip of text items (menu bar, ribbon, doc tabs, status bar).
/// `active` items render in primary text color, the rest muted.
fn chrome_row(
    commands: &mut Commands,
    font: &Handle<Font>,
    name: &str,
    height: f32,
    bg: Color,
    gap: f32,
    pad: f32,
    items: &[(&str, bool)],
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(height),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(gap),
                padding: UiRect::horizontal(Val::Px(pad)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(bg),
            Name::new(name.to_string()),
        ))
        .id();

    let kids: Vec<Entity> = items
        .iter()
        .map(|(label, active)| {
            let color = if *active { TEXT_PRIMARY } else { TEXT_MUTED };
            commands
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    Name::new(format!("item:{label}")),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new(*label),
                        ui_font(font, 12.0),
                        TextColor(rgb(color)),
                    ));
                })
                .id()
        })
        .collect();
    commands.entity(row).add_children(&kids);
    row
}

/// Recursively convert a [`DockTree`] into a bevy_ui entity subtree.
fn build_tree(commands: &mut Commands, font: &Handle<Font>, tree: &DockTree) -> Entity {
    match tree {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let row = matches!(direction, SplitDirection::Horizontal);
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_direction: if row {
                            FlexDirection::Row
                        } else {
                            FlexDirection::Column
                        },
                        ..default()
                    },
                    Name::new("split"),
                ))
                .id();

            let pct = ratio.clamp(0.1, 0.9) * 100.0;

            // First child: fixed fraction of the split.
            let mut wa = Node {
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                wa.width = Val::Percent(pct);
                wa.height = Val::Percent(100.0);
            } else {
                wa.height = Val::Percent(pct);
                wa.width = Val::Percent(100.0);
            }
            let wrap_a = commands.spawn((wa, Name::new("split-first"))).id();
            let child_a = build_tree(commands, font, first);
            commands.entity(wrap_a).add_child(child_a);

            // Divider (static for now; becomes draggable in a later phase).
            let mut dv = Node {
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                dv.width = Val::Px(2.0);
                dv.height = Val::Percent(100.0);
            } else {
                dv.height = Val::Px(2.0);
                dv.width = Val::Percent(100.0);
            }
            let divider = commands
                .spawn((dv, BackgroundColor(rgb(DIVIDER)), Name::new("divider")))
                .id();

            // Second child: fills the remainder.
            let mut wb = Node {
                overflow: Overflow::clip(),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                ..default()
            };
            if row {
                wb.height = Val::Percent(100.0);
            } else {
                wb.width = Val::Percent(100.0);
            }
            let wrap_b = commands.spawn((wb, Name::new("split-second"))).id();
            let child_b = build_tree(commands, font, second);
            commands.entity(wrap_b).add_child(child_b);

            commands
                .entity(container)
                .add_children(&[wrap_a, divider, wrap_b]);
            container
        }
        DockTree::Leaf { tabs, active_tab } => build_leaf(commands, font, tabs, *active_tab),
        DockTree::Empty => commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                Name::new("empty"),
            ))
            .id(),
    }
}

/// A dock leaf: a tab-bar over a content region (placeholder content for now).
fn build_leaf(
    commands: &mut Commands,
    font: &Handle<Font>,
    tabs: &[String],
    active: usize,
) -> Entity {
    let leaf = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(PANEL_BG)),
            Name::new("leaf"),
        ))
        .id();

    // Tab bar.
    let tabbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(HEADER_BG)),
            Name::new("tabbar"),
        ))
        .id();
    // Each tab: icon + label, with a close × on the active tab.
    let mut bar_kids: Vec<Entity> = Vec::new();
    for (i, id) in tabs.iter().enumerate() {
        let is_active = i == active;
        let fg = if is_active { TEXT_PRIMARY } else { TEXT_MUTED };
        let (title, icon) = panel_meta(id);
        let tab = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(if is_active {
                    rgb(TAB_ACTIVE_BG)
                } else {
                    Color::NONE
                }),
                Name::new(format!("tab:{id}")),
            ))
            .id();
        let tab_icon = glyph(commands, icon, fg, 13.0);
        let tab_label = commands
            .spawn((Text::new(title), ui_font(font, 12.0), TextColor(rgb(fg))))
            .id();
        let mut kids = vec![tab_icon, tab_label];
        if is_active {
            kids.push(glyph(commands, "x", TEXT_MUTED, 11.0));
        }
        commands.entity(tab).add_children(&kids);
        bar_kids.push(tab);
    }
    // Trailing add-tab button.
    bar_kids.push(glyph(commands, "plus", TEXT_MUTED, 13.0));
    commands.entity(tabbar).add_children(&bar_kids);

    // Content region — placeholder showing the active panel's title.
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Name::new("content"),
        ))
        .with_children(|p| {
            let title = tabs.get(active).map(|s| panel_meta(s).0).unwrap_or_default();
            p.spawn((
                Text::new(title),
                ui_font(font, 13.0),
                TextColor(rgb(PLACEHOLDER)),
            ));
        })
        .id();

    commands.entity(leaf).add_children(&[tabbar, content]);
    leaf
}

/// `render_pipeline` → `Render Pipeline`, `code_editor` → `Code Editor`.
fn humanize(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
