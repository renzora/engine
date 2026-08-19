//! Bevy-native (ember) inspector panel.
//!
//! Registry-driven: each `InspectorRegistry` entry shows when its `has_fn`
//! matches and renders either a registered native (bevy_ui) drawer, declarative
//! `fields` (a `FieldType` + get/set fn-pointers, rendered generically here), or
//! a placeholder when it has neither.
//!
//! `rebuild_inspector` (exclusive) rebuilds sections + rows whenever the
//! selection / locked entity / component set / add-overlay changes (hashed
//! signature). Ordinary field-value edits — scalars, `Vec3`, colours — do **not**
//! rebuild; they are reactive via `bind_2way`.
//!
//! Two field reads DO feed the signature, so edits to them rebuild the whole
//! panel, and the distinction is easy to lose:
//!   * `is_enabled_fn` — 37 of 39 implementations read a component *field*
//!     (`s.enabled`, `l.active`, …), so flipping any effect's enable switch
//!     rebuilds every section, not just that one.
//!   * `DynamicEnum` field options — recomputed and hashed each frame, so an
//!     edit that grows or shrinks an option list rebuilds too.
//!
//! (The doc here previously claimed field-value edits never rebuild, which was
//! false for exactly those two.)
//!
//! Layout matches the egui inspector: component sections with a header
//! (caret · icon · title · enable toggle · trash) and field rows with a
//! right-aligned label column + boxed value, alternating row striping.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, FocusPolicy, ScrollPosition, UiGlobalTransform};

use renzora_editor_framework::{
    EditorCommands, EditorSelection, EditorSettings, FieldType, FieldValue,
    InspectorComponentFilterStyle, InspectorExpandDefault, InspectorRegistry, NativeInspectorDrawer,
    NativeInspectorRegistry,
};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, bind_text_color, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{
    bind_text_input, drag_value, dropdown, dropdown_with_icons, scroll_view, set_section_open,
    text_input, toggle_switch, DragRange, EmberTextInput, Section,
};
use renzora_theme::ThemeManager;

// Boxed rather than bare `fn` pointers so a row's accessor can *capture* state.
// A hand-written `FieldDef` names its component statically and needs no capture,
// but a reflection-generated row (see [`crate::reflect_source`]) is parameterised
// by a type path + field path known only at runtime — there is no `fn` pointer
// that can carry those. `Arc<dyn Fn>` is `Clone + Send + Sync + 'static`, so it
// still lives in the widget marker Components that drive the edit handlers.
type GetFn = std::sync::Arc<dyn Fn(&World, Entity) -> Option<FieldValue> + Send + Sync>;
type SetFn = std::sync::Arc<dyn Fn(&mut World, Entity, FieldValue) + Send + Sync>;
// Boxed for the same reason as `GetFn`/`SetFn` above: a generated section's
// remove/enable actions are parameterised by a runtime type path.
type Pred = std::sync::Arc<dyn Fn(&World, Entity) -> bool + Send + Sync>;
type Mutate = std::sync::Arc<dyn Fn(&mut World, Entity) + Send + Sync>;
type SetEnabled = std::sync::Arc<dyn Fn(&mut World, Entity, bool) + Send + Sync>;

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
            set_fn: set_fn.clone(),
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
        if let Some(remove_fn) = self.remove_fn.clone() {
            remove_fn(world, self.entity);
        }
    }
}

/// Add/remove for a **plugin** component.
///
/// Separate from [`AddComponentCmd`] because that one carries `fn` pointers with
/// no per-entry state, and a plugin component's identity is only known at
/// runtime — there is no way to mint a distinct `fn` for one. Carrying the
/// `ComponentId` and the default bytes instead is what makes it possible at all.
struct AddPluginComponentCmd {
    entity: Entity,
    component: bevy::ecs::component::ComponentId,
    default_value: Vec<u8>,
}

impl AddPluginComponentCmd {
    fn insert(&self, world: &mut World) {
        let mut bytes = self.default_value.clone();
        // NOT `OwningPtr::make(bytes.into_boxed_slice(), ..)`. That hands the
        // closure a pointer to *the value passed in* — for a `Box<[u8]>` that is
        // the fat pointer itself, so `insert_by_id` copied 16 bytes of
        // `{ heap address, length }` into the component instead of the bytes it
        // points at. The symptom was a first field full of garbage and the rest
        // zero, with single-field components appearing to work by luck.
        //
        // SAFETY: `bytes` holds exactly one instance of this component, as the
        // plugin described it at registration. `insert_by_id` moves the value
        // out of the pointer, so the allocation is ours to drop afterwards but
        // its contents must not be dropped again.
        unsafe {
            let ptr = bevy::ptr::OwningPtr::new(
                std::ptr::NonNull::new_unchecked(bytes.as_mut_ptr().cast()),
            );
            if let Ok(mut e) = world.get_entity_mut(self.entity) {
                e.insert_by_id(self.component, ptr);
            }
        }
    }
}

impl renzora_undo::UndoCommand for AddPluginComponentCmd {
    fn label(&self) -> &str {
        "Add component"
    }
    fn execute(&mut self, world: &mut World) {
        self.insert(world);
    }
    fn undo(&mut self, world: &mut World) {
        if let Ok(mut e) = world.get_entity_mut(self.entity) {
            e.remove_by_id(self.component);
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
    /// Which component this section is for — the key both
    /// [`InspectorSectionsOpen`] and the (future) keyed-list reconcile use.
    ///
    /// Deliberately *not* the section's list position. Position is presentation,
    /// not identity: baking it in made adding a component near the top look like
    /// a content change for every section below it. `stripe_collapsed_headers`
    /// now derives the stripe index from the live child order instead.
    type_id: &'static str,
    header_bg: (u8, u8, u8),
}

/// Should a section start expanded under `policy`, ignoring any remembered state?
///
/// Keyed on `type_id`, not the display name — the two are easy to confuse (the
/// "ID" section's `type_id` is `"name"`), and matching on the localized-adjacent
/// display string would silently stop working if a label were reworded. Shared by
/// `collect_sections` and `apply_expand_policy_change` so the two can never drift.
fn policy_open(policy: InspectorExpandDefault, type_id: &str) -> bool {
    match policy {
        InspectorExpandDefault::AllOpen => true,
        InspectorExpandDefault::AllClosed => false,
        InspectorExpandDefault::Essentials => {
            matches!(type_id, "name" | "transform" | "script_component")
        }
    }
}

/// Remembered collapse state per component type.
///
/// Keyed by **type**, not by `(entity, type_id)` like `ScriptSectionsOpen` —
/// script sections are per-entity instances, component sections are per-type, so
/// "I keep Transform collapsed and Material open" should follow the user across
/// selections rather than resetting on every click.
///
/// The Inspector Expand Default setting stays authoritative: changing it clears
/// this map *and* re-applies to the live sections, so the preference is always
/// reachable and can never be permanently shadowed by accumulated per-section
/// toggles.
#[derive(Resource, Default)]
struct InspectorSectionsOpen(std::collections::HashMap<&'static str, bool>);

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
fn filter_style(world: &Rx) -> InspectorComponentFilterStyle {
    world
        .get_resource::<EditorSettings>()
        .map(|s| s.inspector_component_filter_style)
        .unwrap_or_default()
}

pub fn register_native_inspector(app: &mut App) {
    use renzora_editor_framework::SplashState;
    app.init_resource::<NativeInspectorState>();
    app.init_resource::<InspectorSectionsOpen>();
    // Reflection-generated sections (see `append_reflected_sections`). Settable
    // from the environment so the hand-written and generated renderings of the
    // same component can be compared without a settings-UI round trip:
    //   RENZORA_REFLECT_INSPECTOR=off   hand-written only (today's behaviour)
    //                             gaps  generated only where nothing is registered (default)
    //                             all   generated for every component, alongside
    app.insert_resource(match std::env::var("RENZORA_REFLECT_INSPECTOR").as_deref() {
        Ok("gaps") => crate::reflect_source::ReflectInspectorMode::FillGaps,
        Ok("all") => crate::reflect_source::ReflectInspectorMode::All,
        _ => crate::reflect_source::ReflectInspectorMode::Off,
    });
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
            asset_drop,
            asset_clear_click,
            asset_create_click,
            asset_drop_highlight,
            inspector_filter_sync,
            component_menu_click,
            expand_all_click,
            sync_expand_glyph,
            stripe_collapsed_headers,
            remember_inspector_sections,
            apply_expand_policy_change,
        )
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
    );
    app.add_systems(
        Update,
        // Chained: the reconciler reads the `Section` open flags that
        // `rebuild_inspector` has just (re)created, so running it after in the
        // same frame means a rebuild lands with its open sections already filled
        // — no one-frame flash of empty bodies on selection change.
        //
        // Culling runs first and only sets flags; the reconciler is still the
        // single place rows are built or thrown away. Putting it ahead of the
        // rebuild also means a scroll and a selection change landing on the same
        // frame resolve in one pass rather than two.
        (
            cull_offscreen_sections,
            rebuild_inspector,
            reconcile_section_bodies,
        )
            .chain()
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

#[derive(Clone)]
enum FieldInit {
    Float(f32),
    Vec3([f32; 3]),
    Bool(bool),
    Text(String),
    /// Dynamic-dropdown options (computed from the world) + the selected index.
    DynEnum(Vec<String>, usize),
}

#[derive(Clone)]
struct FieldSpec {
    name: &'static str,
    kind: FieldKind,
    get_fn: GetFn,
    set_fn: SetFn,
    init: FieldInit,
    /// Accepted extensions for `Asset` fields (empty = accept any). Unused for
    /// other kinds.
    extensions: Vec<String>,
    /// `AssetCreatable` fields only: the "+" button's create-in-place action.
    create_fn: Option<Mutate>,
    /// The component this field reads, when it could be resolved from the
    /// section's type path.
    ///
    /// `get_fn` is a contract-crate `fn(&World, Entity)` and cannot take an
    /// `&Rx`, so the binding around it would otherwise have to give up on
    /// tracking entirely — which is what pinned most of the inspector dirty.
    /// Naming the component lets the binding *declare* the dependency instead:
    /// of 248 `get_fn` definitions in the workspace, 247 are literally
    /// `|w, e| w.get::<C>(e).map(..)` and the one exception ignores both
    /// arguments, so `(entity, component)` is what they read.
    ///
    /// `None` falls back to untracked — unchanged behaviour.
    cid: Option<ComponentId>,
}

/// Call a contract `fn(&World, Entity)` while still declaring what it reads.
///
/// The two halves are deliberately in one place: `manually_tracked` is the only
/// hatch where being wrong causes staleness rather than wasted work, so the
/// `track_component_id` that justifies it sits immediately above the call.
fn tracked_read<T>(
    rx: &Rx,
    entity: Entity,
    cid: Option<ComponentId>,
    f: impl FnOnce(&World) -> T,
) -> T {
    match cid {
        Some(cid) => {
            rx.track_component_id(entity, cid);
            f(rx.manually_tracked())
        }
        // Unknown component: stay conservative, exactly as before.
        None => f(rx.untracked()),
    }
}

/// Resolve a reflected type path (`"bevy_transform::components::Transform"`) to
/// the `ComponentId` the world knows it by, so a binding can depend on it.
///
/// `None` for anything not registered or not a component — the caller then
/// falls back to untracked, which is always safe.
fn component_id_for(world: &World, type_path: &str) -> Option<ComponentId> {
    let registry = world.get_resource::<AppTypeRegistry>()?.clone();
    let type_id = {
        let r = registry.read();
        r.get_with_type_path(type_path)?.type_id()
    };
    world.components().get_id(type_id)
}

struct SectionSpec {
    title: &'static str,
    /// The component this section is for, when resolvable — lets the enable
    /// toggle declare its dependency instead of pinning itself dirty.
    cid: Option<ComponentId>,
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
fn inspected_entity(w: &Rx) -> Option<Entity> {
    let locked = w.get_resource::<NativeInspectorState>().and_then(|s| s.locked);
    locked.or_else(|| w.get_resource::<EditorSelection>().and_then(|s| s.get()))
}

/// `(display_name, icon, category)` for every registered component currently on
/// `entity`, in registry order — the source list for the filter dropdown and the
/// vertical menu (matches the set of sections `collect_sections` would show with
/// no filter). The category rides along so the menu can tint each button the same
/// way that component's section header is tinted.
fn present_components(
    world: &Rx,
    entity: Entity,
) -> Vec<(&'static str, &'static str, &'static str)> {
    let Some(reg) = world.get_resource::<InspectorRegistry>() else {
        return Vec::new();
    };
    reg.iter()
        .filter(|e| (e.has_fn)(world.untracked(), entity))
        .map(|e| (e.display_name, e.icon, e.category))
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
                // The rail stretches the panel's full height, so centre the
                // stack in it rather than letting it hang off the top bar.
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            // No fill: the rail dissolves into the inspector panel, so the only
            // things that read are the category-tinted glyphs and the single
            // filled pill marking the active component.
            BackgroundColor(Color::NONE),
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
    present: &[(&'static str, &'static str, &'static str)],
    selected: &Option<String>,
) -> Entity {
    // Options: "All components" + one per present component, each with its icon.
    let filter_all = renzora::lang::t("inspector.filter_all");
    let names: Vec<&str> = present.iter().map(|(n, _, _)| *n).collect();
    let mut options: Vec<(&str, &str)> = Vec::with_capacity(present.len() + 1);
    options.push(("list", filter_all.as_str()));
    options.extend(present.iter().map(|(name, icon, _)| (*icon, *name)));

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
///
/// `colors` is the per-component `(accent, header_bg)` pair from
/// [`category_rgb`], parallel to `present` — the rail can't read the theme
/// itself because the caller holds `Commands` (and so a `&mut World`) while
/// building. Tinting each button by category makes the rail readable at a glance
/// and matches the header colour of the section it filters to.
fn build_component_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    present: &[(&'static str, &'static str, &'static str)],
    colors: &[((u8, u8, u8), (u8, u8, u8))],
    selected: &Option<String>,
) -> Vec<Entity> {
    let mut out = Vec::with_capacity(present.len() + 1);
    // "All" first — no category of its own, so it stays on the neutral theme
    // accent rather than borrowing some component's colour.
    out.push(component_menu_button(
        commands,
        fonts,
        "list",
        None,
        (renzora_ember::theme::accent(), renzora_ember::theme::panel_bg()),
        selected.is_none(),
    ));
    for (i, (name, icon, _)) in present.iter().enumerate() {
        let active = selected.as_deref() == Some(*name);
        let tint = colors
            .get(i)
            .copied()
            .unwrap_or((renzora_ember::theme::accent(), renzora_ember::theme::panel_bg()));
        out.push(component_menu_button(
            commands,
            fonts,
            icon,
            Some((*name).to_string()),
            tint,
            active,
        ));
    }
    out
}

/// One icon button in the left component menu. The rail is icon-only, so each
/// button carries a [`HoverTooltip`] naming its component — the shared global
/// bubble can't be clipped by the rail/panel the way the old per-button
/// bubble children were.
///
/// `tint` is the component category's `(accent, header_bg)`. Idle buttons draw
/// only their glyph in the category accent — no fill, so the rail stays quiet
/// and the one filled button is unambiguously the active one.
fn component_menu_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    name: Option<String>,
    tint: ((u8, u8, u8), (u8, u8, u8)),
    active: bool,
) -> Entity {
    let (accent, _header_bg) = tint;
    let (bg, glyph_color) = if active {
        (c(accent), renzora_ember::theme::on_accent())
    } else {
        (Color::NONE, accent)
    };
    let label = name.clone().unwrap_or_else(|| renzora::lang::t("inspector.filter_all"));
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(32.0),
                height: Val::Px(32.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(bg),
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(label),
            ComponentMenuButton { name },
            Name::new("component-menu-button"),
        ))
        .id();
    let glyph = phosphor_glyph(commands, fonts, icon, glyph_color, 20.0);
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

/// `container_q` is a `Local` rather than a fresh `world.query_filtered(..)` per
/// call: this system runs every frame the Inspector tab is active, and building a
/// `QueryState` each time forces `update_archetypes` down its
/// from-generation-zero full-scan branch. `With<T>` goes through `and_with` and
/// never populates `FilteredAccess::required`, so there is no cheap path — it
/// rescans every archetype in the world, every frame, to find one entity.
fn rebuild_inspector(
    world: &mut World,
    mut container_q: Local<QueryState<Entity, With<InspectorRoot>>>,
) {
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
            .map(|e| present_components(&Rx::new(&*world), e).iter().any(|(n, _, _)| *n == sel))
            .unwrap_or(false);
        if !still_present {
            world.resource_mut::<NativeInspectorState>().selected = None;
        }
    }

    let Some(container) = container_q.iter(world).next() else {
        return;
    };

    let sig = inspector_signature(&Rx::new(&*world), container, entity, locked.is_some());
    if world.resource::<NativeInspectorState>().sig == Some(sig) {
        return;
    }

    let sections = collect_sections(&Rx::new(&*world), entity);
    let state = world.resource::<NativeInspectorState>();
    let filter_active = !state.filter.is_empty() || state.selected.is_some();
    let existing: Vec<Entity> = world
        .get::<Children>(container)
        .map(|ch| ch.iter().collect())
        .unwrap_or_default();

    // The component filter renders as either a left rail of icon buttons or a
    // top-bar dropdown; we rebuild whichever the user picked and clear the other.
    let style = filter_style(&Rx::new(&*world));
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
    let present: Vec<(&'static str, &'static str, &'static str)> =
        entity.map(|e| present_components(&Rx::new(&*world), e)).unwrap_or_default();
    // Resolve each component's category colours up front: the rail is built
    // through `Commands` (which borrows the world mutably), so it can't reach
    // `ThemeManager` itself.
    let menu_colors: Vec<((u8, u8, u8), (u8, u8, u8))> = {
        let theme = world.get_resource::<ThemeManager>();
        present
            .iter()
            .map(|(_, _, category)| {
                theme
                    .map(|tm| category_rgb(&tm.active_theme, category))
                    .unwrap_or(((120, 140, 200), (44, 44, 54)))
            })
            .collect()
    };
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
                    let buttons = build_component_menu(
                        &mut commands,
                        &fonts,
                        &present,
                        &menu_colors,
                        &selected_now,
                    );
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
                for sec in sections.iter() {
                    let (root, body) =
                        build_section(&mut commands, &fonts, sec, entity, locked_here);
                    commands.entity(container).add_child(root);
                    // Only an OPEN section's drawer runs here; a collapsed one is
                    // left to `reconcile_section_bodies` to run if it's expanded.
                    if let (Some(drawer), true) = (sec.native_drawer, sec.open) {
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

/// Known gap, deliberately not closed: a field's *visibility* depends on its
/// `get_fn` returning `Some` (see `collect_sections`), and that predicate is not
/// hashed here — so a field that appears or disappears without any other input
/// changing leaves a stale row.
///
/// Not fixed because the cure is worse: folding it in means calling `get_fn` for
/// every field of every present component **every frame**, before the early-out,
/// to guard against something only three `get_fn`s in the entire workspace can
/// even express (the rest are unconditional). Per-section hashing would make it
/// cheap — it would only re-read the fields of one section — so this belongs with
/// that work rather than as a standalone per-frame cost.
fn inspector_signature(
    world: &Rx,
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
                    if (entry.has_fn)(world.untracked(), e) {
                        entry.type_id.hash(&mut h);
                        // Presence-toggled sections (their enable switch
                        // inserts/removes the underlying component, e.g. 2D
                        // Lighting on a camera) change their rows without
                        // changing the section set — fold the enabled bit in
                        // so flipping the switch rebuilds the body.
                        if let Some(is_enabled) = entry.is_enabled_fn {
                            is_enabled(world.untracked(), e).hash(&mut h);
                        }
                        // A `DynamicEnum` field's options are computed from the
                        // world at build time, so a *mutation* that grows/shrinks
                        // the list (e.g. appending a sprite sheet) wouldn't
                        // otherwise change the signature — leaving a stale option
                        // list and an out-of-range selection (blank dropdown).
                        // Fold the options in so the list rebuilds when it changes.
                        for field in &entry.fields {
                            if let FieldType::DynamicEnum { options } = field.field_type {
                                for opt in options(world.untracked(), e) {
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

fn collect_sections(world: &Rx, entity: Option<Entity>) -> Vec<SectionSpec> {
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
    // Remembered per-type collapse state wins over the policy; the policy is
    // re-asserted (and this map cleared) by `apply_expand_policy_change` whenever
    // the setting itself changes, so it can never become unreachable.
    let remembered = world.get_resource::<InspectorSectionsOpen>();
    let section_open = |type_id: &'static str| -> bool {
        if let Some(&open) = remembered.and_then(|m| m.0.get(type_id)) {
            return open;
        }
        policy_open(expand_policy, type_id)
    };

    let mut out = Vec::new();
    for entry in reg.iter() {
        if !(entry.has_fn)(world.untracked(), entity) {
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
            (Some(g), Some(s)) => Some((std::sync::Arc::new(g) as Pred, std::sync::Arc::new(s) as SetEnabled)),
            _ => None,
        };
        let enabled_now = enable.as_ref().map(|(g, _)| g(world.untracked(), entity)).unwrap_or(true);
        // Priority: a registered native bevy_ui drawer > declarative `fields` >
        // placeholder note (component has neither a native drawer nor any fields).
        let native_drawer = native_reg.and_then(|r| r.get(entry.type_id));
        if native_drawer.is_some() {
            out.push(SectionSpec {
                title: entry.display_name,
            cid: component_id_for(world.untracked(), entry.type_id),
                icon: entry.icon,
                type_id: entry.type_id,
                custom: false,
                native_drawer,
                remove_fn: entry.remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
                enable: enable.clone(),
                enabled_now,
                header_bg,
                accent,
                open: section_open(entry.type_id),
                fields: Vec::new(),
            });
            continue;
        }
        if entry.fields.is_empty() {
            out.push(SectionSpec {
                title: entry.display_name,
                cid: component_id_for(world.untracked(), entry.type_id),
                icon: entry.icon,
                type_id: entry.type_id,
                custom: true,
                native_drawer: None,
                remove_fn: entry.remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
                enable: enable.clone(),
                enabled_now,
                header_bg,
                accent,
                open: section_open(entry.type_id),
                fields: Vec::new(),
            });
            continue;
        }
        let mut fields = Vec::new();
        for f in &entry.fields {
            let val = (f.get_fn)(world.untracked(), entity);
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
                    FieldInit::DynEnum(options(world.untracked(), entity), v.round().max(0.0) as usize),
                ),
                (FieldType::Asset { .. }, Some(FieldValue::Asset(_)))
                | (FieldType::AssetCreatable { .. }, Some(FieldValue::Asset(_))) => {
                    (FieldKind::Asset, FieldInit::Text(String::new()))
                }
                // Buttons have no value to read — match regardless of `val`.
                (FieldType::Button { icon }, _) => {
                    (FieldKind::Button { icon }, FieldInit::Text(String::new()))
                }
                _ => (FieldKind::ReadOnly, FieldInit::Text(format_value(val.as_ref()))),
            };
            let extensions = match &f.field_type {
                FieldType::Asset { extensions }
                | FieldType::AssetCreatable { extensions, .. } => extensions.clone(),
                _ => Vec::new(),
            };
            let create_fn = match &f.field_type {
                FieldType::AssetCreatable { create_fn, .. } => Some(std::sync::Arc::new(*create_fn) as Mutate),
                _ => None,
            };
            fields.push(FieldSpec {
                name: f.name,
                kind,
                // A hand-written `FieldDef` still supplies plain fn pointers;
                // they coerce into the boxed accessor here, at the one seam.
                get_fn: std::sync::Arc::new(f.get_fn),
                set_fn: std::sync::Arc::new(f.set_fn),
                init,
                extensions,
                create_fn: create_fn.clone(),
                cid: component_id_for(world.untracked(), entry.type_id),
            });
        }
        out.push(SectionSpec {
            title: entry.display_name,
            cid: component_id_for(world.untracked(), entry.type_id),
            icon: entry.icon,
            type_id: entry.type_id,
            custom: false,
            native_drawer: None,
            remove_fn: entry.remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
            enable: enable.clone(),
            enabled_now,
            header_bg,
            accent,
            open: section_open(entry.type_id),
            fields,
        });
    }

    append_reflected_sections(world, entity, reg, &mut out);

    // Pin the most-edited components to the top in a fixed order — Name,
    // Transform, then Scripts, then Material — so they're always right where you
    // expect regardless of plugin registration order. A stable sort keeps every
    // other component in its original registry order behind them.
    out.sort_by_key(|s| section_priority(s.title));
    out
}

/// Append sections generated from `bevy_reflect` for components the hand-written
/// [`InspectorRegistry`] does not cover (or, in `All` mode, for every reflected
/// component, so a generated section can be compared side by side against the
/// hand-written one for the same type).
///
/// This is the whole point of [`crate::reflect_source`]: the rows below are
/// produced without any component naming an inspector type, which is what lets a
/// component crate carry no editor dependency at all.
fn append_reflected_sections(
    world: &Rx,
    entity: Entity,
    reg: &InspectorRegistry,
    out: &mut Vec<SectionSpec>,
) {
    let mode = world
        .get_resource::<crate::reflect_source::ReflectInspectorMode>()
        .copied()
        .unwrap_or_default();
    if mode == crate::reflect_source::ReflectInspectorMode::Off {
        return;
    }

    // The hand-written registry keys on a slug (`"transform"`), reflection on a
    // Rust type name (`Transform`). Match on both the slug and the display name
    // with separators removed, which is as close as the two vocabularies get —
    // an over-match only means a component keeps its hand-written section, which
    // is the safe direction.
    let mut covered: std::collections::HashSet<String> = std::collections::HashSet::new();
    if mode == crate::reflect_source::ReflectInspectorMode::FillGaps {
        for entry in reg.iter() {
            for key in [entry.type_id, entry.display_name] {
                let k = key.to_ascii_lowercase().replace([' ', '_', '-'], "");
                // Register the singular too: entries are named for the panel
                // ("Scripts") while the type is singular (`ScriptComponent`).
                covered.insert(k.trim_end_matches('s').to_string());
                covered.insert(k);
            }
        }
    }

    let generated = crate::reflect_source::reflect_sections(world.untracked(), entity, &|short| {
        // Reflected type names carry noise words the curated names never do —
        // `AtmosphereComponentSettings` is the `Atmosphere` entry, `CloudsData`
        // is `Clouds`. Strip those before comparing, or every settings component
        // gets a duplicate generated section next to its hand-written one.
        let bare = short.replace('_', "");
        let mut stem = bare.as_str();
        for suffix in ["componentsettings", "component", "settings", "config", "data"] {
            stem = stem.strip_suffix(suffix).unwrap_or(stem);
        }
        covered.contains(&bare)
            || covered.contains(stem)
            || covered.contains(stem.trim_end_matches('s'))
    });

    for section in generated {
        let type_path = section.type_path;
        // Generic equivalents of a hand-written entry's `remove_fn` and
        // `is_enabled_fn`/`set_enabled_fn`, both parameterised by the type path —
        // which is exactly why these had to stop being bare fn pointers.
        let remove_fn: Option<Mutate> = Some(std::sync::Arc::new(
            move |w: &mut World, e: Entity| {
                crate::reflect_source::remove_component(w, e, type_path);
            },
        ));
        let enable: Option<(Pred, SetEnabled)> = section.has_enabled.then(|| {
            let pred: Pred = std::sync::Arc::new(move |w: &World, e: Entity| {
                matches!(
                    crate::reflect_source::read_field(w, e, type_path, "enabled", false),
                    Some(FieldValue::Bool(true))
                )
            });
            let set: SetEnabled = std::sync::Arc::new(move |w: &mut World, e: Entity, v: bool| {
                crate::reflect_source::write_field(w, e, type_path, "enabled", FieldValue::Bool(v));
            });
            (pred, set)
        });
        let enabled_now = enable.as_ref().map(|(g, _)| g(world.untracked(), entity)).unwrap_or(true);
        let mut fields = Vec::new();
        for f in section.fields {
            let (kind, init) = match (&f.field_type, &f.value) {
                (FieldType::Float { speed, min, max }, FieldValue::Float(v)) => (
                    FieldKind::Float { speed: *speed, min: *min, max: *max },
                    FieldInit::Float(*v),
                ),
                (FieldType::Int { min, max }, FieldValue::Float(v)) => {
                    (FieldKind::Int { min: *min, max: *max }, FieldInit::Float(*v))
                }
                (FieldType::Vec3 { speed }, FieldValue::Vec3(a)) => {
                    (FieldKind::Vec3 { speed: *speed }, FieldInit::Vec3(*a))
                }
                (FieldType::Bool, FieldValue::Bool(b)) => (FieldKind::Bool, FieldInit::Bool(*b)),
                // The colour widgets seed themselves from the live value.
                (FieldType::Color, FieldValue::Color(_)) => {
                    (FieldKind::Color, FieldInit::Text(String::new()))
                }
                (FieldType::ColorRgba, FieldValue::ColorRgba(_)) => {
                    (FieldKind::ColorRgba, FieldInit::Text(String::new()))
                }
                (FieldType::String, FieldValue::String(s)) => {
                    (FieldKind::Text, FieldInit::Text(s.clone()))
                }
                (FieldType::Enum { options }, FieldValue::Enum(s)) => {
                    (FieldKind::Enum { options }, FieldInit::Text(s.clone()))
                }
                _ => (
                    FieldKind::ReadOnly,
                    FieldInit::Text(format_value(Some(&f.value))),
                ),
            };
            let read_only = matches!(kind, FieldKind::ReadOnly);
            let (path, get_path) = (f.path, f.path);
            fields.push(FieldSpec {
                name: f.label,
                kind,
                get_fn: std::sync::Arc::new(move |w: &World, e: Entity| {
                    crate::reflect_source::read_field(w, e, type_path, get_path, read_only)
                }),
                set_fn: std::sync::Arc::new(move |w: &mut World, e: Entity, v: FieldValue| {
                    crate::reflect_source::write_field(w, e, type_path, path, v);
                }),
                init,
                extensions: Vec::new(),
                create_fn: None,
                cid: component_id_for(world.untracked(), type_path),
            });
        }
        out.push(SectionSpec {
            title: section.short_name,
            cid: component_id_for(world.untracked(), type_path),
            icon: "cube",
            type_id: type_path,
            custom: false,
            native_drawer: None,
            remove_fn,
            enable,
            enabled_now,
            header_bg: (44, 44, 54),
            accent: (150, 130, 200),
            // Closed by default: in `All` mode every component gains a second
            // section, and opening them all would bury the hand-written ones.
            open: false,
            fields,
        });
    }
}

/// Display order weight for a section: pinned components come first in a fixed
/// order; everything else shares the same (higher) weight and so keeps its
/// registry order under the stable sort in [`collect_sections`].
fn section_priority(title: &str) -> u8 {
    match title {
        "ID" => 0,
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

/// The recipe for (re)filling a section body, parked on the body entity itself.
///
/// Sections are built collapsed-and-empty and filled on expand, so this has to
/// survive on the body across collapse/expand cycles — it's the only record of
/// how to rebuild rows that were thrown away. `filled` tracks whether the body
/// currently holds its rows, so [`reconcile_section_bodies`] can tell "just
/// expanded" from "already done" without diffing children every frame.
#[derive(Component)]
struct SectionBodySpec {
    fields: Vec<FieldSpec>,
    entity: Entity,
    type_id: &'static str,
    native_drawer: Option<NativeInspectorDrawer>,
    custom: bool,
    filled: bool,
}

/// Build the declarative field rows for a section body (shared by the initial
/// build and the fill-on-expand path so the two can't drift — notably the stripe
/// colour, which is derived from row index).
fn fill_section_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    fields: &[FieldSpec],
    entity: Entity,
    type_id: &'static str,
    body: Entity,
) {
    for (i, field) in fields.iter().enumerate() {
        let r = build_field_row(commands, fonts, field, entity, type_id);
        commands
            .entity(r)
            .insert(BackgroundColor(renzora_ember::inspector::inspector_stripe(i)));
        commands.entity(body).add_child(r);
    }
}

/// Keep each section body's contents in sync with its header's open flag: fill
/// on expand, throw the rows away on collapse.
///
/// Reconciliation rather than click handling, deliberately — `set_section_open`
/// (expand/collapse-all, and the expand-default policy) moves sections without
/// any click, and observing the resulting *state* covers every path at once.
///
/// Exclusive because native drawers are `fn(&mut World, Entity) -> Entity`.
fn reconcile_section_bodies(
    world: &mut World,
    // Scoped to inspector headers: `Section` is a shared ember widget, so a bare
    // `&Section` query would walk every collapsible section in the editor.
    mut headers: Local<QueryState<&Section, With<InspectorSectionHeader>>>,
) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };

    // (body, open) for every section whose body disagrees with its header.
    //
    // "Should hold rows" is the header's open flag AND the body being on screen
    // — see [`cull_offscreen_sections`]. Folding culling in here rather than
    // giving it its own despawn path means there is still exactly one place that
    // builds and one that throws away, so the two can't drift.
    let mut todo: Vec<(Entity, bool)> = Vec::new();
    for sec in headers.iter(world) {
        let body = sec.body();
        let culled = world.get::<SectionCull>(body).is_some_and(|c| c.culled);
        let want = sec.is_open() && !culled;
        if let Some(spec) = world.get::<SectionBodySpec>(body) {
            if spec.filled != want {
                todo.push((body, want));
            }
        }
    }
    if todo.is_empty() {
        return;
    }

    for (body, open) in todo {
        let Some(spec) = world.get::<SectionBodySpec>(body) else {
            continue;
        };
        let (fields, ent, type_id, drawer, custom) = (
            spec.fields.clone(),
            spec.entity,
            spec.type_id,
            spec.native_drawer,
            spec.custom,
        );
        // A despawned inspected entity outlives its rows here (selection can
        // change in the same frame a section is toggled) — skip rather than
        // build rows whose accessors would miss.
        if open && world.get_entity(ent).is_err() {
            continue;
        }

        let existing: Vec<Entity> = world
            .get::<Children>(body)
            .map(|ch| ch.iter().collect())
            .unwrap_or_default();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for child in existing {
                // try_despawn: a row's own binding may have retired it already.
                commands.entity(child).try_despawn();
            }
            if open {
                if drawer.is_some() {
                    // filled below, after the queue applies (needs &mut World)
                } else if custom {
                    let note =
                        empty_label(&mut commands, &fonts, &renzora::lang::t("inspector.custom_pending"));
                    commands.entity(body).add_child(note);
                } else {
                    fill_section_rows(&mut commands, &fonts, &fields, ent, type_id, body);
                }
            }
        }
        queue.apply(world);

        if open {
            if let Some(drawer) = drawer {
                let content = drawer(world, ent);
                if let Ok(mut em) = world.get_entity_mut(body) {
                    em.add_child(content);
                }
            }
        }
        if let Some(mut spec) = world.get_mut::<SectionBodySpec>(body) {
            spec.filled = open;
        }
    }
}

// ── Viewport culling ─────────────────────────────────────────────────────────

/// How far outside the viewport a section body is kept built, as a fraction of
/// the viewport's own height.
///
/// Culling exactly at the viewport edge would rebuild a section the instant one
/// pixel of it scrolls into view, so a slow drag would pay a rebuild every frame.
/// Half a screen of slack on each side means normal scrolling crosses the
/// boundary rarely, and a section is built and laid out well before it is
/// readable.
///
/// Relative rather than a fixed pixel count because both things it trades off
/// scale with the panel: a tall inspector is scrolled in bigger jumps, and a
/// fixed slack generous enough for a tall one would exceed a short panel's whole
/// content — culling nothing at all.
const CULL_OVERSCAN_FRAC: f32 = 0.5;

/// Floor for the overscan, in logical px. A very short inspector (a docked strip)
/// would otherwise get a slack of almost nothing and pop rows in and out under
/// small scrolls.
const CULL_OVERSCAN_MIN_PX: f32 = 200.0;

/// Off-screen culling state for one section body, alongside its
/// [`SectionBodySpec`].
///
/// Collapsing a section is already known to be the single biggest win available
/// to this panel — measured at ~3.3 ms/frame, 60 fps → 50 fps, for one entity's
/// worth of open components — because a collapsed body *builds nothing*
/// ([`build_section`] explains why hiding is not enough). Scrolling a section out
/// of view makes it exactly as invisible as collapsing it does, but the rows
/// stayed built and kept charging taffy for a full tree walk every frame.
///
/// So this applies the same trick on a second axis: an open section whose body
/// has scrolled far enough out of the viewport throws its rows away, and rebuilds
/// them when it scrolls back. The section's *header* is untouched — it is one row
/// and it is what you scroll past to find things.
#[derive(Component, Default)]
struct SectionCull {
    /// The body's height in logical px, measured while it still held its rows.
    ///
    /// Pinned onto the body while culled. Without it an emptied body collapses to
    /// its padding, which drags every section below it up the panel and shrinks
    /// the scroll range under the user's thumb — the content would appear to
    /// dissolve as you scrolled. Recording the height and reserving it keeps the
    /// list's geometry byte-identical to the unculled version.
    placeholder_h: f32,
    /// True while the rows have been thrown away for being off screen.
    culled: bool,
}

/// What [`cull_offscreen_sections`] decided to do with one section this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CullAction {
    /// Leave it alone.
    Keep,
    /// Throw the rows away and reserve `placeholder_h` in their place.
    Cull,
    /// Release the reservation; the reconciler rebuilds the rows.
    Restore,
    /// Still on screen and holding its rows — record its height for later.
    Measure(f32),
}

/// The culling decision for one section, split out from the ECS plumbing.
///
/// Worth isolating because the two rules that make this safe are both "don't"
/// rules, and a "don't" is exactly what silently stops happening after a
/// refactor: never cull a section whose height was never measured (there would
/// be nothing to reserve, and the list would collapse), and never measure a body
/// that is not currently holding its rows (it would record its padding as the
/// section's height and reserve *that* forever). Neither is observable from a
/// screenshot until the panel is already wrong.
fn cull_action(
    state: &SectionCull,
    filled: bool,
    top: f32,
    height: f32,
    keep_top: f32,
    keep_bot: f32,
) -> CullAction {
    // Half-open overlap against the keep band: a section touching it at all
    // stays built.
    let visible = top < keep_bot && top + height > keep_top;
    if visible {
        if state.culled {
            CullAction::Restore
        } else if filled && (state.placeholder_h - height).abs() > 0.5 {
            CullAction::Measure(height)
        } else {
            CullAction::Keep
        }
    } else if !state.culled && state.placeholder_h > 0.0 {
        CullAction::Cull
    } else {
        CullAction::Keep
    }
}

/// Throw away the rows of open sections that have scrolled out of the inspector's
/// viewport, and rebuild them when they scroll back.
///
/// Reads the *previous* frame's layout (`ComputedNode` / `UiGlobalTransform`),
/// which is why [`CULL_OVERSCAN_PX`] exists — a frame of lag at scroll speed is
/// well inside the slack.
///
/// Deliberately not built on [`renzora_ember::virtual_scroll`], which the rest of
/// the editor's lists use. That windows a `keyed_list` by measuring one row stride
/// and assuming every item shares it — exact for the asset grid and the hierarchy,
/// and wrong here: a collapsed section is one header, an open one with a native
/// drawer is hundreds of px, and no single stride describes both. Measuring each
/// section's own height sidesteps the assumption instead of fighting it.
fn cull_offscreen_sections(
    root: Query<Entity, With<InspectorRoot>>,
    parents: Query<&ChildOf>,
    viewports: Query<(&ComputedNode, &UiGlobalTransform), With<ScrollPosition>>,
    headers: Query<&Section, With<InspectorSectionHeader>>,
    mut bodies: Query<(
        &mut SectionCull,
        &mut Node,
        &ComputedNode,
        &UiGlobalTransform,
        &SectionBodySpec,
    )>,
) {
    let Ok(root) = root.single() else {
        return;
    };
    // Walk up to the enclosing scroll viewport. `scroll_view` puts the content
    // directly under it, but going through the parent chain keeps this working if
    // the panel ever gains an intermediate wrapper.
    let mut e = root;
    let mut viewport = None;
    for _ in 0..8 {
        let Ok(parent) = parents.get(e) else { break };
        let parent = parent.parent();
        if let Ok(v) = viewports.get(parent) {
            viewport = Some(v);
            break;
        }
        e = parent;
    }
    let Some((vp_node, vp_xf)) = viewport else {
        return;
    };

    let inv = vp_node.inverse_scale_factor();
    let vp_h = vp_node.size().y * inv;
    // A zero-height viewport means the panel is in a collapsed tab or a hidden
    // dock leaf. Culling against it would cull *everything*, and the rows would
    // then all rebuild at once the moment the tab came back — the opposite of
    // what this is for. Leave the panel exactly as it is.
    if vp_h <= 0.0 {
        return;
    }
    // `UiGlobalTransform` already carries the scroll offset, so viewport and
    // section rects are directly comparable without consulting `ScrollPosition`.
    let vp_top = vp_xf.translation.y * inv - vp_h * 0.5;
    let overscan = (vp_h * CULL_OVERSCAN_FRAC).max(CULL_OVERSCAN_MIN_PX);
    let keep_top = vp_top - overscan;
    let keep_bot = vp_top + vp_h + overscan;

    for sec in &headers {
        // A collapsed section is already empty; there is nothing to cull, and its
        // zero-height body would measure as a bogus placeholder.
        if !sec.is_open() {
            continue;
        }
        let Ok((mut cull, mut node, computed, xf, spec)) = bodies.get_mut(sec.body()) else {
            continue;
        };
        let h = computed.size().y * inv;
        let top = xf.translation.y * inv - h * 0.5;

        // `Node` is only written on a real transition. Touching it unconditionally
        // would dirty the very thing this exists to avoid: any `DerefMut` on a
        // `Node` re-runs taffy for that subtree, so an unguarded write here would
        // charge a relayout every frame per section to save relayouts.
        match cull_action(&cull, spec.filled, top, h, keep_top, keep_bot) {
            CullAction::Keep => {}
            CullAction::Measure(h) => cull.placeholder_h = h,
            CullAction::Restore => {
                cull.culled = false;
                node.height = Val::Auto;
            }
            CullAction::Cull => {
                cull.culled = true;
                node.height = Val::Px(cull.placeholder_h);
            }
        }
    }
}

#[cfg(test)]
mod cull_tests {
    use super::{cull_action, CullAction, SectionCull};

    // A 600px viewport with half a screen of overscan each side. Spelled out
    // rather than derived from the constants, so retuning the overscan can't
    // quietly move the band these cases are pinned to.
    const KEEP_TOP: f32 = -300.0;
    const KEEP_BOT: f32 = 900.0;

    fn measured(h: f32) -> SectionCull {
        SectionCull { placeholder_h: h, culled: false }
    }

    #[test]
    fn a_section_far_below_the_viewport_is_culled() {
        let s = measured(150.0);
        assert_eq!(
            cull_action(&s, true, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Cull
        );
    }

    /// The rule that keeps the list from collapsing. A section built this frame
    /// has no recorded height, so emptying it would reserve nothing and drag
    /// everything below it up the panel.
    #[test]
    fn an_unmeasured_section_is_never_culled() {
        let fresh = SectionCull::default();
        assert_eq!(
            cull_action(&fresh, true, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    #[test]
    fn a_section_inside_the_overscan_band_stays_built() {
        let s = measured(150.0);
        // Entirely below the viewport, but within the overscan slack.
        assert_eq!(
            cull_action(&s, true, 800.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    /// A section straddling the bottom edge is half on screen; culling it would
    /// blank rows the user is looking at.
    #[test]
    fn a_partially_visible_section_stays_built() {
        let s = measured(400.0);
        assert_eq!(
            cull_action(&s, true, 500.0, 400.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    #[test]
    fn a_culled_section_scrolled_back_into_view_restores() {
        let s = SectionCull { placeholder_h: 150.0, culled: true };
        assert_eq!(
            cull_action(&s, false, 100.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Restore
        );
    }

    /// The other "don't" rule. A culled body measures at its reserved height with
    /// no rows in it; re-measuring an unfilled body would let a stale or padding
    /// height overwrite the real one.
    #[test]
    fn an_unfilled_body_is_never_measured() {
        let s = measured(150.0);
        assert_eq!(
            cull_action(&s, false, 100.0, 6.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
        // ...but the same body holding its rows is measured.
        assert_eq!(
            cull_action(&s, true, 100.0, 6.0, KEEP_TOP, KEEP_BOT),
            CullAction::Measure(6.0)
        );
    }

    /// Sub-pixel drift must not re-record, or every frame writes `SectionCull`
    /// for every section.
    #[test]
    fn an_unchanged_height_is_not_re_measured() {
        let s = measured(150.0);
        assert_eq!(
            cull_action(&s, true, 100.0, 150.2, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }

    /// Culling must be a fixed point: a culled section reserves its own height, so
    /// its rect does not move, so nothing below it moves either. If this ever
    /// returned `Restore` the panel would thrash between built and empty forever.
    #[test]
    fn culling_does_not_oscillate() {
        let mut s = measured(150.0);
        assert_eq!(
            cull_action(&s, true, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Cull
        );
        s.culled = true;
        // Same geometry next frame — the reserved height matches what the rows
        // occupied, which is the whole point of recording it.
        assert_eq!(
            cull_action(&s, false, 3000.0, 150.0, KEEP_TOP, KEEP_BOT),
            CullAction::Keep
        );
    }
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
        type_id: sec.type_id,
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
    // A COLLAPSED section builds nothing — it records how to fill itself and
    // `reconcile_section_bodies` does so when (if) the user expands it.
    //
    // Collapsing is not enough on its own: `section_with_header_open` sets the
    // body to `Display::None`, and `bevy_ui` does NOT prune hidden subtrees from
    // its per-frame walk — `compute_hidden_layout` clears the cache and recurses,
    // so a hidden row is *never* cached and pays full layout every frame, forever.
    // An entity with a dozen components and two sections open was laying out every
    // row of the other ten. Native drawers were worse: `drawer(world, ent)` ran and
    // built its whole content for a section nobody could see.
    commands.entity(body).insert((
        SectionBodySpec {
            fields: sec.fields.clone(),
            entity,
            type_id: sec.type_id,
            native_drawer: sec.native_drawer,
            custom: sec.custom,
            filled: sec.open,
        },
        // Starts unmeasured, so a freshly built section is never culled before
        // it has a height to reserve. See `SectionCull::placeholder_h`.
        SectionCull::default(),
    ));
    if !sec.open {
        // nothing built — `reconcile_section_bodies` fills it on expand
    } else if sec.native_drawer.is_some() {
        // Body is filled by the registered native drawer once the build queue
        // has applied (it needs exclusive &mut World). See `rebuild_inspector`.
    } else if sec.custom {
        let note = empty_label(commands, fonts, &renzora::lang::t("inspector.custom_pending"));
        commands.entity(body).add_child(note);
    } else {
        fill_section_rows(commands, fonts, &sec.fields, entity, sec.type_id, body);
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
        // Bind the glyph rather than leaving it baked. `locked` reaches the UI
        // *only* through the inspector's global rebuild signature today, so the
        // baked-in version works purely by accident — and this glyph is the sole
        // consumer of `locked_here`, so under granular rebuilds nothing about the
        // panel would change when you click lock: the state would flip and the
        // icon would sit there, making the button look broken.
        let locked_now = move |w: &Rx| {
            w.get_resource::<NativeInspectorState>()
                .and_then(|s| s.locked)
                == Some(entity)
        };
        let g = locked_now;
        bind_text(commands, lock, move |w| {
            let name = if g(w) { "lock-simple" } else { "lock-simple-open" };
            renzora_ember::phosphor_map::icon_glyph(name)
                .unwrap_or('\u{E4C6}')
                .to_string()
        });
        bind_text_color(commands, lock, move |w| {
            let (r, gg, b) = if locked_now(w) {
                (120, 170, 255)
            } else {
                renzora_ember::theme::text_muted()
            };
            c((r, gg, b))
        });
        extra.push(lock);
    }
    if let Some((_, set_enabled)) = sec.enable.clone() {
        let sw = toggle_switch(commands, sec.enabled_now);
        // Block the press from bubbling to the section header behind it, so
        // flipping the enable switch doesn't also collapse/expand the section
        // (same reason the lock/trash glyphs above set FocusPolicy::Block).
        commands.entity(sw).insert(FocusPolicy::Block);
        let g = sec.enable.clone().unwrap().0;
        let sec_cid = sec.cid;
        bind_2way(
            commands,
            sw,
            move |w| tracked_read(w, entity, sec_cid, |world| g(world, entity)),
            move |w, v: &bool| {
                let target = *v;
                let ctx = renzora_undo::active_context(w);
                renzora_undo::execute(
                    w,
                    ctx,
                    Box::new(EnableToggleCmd { entity, set_enabled: set_enabled.clone(), target }),
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
    if let (Some(remove_fn), false) = (sec.remove_fn.clone(), hide_trash) {
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
        let reset = build_reset_button(commands, fonts, field.name, field.get_fn.clone(), field.set_fn.clone(), entity);
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
    // Which component this field reads, for the dependency the value closures
    // declare — see [`tracked_read`].
    let cid = field.cid;
    match field.kind {
        FieldKind::Float { speed, min, max } => {
            let init = if let FieldInit::Float(v) = field.init { v } else { 0.0 };
            let dv = drag_value(commands, &fonts.ui, "", renzora_ember::theme::value_text(), init, speed.max(0.001));
            if max > min {
                commands.entity(dv).insert(DragRange { min, max });
            }
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_2way(
                commands,
                dv,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Float(*v)),
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
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_2way(
                commands,
                dv,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Float(v)) => v,
                    _ => 0.0,
                },
                move |w, v: &f32| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Float(*v)),
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
                let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
                let get_r = get_fn.clone();
                bind_2way(
                    commands,
                    dv,
                    move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                        Some(FieldValue::Vec3(a)) => a[i],
                        _ => 0.0,
                    },
                    move |w, v: &f32| {
                        if let Some(FieldValue::Vec3(mut a)) = get_fn(w, entity) {
                            a[i] = *v;
                            record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Vec3(a));
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
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_2way(
                commands,
                sw,
                move |w| matches!(tracked_read(w, entity, cid, |world| get_r(world, entity)), Some(FieldValue::Bool(true))),
                move |w, v: &bool| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Bool(*v)),
            );
            commands.entity(value_parent).add_child(sw);
        }
        FieldKind::Color => {
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            let editor = renzora_ember::inspector::color_field(
                commands,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Color(c)) => c,
                    _ => [0.0; 3],
                },
                move |w, rgb: [f32; 3]| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Color(rgb)),
            );
            commands.entity(value_parent).add_child(editor);
        }
        FieldKind::ColorRgba => {
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            let editor = renzora_ember::inspector::color_field_rgba(
                commands,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::ColorRgba(c)) => c,
                    _ => [0.0; 4],
                },
                move |w, rgba: [f32; 4]| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::ColorRgba(rgba)),
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
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            bind_text_input(
                commands,
                ti,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::String(s)) => s,
                    _ => String::new(),
                },
                move |w, v: String| record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::String(v)),
            );
            renzora_ember::inspector::fill_control(commands, ti);
            commands.entity(value_parent).add_child(ti);
        }
        FieldKind::Enum { options } => {
            // Use the shared ember `dropdown` (position-aware — flips up near a
            // panel/window bottom) rather than a bespoke inspector popup, so enum
            // fields get the same behaviour as every other dropdown.
            let refs: Vec<&str> = options.to_vec();
            let cur = if let FieldInit::Text(ref s) = field.init {
                s.clone()
            } else {
                String::new()
            };
            let sel = options.iter().position(|o| *o == cur).unwrap_or(0);
            let dd = dropdown(commands, fonts, &refs, sel);
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            // The dropdown works in option indices; the field stores an enum
            // string, so translate both ways.
            bind_2way(
                commands,
                dd,
                move |w| {
                    let cur = match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                        Some(FieldValue::Enum(s)) => s,
                        _ => String::new(),
                    };
                    options.iter().position(|o| *o == cur).unwrap_or(0)
                },
                move |w, i: &usize| {
                    if let Some(opt) = options.get(*i) {
                        record_field_change(
                            w,
                            entity,
                            name,
                            get_fn.clone(),
                            set_fn.clone(),
                            FieldValue::Enum((*opt).to_string()),
                        );
                    }
                },
            );
            renzora_ember::inspector::fill_control(commands, dd);
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
            let (get_fn, set_fn, name) = (field.get_fn.clone(), field.set_fn.clone(), field.name);
            let get_r = get_fn.clone();
            // The value is the selected index; two-way bind so a keyframed /
            // externally-changed index updates the shown option and vice versa.
            bind_2way(
                commands,
                dd,
                move |w| match tracked_read(w, entity, cid, |world| get_r(world, entity)) {
                    Some(FieldValue::Float(v)) => v.round().max(0.0) as usize,
                    _ => 0,
                },
                move |w, i: &usize| {
                    record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Float(*i as f32));
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
                field.get_fn.clone(),
                field.set_fn.clone(),
                field.extensions.clone(),
                field.create_fn.clone(),
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
                    set_fn: field.set_fn.clone(),
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
            // `ReadOnly` was the ONE field kind with no binding: its value was
            // formatted once at `collect_sections` time and baked into the `Text`.
            // It only *appeared* to stay fresh because the inspector's global
            // signature rebuilds the whole panel so often — accidental
            // reactivity, not real. Anything whose displayed value changes without
            // the component set changing (a mesh's vertex count, a camera's
            // computed projection, a resolved asset path) was already able to go
            // stale, and would freeze outright once rebuilds become granular.
            //
            // `ReadOnly` is also the catch-all arm of `#[derive(Inspectable)]`
            // (`renzora_macros/src/inspectable.rs`), so this is the common case for
            // any field type the derive can't infer — not a corner.
            //
            // One-way `bind_text`: there is no editing to conflict with, so unlike
            // the `bind_2way` fields there's no focus or in-progress drag to
            // destroy by writing to it.
            let get = field.get_fn.clone();
            bind_text(commands, t, move |w| format_value((get)(w.untracked(), entity).as_ref()));
            commands.entity(value_parent).add_child(t);
        }
    }
}

// ── Color editor (swatch + R/G/B popup) ──────────────────────────────────────

// ── Enum dropdown ────────────────────────────────────────────────────────────

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
    // Public signature deliberately still takes fn pointers — external drawers
    // have nothing to capture, and widening it would churn every caller.
    build_asset_field(
        commands,
        fonts,
        entity,
        "asset",
        std::sync::Arc::new(get_fn),
        std::sync::Arc::new(set_fn),
        extensions,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_asset_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    field_name: &'static str,
    get_fn: GetFn,
    set_fn: SetFn,
    extensions: Vec<String>,
    create_fn: Option<Mutate>,
) -> Entity {
    // One handle per `move` closure below: each takes ownership of what it
    // captures, so they cannot share a single binding.
    let get_r = get_fn.clone();
    let get_r2 = get_fn.clone();
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
        move |w| asset_display(get_r(w.untracked(), entity)),
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
                get_fn: get_fn.clone(),
                set_fn: set_fn.clone(),
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
                get_fn: get_fn.clone(),
                set_fn: set_fn.clone(),
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

    // Creatable fields get a "+" that authors a fresh asset in place (e.g. a
    // world-UI panel's template), so an empty field isn't a dead end — you don't
    // have to go make the file in the browser first and drag it back.
    if let Some(create_fn) = create_fn {
        let plus = commands
            .spawn((
                Text::new("\u{FF0B}"), // ＋ (full-width, reads as a button glyph)
                ui_font(&fonts.ui, 11.0),
                TextColor(c(renzora_ember::theme::text_muted())),
                Node {
                    padding: UiRect::horizontal(Val::Px(2.0)),
                    ..default()
                },
                Interaction::default(),
                AssetCreateBtn {
                    create_fn: create_fn.clone(),
                    get_fn: get_fn.clone(),
                    entity,
                },
                Name::new("asset-create"),
            ))
            .id();
        // Only offer "+" while the field is empty — once it points at an asset,
        // the ✕ clears it and drag-drop replaces it. Reactive so it hides the
        // instant a template is created/assigned.
        bind_with(
            commands,
            plus,
            move |w| matches!(get_r2(w.untracked(), entity), Some(FieldValue::Asset(Some(_)))),
            |w, e, has: &bool| {
                if let Some(mut node) = w.get_mut::<Node>(e) {
                    let want = if *has { Display::None } else { Display::Flex };
                    if node.display != want {
                        node.display = want;
                    }
                }
            },
        );
        commands.entity(row).add_child(plus);
    }
    row
}

/// The "+" on an [`FieldType::AssetCreatable`](renzora::FieldType) field. Runs
/// `create_fn` to author + assign the asset in place, but only when the field is
/// still empty — once it points at something, the ✕ clears and drag-drop replaces.
#[derive(Component)]
struct AssetCreateBtn {
    create_fn: Mutate,
    get_fn: GetFn,
    entity: Entity,
}

fn asset_create_click(
    q: Query<(&Interaction, &AssetCreateBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (create_fn, get_fn, entity) = (btn.create_fn.clone(), btn.get_fn.clone(), btn.entity);
        cmds.push(move |w: &mut World| {
            // Guard against a double-fire creating two files: skip if the field
            // already has a value (e.g. a race with drag-drop).
            if !matches!(get_fn(w, entity), Some(FieldValue::Asset(Some(_)))) {
                create_fn(w, entity);
            }
        });
    }
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
        let (get_fn, set_fn, entity, name) = (zone.get_fn.clone(), zone.set_fn.clone(), zone.entity, zone.field_name);
        cmds.push(move |w: &mut World| {
            record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Asset(Some(path_str.clone())))
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
        let (get_fn, set_fn, entity, name) = (btn.get_fn.clone(), btn.set_fn.clone(), btn.entity, btn.field_name);
        cmds.push(move |w: &mut World| {
            record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), FieldValue::Asset(None))
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
        let (remove_fn, entity, type_id) = (btn.remove_fn.clone(), btn.entity, btn.type_id);
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
    root: Query<&Children, With<InspectorRoot>>,
    sections: Query<&Children>,
    mut headers: Query<(&Section, &InspectorSectionHeader, &mut BackgroundColor)>,
) {
    // Derive the stripe index from the LIVE child order rather than a baked-in
    // position. Position is presentation, not identity — storing it on the header
    // made inserting a section near the top look like a content change for
    // everything below it, which would force needless rebuilds once the section
    // list is reconciled rather than rewritten.
    let Ok(children) = root.single() else {
        return;
    };
    for (i, section_root) in children.iter().enumerate() {
        // A section's header is its first child (see `build_section`).
        let Some(header) = sections.get(section_root).ok().and_then(|c| c.iter().next()) else {
            continue;
        };
        let Ok((sec, hdr, mut bg)) = headers.get_mut(header) else {
            continue;
        };
        let want = if sec.is_open() {
            renzora_ember::theme::rgb(hdr.header_bg)
        } else {
            renzora_ember::inspector::inspector_stripe(i)
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Persist a section's collapse state under its component type whenever the user
/// toggles it, so it survives rebuilds and follows the user across selections.
/// Mirrors `scripts.rs`'s `remember_script_sections`, keyed by type rather than
/// by `(entity, script_id)` — see [`InspectorSectionsOpen`].
fn remember_inspector_sections(
    changed: Query<(&Section, &InspectorSectionHeader), Changed<Section>>,
    mut open: ResMut<InspectorSectionsOpen>,
) {
    for (sec, hdr) in &changed {
        open.0.insert(hdr.type_id, sec.is_open());
    }
}

/// Keep the Inspector Expand Default setting authoritative.
///
/// Without this the setting is unreachable once the user has toggled anything:
/// remembered per-type state would always win, and simply clearing the map is not
/// enough either — a rebuild is not guaranteed, so the live sections would keep
/// their old state. Wipe the memory *and* drive the live `Section`s, the same way
/// `expand_all_click` does.
fn apply_expand_policy_change(
    settings: Res<EditorSettings>,
    mut open: ResMut<InspectorSectionsOpen>,
    mut sections: Query<(&mut Section, &InspectorSectionHeader)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
    mut last: Local<Option<InspectorExpandDefault>>,
) {
    let policy = settings.inspector_expand_default;
    if *last == Some(policy) {
        return;
    }
    // Skip the first observation: that is startup, not a user change, and the
    // sections built since already honour the policy.
    let first_run = last.is_none();
    *last = Some(policy);
    if first_run {
        return;
    }

    open.0.clear();
    for (mut sec, hdr) in &mut sections {
        let want = policy_open(policy, hdr.type_id);
        if sec.is_open() != want {
            set_section_open(&mut sec, want, &mut nodes, &mut texts);
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
        let (set_fn, entity) = (btn.set_fn.clone(), btn.entity);
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
        let (get_fn, set_fn, entity, name) = (btn.get_fn.clone(), btn.set_fn.clone(), btn.entity, btn.field_name);
        cmds.push(move |w: &mut World| {
            if let Some(cur) = get_fn(w, entity) {
                record_field_change(w, entity, name, get_fn.clone(), set_fn.clone(), cur.type_default());
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
                        // The Add Component overlay is fed purely from the
                        // hand-written registry, so these are still plain fn
                        // pointers; they coerce here.
                        add_fn: std::sync::Arc::new(add_fn),
                        remove_fn: remove_fn.map(|f| std::sync::Arc::new(f) as Mutate),
                    }),
                );
            },
        ));
    }

    // NOTE: Add Component is deliberately NOT fed from reflection.
    //
    // Inferring addability from `#[reflect(Default)]` was tried and reverted. It
    // is technically correct and practically useless: every ecosystem crate
    // registers its internals, so the menu filled with `AngularDamping`,
    // `CenterOfMass`, `ColliderConstructorHierarchy` — twice each, because
    // avian2d and avian3d both register a type of that name.
    //
    // The deeper reason is a vocabulary mismatch, not a filtering problem.
    // Reflection enumerates *components*; this menu offers *features*. One
    // feature ("Vignette") is a plugin that may own several components, and no
    // amount of per-component metadata reconstructs that grouping. Whatever
    // replaces the registry here has to be declared at plugin level.

    // Plugin components. Injected here rather than through `InspectorRegistry`
    // because `SearchEntry` takes a CLOSURE — so the component id can be captured
    // — whereas `InspectorEntry` is built from bare `fn` pointers that have
    // nowhere to put it.
    let plugin_specs: Vec<(String, bevy::ecs::component::ComponentId, Vec<u8>)> = world
        .get_resource::<renzora_plugin::host::PluginComponentSchemas>()
        .map(|s| {
            s.0.iter()
                // A resource is global — there is no entity to add it to.
                .filter(|i| !i.is_resource)
                .map(|i| (i.display_name.clone(), i.id, i.default_value.clone()))
                .collect()
        })
        .unwrap_or_default();

    for (label, component, default_value) in plugin_specs {
        // Already present — nothing to add.
        if world.get_entity(entity).is_ok_and(|e| e.contains_id(component)) {
            continue;
        }
        let default_value = if default_value.is_empty() {
            // The plugin supplied no default. Zeroed is the only option left, and
            // is at least a valid instance for any POD component.
            let size = world
                .components()
                .get_info(component)
                .map(|i| i.layout().size())
                .unwrap_or(0);
            vec![0u8; size]
        } else {
            default_value
        };
        entries.push(renzora_ember::widgets::SearchEntry::new(
            "puzzle-piece",
            &label,
            "plugin",
            move |w: &mut World| {
                let ctx = renzora_undo::active_context(w);
                renzora_undo::execute(
                    w,
                    ctx,
                    Box::new(AddPluginComponentCmd {
                        entity,
                        component,
                        default_value: default_value.clone(),
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

