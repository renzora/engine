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
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::toggle_switch;

use renzora_terrain::data::{NoiseMode, StampBlendMode};
use renzora_terrain::generate::{next_seed, GenSource, HeightmapFit, TerrainGenSettings};

use crate::brush_bar::{
    cluster, context_bar_bg, labelled_drag, labelled_dropdown, labelled_slider, tool_is,
};

/// Stacking order among the viewport's full-width bars — one past the brush
/// bar, so if both were ever visible the generator's would sit under it. They
/// aren't, but the order still has to be defined.
const BAR_ORDER: i32 = 101;

pub fn register(app: &mut App) {
    renzora_ember::toolbar::register_viewport_top_strip(BAR_ORDER, build);
    app.add_systems(
        Update,
        (
            reroll_click,
            generate_click,
            reset_click,
            heightmap_load_click,
            heightmap_clear_click,
        )
            .run_if(renzora::core::not_in_play_mode),
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
            BackgroundColor(context_bar_bg()),
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
        source_opts(commands, fonts),
        shape_opts(commands, fonts),
        heightmap_opts(commands, fonts),
        height_opts(commands, fonts),
        blend_opts(commands, fonts),
        actions(commands, fonts),
    ];
    commands.entity(bar).add_children(&kids);
    bar
}

// ── Clusters ────────────────────────────────────────────────────────────────

/// Noise or a loaded heightmap. First on the bar because it decides which of the
/// next two clusters you are looking at.
fn source_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-source");
    let dd = labelled_dropdown(
        commands,
        fonts,
        "Source",
        &["Noise", "Heightmap"],
        84.0,
        |w| {
            let cur = get(w, |g| g.source);
            GenSource::all().iter().position(|s| *s == cur).unwrap_or(0)
        },
        |w, i| {
            let s = GenSource::all().get(*i).copied().unwrap_or_default();
            set(w, |g| g.source = s);
        },
    );
    commands.entity(row).add_children(&[dd]);
    row
}

/// The heightmap source's controls. Shares the bar slot with [`shape_opts`] —
/// only one source's dials are ever up, since the other set means nothing.
fn heightmap_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-heightmap");
    bind_display(commands, row, |w| {
        get(w, |g| g.source) == GenSource::Heightmap
    });

    // The file button doubles as the filename readout: on a toolbar there is no
    // room for a label *and* a path, and the button is where you look anyway.
    let (load, load_label) = action_button_parts(commands, fonts, "Load Heightmap…", false);
    commands.entity(load).insert(HeightmapLoadBtn);
    bind_text(commands, load_label, |w| {
        heightmap_label(&get(w, |g| g.heightmap_path.clone()))
    });

    let fit = labelled_dropdown(
        commands,
        fonts,
        "Fit",
        &["Stretch", "Contain"],
        76.0,
        |w| {
            let cur = get(w, |g| g.heightmap_fit);
            HeightmapFit::all()
                .iter()
                .position(|f| *f == cur)
                .unwrap_or(0)
        },
        |w, i| {
            let f = HeightmapFit::all().get(*i).copied().unwrap_or_default();
            set(w, |g| g.heightmap_fit = f);
        },
    );

    let levels = switch_row(
        commands,
        fonts,
        "gen-levels",
        "Levels",
        |w| get(w, |g| g.heightmap_normalize),
        |w, v| set(w, |g| g.heightmap_normalize = *v),
    );
    let invert_row = switch_row(
        commands,
        fonts,
        "gen-invert",
        "Invert",
        |w| get(w, |g| g.heightmap_invert),
        |w, v| set(w, |g| g.heightmap_invert = *v),
    );

    // Only worth a button once there is something to clear.
    let clear = action_button(commands, fonts, "Clear", false);
    commands.entity(clear).insert(HeightmapClearBtn);
    bind_display(commands, clear, |w| get(w, |g| g.heightmap.is_some()));

    commands
        .entity(row)
        .add_children(&[load, fit, levels, invert_row, clear]);
    row
}

/// `[Label] [switch]` — the shape the Preview toggle already uses.
fn switch_row<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    label: &str,
    get_value: G,
    set_value: S,
) -> Entity
where
    G: Fn(&Rx) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, &bool) + Send + Sync + 'static,
{
    let row = cluster(commands, name);
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let sw = toggle_switch(commands, false);
    bind_2way(commands, sw, get_value, set_value);
    commands.entity(row).add_children(&[text, sw]);
    row
}

/// What the landscape looks like: which noise, how big its features, how rough.
fn shape_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-shape");
    bind_display(commands, row, |w| get(w, |g| g.source) == GenSource::Noise);
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
    commands
        .entity(row)
        .add_children(&[mode, scale, octaves, persistence]);
    row
}

/// Where it sits vertically, in the terrain's own metres.
///
/// Peaks lives here rather than with the noise dials because it shapes whatever
/// the source produced — on a heightmap it is the contrast control, and it is
/// the one thing that turns a washed-out 8-bit DEM into ground with valleys.
fn height_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "gen-height");
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
    commands.entity(row).add_children(&[peaks, height, base]);
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
    // A heightmap has no seed to walk. Leaving the field up would suggest it
    // re-rolls something, and it doesn't.
    bind_display(commands, seed, |w| {
        get(w, |g| g.source) == GenSource::Noise
    });

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
    bind_display(commands, reroll, |w| {
        get(w, |g| g.source) == GenSource::Noise
    });
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
#[derive(Component)]
struct HeightmapLoadBtn;
#[derive(Component)]
struct HeightmapClearBtn;

/// Longest filename the button shows before it starts eating the rest of the
/// bar. Heightmaps come out of generators with names like
/// `terrain_export_16bit_4096.r16`, and the tail is the informative half.
const NAME_MAX: usize = 20;

/// The load button's label: the call to action, or the file it stands for.
fn heightmap_label(path: &str) -> String {
    if path.is_empty() {
        return "Load Heightmap…".to_string();
    }
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    if name.chars().count() <= NAME_MAX {
        return name;
    }
    let tail: String = name
        .chars()
        .skip(name.chars().count() - (NAME_MAX - 1))
        .collect();
    format!("…{tail}")
}

/// Open a file dialog, decode the picked heightmap, and hand it to the
/// generator.
///
/// Exclusive and deferred: `rfd` blocks the thread until the dialog closes, and
/// decoding a 4096² PNG is not something to do inside a UI system holding
/// queries. Nothing is written to the terrain here — loading only arms the
/// preview, and Generate is still the button that commits.
fn heightmap_load_click(
    q: Query<&Interaction, (Changed<Interaction>, With<HeightmapLoadBtn>)>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(cmds) = cmds.as_ref() else { return };
    cmds.push(|w: &mut World| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use renzora_terrain::heightmap_import::{load_heightmap_file, HeightmapFormat};

            let Some(path) = rfd::FileDialog::new()
                .add_filter("Heightmap", &["png", "r16", "raw"])
                .set_title("Load Heightmap")
                .pick_file()
            else {
                return;
            };
            match load_heightmap_file(&path, &HeightmapFormat::Auto) {
                Ok(image) => {
                    // The range is worth logging: a file using a fifth of its
                    // container is the normal case, and seeing it explains why
                    // the Levels switch matters and what turning it off costs.
                    let (lo, hi) = image.range();
                    bevy::log::info!(
                        "Loaded heightmap {} ({}x{}, values {lo:.3}–{hi:.3})",
                        path.display(),
                        image.width,
                        image.height,
                    );
                    set(w, move |g| {
                        g.set_heightmap(path.to_string_lossy().to_string(), image)
                    });
                }
                Err(e) => bevy::log::error!("Heightmap load failed: {e}"),
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = w;
    });
}

fn heightmap_clear_click(
    q: Query<&Interaction, (Changed<Interaction>, With<HeightmapClearBtn>)>,
    mut settings: ResMut<TerrainGenSettings>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        settings.clear_heightmap();
    }
}

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
    action_button_parts(commands, fonts, label, primary).0
}

/// The button and its label entity, for the one caller whose label changes.
fn action_button_parts(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    primary: bool,
) -> (Entity, Entity) {
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
    (btn, text)
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
