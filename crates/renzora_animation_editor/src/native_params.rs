//! Bevy-native (ember) port of the egui `AnimatorParamsPanel`: the live animator
//! parameter editor (Unity Animator-window style).
//!
//! Shows the float / bool / trigger parameters from the selected entity's
//! [`AnimatorState::params`], grouped into FLOATS / BOOLS / TRIGGERS sections.
//! Each row is a labelled live editor — a scrubbable number field (floats), a
//! checkbox (bools) or a fire button (triggers) — plus a trash button to remove
//! it. A header "add new parameter" row (name input + kind dropdown + add
//! button) appends new params. Float/bool edits go through the same
//! [`AnimEditorBridge`] → `AnimationCommandQueue` path the egui panel used; add /
//! remove mutate the entity's [`AnimatorState`] component directly via
//! [`EditorCommands`], exactly like the egui version.
//!
//! Panel id: `animator_params`.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_animation::AnimatorState;
use renzora_editor_framework::{EditorCommands, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_display, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, checkbox, drag_value, menu_item, screen_menu, text_input,
};

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

const LABEL_W: f32 = 96.0;

#[derive(Default, Clone, Copy, PartialEq)]
enum ParamKind {
    #[default]
    Float,
    Bool,
    Trigger,
}

impl ParamKind {
    fn label(self) -> &'static str {
        match self {
            ParamKind::Float => "Float",
            ParamKind::Bool => "Bool",
            ParamKind::Trigger => "Trigger",
        }
    }
}

/// Scratch state for the "add new parameter" header row (mirrors the egui
/// panel's `new_param_state`).
#[derive(Resource, Default)]
struct NewParamScratch {
    name: String,
    kind: ParamKind,
}

pub struct NativeAnimParams;

impl Plugin for NativeAnimParams {
    fn build(&self, app: &mut App) {
        app.init_resource::<NewParamScratch>();
        app.register_panel_content("animator_params", true, build);
        app.add_systems(
            Update,
            (add_param_click, trash_click, fire_click, kind_combo_open)
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct AddParamBtn;
#[derive(Component)]
struct KindCombo;
#[derive(Component)]
struct TrashBtn {
    kind: ParamKind,
    name: String,
}
#[derive(Component)]
struct FireBtn {
    name: String,
}

// ── Accessors ────────────────────────────────────────────────────────────────

fn state(w: &World) -> Option<&AnimationEditorState> {
    w.get_resource::<AnimationEditorState>()
}

fn selected_animator(w: &World) -> Option<(Entity, &AnimatorState)> {
    let e = state(w)?.selected_entity?;
    let s = w.get::<AnimatorState>(e)?;
    Some((e, s))
}

/// Whether there is a selected entity with an `AnimatorState` (so the body
/// shows the editor vs the empty-state note).
fn ready(w: &World) -> bool {
    selected_animator(w).is_some()
}

fn empty_msg(w: &World) -> String {
    match state(w).and_then(|s| s.selected_entity) {
        None => "Select an entity with an animator".into(),
        Some(_) => "No AnimatorState on selected entity".into(),
    }
}

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("native-anim-params"),
        ))
        .id();

    // Empty state (no entity / no AnimatorState).
    let note = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            padding: UiRect::vertical(Val::Px(24.0)),
            ..default()
        })
        .id();
    let note_ic = icon_text(commands, &fonts.phosphor, "sliders", text_muted(), 22.0);
    let note_lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text_node(commands, note_lbl, empty_msg);
    commands.entity(note).add_children(&[note_ic, note_lbl]);
    bind_display(commands, note, |w| !ready(w));

    // Editor body (header add-row + parameter list).
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, body, ready);

    let header = build_add_row(commands, fonts);

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    keyed_list(commands, list, params_snapshot);

    commands.entity(body).add_children(&[header, list]);
    commands.entity(root).add_children(&[note, body]);
    root
}

/// The framed "New <name> <kind> [+]" header row.
fn build_add_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let frame = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();

    let lbl = commands
        .spawn((
            Text::new("New"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    // Name input → scratch.name
    let name_in = text_input(commands, &fonts.ui, "name", "");
    commands.entity(name_in).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    bind_text_input(
        commands,
        name_in,
        |w| {
            w.get_resource::<NewParamScratch>()
                .map(|s| s.name.clone())
                .unwrap_or_default()
        },
        |w, v| {
            if let Some(mut s) = w.get_resource_mut::<NewParamScratch>() {
                s.name = v;
            }
        },
    );

    // Kind combo (Float / Bool / Trigger).
    let combo = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            KindCombo,
        ))
        .id();
    let combo_v = commands
        .spawn((
            Text::new(ParamKind::default().label()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            Node {
                min_width: Val::Px(48.0),
                ..default()
            },
        ))
        .id();
    bind_text_node(commands, combo_v, |w| {
        w.get_resource::<NewParamScratch>()
            .map(|s| s.kind.label().to_string())
            .unwrap_or_default()
    });
    let combo_c = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[combo_v, combo_c]);

    // Add (+) button — enabled colour tracks whether the name is non-empty.
    let add_btn = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            AddParamBtn,
        ))
        .id();
    let add_ic = icon_text(commands, &fonts.phosphor, "plus", accent(), 12.0);
    renzora_ember::reactive::bind_text_color(commands, add_ic, |w| {
        let can = w
            .get_resource::<NewParamScratch>()
            .is_some_and(|s| !s.name.trim().is_empty());
        rgb(if can { accent() } else { text_muted() })
    });
    commands.entity(add_btn).add_child(add_ic);

    commands
        .entity(frame)
        .add_children(&[lbl, name_in, combo, add_btn]);
    frame
}

// ── Snapshot ─────────────────────────────────────────────────────────────────

/// One flattened row of the grouped parameter list.
enum Item {
    Header(&'static str),
    Float(String),
    Bool(String),
    Trigger(String),
    Empty,
}

fn params_snapshot(world: &World) -> KeyedSnapshot {
    let Some((_, st)) = selected_animator(world) else {
        return empty();
    };

    let mut floats: Vec<String> = st.params.floats.keys().cloned().collect();
    floats.sort();
    let mut bools: Vec<String> = st.params.bools.keys().cloned().collect();
    bools.sort();
    let mut triggers: Vec<String> = st.params.triggers.keys().cloned().collect();
    triggers.sort();

    let mut items_data: Vec<Item> = Vec::new();
    if !floats.is_empty() {
        items_data.push(Item::Header("FLOATS"));
        items_data.extend(floats.into_iter().map(Item::Float));
    }
    if !bools.is_empty() {
        items_data.push(Item::Header("BOOLS"));
        items_data.extend(bools.into_iter().map(Item::Bool));
    }
    if !triggers.is_empty() {
        items_data.push(Item::Header("TRIGGERS"));
        items_data.extend(triggers.into_iter().map(Item::Trigger));
    }
    if items_data.is_empty() {
        items_data.push(Item::Empty);
    }

    // Hash STRUCTURE only (name + kind discriminant), not the live values, so
    // editing a float/bool doesn't rebuild the row mid-edit.
    let items: Vec<(u64, u64)> = items_data
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match it {
                Item::Header(l) => (0u8, *l).hash(&mut h),
                Item::Float(n) => (1u8, n).hash(&mut h),
                Item::Bool(n) => (2u8, n).hash(&mut h),
                Item::Trigger(n) => (3u8, n).hash(&mut h),
                Item::Empty => 4u8.hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &items_data[i] {
            Item::Header(label) => section_header(c, f, label),
            Item::Float(name) => float_row(c, f, i, name),
            Item::Bool(name) => bool_row(c, f, i, name),
            Item::Trigger(name) => trigger_row(c, f, i, name),
            Item::Empty => empty_note(c, f),
        }),
    }
}

fn section_header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(20.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(row).add_child(lbl);
    row
}

fn row_base(commands: &mut Commands, idx: usize) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(renzora_ember::inspector::inspector_stripe(idx)),
        ))
        .id()
}

fn row_label(commands: &mut Commands, fonts: &EmberFonts, name: &str) -> Entity {
    commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::new_with_no_wrap(),
            Node {
                width: Val::Px(LABEL_W),
                flex_grow: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id()
}

fn float_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str) -> Entity {
    let row = row_base(commands, idx);
    let label = row_label(commands, fonts, name);

    // Seed 0.0 — `bind_2way` immediately reseeds from the live `AnimatorState`.
    // No `DragRange` is inserted: the egui panel used an unbounded `DragValue`.
    let dv = drag_value(commands, &fonts.ui, "", value_text(), 0.0, 0.05);
    {
        let n_get = name.to_string();
        let n_set = name.to_string();
        bind_2way(
            commands,
            dv,
            move |w| read_float(w, &n_get),
            move |w, v: &f32| {
                push_bridge(
                    w,
                    AnimEditorAction::SetParam {
                        name: n_set.clone(),
                        value: *v,
                    },
                );
            },
        );
    }

    let trash = trash_button(commands, fonts, ParamKind::Float, name);
    commands.entity(row).add_children(&[label, dv, trash]);
    row
}

fn bool_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str) -> Entity {
    let row = row_base(commands, idx);
    let label = row_label(commands, fonts, name);

    let cb = checkbox(commands, false);
    {
        let n_get = name.to_string();
        let n_set = name.to_string();
        bind_2way(
            commands,
            cb,
            move |w| read_bool(w, &n_get),
            move |w, v: &bool| {
                push_bridge(
                    w,
                    AnimEditorAction::SetBoolParam {
                        name: n_set.clone(),
                        value: *v,
                    },
                );
            },
        );
    }

    let trash = trash_button(commands, fonts, ParamKind::Bool, name);
    commands.entity(row).add_children(&[label, cb, trash]);
    row
}

fn trigger_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str) -> Entity {
    let row = row_base(commands, idx);
    let label = row_label(commands, fonts, name);

    let fire = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FireBtn {
                name: name.to_string(),
            },
        ))
        .id();
    let fire_ic = icon_text(commands, &fonts.phosphor, "lightning", accent(), 11.0);
    commands.entity(fire).add_child(fire_ic);

    let trash = trash_button(commands, fonts, ParamKind::Trigger, name);
    commands.entity(row).add_children(&[label, fire, trash]);
    row
}

fn trash_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: ParamKind,
    name: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            TrashBtn {
                kind,
                name: name.to_string(),
            },
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 10.0);
    commands.entity(btn).add_child(ic);
    btn
}

fn empty_note(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    commands
        .spawn((
            Text::new("No parameters. Add one above."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::all(Val::Px(8.0)),
                ..default()
            },
        ))
        .id()
}

// ── State helpers ────────────────────────────────────────────────────────────

fn read_float(w: &World, name: &str) -> f32 {
    selected_animator(w)
        .and_then(|(_, s)| s.params.floats.get(name).copied())
        .unwrap_or(0.0)
}

fn read_bool(w: &World, name: &str) -> bool {
    selected_animator(w)
        .and_then(|(_, s)| s.params.bools.get(name).copied())
        .unwrap_or(false)
}

fn push_bridge(w: &mut World, action: AnimEditorAction) {
    if let Some(bridge) = w.get_resource::<AnimEditorBridge>() {
        if let Ok(mut p) = bridge.pending.lock() {
            p.push(action);
        }
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

/// Bind plain `Text` content reactively (no dedicated ember helper takes a
/// closure returning `String` for a bare node label, so use `bind_text`).
fn bind_text_node(
    commands: &mut Commands,
    target: Entity,
    get: impl Fn(&World) -> String + Send + Sync + 'static,
) {
    renzora_ember::reactive::bind_text(commands, target, get);
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn add_param_click(
    q: Query<&Interaction, (With<AddParamBtn>, Changed<Interaction>)>,
    scratch: Option<ResMut<NewParamScratch>>,
    state: Option<Res<AnimationEditorState>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let (Some(mut scratch), Some(state), Some(cmds)) = (scratch, state, cmds) else {
        return;
    };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let name = scratch.name.trim().to_string();
    if name.is_empty() {
        return;
    }
    let Some(entity) = state.selected_entity else {
        return;
    };
    let kind = scratch.kind;
    scratch.name.clear();
    cmds.push(move |world: &mut World| {
        let Some(mut s) = world.get_mut::<AnimatorState>(entity) else {
            return;
        };
        match kind {
            ParamKind::Float => {
                s.params.floats.entry(name).or_insert(0.0);
            }
            ParamKind::Bool => {
                s.params.bools.entry(name).or_insert(false);
            }
            ParamKind::Trigger => {
                s.params.triggers.entry(name).or_insert(false);
            }
        }
    });
}

fn trash_click(
    q: Query<(&Interaction, &TrashBtn), Changed<Interaction>>,
    state: Option<Res<AnimationEditorState>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let (Some(state), Some(cmds)) = (state, cmds) else {
        return;
    };
    let Some(entity) = state.selected_entity else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let kind = btn.kind;
        let name = btn.name.clone();
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_mut::<AnimatorState>(entity) {
                match kind {
                    ParamKind::Float => {
                        s.params.floats.remove(&name);
                    }
                    ParamKind::Bool => {
                        s.params.bools.remove(&name);
                    }
                    ParamKind::Trigger => {
                        s.params.triggers.remove(&name);
                    }
                }
            }
        });
    }
}

fn fire_click(
    q: Query<(&Interaction, &FireBtn), Changed<Interaction>>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let Some(bridge) = bridge else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok(mut p) = bridge.pending.lock() {
            p.push(AnimEditorAction::FireTrigger {
                name: btn.name.clone(),
            });
        }
    }
}

fn kind_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (With<KindCombo>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kinds = [ParamKind::Float, ParamKind::Bool, ParamKind::Trigger];
    let kids: Vec<Entity> = kinds
        .iter()
        .map(|&kind| {
            menu_item(&mut commands, &fonts, "dot", kind.label(), move |w| {
                if let Some(mut s) = w.get_resource_mut::<NewParamScratch>() {
                    s.kind = kind;
                }
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}
