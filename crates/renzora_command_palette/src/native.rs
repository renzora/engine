//! Bevy-native (ember) command palette modal — the bevy_ui counterpart to the
//! egui `render_palette`. Same behavior: a dimmed backdrop + centered panel with
//! a search field and a scrollable, filterable list of every tool / action /
//! layout / panel / setting / menu command. Type to filter, ↑/↓ to navigate,
//! Enter to run, Esc or a backdrop click to dismiss.
//!
//! Shares [`CommandPaletteState`] with the egui version; each renders only under
//! its own [`EditorUiBackend`], so F10 swaps between them.

use std::sync::Arc;

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora::core::keybindings::KeyBindings;
use renzora::EditorUiBackend;
use renzora_editor::{EditorCommands, ShortcutRegistry, SplashState, ToolbarRegistry};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::bind_bg;
use renzora_ember::theme::{accent, border, hover_bg, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::widgets::{bind_text_input, scroll_area, text_input, EmberTextInput, OverlaySurface};

use crate::{collect_items, filter_items, CommandPaletteState};

type Handler = Arc<dyn Fn(&mut World) + Send + Sync>;

/// The current filtered handlers, indexed in parallel with the visible rows so
/// a click / Enter can run the right one.
#[derive(Resource, Default)]
struct PaletteHandlers(Vec<Handler>);

#[derive(Component)]
struct PaletteOverlay {
    sig: Option<u64>,
}
#[derive(Component)]
struct PaletteBackdrop;
#[derive(Component)]
struct PaletteResults;
#[derive(Component)]
struct PaletteSearch;
#[derive(Component)]
struct PaletteRow {
    index: usize,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<PaletteHandlers>();
    app.add_systems(
        Update,
        (
            manage_palette,
            rebuild_palette,
            focus_palette_search,
            palette_keys,
            palette_row_hover,
            palette_row_click,
            palette_backdrop_click,
        )
            .run_if(in_state(SplashState::Editor))
            .run_if(bevy_ui_backend),
    );
}

pub(crate) fn bevy_ui_backend(backend: Option<Res<EditorUiBackend>>) -> bool {
    backend.is_some_and(|b| b.is_bevy_ui())
}

pub(crate) fn egui_backend(backend: Option<Res<EditorUiBackend>>) -> bool {
    backend.is_none_or(|b| !b.is_bevy_ui())
}

fn sig_of(query: &str, len: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    query.hash(&mut h);
    len.hash(&mut h);
    h.finish()
}

/// Spawn the palette UI when it opens; tear it down when it closes.
fn manage_palette(world: &mut World) {
    let open = world.get_resource::<CommandPaletteState>().is_some_and(|s| s.open);
    let mut q = world.query_filtered::<Entity, With<PaletteOverlay>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if open && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_palette(&mut commands, &fonts);
        }
        queue.apply(world);
    } else if !open && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_palette(commands: &mut Commands, fonts: &EmberFonts) {
    // Dimmed full-screen backdrop — click outside the panel to dismiss.
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::top(Val::Percent(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.47)),
            GlobalZIndex(9500),
            FocusPolicy::Block,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlaySurface,
            PaletteOverlay { sig: None },
            PaletteBackdrop,
            Name::new("command-palette"),
        ))
        .id();

    // Panel.
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(560.0),
                max_width: Val::Percent(94.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("command-palette-panel"),
        ))
        .id();

    // Search field (bound to the shared query).
    let search = text_input(commands, &fonts.ui, "Search tools and actions…", "");
    commands.entity(search).insert((
        PaletteSearch,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(30.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));
    bind_text_input(
        commands,
        search,
        |w| w.get_resource::<CommandPaletteState>().map(|s| s.query.clone()).unwrap_or_default(),
        |w, s: String| {
            if let Some(mut st) = w.get_resource_mut::<CommandPaletteState>() {
                st.query = s;
                st.selected = 0;
            }
        },
    );

    let results = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() }, PaletteResults))
        .id();
    let scroll = scroll_area(commands, results, 360.0);

    commands.entity(panel).add_children(&[search, scroll]);
    commands.entity(backdrop).add_child(panel);
}

/// Auto-focus the search field the frame it spawns.
fn focus_palette_search(mut q: Query<&mut EmberTextInput, Added<PaletteSearch>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

/// Recompute the filtered item list each frame; rebuild the rows only when the
/// query (or item count) changes. Handlers are refreshed every frame so a click
/// always runs the current command.
fn rebuild_palette(world: &mut World) {
    if !world.get_resource::<CommandPaletteState>().is_some_and(|s| s.open) {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };

    let items = {
        let toolbar = world.resource::<ToolbarRegistry>().clone();
        let shortcuts = world.resource::<ShortcutRegistry>().clone();
        let keybindings = world.resource::<KeyBindings>().clone();
        let all = collect_items(&toolbar, &shortcuts, &keybindings, world);
        let query = world.resource::<CommandPaletteState>().query.clone();
        filter_items(all, &query)
    };

    // Clamp selection + refresh handlers.
    {
        let mut st = world.resource_mut::<CommandPaletteState>();
        if st.selected >= items.len().max(1) {
            st.selected = items.len().saturating_sub(1);
        }
    }
    world.resource_mut::<PaletteHandlers>().0 = items.iter().map(|i| i.handler.clone()).collect();

    let query = world.resource::<CommandPaletteState>().query.clone();
    let sig = sig_of(&query, items.len());
    let mut q = world.query::<(Entity, &PaletteOverlay)>();
    let Some((root, old_sig)) = q.iter(world).map(|(e, o)| (e, o.sig)).next() else { return };
    if old_sig == Some(sig) {
        return;
    }

    let Some(container) = ({
        let mut rq = world.query_filtered::<Entity, With<PaletteResults>>();
        rq.iter(world).next()
    }) else {
        return;
    };

    let existing: Vec<Entity> = world.get::<Children>(container).map(|c| c.iter().collect()).unwrap_or_default();
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for ch in existing {
            commands.entity(ch).despawn();
        }
        if items.is_empty() {
            let empty = commands
                .spawn(Node { width: Val::Percent(100.0), padding: UiRect::all(Val::Px(12.0)), justify_content: JustifyContent::Center, ..default() })
                .id();
            let t = commands.spawn((Text::new("No matches"), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_muted())))).id();
            commands.entity(empty).add_child(t);
            commands.entity(container).add_child(empty);
        } else {
            let rows: Vec<Entity> = items
                .iter()
                .enumerate()
                .map(|(i, it)| build_row(&mut commands, &fonts, i, it.kind, &it.label, it.detail.as_deref()))
                .collect();
            commands.entity(container).add_children(&rows);
        }
    }
    queue.apply(world);
    if let Some(mut o) = world.get_mut::<PaletteOverlay>(root) {
        o.sig = Some(sig);
    }
}

fn build_row(commands: &mut Commands, fonts: &EmberFonts, index: usize, kind: &str, label: &str, detail: Option<&str>) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            PaletteRow { index },
            Name::new("palette-row"),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        let sel = w.get_resource::<CommandPaletteState>().map(|s| s.selected == index).unwrap_or(false);
        if sel {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });

    let tag = commands
        .spawn((Text::new(kind.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(accent())), FocusPolicy::Pass))
        .id();
    let name = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), FocusPolicy::Pass))
        .id();
    let spacer = commands.spawn(Node { flex_grow: 1.0, min_width: Val::Px(8.0), ..default() }).id();
    let mut kids = vec![tag, name, spacer];
    if let Some(detail) = detail {
        let d = commands
            .spawn((Text::new(detail.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), FocusPolicy::Pass))
            .id();
        kids.push(d);
    }
    commands.entity(row).add_children(&kids);
    row
}

fn palette_keys(world: &mut World) {
    if !world.get_resource::<CommandPaletteState>().is_some_and(|s| s.open) {
        return;
    }
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let escape = keys.just_pressed(KeyCode::Escape);
    let up = keys.just_pressed(KeyCode::ArrowUp);
    let down = keys.just_pressed(KeyCode::ArrowDown);
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);

    if escape {
        world.resource_mut::<CommandPaletteState>().open = false;
        return;
    }
    let len = world.resource::<PaletteHandlers>().0.len();
    if len == 0 {
        return;
    }
    if up || down {
        let mut st = world.resource_mut::<CommandPaletteState>();
        st.selected = if up {
            (st.selected + len - 1) % len
        } else {
            (st.selected + 1) % len
        };
    }
    if enter {
        let idx = world.resource::<CommandPaletteState>().selected;
        run_palette_item(world, idx);
    }
}

fn palette_row_hover(q: Query<(&Interaction, &PaletteRow), Changed<Interaction>>, mut state: Option<ResMut<CommandPaletteState>>) {
    let Some(state) = state.as_mut() else { return };
    for (interaction, row) in &q {
        if *interaction == Interaction::Hovered {
            state.selected = row.index;
        }
    }
}

fn palette_row_click(q: Query<(&Interaction, &PaletteRow), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed {
            let idx = row.index;
            commands.queue(move |w: &mut World| run_palette_item(w, idx));
        }
    }
}

fn palette_backdrop_click(q: Query<&Interaction, (With<PaletteBackdrop>, Changed<Interaction>)>, mut state: Option<ResMut<CommandPaletteState>>) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.open = false;
    }
}

/// Close the palette and run the handler at `index` (deferred through
/// `EditorCommands` so it orders with other deferred work, like the egui path).
fn run_palette_item(world: &mut World, index: usize) {
    let handler = world.resource::<PaletteHandlers>().0.get(index).cloned();
    world.resource_mut::<CommandPaletteState>().open = false;
    let Some(handler) = handler else { return };
    if world.get_resource::<EditorCommands>().is_some() {
        world.resource::<EditorCommands>().push(move |w: &mut World| (handler)(w));
    } else {
        (handler)(world);
    }
}
