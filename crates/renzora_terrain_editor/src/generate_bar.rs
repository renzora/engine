//! The Generate tool's settings bar, across the top of the scene.
//!
//! Same surface and the same reasoning as [`crate::brush_bar`]: a generator you
//! adjust by walking to a dock panel and back is one you adjust twice and then
//! stop adjusting. Here the sliders sit directly above the preview they change,
//! so re-rolling a landscape is a drag and a glance.
//!
//! It registers as its own bar rather than as another cluster inside the brush
//! bar because it shares nothing with it — different resource, different tool,
//! and a full set of controls of its own. Only one of the two is ever visible,
//! since each binds its display to the tool that opens it.
//!
//! The two buttons on the right are the only things in the editor that write
//! heights from this tool. Everything left of them is preview-only, which is
//! what makes the bar safe to fiddle with.

use bevy::prelude::*;

use renzora_editor_framework::{ActiveTool, EditorCommands};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::toggle_switch;

use renzora_terrain::data::{NoiseMode, StampBlendMode};
use renzora_terrain::generate::{next_seed, TerrainGenSettings};

use crate::brush_bar::{cluster, labelled_drag, labelled_dropdown, labelled_slider, tool_is};

/// Stacking order among the viewport's full-width bars — one past the brush
/// bar, so if both were ever visible the generator's would sit under it. They
/// aren't, but the order still has to be defined.
const BAR_ORDER: i32 = 101;

pub fn register(app: &mut App) {
    renzora_ember::toolbar::register_viewport_top_strip(BAR_ORDER, build);
    app.add_systems(
        Update,
        (reroll_click, generate_click, reset_click).run_if(renzora::core::not_in_play_mode),
    );
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                flex_shrink: 0.0,
                min_width: Val::Px(0.0),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(mix(panel_bg(), header_bg(), 0.55)),
            BorderColor::all(rgb(divider())),
            // Same as the brush bar: the strip covers the top of the viewport's
            // picking area, so it has to swallow clicks or a press on the gap
            // between two controls falls through and deselects the terrain.
            bevy::ui::RelativeCursorPosition::default(),
            renzora_ember::widgets::OverlaySurface,
            Name::new("vp-terrain-generate"),
        ))
        .id();
    bind_display(commands, bar, |w| tool_is(w, ActiveTool::TerrainGenerate));

    let kids = vec![
        shape_opts(commands, fonts),
        height_opts(commands, fonts),
        blend_opts(commands, fonts),
        actions(commands, fonts),
    ];
    commands.entity(bar).add_children(&kids);
    bar
}

// ── Clusters ────────────────────────────────────────────────────────────────

/// What the landscape looks like: which noise, how big its features, how rough.
fn shape_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-shape");
    let mode = labelled_dropdown(
        commands,
        fonts,
        "Noise",
        &["FBM", "Ridge", "Billow", "Warped", "Hybrid"],
        68.0,
        |w| {
            let cur = get(w, |g| g.mode);
            NoiseMode::all().iter().position(|m| *m == cur).unwrap_or(0)
        },
        |w, i| {
            let m = NoiseMode::all().get(*i).copied().unwrap_or_default();
            set(w, |g| g.mode = m);
        },
    );
    // Metres per noise unit. The top of the range is deliberately far past any
    // default terrain's width — that is how you get *one* mountain rather than a
    // range of them.
    let scale = labelled_drag(
        commands,
        fonts,
        "Scale",
        10.0,
        2000.0,
        1.0,
        None,
        |w| get(w, |g| g.scale),
        |w, v| set(w, |g| g.scale = *v),
    );
    let octaves = labelled_drag(
        commands,
        fonts,
        "Oct",
        1.0,
        8.0,
        0.1,
        Some(1.0),
        |w| get(w, |g| g.octaves as f32),
        |w, v| set(w, |g| g.octaves = v.round().clamp(1.0, 8.0) as u32),
    );
    let persistence = labelled_slider(
        commands,
        fonts,
        "Rough",
        0.1,
        0.9,
        2,
        |w| get(w, |g| g.persistence),
        |w, v| set(w, |g| g.persistence = *v),
    );
    let peaks = labelled_slider(
        commands,
        fonts,
        "Peaks",
        0.4,
        4.0,
        2,
        |w| get(w, |g| g.exponent),
        |w, v| set(w, |g| g.exponent = *v),
    );
    commands
        .entity(row)
        .add_children(&[mode, scale, octaves, persistence, peaks]);
    row
}

/// Where it sits vertically, in the terrain's own metres.
fn height_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-height");
    let height = labelled_drag(
        commands,
        fonts,
        "Height",
        0.0,
        2000.0,
        0.5,
        None,
        |w| get(w, |g| g.height),
        |w, v| set(w, |g| g.height = *v),
    );
    let base = labelled_drag(
        commands,
        fonts,
        "Base",
        -1000.0,
        1000.0,
        0.5,
        None,
        |w| get(w, |g| g.base),
        |w, v| set(w, |g| g.base = *v),
    );
    commands.entity(row).add_children(&[height, base]);
    row
}

/// How it meets the terrain that is already there.
fn blend_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-blend");
    let blend = labelled_dropdown(
        commands,
        fonts,
        "Blend",
        &["Add", "Subtract", "Replace", "Max", "Min"],
        72.0,
        |w| {
            let cur = get(w, |g| g.blend);
            StampBlendMode::all()
                .iter()
                .position(|m| *m == cur)
                .unwrap_or(0)
        },
        |w, i| {
            let m = StampBlendMode::all().get(*i).copied().unwrap_or_default();
            set(w, |g| g.blend = m);
        },
    );
    let feather = labelled_slider(
        commands,
        fonts,
        "Feather",
        0.0,
        1.0,
        2,
        |w| get(w, |g| g.feather),
        |w, v| set(w, |g| g.feather = *v),
    );
    let seed = labelled_drag(
        commands,
        fonts,
        "Seed",
        0.0,
        u16::MAX as f32,
        1.0,
        Some(1.0),
        |w| get(w, |g| g.seed as f32),
        |w, v| set(w, |g| g.seed = v.max(0.0).round() as u32),
    );

    let preview_row = cluster(commands, "gen-preview");
    let preview_label = commands
        .spawn((
            Text::new("Preview"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let sw = toggle_switch(commands, true);
    bind_2way(
        commands,
        sw,
        |w: &Rx| get(w, |g| g.preview),
        |w: &mut World, v: &bool| set(w, |g| g.preview = *v),
    );
    commands
        .entity(preview_row)
        .add_children(&[preview_label, sw]);

    commands
        .entity(row)
        .add_children(&[blend, feather, seed, preview_row]);
    row
}

/// The three buttons that actually do something.
fn actions(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-actions");
    let reroll = action_button(commands, fonts, "Re-roll", false);
    commands.entity(reroll).insert(RerollBtn);
    let generate = action_button(commands, fonts, "Generate", true);
    commands.entity(generate).insert(GenerateBtn);
    let reset = action_button(commands, fonts, "Flatten", false);
    commands.entity(reset).insert(ResetBtn);
    commands
        .entity(row)
        .add_children(&[reroll, generate, reset]);
    row
}

// ── Buttons ─────────────────────────────────────────────────────────────────

#[derive(Component)]
struct RerollBtn;
#[derive(Component)]
struct GenerateBtn;
#[derive(Component)]
struct ResetBtn;

/// Re-rolling only moves the seed — the preview updates and nothing is written,
/// so you can walk through landscapes until one looks right before committing.
fn reroll_click(
    q: Query<&Interaction, (Changed<Interaction>, With<RerollBtn>)>,
    mut settings: ResMut<TerrainGenSettings>,
) {
    for interaction in &q {
        if *interaction == Interaction::Pressed {
            settings.seed = next_seed(settings.seed);
        }
    }
}

fn generate_click(
    q: Query<&Interaction, (Changed<Interaction>, With<GenerateBtn>)>,
    hover: Res<crate::generate_tool::GenerateHover>,
    cmds: Option<Res<EditorCommands>>,
) {
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (Some(cmds), Some(terrain)) = (cmds.as_ref(), hover.terrain) else {
            continue;
        };
        // Deferred: generating snapshots every chunk twice and records an undo
        // entry, which is not something to do from inside a UI system.
        cmds.push(move |w: &mut World| crate::generate_tool::generate_now(w, terrain));
    }
}

fn reset_click(
    q: Query<&Interaction, (Changed<Interaction>, With<ResetBtn>)>,
    hover: Res<crate::generate_tool::GenerateHover>,
    cmds: Option<Res<EditorCommands>>,
) {
    for interaction in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (Some(cmds), Some(terrain)) = (cmds.as_ref(), hover.terrain) else {
            continue;
        };
        cmds.push(move |w: &mut World| crate::generate_tool::reset_now(w, terrain));
    }
}

fn action_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    primary: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(22.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(if primary { rgb(accent()) } else { rgb(card_bg()) }),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("terrain-generate-{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hovered = matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        match (primary, hovered) {
            (true, _) => rgb(accent()),
            (false, true) => rgb(hover_bg()),
            (false, false) => rgb(card_bg()),
        }
    });
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(if primary {
                Color::WHITE
            } else {
                rgb(text_primary())
            }),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_child(text);
    btn
}

// ── Resource accessors ──────────────────────────────────────────────────────

fn get<T: Default>(w: &Rx, f: impl Fn(&TerrainGenSettings) -> T) -> T {
    w.get_resource::<TerrainGenSettings>()
        .map(f)
        .unwrap_or_default()
}

fn set(w: &mut World, f: impl FnOnce(&mut TerrainGenSettings)) {
    if let Some(mut g) = w.get_resource_mut::<TerrainGenSettings>() {
        f(&mut g);
    }
}
