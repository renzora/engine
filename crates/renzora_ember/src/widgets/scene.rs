//! Component front-ends for the builder widgets.
//!
//! Every widget in this module is an ordinary builder function — `dropdown`,
//! `table`, `timeline` — which is fine when you have a `Commands` and a
//! `&EmberFonts` in hand. It is not fine anywhere that describes UI as *data*: a
//! `.bsn` file, a scene, or a plugin on the far side of the C ABI. Those can
//! name a component; they cannot call a function.
//!
//! So each widget gets a component whose insert hook builds it. `EmberDropdown
//! { options: [..], selected: 1 }` in BSN produces exactly what
//! `dropdown(commands, fonts, &opts, 1)` produces, because it calls it.
//!
//! This is the shape `bevy_feathers` uses for the same reason — a feathers
//! button is a `Component`, not a constructor, so a scene can contain one.
//!
//! ## Why a hook rather than a system
//!
//! A hook runs the moment the component lands, so the widget exists before
//! anything else looks at the entity that frame. A system would leave a visible
//! one-frame gap and would need its own "not built yet" marker to avoid building
//! twice.

use crate::font::EmberFonts;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;

/// Marks an entity whose children this module owns, so a rebuild replaces them
/// rather than appending a second copy.
#[derive(Component)]
pub(crate) struct BuiltWidget;

/// Queue `build` to run against the entity once the world is free.
///
/// A hook only has a `DeferredWorld`, and every builder here wants `&mut
/// Commands` plus resources — so the work is queued as a command, which is also
/// what makes it safe to spawn children from inside an insert hook.
///
/// Generic over the component so its value is **cloned out before** `Commands`
/// borrows the world. Reading it afterwards would need a second `&mut World`
/// alongside the one the command queue holds, which is exactly the aliasing this
/// signature exists to avoid.
fn defer<C: Component + Clone>(
    mut world: DeferredWorld,
    ctx: HookContext,
    build: fn(&mut Commands, &EmberFonts, Entity, &C),
) {
    world.commands().queue(move |world: &mut World| {
        let Some(cfg) = world.get::<C>(ctx.entity).cloned() else {
            return;
        };
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
            warn!("ember: a widget component was inserted before fonts existed");
            return;
        };
        // Anything this module built before is replaced. Re-inserting the
        // component is how a caller asks for a rebuild.
        let rebuild = world.get::<BuiltWidget>(ctx.entity).is_some();

        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            if rebuild {
                commands.entity(ctx.entity).despawn_related::<Children>();
            }
            build(&mut commands, &fonts, ctx.entity, &cfg);
            commands.entity(ctx.entity).insert(BuiltWidget);
        }
        queue.apply(world);
    });
}

// ── Dropdown ─────────────────────────────────────────────────────────────────

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =dropdown_inserted)]
pub struct EmberDropdown {
    pub options: Vec<String>,
    pub selected: usize,
}

fn dropdown_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_dropdown);
}

fn build_dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    cfg: &EmberDropdown,
) {
    let opts: Vec<&str> = cfg.options.iter().map(String::as_str).collect();
    let child = super::dropdown(
        commands,
        fonts,
        &opts,
        cfg.selected.min(opts.len().saturating_sub(1)),
    );
    commands.entity(entity).add_child(child);
}

// ── Table ────────────────────────────────────────────────────────────────────

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =table_inserted)]
pub struct EmberTable {
    pub headers: Vec<String>,
    /// Row-major. A ragged row is drawn as-is rather than padded — the widget
    /// lays out what it is given, and silently inventing cells would hide a
    /// mistake in whatever produced the data.
    pub rows: Vec<Vec<String>>,
}

fn table_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_table);
}

fn build_table(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, cfg: &EmberTable) {
    // `&[&str]` all the way down, so every borrow has to outlive the call —
    // hence the staging vectors rather than a chain of temporaries.
    let headers: Vec<&str> = cfg.headers.iter().map(String::as_str).collect();
    let cells: Vec<Vec<&str>> = cfg
        .rows
        .iter()
        .map(|r| r.iter().map(String::as_str).collect())
        .collect();
    let rows: Vec<&[&str]> = cells.iter().map(Vec::as_slice).collect();
    let child = super::table(commands, &fonts.ui, &headers, &rows);
    commands.entity(entity).add_child(child);
}

// ── Timeline ─────────────────────────────────────────────────────────────────

/// One clip on a timeline track.
#[derive(Reflect, Clone, Default, Debug)]
pub struct EmberClip {
    pub start: f32,
    pub length: f32,
    pub label: String,
}

#[derive(Reflect, Clone, Default, Debug)]
pub struct EmberTrack {
    pub name: String,
    /// `(r, g, b)`. Left at black, a colour is picked from the palette by index.
    pub color: (u8, u8, u8),
    pub clips: Vec<EmberClip>,
}

#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =timeline_inserted)]
pub struct EmberTimeline {
    pub duration: f32,
    pub tracks: Vec<EmberTrack>,
}

impl Default for EmberTimeline {
    fn default() -> Self {
        Self {
            duration: 4.0,
            tracks: Vec::new(),
        }
    }
}

/// Cycled when a track leaves its colour at black, so a handful of tracks read
/// as separate lanes without every caller having to pick hues.
const TRACK_COLORS: [(u8, u8, u8); 5] = [
    (94, 160, 255),
    (120, 200, 140),
    (230, 170, 90),
    (200, 120, 200),
    (230, 120, 120),
];

fn timeline_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_timeline);
}

fn build_timeline(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    cfg: &EmberTimeline,
) {
    let clips: Vec<Vec<(f32, f32, &str)>> = cfg
        .tracks
        .iter()
        .map(|t| {
            t.clips
                .iter()
                .map(|c| (c.start, c.length, c.label.as_str()))
                .collect()
        })
        .collect();
    let tracks: Vec<super::Track> = cfg
        .tracks
        .iter()
        .zip(&clips)
        .enumerate()
        .map(|(i, (t, clips))| super::Track {
            name: t.name.as_str(),
            color: if t.color == (0, 0, 0) {
                TRACK_COLORS[i % TRACK_COLORS.len()]
            } else {
                t.color
            },
            lane: super::Lane::Clips(clips.as_slice()),
        })
        .collect();
    let child = super::timeline(commands, fonts, cfg.duration.max(0.1), &tracks);
    commands.entity(entity).add_child(child);
}

// ── Small ones ───────────────────────────────────────────────────────────────

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =checkbox_inserted)]
pub struct EmberCheckbox {
    pub checked: bool,
}

fn checkbox_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_checkbox);
}

fn build_checkbox(
    commands: &mut Commands,
    _fonts: &EmberFonts,
    entity: Entity,
    cfg: &EmberCheckbox,
) {
    let child = super::checkbox(commands, cfg.checked);
    commands.entity(entity).add_child(child);
}

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =progress_inserted)]
pub struct EmberProgress {
    /// 0..1.
    pub value: f32,
}

fn progress_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_progress);
}

fn build_progress(
    commands: &mut Commands,
    _fonts: &EmberFonts,
    entity: Entity,
    cfg: &EmberProgress,
) {
    let child = super::progress(commands, cfg.value);
    commands.entity(entity).add_child(child);
}

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =slider_inserted)]
pub struct EmberSliderWidget {
    /// 0..1. The declared range belongs to whatever binds the slider, not to the
    /// widget — the widget is a normalised track.
    pub value: f32,
}

fn slider_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_slider);
}

fn build_slider(
    commands: &mut Commands,
    _fonts: &EmberFonts,
    entity: Entity,
    cfg: &EmberSliderWidget,
) {
    let child = super::slider(commands, cfg.value);
    commands.entity(entity).add_child(child);
}

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =toggle_inserted)]
pub struct EmberToggle {
    pub on: bool,
}

fn toggle_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_toggle);
}

fn build_toggle(commands: &mut Commands, _fonts: &EmberFonts, entity: Entity, cfg: &EmberToggle) {
    let child = super::toggle_switch(commands, cfg.on);
    commands.entity(entity).add_child(child);
}

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =input_inserted)]
pub struct EmberInput {
    pub placeholder: String,
    pub value: String,
}

fn input_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_input);
}

fn build_input(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, cfg: &EmberInput) {
    let child = super::text_input(commands, &fonts.ui, &cfg.placeholder, &cfg.value);
    commands.entity(entity).add_child(child);
}

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =button_inserted)]
pub struct EmberButtonWidget {
    pub label: String,
}

fn button_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_button);
}

fn build_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entity: Entity,
    cfg: &EmberButtonWidget,
) {
    let child = super::button(commands, &fonts.ui, &cfg.label);
    commands.entity(entity).add_child(child);
}

#[derive(Component, Reflect, Clone, Default, Debug)]
#[reflect(Component, Default)]
// A UI entity without a `Node` does not participate in layout, and takes its
// children out with it — the widget builds correctly and renders nothing. Bevy's
// own `Text` requires `Node` for the same reason.
#[require(Node)]
#[component(on_insert =tabs_inserted)]
pub struct EmberTabs {
    pub labels: Vec<String>,
}

fn tabs_inserted(world: DeferredWorld, ctx: HookContext) {
    defer(world, ctx, build_tabs);
}

fn build_tabs(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, cfg: &EmberTabs) {
    let labels: Vec<&str> = cfg.labels.iter().map(String::as_str).collect();
    // One empty panel per label: the widget needs as many panels as tabs, and a
    // component has no way to describe their contents. Fill them by spawning
    // children under the returned panels.
    let panels: Vec<Entity> = labels.iter().map(|_| commands.spawn(Node::default()).id()).collect();
    let child = super::tabs(commands, &fonts.ui, &labels, panels);
    commands.entity(entity).add_child(child);
}

/// Register every widget component's type so BSN and scenes can name them.
pub(crate) fn register_types(app: &mut App) {
    app.register_type::<EmberDropdown>()
        .register_type::<EmberTable>()
        .register_type::<EmberTimeline>()
        .register_type::<EmberTrack>()
        .register_type::<EmberClip>()
        .register_type::<EmberCheckbox>()
        .register_type::<EmberProgress>()
        .register_type::<EmberSliderWidget>()
        .register_type::<EmberToggle>()
        .register_type::<EmberInput>()
        .register_type::<EmberButtonWidget>()
        .register_type::<EmberTabs>();
}
