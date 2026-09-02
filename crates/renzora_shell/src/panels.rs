//! Panel chrome metadata and content dispatch.
//!
//! [`PANEL_META`] is the shipped title/icon/category for every editor panel, and
//! the reason a dock tab reads "Hierarchy" with a tree glyph rather than ember's
//! generic circle. It is seeded as *defaults*: a plugin that called
//! `register_shell_panel` for an id keeps its own entry.
//!
//! [`content_dispatch`] fills each dock leaf with its active panel's UI, skipping
//! any id a crate renders natively.

use bevy::prelude::*;

use renzora::NativePanelIds;
use renzora_ember::dock::{tab_pane, DockLeaf, DockTab, TabPane};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{placeholder, rgb};

/// Chrome metadata (id, title, icon, category) for every editor panel, seeded
/// into [`renzora::ShellPanelRegistry`] at startup. Without this the dock tabs
/// fall back to ember's generic circle glyph and the Add-Panel `+` picker is
/// empty (nothing else populates the registry). Icons are kebab-case Phosphor
/// names, resolved to glyphs via `renzora_ember::font::icon_glyph`.
const PANEL_META: &[(&str, &str, &str, &str)] = &[
    // Scene / editing
    ("hierarchy", "Hierarchy", "tree-structure", "Scene"),
    ("inspector", "Inspector", "sliders-horizontal", "Editing"),
    ("scenes", "Scenes", "film-slate", "Scene"),
    ("scene_diagnostics", "Scene Diagnostics", "first-aid", "Debug"),
    ("streaming_debug", "Streaming", "broadcast", "Debug"),
    ("assets", "Assets", "folder-open", "Assets"),
    ("shape_library", "Shapes", "shapes", "Assets"),
    ("level_presets", "Level Presets", "globe", "Scene"),
    ("history", "History", "clock-counter-clockwise", "Editing"),
    ("code_editor", "Code", "code", "Editing"),
    ("console", "Console", "terminal", "Debug"),
    ("problems", "Problems", "warning-circle", "Debug"),
    // Viewports
    ("viewport", "Viewport", "perspective", "Scene"),
    ("viewport-2", "Viewport 2", "perspective", "Scene"),
    ("viewport-3", "Viewport 3", "perspective", "Scene"),
    ("viewport-4", "Viewport 4", "perspective", "Scene"),
    ("camera_preview", "Camera Preview", "video-camera", "Scene"),
    // Audio
    ("mixer", "Mixer", "faders", "Audio"),
    // Animation
    ("timeline", "Timeline", "film-strip", "Animation"),
    ("animation", "Animation", "play-circle", "Animation"),
    ("studio_preview", "Studio Preview", "person", "Animation"),
    ("animator_state_machine", "State Machine", "graph", "Animation"),
    ("animator_params", "Parameters", "sliders-horizontal", "Animation"),
    // Material
    ("material_preview", "Material Preview", "sphere", "Material"),
    ("material_inspector", "Material", "palette", "Material"),
    ("material_graph", "Material Graph", "graph", "Material"),
    // Particle
    ("particle_preview", "Particle Preview", "sparkle", "Particle"),
    ("particle_editor", "Particle Editor", "sparkle", "Particle"),
    ("particle_graph", "Particle Graph", "graph", "Particle"),
    // Shader
    ("shader_properties", "Shader", "graphics-card", "Shader"),
    ("shader_preview", "Shader Preview", "image", "Shader"),
    ("shader_compiler_log", "Compiler Log", "list-dashes", "Shader"),
    // Blueprint
    ("blueprint_graph", "Blueprint", "blueprint", "Blueprint"),
    ("blueprint_properties", "Blueprint Properties", "sliders-horizontal", "Blueprint"),
    // The Marketplace panels (store, library, publish) and the wallet are NOT
    // listed here. They belong to the `marketplace` plugin, which registers its
    // own shell metadata the way any plugin does — so an install without it
    // shows no Marketplace category at all, rather than three panels that open
    // empty.
    // Network
    ("network_monitor", "Network", "broadcast", "Network"),
    ("network_entities", "Net Entities", "users-three", "Network"),
    ("network_settings", "Net Settings", "gear", "Network"),
    // Terrain / foliage / navigation
    ("terrain_tools", "Terrain", "mountains", "Terrain"),
    ("foliage_painting", "Foliage", "tree", "Terrain"),
    ("navmesh", "Navmesh", "path", "Navigation"),
    // Input
    ("gamepad", "Gamepad", "game-controller", "Input"),
    // Debug / profiling
    ("performance", "Performance", "gauge", "Debug"),
    ("render_stats", "Render Stats", "chart-bar", "Debug"),
    ("ecs_stats", "ECS Stats", "list-numbers", "Debug"),
    ("memory_profiler", "Memory", "memory", "Debug"),
    ("system_profiler", "System", "cpu", "Debug"),
    ("physics_debug", "Physics Debug", "atom", "Debug"),
    ("camera_debug", "Camera Debug", "video-camera", "Debug"),
    ("culling_debug", "Culling", "scissors", "Debug"),
    ("material_resolver_diag", "Material Diag", "palette", "Debug"),
    ("lumen_diag", "Lumen Diag", "lightbulb", "Debug"),
    ("scripting_diag", "Scripting Diag", "bug", "Debug"),
    ("ui_reactivity", "UI Reactivity", "lightning", "Debug"),
    ("ui_layout", "UI Layout", "layout", "Debug"),
    ("resources", "Resources", "database", "Debug"),
    // Plugins
    ("plugin_resources", "Plugin Resources", "puzzle-piece", "Tools"),
];

/// Seed [`renzora::ShellPanelRegistry`] from [`PANEL_META`] (as defaults — a
/// plugin that already called `register_shell_panel` for an id wins).
pub(crate) fn seed_panel_meta(app: &mut App) {
    use renzora::ShellPanelInfo;
    let mut reg = app.world_mut().resource_mut::<renzora::ShellPanelRegistry>();
    for &(id, title, icon, category) in PANEL_META {
        reg.panels.entry(id.to_string()).or_insert_with(|| ShellPanelInfo {
            title: title.to_string(),
            icon: icon.to_string(),
            category: category.to_string(),
        });
    }
}

/// Apply real panel titles/icons from [`renzora::ShellPanelRegistry`] onto the
/// dock tabs (overriding ember's humanized defaults). Cheap; only writes on a
/// real change.
pub(crate) fn apply_panel_meta(
    registry: Res<renzora::ShellPanelRegistry>,
    tabs: Query<&DockTab>,
    mut texts: Query<&mut Text>,
) {
    if registry.panels.is_empty() {
        return;
    }
    for tab in &tabs {
        let Some(info) = registry.panels.get(&tab.id) else {
            continue;
        };
        if !info.title.is_empty() {
            // Localize the tab title via `panel.<id>`, falling back to the
            // registry's English title. Compared against the *localized* value so
            // it doesn't thrash, and re-localizes live (this system runs each frame).
            let title = renzora::lang::t_or(&format!("panel.{}", tab.id), &info.title);
            if let Ok(mut t) = texts.get_mut(tab.label) {
                if t.0 != title {
                    t.0 = title;
                }
            }
        }
        if !info.icon.is_empty() {
            // `info.icon` is a kebab-case Phosphor NAME; resolve it to the glyph
            // the tab's phosphor-font Text expects (mirrors the status bar).
            let glyph = renzora_ember::font::icon_glyph(&info.icon)
                .unwrap_or('\u{E4C6}')
                .to_string();
            if let Ok(mut t) = texts.get_mut(tab.icon) {
                if t.0 != glyph {
                    t.0 = glyph;
                }
            }
        }
    }
}

/// Fill each leaf's content with the active panel's UI. Panels that registered a
/// **bevy-native** renderer (`NativePanelIds`) own their own `content` entity and
/// are skipped here. For the rest: the `gallery_*` ember showcases, and a
/// centered title placeholder for everything else. Shares the `PanelContent`
/// marker with native panels so the two never desync over one content entity.
pub(crate) fn content_dispatch(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    native: Option<Res<NativePanelIds>>,
    leaves: Query<&DockLeaf>,
    panes: Query<&TabPane>,
    children_q: Query<&Children>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        if leaf.active.is_empty() {
            continue;
        }
        // A panel crate renders this id itself — leave its content alone.
        if native
            .as_ref()
            .is_some_and(|n| n.0.contains(&leaf.active))
        {
            continue;
        }
        // Build the active tab's pane once (lazily). If it already exists, do
        // nothing — `sync_panes` toggles its visibility on tab switch.
        let exists = children_q.get(leaf.content).is_ok_and(|kids| {
            kids.iter()
                .any(|c| panes.get(c).is_ok_and(|p| p.id == leaf.active))
        });
        if exists {
            continue;
        }
        let built = build_panel_content(&mut commands, &fonts, &leaf.active);
        let pane = tab_pane(&mut commands, &leaf.active, built, true);
        commands.entity(leaf.content).add_child(pane);
    }
}

/// Build the bevy_ui content for a panel id.
fn build_panel_content(commands: &mut Commands, fonts: &EmberFonts, id: &str) -> Entity {
    use renzora_ember::widgets;
    match id {
        "gallery_typography" => widgets::gallery_typography(commands, fonts),
        "gallery_buttons" => widgets::gallery_buttons(commands, fonts),
        "gallery_inputs" => widgets::gallery_inputs(commands, fonts),
        "gallery_selection" => widgets::gallery_selection(commands, fonts),
        "gallery_feedback" => widgets::gallery_feedback(commands, fonts),
        "gallery_inspector" => widgets::gallery_inspector(commands, fonts),
        "gallery_containers" => widgets::gallery_containers(commands, fonts),
        "gallery_nav" => widgets::gallery_nav(commands, fonts),
        "gallery_data" => widgets::gallery_data(commands, fonts),
        "gallery_forms" => widgets::gallery_forms(commands, fonts),
        "gallery_overlays" => widgets::gallery_overlays(commands, fonts),
        "gallery_menus" => widgets::gallery_menus(commands, fonts),
        "gallery_extras" => widgets::gallery_extras(commands, fonts),
        "gallery_node_graph" => widgets::gallery_node_graph(commands, fonts),
        "gallery_timeline" => widgets::gallery_timeline(commands, fonts),
        "gallery_code" => widgets::gallery_code(commands, fonts),
        "gallery_charts" => widgets::gallery_charts(commands, fonts),
        "gallery_pickers" => widgets::gallery_pickers(commands, fonts),
        "gallery_animation" => widgets::gallery_animation(commands, fonts),
        "gallery_audio" => widgets::gallery_audio(commands, fonts),
        "gallery_colors" => widgets::gallery_colors(commands, fonts),
        _ => {
            // Placeholder: the panel's name, centered.
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Name::new("placeholder"),
                ))
                .id();
            let text = commands
                .spawn((
                    Text::new(renzora_ember::dock::humanize(id)),
                    ui_font(&fonts.ui, 13.0),
                    TextColor(rgb(placeholder())),
                ))
                .id();
            commands.entity(container).add_child(text);
            container
        }
    }
}
