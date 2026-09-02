//! The inspector panel: component sections for whatever is selected.
//!
//! Registry-driven: each `InspectorRegistry` entry shows when its `has_fn`
//! matches and renders either a registered bevy_ui drawer, declarative `fields`
//! (a `FieldType` + get/set fn-pointers, rendered generically in [`fields`]), or
//! a placeholder when it has neither.
//!
//! [`rebuild::rebuild_inspector`] (exclusive) rebuilds sections + rows whenever
//! the selection / locked entity / component set / add-overlay changes (hashed
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
//! Layout: component sections with a header (caret · icon · title · enable
//! toggle · trash) and field rows with a right-aligned label column + boxed
//! value, alternating row striping.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`undo`] | The four undo commands for add / remove / enable |
//! | [`spec`] | What a section and a field are, before either is built |
//! | [`collect`] | Reading the registry (and reflection) into those specs |
//! | [`rebuild`] | The signature, the exclusive rebuild, the top bar |
//! | [`section`] | Building one section, and filling its body on expand |
//! | [`cull`] | Throwing away the rows of sections scrolled off screen |
//! | [`fields`] | One field row per [`spec::FieldKind`] |
//! | [`assets`] | The asset drop target, shared with drawers in other crates |
//! | [`systems`] | Every click handler and the Add Component overlay |

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_editor_framework::{EditorSelection, FieldValue, InspectorExpandDefault};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::scroll_view;

pub(crate) mod assets;
pub(crate) mod collect;
pub(crate) mod cull;
pub(crate) mod fields;
pub(crate) mod rebuild;
pub(crate) mod section;
pub(crate) mod spec;
pub(crate) mod systems;
pub(crate) mod undo;

pub use assets::asset_drop_field;

// Boxed rather than bare `fn` pointers so a row's accessor can *capture* state.
// A hand-written `FieldDef` names its component statically and needs no capture,
// but a reflection-generated row (see [`crate::reflect_source`]) is parameterised
// by a type path + field path known only at runtime — there is no `fn` pointer
// that can carry those. `Arc<dyn Fn>` is `Clone + Send + Sync + 'static`, so it
// still lives in the widget marker Components that drive the edit handlers.
pub(crate) type GetFn = std::sync::Arc<dyn Fn(&World, Entity) -> Option<FieldValue> + Send + Sync>;
pub(crate) type SetFn = std::sync::Arc<dyn Fn(&mut World, Entity, FieldValue) + Send + Sync>;
// Boxed for the same reason as `GetFn`/`SetFn` above: a generated section's
// remove/enable actions are parameterised by a runtime type path.
pub(crate) type Pred = std::sync::Arc<dyn Fn(&World, Entity) -> bool + Send + Sync>;
pub(crate) type Mutate = std::sync::Arc<dyn Fn(&mut World, Entity) + Send + Sync>;
pub(crate) type SetEnabled = std::sync::Arc<dyn Fn(&mut World, Entity, bool) + Send + Sync>;

/// Apply a field edit through the undo system instead of calling `set_fn`
/// directly, so every inspector edit is undoable. Captures the pre-edit value
/// via `get_fn` (state still holds it at this instant) and records a
/// [`renzora_undo::FieldChangeCmd`] on whatever stack is currently active
/// ([`renzora_undo::active_context`]) — the focused document's, usually `Scene`.
///
/// Consecutive edits of the *same* field merge into one step (see
/// `FieldChangeCmd::merge`), so a drag-scrub that fires this every frame is a
/// single undo entry; `renzora_undo`'s gesture seal splits separate gestures.
pub(crate) fn record_field_change(
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

pub(crate) fn c(rgb: (u8, u8, u8)) -> Color {
    Color::srgb_u8(rgb.0, rgb.1, rgb.2)
}

#[derive(Component)]
pub(crate) struct InspectorRoot;

/// Marks the (stable, never-rebuilt) component-filter text input.
#[derive(Component)]
pub(crate) struct InspectorFilter;

#[derive(Resource, Default)]
pub(crate) struct InspectorState {
    pub(crate) sig: Option<u64>,
    pub(crate) locked: Option<Entity>,
    /// Lowercased component-name filter (empty = show all).
    pub(crate) filter: String,
}

/// Marks the inspector's expand/collapse-all button in the top bar.
#[derive(Component)]
pub(crate) struct ExpandAllButton;

/// Marks the glyph inside the expand/collapse-all button, so a sync system can
/// keep it showing "expand" vs "collapse" as sections open and close.
#[derive(Component)]
pub(crate) struct ExpandAllGlyph;

/// Marks an inspector component-section header, so the expand/collapse-all
/// button can drive just these sections (not other panels' sections). Carries
/// the section's list position and its category header colour so
/// [`systems::stripe_collapsed_headers`] can zebra-stripe collapsed headers and
/// restore the category colour when open.
#[derive(Component)]
pub(crate) struct InspectorSectionHeader {
    /// Which component this section is for — the key both
    /// [`InspectorSectionsOpen`] and the (future) keyed-list reconcile use.
    ///
    /// Deliberately *not* the section's list position. Position is presentation,
    /// not identity: baking it in made adding a component near the top look like
    /// a content change for every section below it.
    /// `systems::stripe_collapsed_headers` now derives the stripe index from the
    /// live child order instead.
    pub(crate) type_id: &'static str,
    pub(crate) header_bg: (u8, u8, u8),
}

/// Should a section start expanded under `policy`, ignoring any remembered state?
///
/// Keyed on `type_id`, not the display name — the two are easy to confuse (a
/// section's title is localized at render time), and matching on the display
/// string would silently stop working if a label were reworded. Shared by
/// `collect::collect_sections` and `systems::apply_expand_policy_change` so the
/// two can never drift.
pub(crate) fn policy_open(policy: InspectorExpandDefault, type_id: &str) -> bool {
    match policy {
        InspectorExpandDefault::AllOpen => true,
        InspectorExpandDefault::AllClosed => false,
        // No `name` here any more: the entity id is in the fixed header, which
        // is always visible and has nothing to expand.
        InspectorExpandDefault::Essentials => {
            matches!(type_id, "transform" | "script_component")
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
pub(crate) struct InspectorSectionsOpen(pub(crate) std::collections::HashMap<&'static str, bool>);

/// The entity the inspector is showing (the lock wins over the live selection).
pub(crate) fn inspected_entity(w: &Rx) -> Option<Entity> {
    let locked = w.get_resource::<InspectorState>().and_then(|s| s.locked);
    locked.or_else(|| w.get_resource::<EditorSelection>().and_then(|s| s.get()))
}

/// A Phosphor icon by *name* (resolved via ember's map).
pub(crate) fn phosphor_glyph(
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

pub(crate) fn empty_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
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

pub fn register(app: &mut App) {
    use renzora_editor_framework::SplashState;
    app.init_resource::<InspectorState>();
    app.init_resource::<InspectorSectionsOpen>();
    // Reflection-generated sections (see `collect::append_reflected_sections`).
    // Settable from the environment so the hand-written and generated renderings
    // of the same component can be compared without a settings-UI round trip:
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
    // `scroll: false` — we manage scrolling ourselves so the top bar (Add
    // Component + filter input + expand-all) stays *fixed* while only the
    // component list scrolls.
    app.register_panel_content("inspector", false, |commands, fonts| {
        // One column: fixed top bar, fixed entity header, scrolling list.
        let root = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Name::new("inspector-panel"),
            ))
            .id();
        // Fixed top bar: Add Component + component-filter input + expand-all.
        let top = rebuild::build_top_bar(commands, fonts);
        // Fixed entity header (icon · id · label colour · visibility · lock),
        // sitting directly on top of the component list it identifies.
        let entity_header = crate::entity_header::build_entity_header_host(commands);
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
        commands
            .entity(root)
            .add_children(&[top, entity_header, scroll]);
        root
    });
    app.add_systems(
        Update,
        (
            systems::remove_click,
            systems::add_button_click,
            systems::field_button_click,
            systems::reset_click,
            systems::add_keyframe_click,
            systems::lock_click,
            // Deliberately unordered against `screen_menu_dismiss`: an ordering
            // edge inserts an `ApplyDeferred` before dismiss, which flushes the
            // menu this system just queued and lets dismiss despawn it on the
            // frame it appeared — that once broke every right-click menu in the
            // editor. Dismiss skips freshly-added menus on its own.
            crate::entity_header::entity_icon_menu_open,
            crate::entity_header::entity_visibility_click,
            assets::asset_drop,
            assets::asset_clear_click,
            assets::asset_create_click,
            assets::asset_drop_highlight,
            rebuild::inspector_filter_sync,
            systems::expand_all_click,
            systems::sync_expand_glyph,
            systems::stripe_collapsed_headers,
            systems::remember_inspector_sections,
            systems::apply_expand_policy_change,
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
            cull::cull_offscreen_sections,
            rebuild::rebuild_inspector,
            section::reconcile_section_bodies,
        )
            .chain()
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
    );
}
