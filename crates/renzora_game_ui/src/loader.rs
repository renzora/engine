//! Persistent loader UI surface.
//!
//! Provides a plugin-owned UI layer that survives scene swaps — intended for loading
//! screens, fade overlays, and other UI that must exist before/after any scene is
//! resident. Consumers (e.g. the lifecycle graph) interact through `LoaderApi` without
//! knowing about UI entity details.

use bevy::prelude::*;

use renzora::{HideInHierarchy, Persistent, ScriptAction, ScriptActionValue};

use crate::components::{
    LoadingScreenData, UiCanvas, UiOpacity, UiWidget, UiWidgetType,
};

/// Marker for the single root of the persistent loader layer.
#[derive(Component, Default)]
pub struct LoaderRoot;

/// Marker for the currently-active loader widget.
#[derive(Component, Default)]
pub struct LoaderWidget;

/// Queued actions against the loader layer. Applied by `apply_loader_commands`
/// in an exclusive system so callers don't need `Commands` / `&mut World`.
enum LoaderCommand {
    ShowLoadingScreen,
    Hide,
    SetProgress(f32),
    SetMessage(String),
    Fade { from: f32, to: f32, seconds: f32 },
}

#[derive(Resource, Default)]
pub struct LoaderApi {
    queue: Vec<LoaderCommand>,
    /// Last reported progress (0..1). Read by graphs / UI for display.
    pub progress: f32,
}

impl LoaderApi {
    pub fn show_loading_screen(&mut self) {
        self.queue.push(LoaderCommand::ShowLoadingScreen);
    }
    pub fn hide(&mut self) {
        self.queue.push(LoaderCommand::Hide);
    }
    pub fn set_progress(&mut self, p: f32) {
        self.progress = p.clamp(0.0, 1.0);
        self.queue.push(LoaderCommand::SetProgress(self.progress));
    }
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.queue.push(LoaderCommand::SetMessage(msg.into()));
    }
    pub fn fade(&mut self, from: f32, to: f32, seconds: f32) {
        self.queue.push(LoaderCommand::Fade { from, to, seconds });
    }
}

#[derive(Component)]
struct FadeTween {
    from: f32,
    to: f32,
    duration: f32,
    elapsed: f32,
}

pub struct LoaderPlugin;

impl Plugin for LoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoaderApi>()
            .add_systems(Startup, spawn_loader_root)
            .add_systems(
                Update,
                (apply_loader_commands, tick_fade, enforce_canvas_visibility_mode),
            )
            .add_observer(handle_loader_script_actions);
    }
}

/// Observer: translates `ScriptAction`s (emitted by lifecycle graph / blueprints)
/// into calls on `LoaderApi`. Keeps callers decoupled from loader internals.
fn handle_loader_script_actions(trigger: On<ScriptAction>, mut api: ResMut<LoaderApi>) {
    let action = trigger.event();
    match action.name.as_str() {
        "loader_show" => api.show_loading_screen(),
        "loader_hide" => api.hide(),
        "loader_set_progress" => {
            if let Some(ScriptActionValue::Float(p)) = action.args.get("progress") {
                api.set_progress(*p);
            }
        }
        "loader_set_message" => {
            if let Some(ScriptActionValue::String(msg)) = action.args.get("message") {
                api.set_message(msg.clone());
            }
        }
        _ => {}
    }
}

fn spawn_loader_root(mut commands: Commands) {
    commands.spawn((
        Name::new("LoaderRoot"),
        LoaderRoot,
        Persistent,
        HideInHierarchy,
        UiCanvas {
            sort_order: 10_000,
            visibility_mode: "always".into(),
            reference_width: 1280.0,
            reference_height: 720.0,
        },
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        GlobalZIndex(10_000),
        Visibility::Hidden,
    ));
}

fn apply_loader_commands(world: &mut World) {
    let cmds = {
        let mut api = world.resource_mut::<LoaderApi>();
        if api.queue.is_empty() {
            return;
        }
        std::mem::take(&mut api.queue)
    };

    let root = match world
        .query_filtered::<Entity, With<LoaderRoot>>()
        .iter(world)
        .next()
    {
        Some(e) => e,
        None => return,
    };

    for cmd in cmds {
        match cmd {
            LoaderCommand::ShowLoadingScreen => show_loading_screen(world, root),
            LoaderCommand::Hide => hide_loader(world, root),
            LoaderCommand::SetProgress(p) => set_widget_progress(world, p),
            LoaderCommand::SetMessage(msg) => set_widget_message(world, msg),
            LoaderCommand::Fade { from, to, seconds } => start_fade(world, root, from, to, seconds),
        }
    }
}

fn show_loading_screen(world: &mut World, root: Entity) {
    clear_loader_children(world, root);

    let data = LoadingScreenData::default();
    let bg = data.bg_color;
    let bar_bg = data.bar_bg_color;
    let bar = data.bar_color;
    let text_color = data.text_color;

    let widget = world
        .spawn((
            Name::new("LoadingScreen"),
            LoaderWidget,
            UiWidget {
                widget_type: UiWidgetType::LoadingScreen,
                locked: true,
            },
            data,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(bg),
        ))
        .id();

    let bar_bg_entity = world
        .spawn((
            Name::new("ProgressBarBg"),
            Node {
                width: Val::Percent(50.0),
                height: Val::Px(6.0),
                position_type: PositionType::Absolute,
                top: Val::Percent(70.0),
                ..default()
            },
            BackgroundColor(bar_bg),
        ))
        .id();
    world.entity_mut(bar_bg_entity).set_parent_in_place(widget);

    let fill = world
        .spawn((
            Name::new("ProgressBarFill"),
            ProgressFill,
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(bar),
        ))
        .id();
    world.entity_mut(fill).set_parent_in_place(bar_bg_entity);

    let msg = world
        .spawn((
            Name::new("LoadingMessage"),
            LoadingMessage,
            bevy::ui::widget::Text::new("Loading…"),
            TextColor(text_color),
            TextFont {
                font_size: 24.0,
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(75.0),
                ..default()
            },
        ))
        .id();
    world.entity_mut(msg).set_parent_in_place(widget);

    world.entity_mut(widget).set_parent_in_place(root);
    world.entity_mut(root).insert(Visibility::Visible);
}

#[derive(Component)]
struct ProgressFill;

#[derive(Component)]
struct LoadingMessage;

fn clear_loader_children(world: &mut World, root: Entity) {
    let children: Vec<Entity> = world
        .get::<Children>(root)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    for child in children {
        if world.get_entity(child).is_ok() {
            world.despawn(child);
        }
    }
}

fn hide_loader(world: &mut World, root: Entity) {
    clear_loader_children(world, root);
    world.entity_mut(root).insert(Visibility::Hidden);
}

fn set_widget_progress(world: &mut World, p: f32) {
    let p = p.clamp(0.0, 1.0);
    let mut q = world.query_filtered::<&mut LoadingScreenData, With<LoaderWidget>>();
    if let Ok(mut data) = q.single_mut(world) {
        data.progress = p;
    }
    let mut fill_q = world.query_filtered::<&mut Node, With<ProgressFill>>();
    if let Ok(mut node) = fill_q.single_mut(world) {
        node.width = Val::Percent(p * 100.0);
    }
}

fn set_widget_message(world: &mut World, msg: String) {
    let mut data_q = world.query_filtered::<&mut LoadingScreenData, With<LoaderWidget>>();
    if let Ok(mut data) = data_q.single_mut(world) {
        data.message = msg.clone();
    }
    let mut text_q = world.query_filtered::<&mut bevy::ui::widget::Text, With<LoadingMessage>>();
    if let Ok(mut t) = text_q.single_mut(world) {
        t.0 = msg;
    }
}

fn start_fade(world: &mut World, root: Entity, from: f32, to: f32, seconds: f32) {
    world.entity_mut(root).insert((
        UiOpacity(from.clamp(0.0, 1.0)),
        FadeTween {
            from,
            to,
            duration: seconds.max(0.0),
            elapsed: 0.0,
        },
        Visibility::Visible,
    ));
}

fn tick_fade(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut FadeTween, &mut UiOpacity)>,
) {
    for (entity, mut tween, mut opacity) in &mut q {
        if tween.duration <= 0.0 {
            opacity.0 = tween.to;
            commands.entity(entity).remove::<FadeTween>();
            continue;
        }
        tween.elapsed += time.delta_secs();
        let t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);
        opacity.0 = tween.from + (tween.to - tween.from) * t;
        if t >= 1.0 {
            commands.entity(entity).remove::<FadeTween>();
        }
    }
}

/// Enforces `UiCanvas::visibility_mode` at runtime. Editor has its own sync system
/// behind the `editor` feature that overrides this; this one is a no-op there to
/// avoid fighting the editor's play-mode sync.
#[cfg(not(feature = "editor"))]
fn enforce_canvas_visibility_mode(
    mut canvases: Query<(&UiCanvas, &mut Visibility)>,
) {
    for (canvas, mut vis) in &mut canvases {
        let desired = match canvas.visibility_mode.as_str() {
            "editor_only" => Visibility::Hidden,
            _ => Visibility::Inherited,
        };
        if *vis != desired && *vis != Visibility::Hidden {
            // Don't stomp explicit Hidden (loader root uses it when idle).
            *vis = desired;
        }
    }
}

#[cfg(feature = "editor")]
fn enforce_canvas_visibility_mode() {
    // Editor build: the existing `sync_ui_canvas_visibility` system handles canvas visibility.
}
