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
    EditorCommands, EditorSelection, EditorSettings, FieldType, FieldValue,
    InspectorComponentFilterStyle, InspectorExpandDefault, InspectorRegistry, NativeInspectorDrawer,
    NativeInspectorRegistry,
};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, bind_with};
use renzora_ember::widgets::{
    bind_text_input, drag_value, dropdown, dropdown_with_icons, scroll_view, set_section_open,
    text_input, toggle_switch, DragRange, EmberTextInput, Popup, Section,
};
use renzora_theme::ThemeManager;

type GetFn = fn(&World, Entity) -> Option<FieldValue>;
type SetFn = fn(&mut World, Entity, FieldValue);
type Pred = fn(&World, Entity) -> bool;
type Mutate = fn(&mut World, Entity);
type SetEnabled = fn(&mut World, Entity, bool);

/// Apply a field edit through the undo system instead of calling `set_fn`
/// directly, so every inspector edit is undoable. Captures the pre-edit value
/// via `get_fn` (state still holds it at this instant) and records a
/// [`renzora_undo::FieldChangeCmd`] on whatever stack is currently active
/// ([`renzora_undo::active_context`]) — the focused document's, usually `Scene`.
///
/// Consecutive edits of the *same* field merge into one step (see
/// `FieldChangeCmd::merge`), so a drag-scrub that fires this every frame is a
/// single undo entry; `renzora_undo`'s gesture seal splits separate gestures.
fn record_field_change(
    w: &mut World,
    entity: Entity,
    name: &'static str,
    get_fn: GetFn,
    set_fn: SetFn,
    new: FieldValue,
) {
    let old = get_fn(w, entity).unwrap_or_else(|| new.clone());
    let ctx = renzora_undo::active_context(w);
    renzora_undo::execute(
        w,
        ctx,
        Box::new(renzora_undo::FieldChangeCmd {
            entity,
            field_name: name,
            old,
            new,
            set_fn,
        }),
    );
}


fn c(rgb: (u8, u8, u8)) -> Color {
    Color::srgb_u8(rgb.0, rgb.1, rgb.2)
}

// ── Component add / remove / enable undo commands ─────────────────────────────

/// Undo for enabling/disabling a component from its section header toggle.
struct EnableToggleCmd {
    entity: Entity,
    set_enabled: SetEnabled,
    target: bool,
}

impl renzora_undo::UndoCommand for EnableToggleCmd {
    fn label(&self) -> &str {
        "Toggle component"
    }
    fn execute(&mut self, world: &mut World) {
        (self.set_enabled)(world, self.entity, self.target);
    }
    fn undo(&mut self, world: &mut World) {
        (self.set_enabled)(world, self.entity, !self.target);
    }
}

/// Undo for adding a component: `undo` removes it again (redo re-adds a default,
/// same as the original add).
struct AddComponentCmd {
    entity: Entity,
    add_fn: Mutate,
    remove_fn: Option<Mutate>,
}

impl renzora_undo::UndoCommand for AddComponentCmd {
    fn label(&self) -> &str {
        "Add component"
    }
    fn execute(&mut self, world: &mut World) {
        (self.add_fn)(world, self.entity);
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(remove_fn) = self.remove_fn {
            remove_fn(world, self.entity);
        }
    }
}

/// Undo for removing a component: captures the component's reflected value before
/// removing, so `undo` restores it *with its edited fields* (not a default). Redo
/// (`execute`) re-captures the current value and removes again.
struct RemoveComponentCmd {
    entity: Entity,
    type_id: &'static str,
    remove_fn: Mutate,
    captured: Option<Box<dyn bevy::reflect::Reflect>>,
}

impl renzora_undo::UndoCommand for RemoveComponentCmd {
    fn label(&self) -> &str {
        "Remove component"
    }
    fn execute(&mut self, world: &mut World) {
        self.captured = renzora::core::reflection::capture_component(world, self.entity, self.type_id);
        (self.remove_fn)(world, self.entity);
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(value) = &self.captured {
            renzora::core::reflection::insert_component_reflected(
                world,
                self.entity,
                self.type_id,
                value.as_ref(),
            );
        }
    }
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
    /// Exact component display-name picked from the left component menu
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
/// button can drive just these sections (not other panels' sections). Carries
/// the section's list position and its category header colour so
/// [`stripe_collapsed_headers`] can zebra-stripe collapsed headers and restore
/// the category colour when open.
#[derive(Component)]
struct InspectorSectionHeader {
    index: usize,
    header_bg: (u8, u8, u8),
}

/// Stable host for the vertical component menu down the left of the inspector.
/// `rebuild_inspector` despawns this host's children and rebuilds one icon button
/// per component (plus an "All" entry) whenever the component set changes.
#[derive(Component)]
struct ComponentMenuHost;

/// A single button in the left-side component menu. `name` is the component's
/// display name to filter to, or `None` for the "All components" entry.
#[derive(Component)]
struct ComponentMenuButton {
    name: Option<String>,
}

/// Stable host for the top-bar component-filter dropdown (the alternative to the
/// vertical menu, chosen via `inspector_component_filter_style`). Rebuilt in
/// place by `rebuild_inspector`; shown only in `Dropdown` mode.
#[derive(Component)]
struct FilterDropdownHost;

/// The user's chosen component-filter presentation (defaults to the vertical
/// menu if settings aren't available yet).
fn filter_style(world: &World) -> InspectorComponentFilterStyle {
    world
        .get_resource::<EditorSettings>()
        .map(|s| s.inspector_component_filter_style)
        .unwrap_or_default()
}

pub fn register_native_inspector(app: &mut App) {
    use renzora_editor_framework::SplashState;
    app.init_resource::<NativeInspectorState>();
    // Bridge to the timeline editor's per-property keyframe buttons.
    // `init_resource` is idempotent — the timeline editor inits these too, so
    // they exist whichever crate loads first (and stay default when it's absent).
    app.init_resource::<renzora::ActiveTimeline>();
    app.init_resource::<renzora::KeyframeRequests>();
    // `scroll: false` — we manage scrolling ourselves so the top bar (filter
    // input + expand-all) and the Add Component row beneath it stay *fixed* while
    // only the component list scrolls.
    app.register_panel_content("inspector", false, |commands, fonts| {
        // Outer row: the vertical component menu down the left + the main column
        // (top bar, add row, scrolling list) on the right.
        let root = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Row,
                    ..default()
                },
                Name::new("inspector-panel"),
            ))
            .id();
        // Left rail: vertical component menu (rebuilt by `rebuild_inspector`).
        let menu = build_component_menu_host(commands);
        // Fixed top bar: component-filter input + expand-all.
        let top = build_top_bar(commands, fonts);
        // Fixed Add Component row, pinned directly under the top bar.
        let add_row = build_add_row(commands, fonts);
        // Scrolling component list (`InspectorRoot` is despawned/repopulated by
        // `rebuild_inspector`; the bars around it are stable).
        let content = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(4.0)),
                    // No gap — component sections stack flush against each other
                    // (each section root's own margin is also zeroed in
                    // `build_section`); collapsed headers read as one tight list.
                    row_gap: Val::Px(0.0),
                    ..default()
                },
                InspectorRoot,
                Name::new("inspector-root"),
            ))
            .id();
        let scroll = scroll_view(commands, content);
        let main = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Name::new("inspector-main"),
            ))
            .id();
        commands.entity(main).add_children(&[top, add_row, scroll]);
        commands.entity(root).add_children(&[menu, main]);
        root
    });
    app.add_systems(
        Update,
        (
            remove_click,
            add_button_click,
            field_button_click,
            reset_click,
            add_keyframe_click,
            lock_click,
            enum_option_click,
            asset_drop,
            asset_clear_click,
            asset_drop_highlight,
            inspector_filter_sync,
            component_menu_click,
            expand_all_click,
            sync_expand_glyph,
            stripe_collapsed_headers,
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
    /// Whole-number drag field: the widget's model snaps to integers
    /// (`DragSnap`), matching a `set_fn` that rounds into an int component
    /// field — see `FieldType::Int`.
    Int { min: f32, max: f32 },
    Vec3 { speed: f32 },
    Bool,
    Color,
    ColorRgba,
    Text,
    Asset,
    Enum { options: &'static [&'static str] },
    /// Dynamic dropdown; options + selected index live in [`FieldInit::DynEnum`]
    /// (so this stays `Copy`). Value is the selected index (`FieldValue::Float`).
    DynamicEnum,
    Button { icon: &'static str },
    ReadOnly,
}

enum FieldInit {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Text(String),
    /// Dynamic-dropdown options (computed from the world) + the selected index.
    DynEnum(Vec<String>, usize),
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

/// The fixed top bar: the component-filter dropdown (shown only in `Dropdown`
/// mode) + the component-filter text input + the expand/collapse-all toggle. (In
/// `VerticalMenu` mode the component menu lives in the left rail; the Add
/// Component button is in the row directly below.) Hidden when nothing is selected.
fn build_top_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Stable host for the dropdown; populated by `rebuild_inspector` and shown
    // only when the user picked the `Dropdown` filter style.
    let dropdown_host = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                ..default()
            },
            FilterDropdownHost,
            Name::new("filter-dropdown-host"),
        ))
        .id();
    bind_display(commands, dropdown_host, |w| {
        inspected_entity(w).is_some()
            && filter_style(w) == InspectorComponentFilterStyle::Dropdown
    });
    let input = text_input(commands, &fonts.ui, &renzora::lang::t("inspector.filter_placeholder"), "");
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

/// Stable host for the left-side vertical component menu. `rebuild_inspector`
/// repopulates it with one icon button per component whenever the set changes.
/// Hidden when nothing is selected.
fn build_component_menu_host(commands: &mut Commands) -> Entity {
    let host = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c(renzora_ember::theme::window_bg())),
            ComponentMenuHost,
            Name::new("inspector-component-menu"),
        ))
        .id();
    bind_display(commands, host, |w| {
        inspected_entity(w).is_some()
            && filter_style(w) == InspectorComponentFilterStyle::VerticalMenu
    });
    host
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
    let filter_all = renzora::lang::t("inspector.filter_all");
    let names: Vec<&str> = present.iter().map(|(n, _)| *n).collect();
    let mut options: Vec<(&str, &str)> = Vec::with_capacity(present.len() + 1);
    options.push(("list", filter_all.as_str()));
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

/// Build the vertical component menu's buttons: an "All" entry (clears the
/// filter) followed by one icon button per present component. Clicking a button
/// filters the inspector to that component; the active one is highlighted. Each
/// carries `ComponentMenuButton` so `component_menu_click` can toggle the filter.
fn build_component_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    present: &[(&'static str, &'static str)],
    selected: &Option<String>,
) -> Vec<Entity> {
    let mut out = Vec::with_capacity(present.len() + 1);
    // "All" first — active when no specific component is selected.
    out.push(component_menu_button(
        commands,
        fonts,
        "list",
        None,
        selected.is_none(),
    ));
    for (name, icon) in present {
        let active = selected.as_deref() == Some(*name);
        out.push(component_menu_button(
            commands,
            fonts,
            icon,
            Some((*name).to_string()),
            active,
        ));
    }
    out
}

/// One icon button in the left component menu. The rail is icon-only, so each
/// button carries a [`HoverTooltip`] naming its component — the shared global
/// bubble can't be clipped by the rail/panel the way the old per-button
/// bubble children were.
fn component_menu_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    name: Option<String>,
    active: bool,
) -> Entity {
    let (bg, glyph_color) = if active {
        (renzora_ember::theme::accent(), renzora_ember::theme::on_accent())
    } else {
        // Inactive buttons blend into the (darker) rail so only the active one reads.
        (renzora_ember::theme::window_bg(), renzora_ember::theme::text_muted())
    };
    let label = name.clone().unwrap_or_else(|| renzora::lang::t("inspector.filter_all"));
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(c(bg)),
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(label),
            ComponentMenuButton { name },
            Name::new("component-menu-button"),
        ))
        .id();
    let glyph = phosphor_glyph(commands, fonts, icon, glyph_color, 15.0);
    commands.entity(btn).add_child(glyph);
    btn
}

/// The fixed Add Component row, pinned under the top bar: a full-width Add
/// Component button. Hidden when nothing is selected.
fn build_add_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = add_bar(commands, fonts);
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("inspector-add-row"),
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

/// Click a left-menu button to filter the inspector to that component. Clicking
/// the already-active button (or "All") clears the filter. Mirrors what the old
/// filter dropdown did into `NativeInspectorState::selected`; `rebuild_inspector`
/// then rebuilds the menu so the highlight follows.
fn component_menu_click(
    q: Query<(&Interaction, &ComponentMenuButton), Changed<Interaction>>,
    mut state: ResMut<NativeInspectorState>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Toggle: re-clicking the active component clears back to "All".
        let next = if state.selected == btn.name {
            None
        } else {
            btn.name.clone()
        };
        if state.selected != next {
            state.selected = next;
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

    // Drop a stale menu pick if that component isn't on the current entity
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

    // The component filter renders as either a left rail of icon buttons or a
    // top-bar dropdown; we rebuild whichever the user picked and clear the other.
    let style = filter_style(world);
    let menu_host = {
        let mut hq = world.query_filtered::<Entity, With<ComponentMenuHost>>();
        hq.iter(world).next()
    };
    let menu_host_children: Vec<Entity> = menu_host
        .and_then(|h| world.get::<Children>(h).map(|ch| ch.iter().collect()))
        .unwrap_or_default();
    let dropdown_host = {
        let mut hq = world.query_filtered::<Entity, With<FilterDropdownHost>>();
        hq.iter(world).next()
    };
    let dropdown_host_children: Vec<Entity> = dropdown_host
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

        // Clear both filter hosts, then rebuild only the active style's widget.
        // (The inactive host stays empty and is hidden by its `bind_display`.)
        for child in &menu_host_children {
            commands.entity(*child).despawn();
        }
        for child in &dropdown_host_children {
            commands.entity(*child).despawn();
        }
        match style {
            InspectorComponentFilterStyle::VerticalMenu => {
                if let Some(host) = menu_host {
                    let buttons =
                        build_component_menu(&mut commands, &fonts, &present, &selected_now);
                    commands.entity(host).add_children(&buttons);
                }
            }
            InspectorComponentFilterStyle::Dropdown => {
                if let Some(host) = dropdown_host {
                    let dd =
                        build_filter_dropdown(&mut commands, &fonts, &present, &selected_now);
                    commands.entity(host).add_child(dd);
                }
            }
        }

        match entity {
            None => {
                let l = empty_label(&mut commands, &fonts, &renzora::lang::t("inspector.no_selection"));
                commands.entity(container).add_child(l);
            }
            Some(entity) => {
                if sections.is_empty() {
                    let msg = if filter_active {
                        renzora::lang::t("inspector.no_match")
                    } else {
                        renzora::lang::t("inspector.no_components")
                    };
                    let l = empty_label(&mut commands, &fonts, &msg);
                    commands.entity(container).add_child(l);
                }
                let locked_here = locked == Some(entity);
                for (i, sec) in sections.iter().enumerate() {
                    let (root, body) =
                        build_section(&mut commands, &fonts, sec, entity, locked_here, i);
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
    // Changing the default-expand policy re-applies it to the current view, and
    // switching the filter style swaps the rail/dropdown — both force a rebuild.
    if let Some(s) = world.get_resource::<EditorSettings>() {
        (s.inspector_expand_default as u8).hash(&mut h);
        (s.inspector_component_filter_style as u8).hash(&mut h);
    }
    match entity {
        Some(e) => {
            1u8.hash(&mut h);
            e.to_bits().hash(&mut h);
            if let Some(reg) = world.get_resource::<InspectorRegistry>() {
                for entry in reg.iter() {
                    if (entry.has_fn)(world, e) {
                        entry.type_id.hash(&mut h);
                        // Presence-toggled sections (their enable switch
                        // inserts/removes the underlying component, e.g. 2D
                        // Lighting on a camera) change their rows without
                        // changing the section set — fold the enabled bit in
                        // so flipping the switch rebuilds the body.
                        if let Some(is_enabled) = entry.is_enabled_fn {
                            is_enabled(world, e).hash(&mut h);
                        }
                        // A `DynamicEnum` field's options are computed from the
                        // world at build time, so a *mutation* that grows/shrinks
                        // the list (e.g. appending a sprite sheet) wouldn't
                        // otherwise change the signature — leaving a stale option
                        // list and an out-of-range selection (blank dropdown).
                        // Fold the options in so the list rebuilds when it changes.
                        for field in &entry.fields {
                            if let FieldType::DynamicEnum { options } = field.field_type {
                                for opt in options(world, e) {
                                    opt.hash(&mut h);
                                }
                            }
                        }
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
            // A `None` read means "row not applicable right now" — the section's
            // component is toggled off, or the field only applies to some
            // states (e.g. occluder Width/Height on a polygon shape). Hide the
            // row rather than falling through to a junk ReadOnly. Buttons are
            // the exception: they have no value to read by design.
            if val.is_none() && !matches!(f.field_type, FieldType::Button { .. }) {
                continue;
            }
            let (kind, init) = match (&f.field_type, &val) {
                (FieldType::Float { speed, min, max }, Some(FieldValue::Float(v))) => (
                    FieldKind::Float {
                        speed: *speed,
                        min: *min,
                        max: *max,
                    },
                    FieldInit::Float(*v),
                ),
                (FieldType::Int { min, max }, Some(FieldValue::Float(v))) => (
                    FieldKind::Int { min: *min, max: *max },
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
                // Options are computed from the world here (mapping has `world`);
                // stored in the init so `FieldKind` stays `Copy`.
                (FieldType::DynamicEnum { options }, Some(FieldValue::Float(v))) => (
                    FieldKind::DynamicEnum,
                    FieldInit::DynEnum(options(world, entity), v.round().max(0.0) as usize),
                ),
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

/// Lowercase, collapsing each run of non-alphanumerics to one `_`, for deriving a
/// stable localization-key segment from a human label
/// ("Wind Direction" → `wind_direction`). The reflection-driven component and
/// field labels have no literal in source to translate, so we key off this.
fn loc_slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_us = false;
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_us = false;
        } else if !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Localized component header name, falling back to the English `display_name`.
/// Keyed `comp.<slug>.name` (e.g. "Clouds" → `comp.clouds.name`).
fn comp_name_loc(display_name: &str) -> String {
    renzora::lang::t_or(&format!("comp.{}.name", loc_slug(display_name)), display_name)
}

/// Localized field label, falling back to the English `name`. Keyed in a SHARED
/// `field.<slug>` namespace (e.g. "Wind Direction" → `field.wind_direction`) so a
/// field name common to many components is translated once, not per component.
fn field_label_loc(name: &str) -> String {
    renzora::lang::t_or(&format!("field.{}", loc_slug(name)), name)
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
    /// Component type id (short name), so undo can reflect-restore the removed
    /// component's captured value.
    type_id: &'static str,
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
    field_name: &'static str,
}

/// Marks a per-field "add keyframe" button. Carries the reflection path the
/// timeline editor matches against the open clip's tracks (see
/// [`add_keyframe_click`]).
#[derive(Component)]
struct AddKeyframeBtn {
    entity: Entity,
    component: String,
    field: String,
}

fn build_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    sec: &SectionSpec,
    entity: Entity,
    locked_here: bool,
    index: usize,
) -> (Entity, Entity) {
    // Compose the shared ember section (caret · accent icon · title + colored
    // header + ember-owned collapse); override the body padding to the inspector's
    // tighter spacing and add the lock/enable/trash affordances to the header.
    // `sec.title` stays the English identity (sort priority, collapse-state key);
    // localize only the displayed string.
    let sec_title = comp_name_loc(sec.title);
    let (root, header, body) = renzora_ember::widgets::section_with_header_open(
        commands,
        fonts,
        sec.icon,
        &sec_title,
        sec.accent,
        sec.header_bg,
        sec.open,
    );
    commands.entity(header).insert(InspectorSectionHeader {
        index,
        header_bg: sec.header_bg,
    });
    // Compact the shared section for the inspector: kill the widget's 8px
    // bottom margin + header↔body gap so component cards stack flush, and
    // tighten the header's vertical padding. (Full `Node` overrides — mirror
    // the widget's other layout fields when changing them.)
    commands.entity(root).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        ..default()
    });
    commands.entity(header).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    commands.entity(body).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(4.0)),
        // Preserve the collapsed state `section_with_header_open` encoded in the
        // body's `display`; a bare `Node` would default to `Flex` and show a
        // start-collapsed section, desyncing it from its `Section.open` flag (the
        // first collapse click would then no-op).
        display: if sec.open { Display::Flex } else { Display::None },
        ..default()
    });
    if sec.native_drawer.is_some() {
        // Body is filled by the registered native drawer once the build queue
        // has applied (it needs exclusive &mut World). See `rebuild_inspector`.
    } else if sec.custom {
        let note = empty_label(commands, fonts, &renzora::lang::t("inspector.custom_pending"));
        commands.entity(body).add_child(note);
    } else {
        for (i, field) in sec.fields.iter().enumerate() {
            let r = build_field_row(commands, fonts, field, entity, sec.type_id);
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
            move |w, v: &bool| {
                let target = *v;
                let ctx = renzora_undo::active_context(w);
                renzora_undo::execute(
                    w,
                    ctx,
                    Box::new(EnableToggleCmd { entity, set_enabled, target }),
                );
            },
        );
        extra.push(sw);
    }
    // Scripts and Material hide the header trash: both manage their own
    // contents (per-script remove; the material drawer's own binding controls),
    // so a whole-component delete here is a one-click data-loss hazard. Their
    // registry `remove_fn` stays — it's also the undo half of Add Component.
    let hide_trash = matches!(sec.type_id, "script_component" | "material_ref");
    if let (Some(remove_fn), false) = (sec.remove_fn, hide_trash) {
        let trash = phosphor_glyph(commands, fonts, "trash", renzora_ember::theme::text_muted(), 13.0);
        commands.entity(trash).insert((
            Interaction::default(),
            FocusPolicy::Block,
            RemoveBtn {
                remove_fn,
                entity,
                type_id: sec.type_id,
            },
        ));
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
    type_id: &'static str,
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
    // Growable controls (drag values, text inputs, dropdowns, asset slots) are
    // stretched by `fill_control` inside `build_field_value`, which pushes the
    // trailing keyframe/reset buttons to the row's right edge. Controls with an
    // intrinsic size (toggle, color swatch, read-only text) can't stretch, so a
    // spacer absorbs the free width instead — the buttons stay pinned right at
    // a fixed size either way, however the panel is resized.
    if matches!(
        field.kind,
        FieldKind::Bool | FieldKind::Color | FieldKind::ColorRgba | FieldKind::ReadOnly
    ) {
        let spacer = commands
            .spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass))
            .id();
        commands.entity(value).add_child(spacer);
    }
    // A per-field "add keyframe" affordance, left of the reset button. Reactively
    // hidden unless the timeline has a clip open with a bound track for this
    // property (see `build_add_keyframe_button`); pressing it keys the live value.
    if let Some((component, field_path)) = field_anim_path(type_id, field.name, field.kind) {
        let kf = build_add_keyframe_button(commands, fonts, entity, component, field_path);
        commands.entity(value).add_child(kf);
    }
    // A per-field "reset to default" affordance, right of the editable widget(s).
    // Skipped for kinds that have no value to reset (action buttons, read-only
    // text) — resetting those would be meaningless.
    if field_is_resettable(field.kind) {
        let reset = build_reset_button(commands, fonts, field.name, field.get_fn, field.set_fn, entity);
        commands.entity(value).add_child(reset);
    }
    let label = field_label_loc(field.name);
    renzora_ember::inspector::inspector_row(commands, &fonts.ui, &label, value)
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
    field_name: &'static str,
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
            ResetBtn { get_fn, set_fn, entity, field_name },
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

/// Guess the `(component, field)` reflection path an inspector row animates, for
/// matching against the open clip's property tracks. `type_id` is already the
/// reflected component short-name; the field path is the display name reversed to
/// snake_case (the `Inspectable` derive title-cases the field ident, so
/// lowercasing + underscoring recovers it) — except Transform, whose hand-written
/// labels ("Position") differ from the animated channels ("translation"). Returns
/// `None` for non-animatable kinds (text/asset/enum/button/read-only). Wrong
/// guesses are harmless: they just never match a track, so no button shows.
fn field_anim_path(type_id: &str, field_name: &str, kind: FieldKind) -> Option<(String, String)> {
    if !matches!(
        kind,
        FieldKind::Float { .. }
            | FieldKind::Int { .. }
            | FieldKind::Vec3 { .. }
            | FieldKind::Bool
            | FieldKind::Color
            | FieldKind::ColorRgba
            | FieldKind::DynamicEnum
    ) {
        return None;
    }
    // The "Sprite Image" section aggregates fields that animate *different*
    // components than its `type_id`: the `Image` dropdown → `SpriteImages.index`
    // (switchable sheet), and the merged-in grid → `SpriteSheet.{h,v}frames` /
    // `frame`. Map them explicitly (as with Transform). The single-image asset
    // slot is `Asset` kind and already bailed above as non-animatable.
    if type_id == "sprite_image" {
        match field_name {
            "Image" => return Some(("SpriteImages".to_string(), "index".to_string())),
            "H Frames" => return Some(("SpriteSheet".to_string(), "hframes".to_string())),
            "V Frames" => return Some(("SpriteSheet".to_string(), "vframes".to_string())),
            "Frame" => return Some(("SpriteSheet".to_string(), "frame".to_string())),
            _ => {}
        }
    }
    let field = if type_id == "transform" {
        match field_name {
            "Position" => "translation",
            "Rotation" => "rotation",
            "Scale" => "scale",
            _ => return None,
        }
        .to_string()
    } else {
        field_name.trim().to_lowercase().replace(' ', "_")
    };
    Some((type_id.to_string(), field))
}

/// A small per-field "add keyframe" button (a keyframe diamond, matching the
/// timeline's add-key glyph). Hidden by default and shown reactively while the
/// timeline has a clip open on the inspected entity — see
/// [`renzora::ActiveTimeline::animates`]. Pressing it queues a
/// [`renzora::KeyframeRequests`] entry that the timeline editor keys at the
/// playhead from the entity's live value, creating the track first if this field
/// isn't animated yet.
fn build_add_keyframe_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    component: String,
    field: String,
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
                // Start hidden; `bind_display` reveals it on the next reaction
                // frame if the timeline is animating this entity (avoids a
                // one-frame flash on rows built while no clip is open).
                display: Display::None,
                ..default()
            },
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            AddKeyframeBtn { entity, component, field },
            Name::new("field-add-keyframe"),
        ))
        .id();
    bind_display(commands, btn, move |w| {
        w.get_resource::<renzora::ActiveTimeline>()
            .is_some_and(|t| t.animates(entity))
    });
    // Amber diamond — the timeline's keyframe color, so the affordance reads as
    // "add a keyframe" rather than another neutral inspector control.
    let glyph = phosphor_glyph(commands, fonts, "diamond", (230, 170, 90), 11.0);
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
            let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
            bind_2way(
                commands,
                dv,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Float(*v)),
            );
            renzora_ember::inspector::fill_control(commands, dv);
            commands.entity(value_parent).add_child(dv);
        }
        FieldKind::Int { min, max } => {
            let init = if let FieldInit::Float(v) = field.init { v } else { 0.0 };
            // Quarter-unit-per-pixel scrub (4 px per whole step) with the model
            // snapped to integers — the snap is what stops the rounded set_fn
            // read-back from fighting the drag.
            let dv = drag_value(commands, &fonts.ui, "", renzora_ember::theme::value_text(), init, 0.25);
            commands.entity(dv).insert(renzora_ember::widgets::DragSnap(1.0));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
            bind_2way(
                commands,
                dv,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Float(*v)),
            );
            renzora_ember::inspector::fill_control(commands, dv);
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
                let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
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
                            record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Vec3(a));
                        }
                    },
                );
                renzora_ember::inspector::fill_control(commands, dv);
                commands.entity(value_parent).add_child(dv);
            }
        }
        FieldKind::Bool => {
            let init = matches!(field.init, FieldInit::Bool(true));
            let sw = toggle_switch(commands, init);
            let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
            bind_2way(
                commands,
                sw,
                move |w| matches!(get_fn(w, entity), Some(FieldValue::Bool(true))),
                move |w, v: &bool| record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Bool(*v)),
            );
            commands.entity(value_parent).add_child(sw);
        }
        FieldKind::Color => {
            let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
            let editor = renzora_ember::inspector::color_field(
                commands,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Color(c)) => c,
                    _ => [0.0; 3],
                },
                move |w, rgb: [f32; 3]| record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Color(rgb)),
            );
            commands.entity(value_parent).add_child(editor);
        }
        FieldKind::ColorRgba => {
            let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
            let editor = renzora_ember::inspector::color_field_rgba(
                commands,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::ColorRgba(c)) => c,
                    _ => [0.0; 4],
                },
                move |w, rgba: [f32; 4]| record_field_change(w, entity, name, get_fn, set_fn, FieldValue::ColorRgba(rgba)),
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
            let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
            bind_text_input(
                commands,
                ti,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::String(s)) => s,
                    _ => String::new(),
                },
                move |w, v: String| record_field_change(w, entity, name, get_fn, set_fn, FieldValue::String(v)),
            );
            renzora_ember::inspector::fill_control(commands, ti);
            commands.entity(value_parent).add_child(ti);
        }
        FieldKind::Enum { options } => {
            let cur = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let dd = build_enum_dropdown(commands, fonts, entity, field.name, field.get_fn, field.set_fn, options, &cur);
            commands.entity(value_parent).add_child(dd);
        }
        FieldKind::DynamicEnum => {
            let (options, selected) = if let FieldInit::DynEnum(ref o, s) = field.init {
                (o.clone(), s)
            } else {
                (Vec::new(), 0)
            };
            let refs: Vec<&str> = options.iter().map(|s| s.as_str()).collect();
            let sel = selected.min(refs.len().saturating_sub(1));
            let dd = dropdown(commands, fonts, &refs, sel);
            let (get_fn, set_fn, name) = (field.get_fn, field.set_fn, field.name);
            // The value is the selected index; two-way bind so a keyframed /
            // externally-changed index updates the shown option and vice versa.
            bind_2way(
                commands,
                dd,
                move |w| match get_fn(w, entity) {
                    Some(FieldValue::Float(v)) => v.round().max(0.0) as usize,
                    _ => 0,
                },
                move |w, i: &usize| {
                    record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Float(*i as f32));
                },
            );
            renzora_ember::inspector::fill_control(commands, dd);
            commands.entity(value_parent).add_child(dd);
        }
        FieldKind::Asset => {
            let f = build_asset_field(
                commands,
                fonts,
                entity,
                field.name,
                field.get_fn,
                field.set_fn,
                field.extensions.clone(),
            );
            commands.entity(value_parent).add_child(f);
        }
        FieldKind::Button { icon } => {
            let btn_label = field_label_loc(field.name);
            let btn = renzora_ember::widgets::icon_label_button(commands, fonts, icon, &btn_label);
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
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
    /// The field name, so the recorded undo step is labelled + merges correctly.
    field_name: &'static str,
    label: &'static str,
    panel: Entity,
}

#[allow(clippy::too_many_arguments)]
fn build_enum_dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    field_name: &'static str,
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
                    get_fn,
                    set_fn,
                    entity,
                    field_name,
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
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
    field_name: &'static str,
}

#[derive(Component)]
struct AssetClearBtn {
    get_fn: GetFn,
    set_fn: SetFn,
    entity: Entity,
    field_name: &'static str,
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
        _ => (renzora::lang::t("inspector.drag_asset"), false),
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
    build_asset_field(commands, fonts, entity, "asset", get_fn, set_fn, extensions)
}

fn build_asset_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    field_name: &'static str,
    get_fn: GetFn,
    set_fn: SetFn,
    extensions: Vec<String>,
) -> Entity {
    let path_text = commands
        .spawn((
            Text::new(renzora::lang::t("inspector.drag_asset")),
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
                get_fn,
                set_fn,
                entity,
                field_name,
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
            AssetClearBtn {
                get_fn,
                set_fn,
                entity,
                field_name,
            },
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
        let (get_fn, set_fn, entity, name) = (zone.get_fn, zone.set_fn, zone.entity, zone.field_name);
        cmds.push(move |w: &mut World| {
            record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Asset(Some(path_str.clone())))
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
        let (get_fn, set_fn, entity, name) = (btn.get_fn, btn.set_fn, btn.entity, btn.field_name);
        cmds.push(move |w: &mut World| {
            record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Asset(None))
        });
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
    let btn = renzora_ember::widgets::icon_label_button(commands, fonts, "puzzle-piece", &renzora::lang::t("inspector.add_component"));
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
        let (remove_fn, entity, type_id) = (btn.remove_fn, btn.entity, btn.type_id);
        cmds.push(move |w: &mut World| {
            let ctx = renzora_undo::active_context(w);
            renzora_undo::execute(
                w,
                ctx,
                Box::new(RemoveComponentCmd {
                    entity,
                    type_id,
                    remove_fn,
                    captured: None,
                }),
            );
        });
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

/// Zebra-stripe collapsed component headers: a closed section's header takes
/// the odd/even row colour (by its position in the component list) so the
/// flush-stacked collapsed cards read as distinct rows; an open header keeps
/// its per-category colour. Runs off the live [`Section`] flag, so it tracks
/// header clicks and the expand/collapse-all button without a rebuild.
fn stripe_collapsed_headers(
    mut headers: Query<(&Section, &InspectorSectionHeader, &mut BackgroundColor)>,
) {
    for (sec, hdr, mut bg) in &mut headers {
        let want = if sec.is_open() {
            renzora_ember::theme::rgb(hdr.header_bg)
        } else {
            renzora_ember::inspector::inspector_stripe(hdr.index)
        };
        if bg.0 != want {
            bg.0 = want;
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
        let (get_fn, set_fn, entity, name) = (btn.get_fn, btn.set_fn, btn.entity, btn.field_name);
        cmds.push(move |w: &mut World| {
            if let Some(cur) = get_fn(w, entity) {
                record_field_change(w, entity, name, get_fn, set_fn, cur.type_default());
            }
        });
    }
}

/// Queue a keyframe-add when a field's keyframe button is pressed. The timeline
/// editor drains [`renzora::KeyframeRequests`] and keys the entity's live value
/// at the playhead onto the matching track (the undo is recorded there).
fn add_keyframe_click(
    q: Query<(&Interaction, &AddKeyframeBtn), Changed<Interaction>>,
    reqs: Option<ResMut<renzora::KeyframeRequests>>,
) {
    let Some(mut reqs) = reqs else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        reqs.push(btn.entity, btn.component.clone(), btn.field.clone());
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
        Option<fn(&mut World, Entity)>,
    );
    let specs: Vec<Spec> = world
        .get_resource::<renzora_editor_framework::InspectorRegistry>()
        .map(|reg| {
            reg.iter()
                .filter_map(|e| {
                    e.add_fn
                        .map(|af| (e.display_name, e.icon, e.category, e.has_fn, af, e.remove_fn))
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
    for (label, icon, category, has_fn, add_fn, remove_fn) in specs {
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
            move |w: &mut World| {
                let ctx = renzora_undo::active_context(w);
                renzora_undo::execute(
                    w,
                    ctx,
                    Box::new(AddComponentCmd {
                        entity,
                        add_fn,
                        remove_fn,
                    }),
                );
            },
        ));
    }

    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        renzora_ember::widgets::search_overlay(&mut commands, &fonts, &renzora::lang::t("inspector.add_component"), entries);
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
        let (get_fn, set_fn, entity, name, label) =
            (opt.get_fn, opt.set_fn, opt.entity, opt.field_name, opt.label.to_string());
        cmds.push(move |w: &mut World| {
            record_field_change(w, entity, name, get_fn, set_fn, FieldValue::Enum(label.clone()))
        });
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
