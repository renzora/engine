//! Bevy-native (ember) inspector panel.
//!
//! Registry-driven: each `InspectorRegistry` entry shows when its `has_fn`
//! matches and renders either a registered native (bevy_ui) drawer, declarative
//! `fields` (a `FieldType` + get/set fn-pointers, rendered generically here), or
//! a placeholder when it has neither.
//!
//! `rebuild_inspector` (exclusive) rebuilds sections + rows whenever the
//! selection / locked entity / component set / add-overlay changes (hashed
//! signature, so field-value edits don't trigger a rebuild — those are reactive
//! via `bind_2way`).
//!
//! Layout matches the egui inspector: component sections with a header
//! (caret · icon · title · enable toggle · trash) and field rows with a
//! right-aligned label column + boxed value, alternating row striping.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor_framework::{
    EditorCommands, EditorSelection, EditorSettings, FieldType, FieldValue, InspectorExpandDefault,
    InspectorRegistry, NativeInspectorDrawer, NativeInspectorRegistry,
};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_with};
use renzora_ember::widgets::{
    bind_text_input, drag_value, dropdown_with_icons, scroll_view, set_section_open, text_input,
    toggle_switch, DragRange, EmberTextInput, Popup, Section,
};
use renzora_theme::ThemeManager;

type GetFn = fn(&World, Entity) -> Option<FieldValue>;
type SetFn = fn(&mut World, Entity, FieldValue);
type Pred = fn(&World, Entity) -> bool;
type Mutate = fn(&mut World, Entity);
type SetEnabled = fn(&mut World, Entity, bool);


fn c(rgb: (u8, u8, u8)) -> Color {
    Color::srgb_u8(rgb.0, rgb.1, rgb.2)
}

#[derive(Component)]
struct InspectorRoot;

/// Marks the (stable, never-rebuilt) component-filter text input.
#[derive(Component)]
struct InspectorFilter;

#[derive(Resource, Default)]
struct NativeInspectorState {
    sig: Option<u64>,
    locked: Option<Entity>,
    /// Lowercased component-name filter (empty = show all).
    filter: String,
    /// Exact component display-name picked from the filter dropdown
    /// (`None` = show all components). ANDed with `filter`.
    selected: Option<String>,
}

/// Marks the inspector's expand/collapse-all button in the top bar.
#[derive(Component)]
struct ExpandAllButton;

/// Marks the glyph inside the expand/collapse-all button, so a sync system can
/// keep it showing "expand" vs "collapse" as sections open and close.
#[derive(Component)]
struct ExpandAllGlyph;

/// Marks an inspector component-section header, so the expand/collapse-all
/// button can drive just these sections (not other panels' sections).
#[derive(Component)]
struct InspectorSectionHeader;

/// Stable host for the component-filter dropdown. The ember `dropdown` widget
/// bakes its options in at build time, so `rebuild_inspector` despawns this
/// host's child and rebuilds the dropdown whenever the component set changes.
#[derive(Component)]
struct FilterDropdownHost;

/// The "show everything" entry, shown as index 0 in the filter dropdown.
const FILTER_ALL: &str = "All components";

pub fn register_native_inspector(app: &mut App) {
    use renzora_editor_framework::SplashState;
    app.init_resource::<NativeInspectorState>();
    // `scroll: false` — we manage scrolling ourselves so the top bar (Add
    // Component + filter) and the bottom Add Component button stay *fixed* while
    // only the component list scrolls.
    app.register_panel_content("inspector", false, |commands, fonts| {
        let root = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Name::new("inspector-panel"),
            ))
            .id();
        // Fixed top bar: Add Component button + filter input to its right.
        let top = build_top_bar(commands, fonts);
        // Scrolling component list (`InspectorRoot` is despawned/repopulated by
        // `rebuild_inspector`; the bars around it are stable).
        let content = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(4.0)),
                    row_gap: Val::Px(3.0),
                    ..default()
                },
                InspectorRoot,
                Name::new("inspector-root"),
            ))
            .id();
        let scroll = scroll_view(commands, content);
        // Fixed bottom Add Component button.
        let bottom = build_bottom_add(commands, fonts);
        commands.entity(root).add_children(&[top, scroll, bottom]);
        root
    });
    app.add_systems(
        Update,
        (
            remove_click,
            add_button_click,
            field_button_click,
            reset_click,
            lock_click,
            enum_option_click,
            asset_drop,
            asset_clear_click,
            asset_drop_highlight,
            inspector_filter_sync,
            expand_all_click,
            sync_expand_glyph,
        )
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
    );
    app.add_systems(
        Update,
        rebuild_inspector
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
    );
}

// ── Specs collected (under the exclusive borrow) before building ─────────────

#[derive(Clone, Copy)]
enum FieldKind {
    Float { speed: f32, min: f32, max: f32 },
    Vec3 { speed: f32 },
    Bool,
    Color,
    ColorRgba,
    Text,
    Asset,
    Enum { options: &'static [&'static str] },
    Button { icon: &'static str },
    ReadOnly,
}

enum FieldInit {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Text(String),
}

struct FieldSpec {
    name: &'static str,
    kind: FieldKind,
    get_fn: GetFn,
    set_fn: SetFn,
    init: FieldInit,
    /// Accepted extensions for `Asset` fields (empty = accept any). Unused for
    /// other kinds.
    extensions: Vec<String>,
}

struct SectionSpec {
    title: &'static str,
    icon: &'static str, // phosphor icon name (resolved via icon_glyph)
    type_id: &'static str,
    custom: bool,
    /// Native (bevy_ui) drawer, if the component registered one. Takes priority
    /// over declarative fields.
    native_drawer: Option<NativeInspectorDrawer>,
    remove_fn: Option<Mutate>,
    enable: Option<(Pred, SetEnabled)>,
    enabled_now: bool,
    /// Category-derived header background + accent (icon tint).
    header_bg: (u8, u8, u8),
    accent: (u8, u8, u8),
    /// Whether this section starts expanded (per the expand-default policy /
    /// expand-all override, computed in [`collect_sections`]).
    open: bool,
    fields: Vec<FieldSpec>,
}

/// Extract an `(r, g, b)` triple from a theme color (no egui types in scope).
fn c32(col: renzora_theme::ThemeColor) -> (u8, u8, u8) {
    let [r, g, b, _] = col.to_array();
    (r, g, b)
}

/// Replicates `renzora_ui::category_colors`: maps a component category to its
/// themed (accent, header_bg). So lights get an amber header, environment a
/// blue-grey one, etc. — not all the same.
fn category_rgb(theme: &renzora_theme::Theme, category: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
    let s = match category {
        "environment" => &theme.categories.environment,
        "light" | "lighting" => &theme.categories.lighting,
        "camera" => &theme.categories.camera,
        "script" | "scripting" => &theme.categories.scripting,
        "physics" => &theme.categories.physics,
        "plugin" => &theme.categories.plugin,
        "nodes2d" | "nodes_2d" => &theme.categories.nodes_2d,
        "ui" => &theme.categories.ui,
        "rendering" => &theme.categories.rendering,
        "effects" | "particles" => &theme.categories.effects,
        _ => &theme.categories.transform,
    };
    (c32(s.accent), c32(s.header_bg))
}

// ── Component filter ─────────────────────────────────────────────────────────

/// The entity the inspector is showing (the lock wins over the live selection).
fn inspected_entity(w: &World) -> Option<Entity> {
    let locked = w.get_resource::<NativeInspectorState>().and_then(|s| s.locked);
    locked.or_else(|| w.get_resource::<EditorSelection>().and_then(|s| s.get()))
}

/// `(display_name, icon)` for every registered component currently on `entity`,
/// in registry order — the source list for the filter dropdown (matches the set
/// of sections `collect_sections` would show with no filter).
fn present_components(world: &World, entity: Entity) -> Vec<(&'static str, &'static str)> {
    let Some(reg) = world.get_resource::<InspectorRegistry>() else {
        return Vec::new();
    };
    reg.iter()
        .filter(|e| (e.has_fn)(world, entity))
        .map(|e| (e.display_name, e.icon))
        .collect()
}

/// The fixed top bar: a component-filter dropdown on the left + the
/// component-filter text input to its right. (The Add Component button lives in
/// the bottom bar.) Hidden when nothing is selected.
fn build_top_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // A stable host for the component-filter dropdown. The dropdown itself is
    // (re)built by `rebuild_inspector` from the components currently on the
    // entity; picking one filters the inspector to it ("All components" clears
    // it).
    let dropdown_host = commands
        .spawn((
            // Hug the dropdown (which sizes to its content, capped) so a short
            // selection like "TAA" gives a short box.
            Node {
                flex_shrink: 0.0,
                ..default()
            },
            FilterDropdownHost,
            Name::new("filter-dropdown-host"),
        ))
        .id();
    let input = text_input(commands, &fonts.ui, "Filter components...", "");
    commands.entity(input).insert((
        InspectorFilter,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    // Expand / collapse-all toggle: forces every section open or closed for the
    // current view. Its glyph reflects the live state — "expand" when anything
    // could still open, "collapse" once everything is forced open.
    let expand_btn = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(26.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ExpandAllButton,
            Name::new("inspector-expand-all"),
        ))
        .id();
    let glyph = phosphor_glyph(
        commands,
        fonts,
        "arrows-out-line-vertical",
        renzora_ember::theme::text_muted(),
        15.0,
    );
    // `sync_expand_glyph` flips this between expand/collapse as sections change.
    commands.entity(glyph).insert(ExpandAllGlyph);
    commands.entity(expand_btn).add_child(glyph);

    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("inspector-top-bar"),
        ))
        .id();
    commands.entity(bar).add_children(&[dropdown_host, input, expand_btn]);
    bind_display(commands, bar, |w| inspected_entity(w).is_some());
    bar
}

/// Build the ember `dropdown` filtering the inspector by component, from the
/// `present` components (display names). Index 0 is "All components"; selecting
/// it clears the filter. Two-way bound to `NativeInspectorState::selected`.
fn build_filter_dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    present: &[(&'static str, &'static str)],
    selected: &Option<String>,
) -> Entity {
    // Options: "All components" + one per present component, each with its icon.
    let names: Vec<&str> = present.iter().map(|(n, _)| *n).collect();
    let mut options: Vec<(&str, &str)> = Vec::with_capacity(present.len() + 1);
    options.push(("list", FILTER_ALL));
    options.extend(present.iter().map(|(name, icon)| (*icon, *name)));

    let init = selected
        .as_deref()
        .and_then(|s| names.iter().position(|n| *n == s).map(|i| i + 1))
        .unwrap_or(0);

    let dd = dropdown_with_icons(commands, fonts, &options, init);
    // Size to the selected label (caps at max_width, where the label truncates),
    // instead of the widget's fixed 140px min-width.
    commands.entity(dd).insert(Node {
        max_width: Val::Px(190.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        position_type: PositionType::Relative,
        ..default()
    });

    // index ↔ selected display-name (index 0 ⇒ None).
    let names_get: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    let names_set = names_get.clone();
    bind_2way(
        commands,
        dd,
        move |w| {
            w.get_resource::<NativeInspectorState>()
                .and_then(|s| s.selected.clone())
                .and_then(|s| names_get.iter().position(|n| *n == s).map(|i| i + 1))
                .unwrap_or(0)
        },
        move |w, idx: &usize| {
            let sel = if *idx == 0 {
                None
            } else {
                names_set.get(*idx - 1).cloned()
            };
            if let Some(mut st) = w.get_resource_mut::<NativeInspectorState>() {
                if st.selected != sel {
                    st.selected = sel;
                }
            }
        },
    );
    dd
}

/// The fixed bottom bar: a full-width Add Component button. Hidden when nothing
/// is selected.
fn build_bottom_add(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = add_bar(commands, fonts);
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("inspector-bottom-bar"),
        ))
        .id();
    commands.entity(bar).add_child(btn);
    bind_display(commands, bar, |w| inspected_entity(w).is_some());
    bar
}

/// Sync the filter input's text into state (lowercased) so `collect_sections`
/// and the rebuild signature pick it up.
fn inspector_filter_sync(
    input: Query<&EmberTextInput, With<InspectorFilter>>,
    mut state: ResMut<NativeInspectorState>,
) {
    for inp in &input {
        let v = inp.value.to_lowercase();
        if state.filter != v {
            state.filter = v;
        }
    }
}

// ── Rebuild ──────────────────────────────────────────────────────────────────

fn rebuild_inspector(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // Drop a stale lock, then resolve the inspected entity (lock wins).
    {
        let locked = world.resource::<NativeInspectorState>().locked;
        if let Some(e) = locked {
            if world.get_entity(e).is_err() {
                world.resource_mut::<NativeInspectorState>().locked = None;
            }
        }
    }
    let locked = world.resource::<NativeInspectorState>().locked;
    let entity = locked.or_else(|| {
        world
            .get_resource::<EditorSelection>()
            .and_then(|s| s.get())
    });

    // Drop a stale dropdown pick if that component isn't on the current entity
    // (e.g. selection changed) so we don't strand the inspector on an empty list.
    if let Some(sel) = world.resource::<NativeInspectorState>().selected.clone() {
        let still_present = entity
            .map(|e| present_components(world, e).iter().any(|(n, _)| *n == sel))
            .unwrap_or(false);
        if !still_present {
            world.resource_mut::<NativeInspectorState>().selected = None;
        }
    }

    let mut cq = world.query_filtered::<Entity, With<InspectorRoot>>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    let sig = inspector_signature(world, container, entity, locked.is_some());
    if world.resource::<NativeInspectorState>().sig == Some(sig) {
        return;
    }

    let sections = collect_sections(world, entity);
    let state = world.resource::<NativeInspectorState>();
    let filter_active = !state.filter.is_empty() || state.selected.is_some();
    let existing: Vec<Entity> = world
        .get::<Children>(container)
        .map(|ch| ch.iter().collect())
        .unwrap_or_default();

    // Rebuild the filter dropdown from the entity's components (the ember widget
    // bakes options in at build time, so it's recreated when the set changes).
    let filter_host = {
        let mut hq = world.query_filtered::<Entity, With<FilterDropdownHost>>();
        hq.iter(world).next()
    };
    let filter_host_children: Vec<Entity> = filter_host
        .and_then(|h| world.get::<Children>(h).map(|ch| ch.iter().collect()))
        .unwrap_or_default();
    let present: Vec<(&'static str, &'static str)> =
        entity.map(|e| present_components(world, e)).unwrap_or_default();
    let selected_now = world.resource::<NativeInspectorState>().selected.clone();

    // Native-drawer sections: (body, drawer, entity) — filled after the queue
    // applies, since drawers need exclusive &mut World.
    let mut native_pending: Vec<(Entity, NativeInspectorDrawer, Entity)> = Vec::new();

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for child in existing {
            commands.entity(child).despawn();
        }

        // Rebuild the filter dropdown with the entity's current components.
        if let Some(host) = filter_host {
            for child in &filter_host_children {
                commands.entity(*child).despawn();
            }
            let dd = build_filter_dropdown(&mut commands, &fonts, &present, &selected_now);
            commands.entity(host).add_child(dd);
        }

        match entity {
            None => {
                let l = empty_label(&mut commands, &fonts, "No entity selected");
                commands.entity(container).add_child(l);
            }
            Some(entity) => {
                if sections.is_empty() {
                    let msg = if filter_active {
                        "No components match the filter."
                    } else {
                        "No inspectable components."
                    };
                    let l = empty_label(&mut commands, &fonts, msg);
                    commands.entity(container).add_child(l);
                }
                let locked_here = locked == Some(entity);
                for sec in &sections {
                    let (root, body) = build_section(&mut commands, &fonts, sec, entity, locked_here);
                    commands.entity(container).add_child(root);
                    if let Some(drawer) = sec.native_drawer {
                        native_pending.push((body, drawer, entity));
                    }
                }
            }
        }
    }
    queue.apply(world);

    // Run each native drawer (exclusive World) and parent its content under the
    // section body.
    for (body, drawer, ent) in native_pending {
        let content = drawer(world, ent);
        if let Ok(mut em) = world.get_entity_mut(body) {
            em.add_child(content);
        }
    }

    world.resource_mut::<NativeInspectorState>().sig = Some(sig);
}

fn inspector_signature(
    world: &World,
    container: Entity,
    entity: Option<Entity>,
    locked: bool,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    container.to_bits().hash(&mut h);
    locked.hash(&mut h);
    if let Some(s) = world.get_resource::<NativeInspectorState>() {
        s.filter.hash(&mut h);
        s.selected.hash(&mut h);
    }
    // Changing the default-expand policy re-applies it to the current view.
    if let Some(s) = world.get_resource::<EditorSettings>() {
        (s.inspector_expand_default as u8).hash(&mut h);
    }
    match entity {
        Some(e) => {
            1u8.hash(&mut h);
            e.to_bits().hash(&mut h);
            if let Some(reg) = world.get_resource::<InspectorRegistry>() {
                for entry in reg.iter() {
                    if (entry.has_fn)(world, e) {
                        entry.type_id.hash(&mut h);
                    }
                }
            }
        }
        None => 0u8.hash(&mut h),
    }
    h.finish()
}

fn collect_sections(world: &World, entity: Option<Entity>) -> Vec<SectionSpec> {
    let Some(entity) = entity else {
        return Vec::new();
    };
    let Some(reg) = world.get_resource::<InspectorRegistry>() else {
        return Vec::new();
    };
    let theme = world.get_resource::<ThemeManager>();
    let native_reg = world.get_resource::<NativeInspectorRegistry>();
    let (filter, selected) = world
        .get_resource::<NativeInspectorState>()
        .map(|s| (s.filter.clone(), s.selected.clone()))
        .unwrap_or_default();

    // Initial expand state per section, from the user's `inspector_expand_default`
    // policy (Essentials keeps only Name/Transform/Scripts open). After build the
    // expand/collapse-all button drives sections live (see `expand_all_click`).
    let expand_policy = world
        .get_resource::<EditorSettings>()
        .map(|s| s.inspector_expand_default)
        .unwrap_or_default();
    let section_open = |title: &str| -> bool {
        match expand_policy {
            InspectorExpandDefault::AllOpen => true,
            InspectorExpandDefault::AllClosed => false,
            InspectorExpandDefault::Essentials => {
                matches!(title, "Name" | "Transform" | "Scripts")
            }
        }
    };

    let mut out = Vec::new();
    for entry in reg.iter() {
        if !(entry.has_fn)(world, entity) {
            continue;
        }
        // Exact component pick from the dropdown (ANDed with the text filter).
        if let Some(sel) = &selected {
            if entry.display_name != sel {
                continue;
            }
        }
        // Component-name filter (case-insensitive substring on the display name).
        if !filter.is_empty() && !entry.display_name.to_lowercase().contains(&filter) {
            continue;
        }
        let (accent, header_bg) = theme
            .map(|tm| category_rgb(&tm.active_theme, entry.category))
            .unwrap_or(((120, 140, 200), (44, 44, 54)));
        let enable = match (entry.is_enabled_fn, entry.set_enabled_fn) {
            (Some(g), Some(s)) => Some((g, s)),
            _ => None,
        };
        let enabled_now = enable.map(|(g, _)| g(world, entity)).unwrap_or(true);
        // Priority: a registered native bevy_ui drawer > declarative `fields` >
        // placeholder note (component has neither a native drawer nor any fields).
        let native_drawer = native_reg.and_then(|r| r.get(entry.type_id));
        if native_drawer.is_some() {
            out.push(SectionSpec {
                title: entry.display_name,
                icon: entry.icon,
                type_id: entry.type_id,
                custom: false,
                native_drawer,
                remove_fn: entry.remove_fn,
                enable,
                enabled_now,
                header_bg,
                accent,
                open: section_open(entry.display_name),
                fields: Vec::new(),
            });
            continue;
        }
        if entry.fields.is_empty() {
            out.push(SectionSpec {
                title: entry.display_name,
                icon: entry.icon,
                type_id: entry.type_id,
                custom: true,
                native_drawer: None,
                remove_fn: entry.remove_fn,
                enable,
                enabled_now,
                header_bg,
                accent,
                open: section_open(entry.display_name),
                fields: Vec::new(),
            });
            continue;
        }
        let mut fields = Vec::new();
        for f in &entry.fields {
            let val = (f.get_fn)(world, entity);
            let (kind, init) = match (&f.field_type, &val) {
                (FieldType::Float { speed, min, max }, Some(FieldValue::Float(v))) => (
                    FieldKind::Float {
                        speed: *speed,
                        min: *min,
                        max: *max,
                    },
                    FieldInit::Float(*v),
                ),
                (FieldType::Vec3 { speed }, Some(FieldValue::Vec3(a))) => {
                    (FieldKind::Vec3 { speed: *speed }, FieldInit::Vec3(*a))
                }
                (FieldType::Bool, Some(FieldValue::Bool(b))) => {
                    (FieldKind::Bool, FieldInit::Bool(*b))
                }
                (FieldType::Color, Some(FieldValue::Color(_))) => {
                    // color_field seeds itself from the live value; no init needed.
                    (FieldKind::Color, FieldInit::Text(String::new()))
                }
                (FieldType::ColorRgba, Some(FieldValue::ColorRgba(_))) => {
                    (FieldKind::ColorRgba, FieldInit::Text(String::new()))
                }
                (FieldType::String, Some(FieldValue::String(s))) => {
                    (FieldKind::Text, FieldInit::Text(s.clone()))
                }
                (FieldType::Enum { options }, Some(FieldValue::Enum(s))) => {
                    (FieldKind::Enum { options }, FieldInit::Text(s.clone()))
                }
                (FieldType::Asset { .. }, Some(FieldValue::Asset(_))) => {
                    (FieldKind::Asset, FieldInit::Text(String::new()))
                }
                // Buttons have no value to read — match regardless of `val`.
                (FieldType::Button { icon }, _) => {
                    (FieldKind::Button { icon }, FieldInit::Text(String::new()))
                }
                _ => (FieldKind::ReadOnly, FieldInit::Text(format_value(val.as_ref()))),
            };
            let extensions = match &f.field_type {
                FieldType::Asset { extensions } => extensions.clone(),
                _ => Vec::new(),
            };
            fields.push(FieldSpec {
                name: f.name,
                kind,
                get_fn: f.get_fn,
                set_fn: f.set_fn,
                init,
                extensions,
            });
        }
        out.push(SectionSpec {
            title: entry.display_name,
            icon: entry.icon,
            type_id: entry.type_id,
            custom: false,
            native_drawer: None,
            remove_fn: entry.remove_fn,
            enable,
            enabled_now,
            header_bg,
            accent,
            open: section_open(entry.display_name),
            fields,
        });
    }

    // Pin the most-edited components to the top in a fixed order — Name,
    // Transform, then Scripts, then Material — so they're always right where you
    // expect regardless of plugin registration order. A stable sort keeps every
    // other component in its original registry order behind them.
    out.sort_by_key(|s| section_priority(s.title));
    out
}

/// Display order weight for a section: pinned components come first in a fixed
/// order; everything else shares the same (higher) weight and so keeps its
/// registry order under the stable sort in [`collect_sections`].
fn section_priority(title: &str) -> u8 {
    match title {
        "Name" => 0,
        "Transform" => 1,
        "Scripts" => 2,
        "Material" => 3,
        _ => 4,
    }
}

fn format_value(v: Option<&FieldValue>) -> String {
    match v {
        Some(FieldValue::Float(f)) => format!("{f:.3}"),
        Some(FieldValue::Vec3(a)) => format!("{:.3}, {:.3}, {:.3}", a[0], a[1], a[2]),
        Some(FieldValue::Bool(b)) => b.to_string(),
        Some(FieldValue::Color(col)) => format!(
            "#{:02X}{:02X}{:02X}",
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8
        ),
        Some(FieldValue::ColorRgba(col)) => format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            (col[0] * 255.0) as u8,
            (col[1] * 255.0) as u8,
            (col[2] * 255.0) as u8,
            (col[3] * 255.0) as u8
        ),
        Some(FieldValue::String(s)) | Some(FieldValue::ReadOnly(s)) | Some(FieldValue::Enum(s)) => {
            s.clone()
        }
        Some(FieldValue::Asset(a)) => a.clone().unwrap_or_else(|| "—".into()),
        None => "—".into(),
    }
}

// ── Section ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct RemoveBtn {
    remove_fn: Mutate,
    entity: Entity,
}

#[derive(Component)]
struct LockBtn {
    entity: Entity,
}

/// Marks a `FieldType::Button` widget so [`field_button_click`] runs its action.
#[derive(Component)]
struct FieldButton {
    set_fn: SetFn,
    entity: Entity,
}

/// Marks a per-field reset button so [`reset_click`] writes the field's default.
#[derive(Component)]
struct ResetBtn {
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
}

fn build_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    sec: &SectionSpec,
    entity: Entity,
    locked_here: bool,
) -> (Entity, Entity) {
    // Compose the shared ember section (caret · accent icon · title + colored
    // header + ember-owned collapse); override the body padding to the inspector's
    // tighter spacing and add the lock/enable/trash affordances to the header.
    let (root, header, body) = renzora_ember::widgets::section_with_header_open(
        commands,
        fonts,
        sec.icon,
        sec.title,
        sec.accent,
        sec.header_bg,
        sec.open,
    );
    commands.entity(header).insert(InspectorSectionHeader);
    commands.entity(body).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(4.0)),
        ..default()
    });
    if sec.native_drawer.is_some() {
        // Body is filled by the registered native drawer once the build queue
        // has applied (it needs exclusive &mut World). See `rebuild_inspector`.
    } else if sec.custom {
        let note = empty_label(commands, fonts, "Custom inspector — pending native UI");
        commands.entity(body).add_child(note);
    } else {
        for (i, field) in sec.fields.iter().enumerate() {
            let r = build_field_row(commands, fonts, field, entity);
            commands
                .entity(r)
                .insert(BackgroundColor(renzora_ember::inspector::inspector_stripe(i)));
            commands.entity(body).add_child(r);
        }
    }

    // Header affordances: a spacer pushes the optional lock / enable / trash to
    // the right of the title.
    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass))
        .id();
    let mut extra = vec![spacer];
    if sec.type_id == "name" {
        let lock = phosphor_glyph(
            commands,
            fonts,
            if locked_here { "lock-simple" } else { "lock-simple-open" },
            if locked_here { (120, 170, 255) } else { renzora_ember::theme::text_muted() },
            14.0,
        );
        commands
            .entity(lock)
            .insert((Interaction::default(), FocusPolicy::Block, LockBtn { entity }));
        extra.push(lock);
    }
    if let Some((_, set_enabled)) = sec.enable {
        let sw = toggle_switch(commands, sec.enabled_now);
        // Block the press from bubbling to the section header behind it, so
        // flipping the enable switch doesn't also collapse/expand the section
        // (same reason the lock/trash glyphs above set FocusPolicy::Block).
        commands.entity(sw).insert(FocusPolicy::Block);
        let g = sec.enable.unwrap().0;
        bind_2way(
            commands,
            sw,
            move |w| g(w, entity),
            move |w, v: &bool| set_enabled(w, entity, *v),
        );
        extra.push(sw);
    }
    if let Some(remove_fn) = sec.remove_fn {
        let trash = phosphor_glyph(commands, fonts, "trash", renzora_ember::theme::text_muted(), 13.0);
        commands
            .entity(trash)
            .insert((Interaction::default(), FocusPolicy::Block, RemoveBtn { remove_fn, entity }));
        extra.push(trash);
    }
    commands.entity(header).add_children(&extra);

    (root, body)
}

fn build_field_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
) -> Entity {
    // The field's control(s) sit in a value container, then the shared
    // `inspector_row` adds a left-aligned label column — so declarative fields
    // and native drawers (which also use `inspector_row`) line up identically.
    let value = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("field-value"),
        ))
        .id();
    build_field_value(commands, fonts, field, entity, value);
    // A per-field "reset to default" affordance, right of the editable widget(s).
    // Skipped for kinds that have no value to reset (action buttons, read-only
    // text) — resetting those would be meaningless.
    if field_is_resettable(field.kind) {
        let reset = build_reset_button(commands, fonts, field.get_fn, field.set_fn, entity);
        commands.entity(value).add_child(reset);
    }
    renzora_ember::inspector::inspector_row(commands, &fonts.ui, field.name, value)
}

/// Whether a field carries an editable value worth a reset button. `Button` is a
/// fire-and-forget action and `ReadOnly` can't be edited, so neither gets one.
fn field_is_resettable(kind: FieldKind) -> bool {
    !matches!(kind, FieldKind::Button { .. } | FieldKind::ReadOnly)
}

/// A small icon button that resets a field to its type-appropriate default
/// (via [`FieldValue::type_default`]). Reads the field's current value only to
/// learn its `FieldValue` variant, then writes the matching default back; the
/// field's two-way binding refreshes the widget on the next frame.
fn build_reset_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ResetBtn { get_fn, set_fn, entity },
            Name::new("field-reset"),
        ))
        .id();
    let glyph = phosphor_glyph(
        commands,
        fonts,
        "arrow-counter-clockwise",
        renzora_ember::theme::text_muted(),
        11.0,
    );
    commands.entity(btn).add_child(glyph);
    btn
}

fn build_field_value(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
    value_parent: Entity,
) {
    match field.kind {
        FieldKind::Float { speed, min, max } => {
            let init = if let FieldInit::Float(v) = field.init { v } else { 0.0 };
            let dv = drag_value(commands, &fonts.ui, "", renzora_ember::theme::value_text(), init, speed.max(0.001));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            bind_2way(
                commands,
                dv,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| set_fn(w, entity, FieldValue::Float(*v)),
            );
            commands.entity(value_parent).add_child(dv);
        }
        FieldKind::Vec3 { speed } => {
            let init = if let FieldInit::Vec3(a) = field.init {
                a
            } else {
                [0.0; 3]
            };
            const AXES: [(&str, (u8, u8, u8)); 3] = [
                ("X", (230, 90, 90)),
                ("Y", (130, 200, 90)),
                ("Z", (90, 150, 230)),
            ];
            for (i, (axis, color)) in AXES.iter().enumerate() {
                let dv = drag_value(commands, &fonts.ui, axis, *color, init[i], speed.max(0.001));
                let (get_fn, set_fn) = (field.get_fn, field.set_fn);
                bind_2way(
                    commands,
                    dv,
                    move |w| match get_fn(w, entity) {
                        Some(FieldValue::Vec3(a)) => a[i],
                        _ => 0.0,
                    },
                    move |w, v: &f32| {
                        if let Some(FieldValue::Vec3(mut a)) = get_fn(w, entity) {
                            a[i] = *v;
                            set_fn(w, entity, FieldValue::Vec3(a));
                        }
                    },
                );
                commands.entity(value_parent).add_child(dv);
            }
        }
        FieldKind::Bool => {
            let init = matches!(field.init, FieldInit::Bool(true));
            let sw = toggle_switch(commands, init);
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            bind_2way(
                commands,
                sw,
                move |w| matches!(get_fn(w, entity), Some(FieldValue::Bool(true))),
                move |w, v: &bool| set_fn(w, entity, FieldValue::Bool(*v)),
            );
            commands.entity(value_parent).add_child(sw);
        }
        FieldKind::Color => {
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            let editor = renzora_ember::inspector::color_field(
                commands,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Color(c)) => c,
                    _ => [0.0; 3],
                },
                move |w, rgb: [f32; 3]| set_fn(w, entity, FieldValue::Color(rgb)),
            );
            commands.entity(value_parent).add_child(editor);
        }
        FieldKind::ColorRgba => {
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            let editor = renzora_ember::inspector::color_field_rgba(
                commands,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::ColorRgba(c)) => c,
                    _ => [0.0; 4],
                },
                move |w, rgba: [f32; 4]| set_fn(w, entity, FieldValue::ColorRgba(rgba)),
            );
            commands.entity(value_parent).add_child(editor);
        }
        FieldKind::Text => {
            let init = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let ti = text_input(commands, &fonts.ui, "—", &init);
            let (get_fn, set_fn) = (field.get_fn, field.set_fn);
            bind_text_input(
                commands,
                ti,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::String(s)) => s,
                    _ => String::new(),
                },
                move |w, v: String| set_fn(w, entity, FieldValue::String(v)),
            );
            commands.entity(value_parent).add_child(ti);
        }
        FieldKind::Enum { options } => {
            let cur = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let dd = build_enum_dropdown(commands, fonts, entity, field.get_fn, field.set_fn, options, &cur);
            commands.entity(value_parent).add_child(dd);
        }
        FieldKind::Asset => {
            let f = build_asset_field(
                commands,
                fonts,
                entity,
                field.get_fn,
                field.set_fn,
                field.extensions.clone(),
            );
            commands.entity(value_parent).add_child(f);
        }
        FieldKind::Button { icon } => {
            let btn = renzora_ember::widgets::icon_label_button(commands, fonts, icon, field.name);
            commands.entity(btn).insert((
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                FieldButton {
                    set_fn: field.set_fn,
                    entity,
                },
            ));
            commands.entity(value_parent).add_child(btn);
        }
        FieldKind::ReadOnly => {
            let text = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let t = commands
                .spawn((
                    Text::new(text),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(c(renzora_ember::theme::text_muted())),
                ))
                .id();
            commands.entity(value_parent).add_child(t);
        }
    }
}

// ── Color editor (swatch + R/G/B popup) ──────────────────────────────────────

// ── Enum dropdown ────────────────────────────────────────────────────────────

#[derive(Component)]
struct EnumOption {
    set_fn: SetFn,
    entity: Entity,
    label: &'static str,
    panel: Entity,
}

fn build_enum_dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    get_fn: GetFn,
    set_fn: SetFn,
    options: &'static [&'static str],
    current: &str,
) -> Entity {
    // Popup of options.
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(c(renzora_ember::theme::popup_bg())),
            BorderColor::all(c(renzora_ember::theme::border())),
            GlobalZIndex(700),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("enum-panel"),
        ))
        .id();
    let mut rows = Vec::with_capacity(options.len());
    for opt in options {
        let txt = commands
            .spawn((
                Text::new(*opt),
                ui_font(&fonts.ui, 11.0),
                TextColor(c(renzora_ember::theme::value_text())),
                FocusPolicy::Pass,
            ))
            .id();
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                EnumOption {
                    set_fn,
                    entity,
                    label: opt,
                    panel,
                },
                Name::new("enum-option"),
            ))
            .id();
        commands.entity(row).add_child(txt);
        rows.push(row);
    }
    commands.entity(panel).add_children(&rows);

    // Trigger: current value + caret.
    let value_text = commands
        .spawn((
            Text::new(current),
            ui_font(&fonts.ui, 11.0),
            TextColor(c(renzora_ember::theme::value_text())),
            FocusPolicy::Pass,
        ))
        .id();
    // Keep the trigger label in sync with the live value.
    bind_with(
        commands,
        value_text,
        move |w| match get_fn(w, entity) {
            Some(FieldValue::Enum(s)) => s,
            _ => String::new(),
        },
        |w, e, s: &String| {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                if t.0 != *s {
                    t.0 = s.clone();
                }
            }
        },
    );
    let caret = phosphor_glyph(commands, fonts, "caret-down", renzora_ember::theme::text_muted(), 9.0);
    commands.entity(caret).insert(FocusPolicy::Pass);
    let trigger = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c((28, 28, 34))),
            BorderColor::all(c((70, 70, 82))),
            Interaction::default(),
            Popup::new(panel),
            Name::new("enum-trigger"),
        ))
        .id();
    commands.entity(trigger).add_children(&[value_text, caret]);

    let wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("enum-wrap"),
        ))
        .id();
    commands.entity(wrap).add_children(&[trigger, panel]);
    wrap
}

// ── Asset field (drop target from the asset browser) ─────────────────────────

#[derive(Component)]
struct AssetDropZone {
    extensions: Vec<String>,
    set_fn: SetFn,
    entity: Entity,
}

#[derive(Component)]
struct AssetClearBtn {
    set_fn: SetFn,
    entity: Entity,
}

/// `(display text, has-value)` for an asset field value (filename or prompt).
fn asset_display(v: Option<FieldValue>) -> (String, bool) {
    match v {
        Some(FieldValue::Asset(Some(p))) if !p.is_empty() => {
            let name = std::path::Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(p);
            (name, true)
        }
        _ => ("Drag asset here".to_string(), false),
    }
}

/// Reusable native asset-drop field (drag from the asset browser + clear button),
/// for component drawers outside this crate. The drop / clear / highlight systems
/// registered by `register_native_inspector` drive any `AssetDropZone`, so callers
/// only supply get/set fn-pointers (using `FieldValue::Asset`) and the accepted
/// extensions. Returns the row entity to place inside an `inspector_row`.
pub fn asset_drop_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    get_fn: fn(&World, Entity) -> Option<FieldValue>,
    set_fn: fn(&mut World, Entity, FieldValue),
    extensions: Vec<String>,
) -> Entity {
    build_asset_field(commands, fonts, entity, get_fn, set_fn, extensions)
}

fn build_asset_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    get_fn: GetFn,
    set_fn: SetFn,
    extensions: Vec<String>,
) -> Entity {
    let path_text = commands
        .spawn((
            Text::new("Drag asset here"),
            ui_font(&fonts.ui, 11.0),
            TextColor(c(renzora_ember::theme::text_muted())),
            bevy::text::TextLayout::no_wrap(),
            FocusPolicy::Pass,
        ))
        .id();
    bind_with(
        commands,
        path_text,
        move |w| asset_display(get_fn(w, entity)),
        |w, e, (text, has): &(String, bool)| {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                if t.0 != *text {
                    t.0 = text.clone();
                }
            }
            if let Some(mut col) = w.get_mut::<TextColor>(e) {
                col.0 = c(if *has { (210, 210, 220) } else { (140, 140, 152) });
            }
        },
    );
    let drop_box = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(c((28, 28, 34))),
            BorderColor::all(c((70, 70, 82))),
            bevy::ui::RelativeCursorPosition::default(),
            AssetDropZone {
                extensions,
                set_fn,
                entity,
            },
            Name::new("asset-drop"),
        ))
        .id();
    commands.entity(drop_box).add_child(path_text);

    let clear = commands
        .spawn((
            Text::new("\u{2715}"), // ✕
            ui_font(&fonts.ui, 11.0),
            TextColor(c(renzora_ember::theme::text_muted())),
            Node {
                padding: UiRect::horizontal(Val::Px(2.0)),
                ..default()
            },
            Interaction::default(),
            AssetClearBtn { set_fn, entity },
            Name::new("asset-clear"),
        ))
        .id();

    let row = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("asset-field"),
        ))
        .id();
    commands.entity(row).add_children(&[drop_box, clear]);
    row
}

/// Drop an asset (dragged from the asset browser) onto the hovered, extension-
/// matching field → set its project-relative path.
fn asset_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    zones: Query<(&bevy::ui::RelativeCursorPosition, &AssetDropZone)>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(cmds)) = (payload, cmds) else {
        return;
    };
    if !payload.is_detached {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let ext_refs: Vec<&str> = zone.extensions.iter().map(|s| s.as_str()).collect();
        if !ext_refs.is_empty() && !payload.matches_extensions(&ext_refs) {
            continue;
        }
        let path_str = project
            .as_ref()
            .map(|p| p.make_asset_relative(&payload.path))
            .unwrap_or_else(|| payload.path.to_string_lossy().to_string());
        let (set_fn, entity) = (zone.set_fn, zone.entity);
        cmds.push(move |w: &mut World| {
            set_fn(w, entity, FieldValue::Asset(Some(path_str.clone())))
        });
        break;
    }
}

fn asset_clear_click(
    q: Query<(&Interaction, &AssetClearBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (set_fn, entity) = (btn.set_fn, btn.entity);
        cmds.push(move |w: &mut World| set_fn(w, entity, FieldValue::Asset(None)));
    }
}

/// Highlight a drop zone's border while a compatible asset is dragged over it.
fn asset_drop_highlight(
    payload: Option<Res<renzora_ui::AssetDragPayload>>,
    theme: Option<Res<ThemeManager>>,
    mut zones: Query<(&bevy::ui::RelativeCursorPosition, &AssetDropZone, &mut BorderColor)>,
) {
    let accent = theme
        .map(|t| c(c32(t.active_theme.semantic.accent)))
        .unwrap_or(c((120, 140, 200)));
    for (rcp, zone, mut bc) in &mut zones {
        let active = payload.as_ref().is_some_and(|p| {
            let ext_refs: Vec<&str> = zone.extensions.iter().map(|s| s.as_str()).collect();
            p.is_detached
                && rcp.cursor_over
                && (ext_refs.is_empty() || p.matches_extensions(&ext_refs))
        });
        let want = BorderColor::all(if active { accent } else { c((70, 70, 82)) });
        if *bc != want {
            *bc = want;
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A Phosphor icon by *name* (resolved via ember's map).
fn phosphor_glyph(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    let e = renzora_ember::font::icon_text(commands, &fonts.phosphor, name, color, size);
    commands.entity(e).insert(FocusPolicy::Pass);
    e
}

fn empty_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 12.0),
            TextColor(c(renzora_ember::theme::text_muted())),
            Node {
                margin: UiRect::all(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}

// ── Add Component bar + overlay ──────────────────────────────────────────────

#[derive(Component)]
struct AddButton;

fn add_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // A themed ember button (Styled(Role::Button)) — picks up Theme.button +
    // hover/press states, and is editable under "Button" in the Theme editor.
    let btn = renzora_ember::widgets::icon_label_button(commands, fonts, "puzzle-piece", "Add Component");
    commands.entity(btn).insert((
        AddButton,
        // Full-width + centered; the theme fills padding/radius/colors.
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(5.0),
            ..default()
        },
        Name::new("add-component"),
    ));
    btn
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn remove_click(
    q: Query<(&Interaction, &RemoveBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (remove_fn, entity) = (btn.remove_fn, btn.entity);
        cmds.push(move |w: &mut World| remove_fn(w, entity));
    }
}

fn lock_click(
    q: Query<(&Interaction, &LockBtn), Changed<Interaction>>,
    mut state: ResMut<NativeInspectorState>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        state.locked = if state.locked == Some(btn.entity) {
            None
        } else {
            Some(btn.entity)
        };
    }
}

/// Expand/collapse-all button: drives the live section headers directly (no
/// rebuild, so it's instant and can't flicker). Smart toggle — if *any* section
/// is collapsed, open them all; otherwise collapse them all.
fn expand_all_click(
    q: Query<&Interaction, (With<ExpandAllButton>, Changed<Interaction>)>,
    mut sections: Query<&mut Section, With<InspectorSectionHeader>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let target_open = sections.iter().any(|s| !s.is_open());
    for mut sec in &mut sections {
        if sec.is_open() != target_open {
            set_section_open(&mut sec, target_open, &mut nodes, &mut texts);
        }
    }
}

/// Keep the expand-all button's glyph reflecting the current state: a "collapse"
/// icon once every section is open, an "expand" icon otherwise.
fn sync_expand_glyph(
    sections: Query<&Section, With<InspectorSectionHeader>>,
    mut glyph: Query<&mut Text, With<ExpandAllGlyph>>,
) {
    // No sections (nothing selected) → leave it on the default "expand" glyph.
    let all_open = !sections.is_empty() && sections.iter().all(|s| s.is_open());
    let name = if all_open {
        "arrows-in-line-vertical"
    } else {
        "arrows-out-line-vertical"
    };
    let Some(g) = renzora_ember::font::icon_glyph(name) else {
        return;
    };
    let g = g.to_string();
    for mut t in &mut glyph {
        if t.0 != g {
            t.0 = g.clone();
        }
    }
}

fn add_button_click(
    q: Query<&Interaction, (With<AddButton>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        cmds.push(open_add_component);
    }
}

/// Run a `FieldType::Button`'s action when its widget is pressed. The set_fn is
/// invoked with `FieldValue::Bool(true)` as the "pressed" signal.
fn field_button_click(
    q: Query<(&Interaction, &FieldButton), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (set_fn, entity) = (btn.set_fn, btn.entity);
        cmds.push(move |w: &mut World| set_fn(w, entity, FieldValue::Bool(true)));
    }
}

/// Reset a field to its default when its reset button is pressed. We read the
/// current value first only to recover the `FieldValue` variant, then write the
/// matching `type_default()` back through the field's own `set_fn`.
fn reset_click(
    q: Query<(&Interaction, &ResetBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (get_fn, set_fn, entity) = (btn.get_fn, btn.set_fn, btn.entity);
        cmds.push(move |w: &mut World| {
            if let Some(cur) = get_fn(w, entity) {
                set_fn(w, entity, cur.type_default());
            }
        });
    }
}

/// Open the shared ember search overlay listing every addable component that the
/// inspected entity doesn't already have.
fn open_add_component(world: &mut World) {
    let entity = {
        let st = world.resource::<NativeInspectorState>();
        st.locked
            .or_else(|| world.get_resource::<EditorSelection>().and_then(|s| s.get()))
    };
    let Some(entity) = entity else {
        return;
    };
    // Snapshot the registry (copying fn ptrs + &'static metadata) so the
    // has_fn / overlay build don't alias the registry borrow.
    type Spec = (
        &'static str,
        &'static str,
        &'static str,
        fn(&World, Entity) -> bool,
        fn(&mut World, Entity),
    );
    let specs: Vec<Spec> = world
        .get_resource::<renzora_editor_framework::InspectorRegistry>()
        .map(|reg| {
            reg.iter()
                .filter_map(|e| {
                    e.add_fn
                        .map(|af| (e.display_name, e.icon, e.category, e.has_fn, af))
                })
                .collect()
        })
        .unwrap_or_default();

    // Per-camera effects only render on a `Camera3d`: the curated `"camera"`
    // image-quality set (tonemapping, exposure, bloom, DOF, AA, …) and the open
    // `"post_process"` shader effects (rain, glitch, CRT, … — they carry an
    // `extract_component_filter(With<Camera3d>)`). Offer them only when a camera
    // is selected, so they don't show on a cube where they'd silently do nothing.
    let is_camera = world.get::<Camera3d>(entity).is_some();

    let mut entries: Vec<renzora_ember::widgets::SearchEntry> = Vec::new();
    for (label, icon, category, has_fn, add_fn) in specs {
        if has_fn(world, entity) {
            continue; // already present
        }
        if matches!(category, "camera" | "post_process") && !is_camera {
            continue; // per-camera effect on a non-camera entity
        }
        entries.push(renzora_ember::widgets::SearchEntry::new(
            icon,
            label,
            category,
            move |w: &mut World| add_fn(w, entity),
        ));
    }

    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        renzora_ember::widgets::search_overlay(&mut commands, &fonts, "Add Component", entries);
    }
    queue.apply(world);
}

// Open/close is handled by ember's generic `Popup` (toggle + click-outside
// dismiss); this only applies the selection + closes the popup.
fn enum_option_click(
    q: Query<(&Interaction, &EnumOption), Changed<Interaction>>,
    mut popups: Query<&mut Popup>,
    mut nodes: Query<&mut Node>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, opt) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (set_fn, entity, label) = (opt.set_fn, opt.entity, opt.label.to_string());
        cmds.push(move |w: &mut World| set_fn(w, entity, FieldValue::Enum(label.clone())));
        // Close the popup whose panel this option belongs to.
        for mut p in &mut popups {
            if p.panel == opt.panel {
                p.open = false;
            }
        }
        if let Ok(mut n) = nodes.get_mut(opt.panel) {
            n.display = Display::None;
        }
    }
}
