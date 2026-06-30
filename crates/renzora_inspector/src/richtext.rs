//! Native inspector drawer for **rich text** — the styled `TextSpan` runs that
//! live as children of a `Text` / `Text2d` entity. Bevy 0.19 models rich text as
//! a root text component plus one child entity per span, each carrying its own
//! `TextColor` (and font), so a paragraph can mix colors/styles on one line.
//!
//! This drawer surfaces that hierarchy: it lists each span with an editable text
//! field, compact R/G/B color controls, and a remove button, plus an *Add span*
//! action. It mirrors the dynamic-list pattern in [`crate::camera_presets`] — a
//! stable `SpansRoot` container that a `rebuild_text_spans` system re-fills
//! whenever the span *count* changes, with structural edits deferred through
//! [`EditorCommands`] so they don't tear down the UI mid-frame.
//!
//! Paired with the `"text_rich"` `InspectorEntry` (editor framework), shown on
//! any entity that has a `Text` or `Text2d`.

use std::hash::{Hash, Hasher};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::text::{TextColor, TextFont, TextSpan};

use renzora_editor_framework::EditorCommands;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::inspector::inspector_stripe;
use renzora_ember::reactive::bind_2way;
use renzora_ember::theme::{rgb, section_bg, text_muted};
use renzora_ember::widgets::{
    bind_text_input, drag_value, icon_button, icon_label_button, text_input, DragRange,
};

pub fn register(app: &mut App) {
    use renzora_editor_framework::{AppEditorExt, SplashState};
    app.register_native_inspector_ui("text_rich", text_rich_native);
    app.add_systems(
        Update,
        (rebuild_text_spans, add_span_click, remove_span_click)
            .run_if(in_state(SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
    );
}

#[derive(Component)]
struct SpansRoot {
    /// The text entity whose `TextSpan` children we edit.
    entity: Entity,
    sig: Option<u64>,
}

#[derive(Component)]
struct AddSpanBtn {
    entity: Entity,
}

#[derive(Component)]
struct RemoveSpanBtn {
    span: Entity,
}

fn text_rich_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            SpansRoot { entity, sig: None },
            Name::new("text-spans-root"),
        ))
        .id()
}

// ── Rebuild ──────────────────────────────────────────────────────────────────

/// Ordered `TextSpan` children of `entity`. Order is the paint order, so we keep
/// it (don't sort).
fn span_children(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .get::<Children>(entity)
        .map(|c| c.iter().filter(|&ch| world.get::<TextSpan>(ch).is_some()).collect())
        .unwrap_or_default()
}

/// The structure signature: only the span *count* (and root id) — content/color
/// edits are two-way bound and must NOT trigger a rebuild (it would despawn the
/// field being typed into). Add/remove changes the count → rebuild.
fn spans_sig(world: &World, entity: Entity, root: Entity) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    root.to_bits().hash(&mut h);
    span_children(world, entity).len().hash(&mut h);
    h.finish()
}

fn rebuild_text_spans(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut q = world.query::<(Entity, &SpansRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> =
        q.iter(world).map(|(re, sr)| (re, sr.entity, sr.sig)).collect();

    for (root, entity, old_sig) in roots {
        let sig = spans_sig(world, entity, root);
        if old_sig == Some(sig) {
            continue;
        }
        let spans = span_children(world, entity);
        let existing: Vec<Entity> =
            world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();

        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            if spans.is_empty() {
                let l = muted_label(&mut commands, &fonts, &renzora::lang::t("comp.text_rich.empty"));
                commands.entity(root).add_child(l);
            }
            for (i, span) in spans.iter().enumerate() {
                let row = build_span_row(&mut commands, &fonts, *span);
                commands.entity(row).insert(BackgroundColor(inspector_stripe(i)));
                commands.entity(root).add_child(row);
            }
            let add = build_add_button(&mut commands, &fonts, entity);
            commands.entity(root).add_child(add);
        }
        queue.apply(world);
        if let Some(mut sr) = world.get_mut::<SpansRoot>(root) {
            sr.sig = Some(sig);
        }
    }
}

/// One channel (0..255) of a span's `TextColor`, as an f32 for the drag control.
fn channel(world: &World, span: Entity, idx: usize) -> f32 {
    world
        .get::<TextColor>(span)
        .map(|c| {
            let s = c.0.to_srgba();
            let v = [s.red, s.green, s.blue][idx];
            (v * 255.0).round()
        })
        .unwrap_or(255.0)
}

fn set_channel(world: &mut World, span: Entity, idx: usize, v: f32) {
    if let Some(mut c) = world.get_mut::<TextColor>(span) {
        let mut s = c.0.to_srgba();
        let nv = (v / 255.0).clamp(0.0, 1.0);
        match idx {
            0 => s.red = nv,
            1 => s.green = nv,
            _ => s.blue = nv,
        }
        c.0 = Color::Srgba(s);
    }
}

fn build_span_row(commands: &mut Commands, fonts: &EmberFonts, span: Entity) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            Name::new("text-span-row"),
        ))
        .id();

    // Editable span content (two-way bound to the `TextSpan` string).
    let content = world_span_text(commands, fonts, span);

    // Compact R/G/B controls bound to the span's `TextColor`.
    let mut chans = Vec::with_capacity(3);
    for idx in 0..3usize {
        let init = 255.0; // refreshed by the bind's getter
        let dv = drag_value(commands, &fonts.ui, "", text_muted(), init, 5.0);
        commands.entity(dv).insert((
            DragRange { min: 0.0, max: 255.0 },
            Node { width: Val::Px(34.0), ..default() },
        ));
        bind_2way::<f32, _, _>(
            commands,
            dv,
            move |w| channel(w, span, idx),
            move |w, &v| set_channel(w, span, idx, v),
        );
        chans.push(dv);
    }

    let del = icon_button(commands, fonts, "trash");
    commands.entity(del).insert(RemoveSpanBtn { span });

    commands.entity(row).add_child(content);
    commands.entity(row).add_children(&chans);
    commands.entity(row).add_child(del);
    row
}

/// The editable text field for a span, bound to its `TextSpan` content.
fn world_span_text(commands: &mut Commands, fonts: &EmberFonts, span: Entity) -> Entity {
    let ti = text_input(commands, &fonts.ui, &renzora::lang::t("comp.text_rich.span_placeholder"), "");
    commands.entity(ti).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        ..default()
    });
    bind_text_input(
        commands,
        ti,
        move |w| w.get::<TextSpan>(span).map(|s| s.0.clone()).unwrap_or_default(),
        move |w, v: String| {
            if let Some(mut s) = w.get_mut::<TextSpan>(span) {
                s.0 = v;
            }
        },
    );
    ti
}

fn build_add_button(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let btn = icon_label_button(commands, fonts, "plus", &renzora::lang::t("comp.text_rich.add_span"));
    commands.entity(btn).insert((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(5.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(rgb(section_bg())),
        AddSpanBtn { entity },
    ));
    btn
}

// ── Click handlers ────────────────────────────────────────────────────────────

fn add_span_click(
    q: Query<(&Interaction, &AddSpanBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let entity = btn.entity;
        cmds.push(move |w: &mut World| {
            // New spans inherit the root's font so they render consistently.
            let font = w.get::<TextFont>(entity).cloned().unwrap_or_default();
            let span = w
                .spawn((TextSpan::new("New span"), font, TextColor(Color::WHITE)))
                .id();
            w.entity_mut(entity).add_child(span);
        });
    }
}

fn remove_span_click(
    q: Query<(&Interaction, &RemoveSpanBtn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let span = btn.span;
        cmds.push(move |w: &mut World| {
            if let Ok(em) = w.get_entity_mut(span) {
                em.despawn();
            }
        });
    }
}

fn muted_label(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::all(Val::Px(6.0)),
                ..default()
            },
        ))
        .id()
}
