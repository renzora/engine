//! Bevy-native (ember) port of the egui `StateMachinePanel` (panel id
//! `animator_state_machine`, titled "State Machine"): the form-based editor for
//! `.animsm` files — a list of states (name + clip + speed + loop + entry star)
//! and a list of transitions (from → to + blend + condition).
//!
//! The egui panel is *disk-backed*: it loads the `.animsm` referenced by the
//! selected entity's [`AnimatorComponent::state_machine`] into a local
//! `SmBuffer`, mutates that buffer in place as the user edits, and flushes it to
//! disk on Save. The native port keeps that exact model but the buffer lives in
//! a [`SmEditorState`] resource instead of an `Arc<Mutex<_>>`, so the reactive
//! closures (`bind_2way` / `bind_text_input` / `bind_text`) can read it and the
//! click systems can mutate it via [`EditorCommands`]. A `load_state_machine`
//! system replicates the egui `ensure_loaded` (idempotent disk read keyed on the
//! resolved path); the Save button writes RON back, exactly like the egui panel.
//!
//! Two columns (States | Transitions), each with an Add button. The keyed lists
//! are keyed on *structure* (state count + names, transition count + condition
//! kind) so editing a value in place never rebuilds the row mid-edit.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_animation::{
    state_machine::{AnimCondition, AnimState, AnimTransition, AnimationStateMachine, StateMotion},
    AnimatorComponent,
};
use renzora_editor_framework::{EditorCommands, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_2way, bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, checkbox, drag_value, menu_item, screen_menu, text_input, DragRange,
};

use crate::AnimationEditorState;

const ERR_COLOR: (u8, u8, u8) = (220, 100, 100);

// ── Editor buffer (resource) ─────────────────────────────────────────────────

/// Local editor buffer mirroring the on-disk `.animsm` while editing — the
/// native analogue of the egui panel's `Arc<Mutex<SmBuffer>>`. Flushed to disk
/// on Save.
#[derive(Resource)]
struct SmEditorState {
    /// Absolute path of the loaded `.animsm` (the key `ensure_loaded` dedupes on).
    loaded_for: Option<PathBuf>,
    path: Option<PathBuf>,
    sm: AnimationStateMachine,
    dirty: bool,
    error: Option<String>,
}

/// A fresh empty state machine (`AnimationStateMachine` lives in another crate
/// and has no `Default`).
fn fresh_sm() -> AnimationStateMachine {
    AnimationStateMachine {
        states: Vec::new(),
        transitions: Vec::new(),
        default_state: String::new(),
    }
}

pub struct NativeStateMachine;

impl Plugin for NativeStateMachine {
    fn build(&self, app: &mut App) {
        app.insert_resource(SmEditorState {
            loaded_for: None,
            path: None,
            sm: fresh_sm(),
            dirty: false,
            error: None,
        });
        app.register_panel_content("animator_state_machine", true, build);
        app.add_systems(
            Update,
            (
                load_state_machine,
                save_click,
                add_state_click,
                add_transition_click,
                star_click,
                state_clip_combo_open,
                trans_from_combo_open,
                trans_to_combo_open,
                cond_kind_combo_open,
                trash_state_click,
                trash_transition_click,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct SaveBtn;
#[derive(Component)]
struct AddStateBtn;
#[derive(Component)]
struct AddTransitionBtn;
#[derive(Component)]
struct StarBtn(String);
#[derive(Component)]
struct TrashStateBtn(usize);
#[derive(Component)]
struct TrashTransitionBtn(usize);
#[derive(Component)]
struct StateClipCombo(usize);
#[derive(Component)]
struct TransFromCombo(usize);
#[derive(Component)]
struct TransToCombo(usize);
#[derive(Component)]
struct CondKindCombo(usize);

// ── Accessors ────────────────────────────────────────────────────────────────

fn editor_state(w: &World) -> Option<&AnimationEditorState> {
    w.get_resource::<AnimationEditorState>()
}

fn selected_entity(w: &World) -> Option<Entity> {
    editor_state(w)?.selected_entity
}

fn animator(w: &World) -> Option<&AnimatorComponent> {
    let e = selected_entity(w)?;
    w.get::<AnimatorComponent>(e)
}

fn sm(w: &World) -> Option<&SmEditorState> {
    w.get_resource::<SmEditorState>()
}

/// Whether the selected entity has an animator with an `.animsm` path assigned —
/// gates the editor body vs the empty-state note (mirrors the egui guard chain).
fn ready(w: &World) -> bool {
    selected_entity(w).is_some()
        && animator(w).is_some()
        && animator(w).is_some_and(|a| a.state_machine.is_some())
        && project_root(w).is_some()
}

fn project_root(w: &World) -> Option<PathBuf> {
    w.get_resource::<renzora::CurrentProject>().map(|p| p.path.clone())
}

fn empty_msg(w: &World) -> String {
    match selected_entity(w) {
        None => renzora::lang::t("animation.select_animated_entity"),
        Some(e) => match w.get::<AnimatorComponent>(e) {
            None => renzora::lang::t("animation.no_animator"),
            Some(a) if a.clips.is_empty() => {
                renzora::lang::t("animation.no_clips_scan_first")
            }
            Some(a) if a.state_machine.is_none() => {
                renzora::lang::t("animation.sm_description")
            }
            Some(_) if project_root(w).is_none() => renzora::lang::t("animation.no_project_open"),
            Some(_) => String::new(),
        },
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
                row_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("native-state-machine"),
        ))
        .id();

    // Empty state.
    let note = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            padding: UiRect::vertical(Val::Px(28.0)),
            ..default()
        })
        .id();
    let note_ic = icon_text(commands, &fonts.phosphor, "graph", text_muted(), 24.0);
    let note_lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::justify(bevy::text::Justify::Center),
        ))
        .id();
    bind_text(commands, note_lbl, empty_msg);
    // One-click starter: writes a `.animsm` (one state per clip) next to the
    // clips and assigns it on the animator; `load_state_machine` then opens it.
    let create_btn = crate::setup::action_button(
        commands,
        fonts,
        "plus-circle",
        &renzora::lang::t("animation.create_state_machine"),
        crate::setup::CreateSmBtn,
    );
    bind_display(commands, create_btn, crate::setup::can_create_sm);
    let feedback = crate::setup::feedback_label(commands, fonts);
    commands
        .entity(note)
        .add_children(&[note_ic, note_lbl, create_btn, feedback]);
    bind_display(commands, note, |w| !ready(w));

    // Editor body.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    bind_display(commands, body, ready);

    let header = build_header(commands, fonts);
    let error = build_error_row(commands, fonts);

    // Two columns: States | Transitions.
    let cols = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            align_items: AlignItems::FlexStart,
            ..default()
        })
        .id();
    let states_col = build_states_column(commands, fonts);
    let trans_col = build_transitions_column(commands, fonts);
    commands.entity(cols).add_children(&[states_col, trans_col]);

    commands.entity(body).add_children(&[header, error, cols]);
    commands.entity(root).add_children(&[note, body]);
    root
}

/// Framed header: `.animsm` path on the left, Save button on the right.
fn build_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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

    let file_ic = icon_text(commands, &fonts.phosphor, "file", text_muted(), 12.0);
    let path_lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    bind_text(commands, path_lbl, |w| {
        animator(w)
            .and_then(|a| a.state_machine.clone())
            .unwrap_or_else(|| renzora::lang::t("animation.no_animsm_assigned"))
    });
    bind_text_color(commands, path_lbl, |w| {
        let has = animator(w).is_some_and(|a| a.state_machine.is_some());
        rgb(if has { text_primary() } else { text_muted() })
    });

    // Save button — accent when the buffer is dirty, muted otherwise.
    let save_btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            SaveBtn,
        ))
        .id();
    let save_ic = icon_text(commands, &fonts.phosphor, "floppy-disk", text_muted(), 11.0);
    let save_lbl = commands
        .spawn((
            Text::new(renzora::lang::t("common.save")),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text_color(commands, save_ic, dirty_color);
    bind_text_color(commands, save_lbl, dirty_color);
    commands.entity(save_btn).add_children(&[save_ic, save_lbl]);

    commands
        .entity(frame)
        .add_children(&[file_ic, path_lbl, save_btn]);
    frame
}

fn dirty_color(w: &World) -> Color {
    let dirty = sm(w).is_some_and(|s| s.dirty);
    rgb(if dirty { accent() } else { text_muted() })
}

/// A row that only shows when the buffer has a parse/save error.
fn build_error_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(ERR_COLOR)),
        ))
        .id();
    bind_text(commands, lbl, |w| {
        sm(w).and_then(|s| s.error.clone()).unwrap_or_default()
    });
    bind_display(commands, lbl, |w| {
        sm(w).is_some_and(|s| s.error.is_some())
    });
    lbl
}

// ── States column ────────────────────────────────────────────────────────────

fn build_states_column(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(50.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();

    let heading = section_heading(commands, fonts, &renzora::lang::t("animation.states"));
    let add = add_button(commands, fonts, &renzora::lang::t("animation.add_state"), AddStateBtn, |_w| true);

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, states_snapshot);

    commands.entity(col).add_children(&[heading, add, list]);
    col
}

fn states_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = sm(world) else {
        return empty();
    };
    // (name, default?) keyed on structure: index + name + is_default.
    let rows: Vec<(String, bool)> = state
        .sm
        .states
        .iter()
        .map(|s| (s.name.clone(), state.sm.default_state == s.name))
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, (name, is_def))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (name, is_def).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(state_row),
    }
}

fn state_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize) -> Entity {
    let block = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();

    // Row 1: ★ entry, name input, clip combo, trash.
    let r1 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();

    // Entry-state star button.
    let star = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            StarBtn(String::new()), // filled in after we know the name (via system reading idx)
        ))
        .id();
    let star_ic = icon_text(commands, &fonts.phosphor, "star", text_muted(), 11.0);
    {
        let i = idx;
        bind_text_color(commands, star_ic, move |w| {
            let is_def = sm(w).is_some_and(|s| {
                s.sm
                    .states
                    .get(i)
                    .is_some_and(|st| s.sm.default_state == st.name)
            });
            rgb(if is_def { accent() } else { text_muted() })
        });
    }
    // Resolve the star's target name reactively (state names can be renamed).
    {
        let i = idx;
        renzora_ember::reactive::bind_with(
            commands,
            star,
            move |w| {
                sm(w)
                    .and_then(|s| s.sm.states.get(i))
                    .map(|st| st.name.clone())
                    .unwrap_or_default()
            },
            move |world, target, name: &String| {
                if let Some(mut b) = world.get_mut::<StarBtn>(target) {
                    b.0 = name.clone();
                }
            },
        );
    }
    commands.entity(star).add_child(star_ic);

    // Name input → state.name.
    let name_in = text_input(commands, &fonts.ui, "name", "");
    commands.entity(name_in).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(60.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    bind_text_input(
        commands,
        name_in,
        {
            let i = idx;
            move |w| {
                sm(w)
                    .and_then(|s| s.sm.states.get(i))
                    .map(|st| st.name.clone())
                    .unwrap_or_default()
            }
        },
        {
            let i = idx;
            move |w, v| {
                set_state(w, i, move |st| st.name = v.clone());
            }
        },
    );

    // Clip combo (pick the motion clip).
    let clip_combo = combo_box(commands, fonts, StateClipCombo(idx), {
        let i = idx;
        move |w| {
            sm(w)
                .and_then(|s| s.sm.states.get(i))
                .map(|st| match &st.motion {
                    StateMotion::Clip(n) | StateMotion::BlendTree(n) => n.clone(),
                })
                .unwrap_or_default()
        }
    });

    let trash = trash_button(commands, fonts, TrashStateBtn(idx));

    commands
        .entity(r1)
        .add_children(&[star, name_in, clip_combo, trash]);

    // Row 2: speed drag + loop checkbox.
    let r2 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let speed_lbl = small_label(commands, fonts, "speed");
    let speed_dv = drag_value(commands, &fonts.ui, "", value_text(), 1.0, 0.05);
    commands
        .entity(speed_dv)
        .insert(DragRange { min: 0.05, max: 5.0 });
    bind_2way(
        commands,
        speed_dv,
        {
            let i = idx;
            move |w| sm(w).and_then(|s| s.sm.states.get(i)).map(|st| st.speed).unwrap_or(1.0)
        },
        {
            let i = idx;
            move |w, v: &f32| {
                let v = *v;
                set_state(w, i, move |st| st.speed = v);
            }
        },
    );
    let loop_cb = checkbox(commands, true);
    bind_2way(
        commands,
        loop_cb,
        {
            let i = idx;
            move |w| sm(w).and_then(|s| s.sm.states.get(i)).map(|st| st.looping).unwrap_or(true)
        },
        {
            let i = idx;
            move |w, v: &bool| {
                let v = *v;
                set_state(w, i, move |st| st.looping = v);
            }
        },
    );
    let loop_lbl = small_label(commands, fonts, "loop");
    commands
        .entity(r2)
        .add_children(&[speed_lbl, speed_dv, loop_cb, loop_lbl]);

    commands.entity(block).add_children(&[r1, r2]);
    block
}

// ── Transitions column ───────────────────────────────────────────────────────

fn build_transitions_column(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(50.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();

    let heading = section_heading(commands, fonts, &renzora::lang::t("animation.transitions"));
    // Add enabled only when there is at least one state.
    let add = add_button(commands, fonts, &renzora::lang::t("animation.add_transition"), AddTransitionBtn, |w| {
        sm(w).is_some_and(|s| !s.sm.states.is_empty())
    });

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, transitions_snapshot);

    commands.entity(col).add_children(&[heading, add, list]);
    col
}

fn transitions_snapshot(world: &World) -> KeyedSnapshot {
    let Some(state) = sm(world) else {
        return empty();
    };
    // Key on index + condition discriminant so swapping the condition *kind*
    // rebuilds the row (different editor widgets), but editing from/to/values
    // in place does not.
    let kinds: Vec<u8> = state
        .sm
        .transitions
        .iter()
        .map(|t| cond_discriminant(&t.condition))
        .collect();
    let items: Vec<(u64, u64)> = kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            kind.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| transition_row(c, f, i, shape_of(kinds[i]))),
    }
}

fn transition_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    idx: usize,
    shape: CondShape,
) -> Entity {
    let block = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(inspector_stripe(idx)),
        ))
        .id();

    // Row 1: from → to + trash.
    let r1 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let from = combo_box(commands, fonts, TransFromCombo(idx), {
        let i = idx;
        move |w| {
            sm(w)
                .and_then(|s| s.sm.transitions.get(i))
                .map(|t| t.from.clone())
                .unwrap_or_default()
        }
    });
    let arrow = icon_text(commands, &fonts.phosphor, "arrow-right", text_muted(), 11.0);
    let to = combo_box(commands, fonts, TransToCombo(idx), {
        let i = idx;
        move |w| {
            sm(w)
                .and_then(|s| s.sm.transitions.get(i))
                .map(|t| t.to.clone())
                .unwrap_or_default()
        }
    });
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let trash = trash_button(commands, fonts, TrashTransitionBtn(idx));
    commands
        .entity(r1)
        .add_children(&[from, arrow, to, gap, trash]);

    // Row 2: blend duration.
    let r2 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let blend_lbl = small_label(commands, fonts, "blend");
    let blend_dv = drag_value(commands, &fonts.ui, "", value_text(), 0.2, 0.02);
    commands
        .entity(blend_dv)
        .insert(DragRange { min: 0.0, max: 2.0 });
    bind_2way(
        commands,
        blend_dv,
        {
            let i = idx;
            move |w| {
                sm(w)
                    .and_then(|s| s.sm.transitions.get(i))
                    .map(|t| t.blend_duration)
                    .unwrap_or(0.2)
            }
        },
        {
            let i = idx;
            move |w, v: &f32| {
                let v = *v;
                set_transition(w, i, move |t| t.blend_duration = v);
            }
        },
    );
    commands.entity(r2).add_children(&[blend_lbl, blend_dv]);

    // Row 3: condition editor (when <kind> [params]).
    let r3 = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let when_lbl = small_label(commands, fonts, "when");
    let kind_combo = combo_box(commands, fonts, CondKindCombo(idx), {
        let i = idx;
        move |w| {
            sm(w)
                .and_then(|s| s.sm.transitions.get(i))
                .map(|t| cond_label_tr(cond_kind_label(&t.condition)))
                .unwrap_or_default()
        }
    });
    let mut kids = vec![when_lbl, kind_combo];
    kids.extend(condition_param_widgets(commands, fonts, idx, shape));
    commands.entity(r3).add_children(&kids);

    commands.entity(block).add_children(&[r1, r2, r3]);
    block
}

/// Build the parameter editor(s) for a transition's condition — depends on the
/// condition kind (string param input, float threshold, time). Built once per
/// row; the row is rebuilt by `keyed_list` when the kind changes.
fn condition_param_widgets(
    commands: &mut Commands,
    fonts: &EmberFonts,
    idx: usize,
    shape: CondShape,
) -> Vec<Entity> {
    match shape {
        CondShape::FloatCompare => {
            let name = cond_param_input(commands, fonts, idx);
            let dv = drag_value(commands, &fonts.ui, "", value_text(), 0.0, 0.05);
            bind_2way(
                commands,
                dv,
                {
                    let i = idx;
                    move |w| cond_float(w, i)
                },
                {
                    let i = idx;
                    move |w, v: &f32| {
                        let v = *v;
                        set_transition(w, i, move |t| set_cond_float(&mut t.condition, v));
                    }
                },
            );
            vec![name, dv]
        }
        CondShape::Param => {
            vec![cond_param_input(commands, fonts, idx)]
        }
        CondShape::Time => {
            let dv = drag_value(commands, &fonts.ui, "", value_text(), 0.0, 0.05);
            commands.entity(dv).insert(DragRange { min: 0.0, max: 60.0 });
            bind_2way(
                commands,
                dv,
                {
                    let i = idx;
                    move |w| cond_float(w, i)
                },
                {
                    let i = idx;
                    move |w, v: &f32| {
                        let v = *v;
                        set_transition(w, i, move |t| set_cond_float(&mut t.condition, v));
                    }
                },
            );
            vec![dv]
        }
        CondShape::None => Vec::new(),
    }
}

/// The "param" text input shared by Float/Bool/Trigger conditions.
fn cond_param_input(commands: &mut Commands, fonts: &EmberFonts, idx: usize) -> Entity {
    let input = text_input(commands, &fonts.ui, "param", "");
    commands.entity(input).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(50.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    bind_text_input(
        commands,
        input,
        {
            let i = idx;
            move |w| cond_param_name(w, i)
        },
        {
            let i = idx;
            move |w, v| {
                set_transition(w, i, move |t| set_cond_param_name(&mut t.condition, &v));
            }
        },
    );
    input
}

// ── Shared widget helpers ────────────────────────────────────────────────────

fn section_heading(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id()
}

fn small_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id()
}

/// A framed "+ <label>" add button whose colour tracks an `enabled` predicate.
fn add_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    marker: impl Component,
    enabled: fn(&World) -> bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                align_self: AlignSelf::FlexStart,
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            marker,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "plus", accent(), 11.0);
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(accent())),
        ))
        .id();
    bind_text_color(commands, ic, move |w| {
        rgb(if enabled(w) { accent() } else { text_muted() })
    });
    bind_text_color(commands, lbl, move |w| {
        rgb(if enabled(w) { accent() } else { text_muted() })
    });
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

fn trash_button(commands: &mut Commands, fonts: &EmberFonts, marker: impl Component) -> Entity {
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
            marker,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 10.0);
    commands.entity(btn).add_child(ic);
    btn
}

/// A combo trigger (selected text + caret) carrying `marker`; its open systems
/// spawn the popup menu. `get` drives the displayed selection reactively.
fn combo_box(
    commands: &mut Commands,
    fonts: &EmberFonts,
    marker: impl Component,
    get: impl Fn(&World) -> String + Send + Sync + 'static,
) -> Entity {
    let combo = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                min_width: Val::Px(60.0),
                flex_shrink: 1.0,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            marker,
        ))
        .id();
    let val = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    bind_text(commands, val, get);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[val, caret]);
    combo
}

// ── Condition shape / accessors ──────────────────────────────────────────────

#[derive(PartialEq)]
enum CondShape {
    FloatCompare,
    Param,
    Time,
    None,
}

fn cond_discriminant(c: &AnimCondition) -> u8 {
    match c {
        AnimCondition::FloatGreater(_, _) => 0,
        AnimCondition::FloatLess(_, _) => 1,
        AnimCondition::BoolTrue(_) => 2,
        AnimCondition::BoolFalse(_) => 3,
        AnimCondition::Trigger(_) => 4,
        AnimCondition::TimeElapsed(_) => 5,
        AnimCondition::Always => 6,
    }
}

/// Localized display string for a condition's English identity label. The
/// English `cond_kind_label` / `COND_LABELS` strings stay the stable identity
/// (round-tripped through `cond_from_label`); this maps them to the UI text.
fn cond_label_tr(label: &str) -> String {
    renzora::lang::t(match label {
        "Float >" => "animation.cond_float_gt",
        "Float <" => "animation.cond_float_lt",
        "Bool true" => "animation.cond_bool_true",
        "Bool false" => "animation.cond_bool_false",
        "Trigger" => "animation.cond_trigger",
        "Time >=" => "animation.cond_time_ge",
        _ => "animation.cond_always",
    })
}

fn cond_kind_label(c: &AnimCondition) -> &'static str {
    match c {
        AnimCondition::FloatGreater(_, _) => "Float >",
        AnimCondition::FloatLess(_, _) => "Float <",
        AnimCondition::BoolTrue(_) => "Bool true",
        AnimCondition::BoolFalse(_) => "Bool false",
        AnimCondition::Trigger(_) => "Trigger",
        AnimCondition::TimeElapsed(_) => "Time >=",
        AnimCondition::Always => "Always",
    }
}

const COND_LABELS: [&str; 7] = [
    "Float >",
    "Float <",
    "Bool true",
    "Bool false",
    "Trigger",
    "Time >=",
    "Always",
];

fn cond_from_label(label: &str) -> AnimCondition {
    match label {
        "Float >" => AnimCondition::FloatGreater(String::new(), 0.0),
        "Float <" => AnimCondition::FloatLess(String::new(), 0.0),
        "Bool true" => AnimCondition::BoolTrue(String::new()),
        "Bool false" => AnimCondition::BoolFalse(String::new()),
        "Trigger" => AnimCondition::Trigger(String::new()),
        "Time >=" => AnimCondition::TimeElapsed(0.0),
        _ => AnimCondition::Always,
    }
}

fn shape_of(discriminant: u8) -> CondShape {
    match discriminant {
        0 | 1 => CondShape::FloatCompare, // Float >, Float <
        2..=4 => CondShape::Param,        // Bool true/false, Trigger
        5 => CondShape::Time,             // Time >=
        _ => CondShape::None,             // Always
    }
}

fn cond_param_name(w: &World, idx: usize) -> String {
    sm(w)
        .and_then(|s| s.sm.transitions.get(idx))
        .map(|t| match &t.condition {
            AnimCondition::FloatGreater(n, _)
            | AnimCondition::FloatLess(n, _)
            | AnimCondition::BoolTrue(n)
            | AnimCondition::BoolFalse(n)
            | AnimCondition::Trigger(n) => n.clone(),
            _ => String::new(),
        })
        .unwrap_or_default()
}

fn cond_float(w: &World, idx: usize) -> f32 {
    sm(w)
        .and_then(|s| s.sm.transitions.get(idx))
        .map(|t| match &t.condition {
            AnimCondition::FloatGreater(_, v)
            | AnimCondition::FloatLess(_, v)
            | AnimCondition::TimeElapsed(v) => *v,
            _ => 0.0,
        })
        .unwrap_or(0.0)
}

fn set_cond_param_name(c: &mut AnimCondition, name: &str) {
    match c {
        AnimCondition::FloatGreater(n, _)
        | AnimCondition::FloatLess(n, _)
        | AnimCondition::BoolTrue(n)
        | AnimCondition::BoolFalse(n)
        | AnimCondition::Trigger(n) => *n = name.to_string(),
        _ => {}
    }
}

fn set_cond_float(c: &mut AnimCondition, v: f32) {
    match c {
        AnimCondition::FloatGreater(_, x)
        | AnimCondition::FloatLess(_, x)
        | AnimCondition::TimeElapsed(x) => *x = v,
        _ => {}
    }
}

// ── Mutation helpers (run via EditorCommands) ────────────────────────────────

fn set_state(w: &mut World, idx: usize, f: impl FnOnce(&mut AnimState)) {
    if let Some(mut s) = w.get_resource_mut::<SmEditorState>() {
        if let Some(st) = s.sm.states.get_mut(idx) {
            f(st);
            s.dirty = true;
        }
    }
}

fn set_transition(w: &mut World, idx: usize, f: impl FnOnce(&mut AnimTransition)) {
    if let Some(mut s) = w.get_resource_mut::<SmEditorState>() {
        if let Some(t) = s.sm.transitions.get_mut(idx) {
            f(t);
            s.dirty = true;
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

fn unique_name(base: &str, collides: impl Fn(&str) -> bool) -> String {
    let mut candidate = base.to_string();
    let mut n = 1;
    while collides(&candidate) {
        n += 1;
        candidate = format!("{base}{n}");
    }
    candidate
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Idempotent disk load of the `.animsm` referenced by the selected animator —
/// the native analogue of the egui panel's `ensure_loaded`.
fn load_state_machine(
    mut state: ResMut<SmEditorState>,
    anim_state: Res<AnimationEditorState>,
    animators: Query<&AnimatorComponent>,
    project: Option<Res<renzora::core::CurrentProject>>,
) {
    let (Some(entity), Some(project)) = (anim_state.selected_entity, project) else {
        return;
    };
    let Ok(animator) = animators.get(entity) else {
        return;
    };
    let Some(rel) = animator.state_machine.as_deref() else {
        return;
    };
    let abs = project.path.join(rel);
    if state.loaded_for.as_deref() == Some(abs.as_path()) {
        return;
    }
    state.error = None;
    state.dirty = false;
    state.path = Some(abs.clone());
    state.sm = match std::fs::read_to_string(&abs) {
        Ok(s) => match ron::de::from_str::<AnimationStateMachine>(&s) {
            Ok(sm) => sm,
            Err(e) => {
                state.error = Some(format!("Parse error: {e}"));
                fresh_sm()
            }
        },
        Err(_) => fresh_sm(),
    };
    state.loaded_for = Some(abs);
}

fn save_click(
    q: Query<&Interaction, (With<SaveBtn>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    cmds.push(|world: &mut World| {
        let Some(mut s) = world.get_resource_mut::<SmEditorState>() else {
            return;
        };
        let Some(path) = s.path.clone() else {
            s.error = Some("no path".into());
            return;
        };
        let ron = ron::ser::to_string_pretty(
            &s.sm,
            ron::ser::PrettyConfig::new().indentor("  ".into()),
        );
        match ron {
            Ok(ron) => {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, ron) {
                    Ok(()) => {
                        s.dirty = false;
                        s.error = None;
                    }
                    Err(e) => s.error = Some(e.to_string()),
                }
            }
            Err(e) => s.error = Some(e.to_string()),
        }
    });
}

fn add_state_click(
    q: Query<&Interaction, (With<AddStateBtn>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
    anim_state: Option<Res<AnimationEditorState>>,
    animators: Query<&AnimatorComponent>,
) {
    let (Some(cmds), Some(anim_state)) = (cmds, anim_state) else {
        return;
    };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let first_clip = anim_state
        .selected_entity
        .and_then(|e| animators.get(e).ok())
        .and_then(|a| a.clips.first().map(|c| c.name.clone()))
        .unwrap_or_default();
    cmds.push(move |world: &mut World| {
        if let Some(mut s) = world.get_resource_mut::<SmEditorState>() {
            let name = unique_name("NewState", |n| s.sm.states.iter().any(|st| st.name == n));
            s.sm.states.push(AnimState {
                name,
                motion: StateMotion::Clip(first_clip.clone()),
                speed: 1.0,
                looping: true,
            });
            s.dirty = true;
        }
    });
}

fn add_transition_click(
    q: Query<&Interaction, (With<AddTransitionBtn>, Changed<Interaction>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    cmds.push(|world: &mut World| {
        if let Some(mut s) = world.get_resource_mut::<SmEditorState>() {
            let Some(from) = s.sm.states.first().map(|st| st.name.clone()) else {
                return;
            };
            let to = s
                .sm
                .states
                .get(1)
                .map(|st| st.name.clone())
                .unwrap_or_else(|| from.clone());
            s.sm.transitions.push(AnimTransition {
                from,
                to,
                condition: AnimCondition::Always,
                blend_duration: 0.2,
            });
            s.dirty = true;
        }
    });
}

fn star_click(
    q: Query<(&Interaction, &StarBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let name = btn.0.clone();
        if name.is_empty() {
            continue;
        }
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_resource_mut::<SmEditorState>() {
                s.sm.default_state = name.clone();
                s.dirty = true;
            }
        });
    }
}

fn trash_state_click(
    q: Query<(&Interaction, &TrashStateBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = btn.0;
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_resource_mut::<SmEditorState>() {
                if idx >= s.sm.states.len() {
                    return;
                }
                let removed = s.sm.states[idx].name.clone();
                s.sm.states.remove(idx);
                s.sm
                    .transitions
                    .retain(|t| t.from != removed && t.to != removed);
                if s.sm.default_state == removed {
                    s.sm.default_state =
                        s.sm.states.first().map(|st| st.name.clone()).unwrap_or_default();
                }
                s.dirty = true;
            }
        });
    }
}

fn trash_transition_click(
    q: Query<(&Interaction, &TrashTransitionBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = btn.0;
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_resource_mut::<SmEditorState>() {
                if idx < s.sm.transitions.len() {
                    s.sm.transitions.remove(idx);
                    s.dirty = true;
                }
            }
        });
    }
}

// ── Combo-open systems ───────────────────────────────────────────────────────

/// Compute the top-left of a clicked combo so the popup drops just beneath it.
fn combo_top_left(
    rcp: &RelativeCursorPosition,
    cn: &ComputedNode,
    cursor: Vec2,
) -> Vec2 {
    let size = cn.size() * cn.inverse_scale_factor();
    let tl = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    Vec2::new(tl.x, tl.y + size.y + 2.0)
}

fn state_clip_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode, &StateClipCombo),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    anim_state: Option<Res<AnimationEditorState>>,
    animators: Query<&AnimatorComponent>,
    mut commands: Commands,
) {
    let (Some(fonts), Some(anim_state)) = (fonts, anim_state) else {
        return;
    };
    let Some((_, rcp, cn, combo)) = q.iter().find(|(i, _, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let clips: Vec<String> = anim_state
        .selected_entity
        .and_then(|e| animators.get(e).ok())
        .map(|a| a.clips.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
    let idx = combo.0;
    let tl = combo_top_left(rcp, cn, cursor);
    let menu = screen_menu(&mut commands, tl.x, tl.y);
    let kids: Vec<Entity> = clips
        .iter()
        .map(|name| {
            let target = name.clone();
            menu_item(&mut commands, &fonts, "dot", name, move |w| {
                set_state(w, idx, |st| st.motion = StateMotion::Clip(target.clone()));
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

fn trans_from_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode, &TransFromCombo),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    sm_state: Option<Res<SmEditorState>>,
    mut commands: Commands,
) {
    let (Some(fonts), Some(sm_state)) = (fonts, sm_state) else {
        return;
    };
    let Some((_, rcp, cn, combo)) = q.iter().find(|(i, _, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let names: Vec<String> = sm_state.sm.states.iter().map(|s| s.name.clone()).collect();
    let idx = combo.0;
    let tl = combo_top_left(rcp, cn, cursor);
    let menu = screen_menu(&mut commands, tl.x, tl.y);
    let kids: Vec<Entity> = names
        .iter()
        .map(|name| {
            let target = name.clone();
            menu_item(&mut commands, &fonts, "dot", name, move |w| {
                set_transition(w, idx, |t| t.from = target.clone());
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

fn trans_to_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode, &TransToCombo),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    sm_state: Option<Res<SmEditorState>>,
    mut commands: Commands,
) {
    let (Some(fonts), Some(sm_state)) = (fonts, sm_state) else {
        return;
    };
    let Some((_, rcp, cn, combo)) = q.iter().find(|(i, _, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let names: Vec<String> = sm_state.sm.states.iter().map(|s| s.name.clone()).collect();
    let idx = combo.0;
    let tl = combo_top_left(rcp, cn, cursor);
    let menu = screen_menu(&mut commands, tl.x, tl.y);
    let kids: Vec<Entity> = names
        .iter()
        .map(|name| {
            let target = name.clone();
            menu_item(&mut commands, &fonts, "dot", name, move |w| {
                set_transition(w, idx, |t| t.to = target.clone());
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

fn cond_kind_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode, &CondKindCombo),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn, combo)) = q.iter().find(|(i, _, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let idx = combo.0;
    let tl = combo_top_left(rcp, cn, cursor);
    let menu = screen_menu(&mut commands, tl.x, tl.y);
    let kids: Vec<Entity> = COND_LABELS
        .iter()
        .map(|label| {
            let label = *label;
            menu_item(&mut commands, &fonts, "dot", &cond_label_tr(label), move |w| {
                set_transition(w, idx, |t| {
                    // Only replace when the kind actually changes, mirroring the
                    // egui panel (so editing the same kind keeps its values).
                    if cond_kind_label(&t.condition) != label {
                        t.condition = cond_from_label(label);
                    }
                });
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}
