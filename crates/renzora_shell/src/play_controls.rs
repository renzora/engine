//! The top bar's Play control: the pill itself, the caret that picks where Play
//! runs (viewport / runtime window / VR headset / Simulate), and the fullscreen
//! takeover shown while a headset owns the session.
//!
//! This is the editor's only play control — the viewport toolbar's play and
//! scripts buttons are gone. Running the game is not a viewport action, and the
//! top bar is on screen in every workspace.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{glyph, ui_font, EmberFonts};
use renzora_ember::theme::{divider, play_green, rgb, text_muted, text_primary};
use renzora_ember::widgets::Popup;

/// Play + its target caret as one tight split button, in the top bar's left
/// zone. Kept as its own group so the zone's item spacing doesn't pull the caret
/// away from the pill it belongs to; a left margin sets it off from the session
/// actions before it. It has previously lived at the trailing end of the
/// viewport's own tool strip — the top bar wins because running the game is not
/// a viewport action, and this bar is on screen in every workspace.
pub(crate) fn build_play_group(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let play = build_play_button(commands, font);
    let caret = build_play_target_caret(commands, font);
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                margin: UiRect::left(Val::Px(8.0)),
                ..default()
            },
            Name::new("play-group"),
        ))
        .id();
    commands.entity(group).add_children(&[play, caret]);
    group
}

/// The Play / Stop button (icon + text). This is the editor's single play
/// control now that the viewport toolbar's play/scripts buttons are gone.
#[derive(Component)]
pub(crate) struct TopBarPlayBtn;
/// The play button's phosphor glyph (swaps play ↔ stop with state).
#[derive(Component)]
pub(crate) struct TopBarPlayIcon;
/// The play button's "Play" / "Stop" text label.
#[derive(Component)]
pub(crate) struct TopBarPlayLabel;

/// Build the top-bar Play button: a phosphor glyph + a "Play" label in one
/// clickable pill. The glyph + label live as `FocusPolicy::Pass` children so the
/// hover/click lands on the parent (where `Interaction` lives).
fn build_play_button(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            TopBarPlayBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("top-bar-play"),
        ))
        .id();
    let icon = glyph(commands, "play", play_green(), 13.0);
    commands
        .entity(icon)
        .insert((TopBarPlayIcon, bevy::ui::FocusPolicy::Pass));
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("common.play")),
            ui_font(font, 11.0),
            TextColor(rgb(play_green())),
            TopBarPlayLabel,
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[icon, label]);
    btn
}

/// Whether any global (autoload) scene supplies a camera.
///
/// The Play gate asks "is there a scene camera", and until now asked it of the
/// *live world*. Global scenes don't load until Play, so a project whose only
/// camera lives in one could never start: the camera isn't there to open the
/// gate, and the gate is what would load it.
///
/// Answered from the scene files rather than the world, since that is the only
/// place the information exists while editing.
#[derive(Resource, Default)]
pub(crate) struct GlobalSceneHasCamera(bool);

/// Recompute [`GlobalSceneHasCamera`] when the autoload list changes.
///
/// A substring test for the component's type name, not a parse: the answer only
/// gates a button, both scene formats spell the type the same way, and a wrong
/// answer degrades safely — a false positive lets Play start and
/// `enter_play_mode` reports "no scene camera found" as it already does for an
/// empty scene.
pub(crate) fn track_global_scene_cameras(
    project: Option<Res<renzora::CurrentProject>>,
    mut state: ResMut<GlobalSceneHasCamera>,
    mut last: Local<Option<Vec<String>>>,
) {
    let Some(project) = project else { return };
    if last.as_ref() == Some(&project.config.autoload) {
        return;
    }
    *last = Some(project.config.autoload.clone());
    state.0 = project.config.autoload.iter().any(|rel| {
        std::fs::read_to_string(project.resolve_path(rel))
            .map(|text| text.contains("SceneCamera"))
            .unwrap_or(false)
    });
}

/// Click the top-bar Play button → launch the mode picked in the play-target
/// dropdown (full play, or Simulate when that's the selection) from Editing
/// with a scene camera; or stop (while playing, simulating, or while an
/// external runtime is alive).
pub(crate) fn play_btn_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<TopBarPlayBtn>)>,
    play_mode: Option<ResMut<renzora::core::PlayModeState>>,
    runtime: Option<Res<renzora_viewport::external_runtime::ExternalRuntime>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    global_cam: Option<Res<GlobalSceneHasCamera>>,
) {
    let Some(mut pm) = play_mode else { return };
    let runtime_alive = runtime.is_some_and(|r| r.is_alive());
    // A camera in a global scene counts even though it isn't loaded yet.
    let has_cam = !scene_cams.is_empty() || global_cam.is_some_and(|g| g.0);
    let simulate = settings.is_some_and(|s| s.play_launch_simulate);
    for interaction in &btns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // `is_in_play_mode` deliberately EXCLUDES Simulating, so cover it too.
        if runtime_alive || pm.is_in_play_mode() || pm.is_simulating() {
            pm.request_stop = true;
        } else if pm.is_editing() && has_cam {
            if simulate {
                pm.request_simulate = true;
            } else {
                pm.request_play = true;
            }
        }
    }
}

/// Fullscreen takeover shown while an in-process VR session renders to the
/// headset. The editor's offscreen cameras are suspended meanwhile (see
/// `renzora_viewport::sync_viewport_camera_activation`), so without this the
/// panels would sit on a frozen stale frame; instead the whole window reads
/// unambiguously as "the headset owns the session". Stop (or taking the
/// session down from the headset) removes it.
#[derive(Component)]
pub(crate) struct VrActiveOverlay;

pub(crate) fn vr_active_overlay(
    mut commands: Commands,
    vr: Option<Res<renzora::VrPlayState>>,
    existing: Query<Entity, With<VrActiveOverlay>>,
    fonts: Option<Res<EmberFonts>>,
) {
    let active = vr.as_ref().is_some_and(|v| v.active);
    match (active, existing.iter().next()) {
        (true, None) => {
            let Some(fonts) = fonts else { return };
            let root = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.96)),
                    GlobalZIndex(5000),
                    bevy::ui::FocusPolicy::Block,
                    // Registers with ember's pointer-blocking pass so clicks
                    // can't reach panels underneath (see overlay conventions).
                    renzora_ember::widgets::OverlaySurface,
                    VrActiveOverlay,
                    Name::new("vr-active-overlay"),
                ))
                .id();
            let icon = glyph(&mut commands, "virtual-reality", (140, 160, 220), 56.0);
            let title = commands
                .spawn((
                    Text::new(renzora::lang::t_or("shell.vr_active", "VR Mode Active")),
                    ui_font(&fonts.ui, 22.0),
                    TextColor(Color::srgb(0.92, 0.94, 1.0)),
                ))
                .id();
            let hint = commands
                .spawn((
                    Text::new(renzora::lang::t_or(
                        "shell.vr_active_hint",
                        "The scene is playing in the headset. Press Stop to return.",
                    )),
                    ui_font(&fonts.ui, 13.0),
                    TextColor(Color::srgb(0.55, 0.58, 0.68)),
                ))
                .id();
            commands.entity(root).add_children(&[icon, title, hint]);
        }
        (false, Some(entity)) => {
            commands.entity(entity).try_despawn();
        }
        _ => {}
    }
}

/// Drive the Play button's glyph + label + color from play state and the
/// selected launch mode: green "Play" (or blue flask "Simulate" when that mode
/// is picked) when editing — muted if there's no scene camera — and red "Stop"
/// while playing, simulating, or an external runtime is alive. The idle label
/// also names the play target ("Play Viewport", "Play VR"; see
/// [`PlayLaunchChoice::play_label`]), so the caret menu's selection is visible
/// on the button itself.
pub(crate) fn update_play_button(
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    runtime: Option<Res<renzora_viewport::external_runtime::ExternalRuntime>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    global_cam: Option<Res<GlobalSceneHasCamera>>,
    mut icons: Query<&mut renzora_ember::icons::Icon, With<TopBarPlayIcon>>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TopBarPlayLabel>>,
    mut fills: Query<(&mut BackgroundColor, &Interaction), With<TopBarPlayBtn>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let tc = |c: renzora_theme::ThemeColor| {
        let [r, g, b, _] = c.to_array();
        Color::srgb_u8(r, g, b)
    };
    let green = tc(t.semantic.success);
    let red = tc(t.semantic.error);
    let muted = tc(t.text.muted);

    let active = runtime.is_some_and(|r| r.is_alive())
        || play_mode
            .as_ref()
            .is_some_and(|p| p.is_in_play_mode() || p.is_simulating());
    // Matches `play_btn_click`: a global scene's camera counts, so the button
    // doesn't read as disabled while the click handler would accept it.
    let has_cam = !scene_cams.is_empty() || global_cam.is_some_and(|g| g.0);
    let choice = settings
        .as_deref()
        .map(PlayLaunchChoice::current)
        .unwrap_or(PlayLaunchChoice::Viewport);
    let simulate = choice == PlayLaunchChoice::Simulate;

    // `icon_name` is a phosphor glyph name (not localized); the label IS localized.
    let (icon_name, color, playing) = if active {
        ("stop", red, true)
    } else {
        let (idle_icon, idle_color) = if simulate {
            ("flask", rgb(SIM_BLUE))
        } else {
            ("play", green)
        };
        (idle_icon, if has_cam { idle_color } else { muted }, false)
    };
    let label_text = if playing {
        renzora::lang::t("common.stop")
    } else {
        choice.play_label()
    };

    for mut icon in &mut icons {
        if icon.name != icon_name {
            icon.name = icon_name.to_string();
            icon.resolved = false; // force `apply_icons` to re-render the glyph
        }
        if icon.color != Some(color) {
            icon.color = Some(color);
            icon.resolved = false;
        }
    }
    for (mut text, mut tcolor) in &mut labels {
        if text.0 != label_text {
            text.0 = label_text.clone();
        }
        if tcolor.0 != color {
            tcolor.0 = color;
        }
    }
    // A tinted fill so the control reads as a *button*, not as green text on the
    // toolbar. Derived from the same state color the icon and label use, so Play
    // / Simulate / Stop each wash the pill in their own hue, and dimmed along
    // with them when there's no scene camera to play through.
    for (mut bg, interaction) in &mut fills {
        let alpha = match interaction {
            Interaction::Pressed => 0.34,
            Interaction::Hovered => 0.26,
            Interaction::None => 0.16,
        };
        let want = color.with_alpha(alpha);
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// The slim caret beside the Play pill that opens the play-target menu.
#[derive(Component)]
pub(crate) struct PlayTargetCaret;

/// What the Play button launches — the selection made in the play-target menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayLaunchChoice {
    /// Full play inside the editor viewport panel.
    Viewport,
    /// Full play in its own OS runtime window (project window settings).
    Window,
    /// Full play in a VR headset: the external runtime process launched with
    /// `--vr` (OpenXR stereo rendering + a desktop mirror window).
    Vr,
    /// Simulate: scripts + physics tick while the editor stays live.
    Simulate,
}

impl PlayLaunchChoice {
    /// The mode currently selected, resolved from
    /// [`renzora_editor_framework::EditorSettings`].
    fn current(s: &renzora_editor_framework::EditorSettings) -> Self {
        if s.play_launch_simulate {
            Self::Simulate
        } else if s.play_launch_vr {
            Self::Vr
        } else if s.external_play_window {
            Self::Window
        } else {
            Self::Viewport
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Viewport => "frame-corners",
            Self::Window => "app-window",
            Self::Vr => "virtual-reality",
            Self::Simulate => "flask",
        }
    }

    /// What the Play button reads while idle. Window stays the plain "Play" —
    /// launching the game in its own window is what a play button ordinarily
    /// means — while the targets that put the game somewhere else name
    /// themselves, so the button says where the next Play will run without
    /// having to open the caret menu to check.
    fn play_label(self) -> String {
        match self {
            Self::Viewport => renzora::lang::t_or("shell.play_button.viewport", "Play Viewport"),
            Self::Window => renzora::lang::t("common.play"),
            Self::Vr => renzora::lang::t_or("shell.play_button.vr", "Play VR"),
            Self::Simulate => renzora::lang::t("common.simulate"),
        }
    }
}

/// A row in the play-target menu; picking it makes the Play button launch that
/// mode.
#[derive(Component)]
pub(crate) struct PlayTargetOption {
    choice: PlayLaunchChoice,
}
/// The leading glyph of a play-target row — a check on the selected row, the
/// option's own icon on the others (mirrors the theme menu's check-or-icon slot).
#[derive(Component)]
pub(crate) struct PlayTargetOptionIcon {
    choice: PlayLaunchChoice,
}

/// Build the play-target dropdown: a caret beside the Play pill opening a menu
/// that picks where Play runs — inside the editor viewport, or in an actual
/// runtime window using the project's window settings (title, resolution,
/// window mode, resizable). Picking an option writes
/// `EditorSettings.external_play_window` and persists it per-user, so the
/// choice sticks across sessions; the next Play uses it.
fn build_play_target_caret(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                min_width: Val::Px(120.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(600),
            RelativeCursorPosition::default(),
            Name::new("play-target-menu"),
        ))
        .id();

    // Window and VR are the two targets that need something outside this
    // process: Window spawns `<exe_dir>/renzora` as a child process (see
    // `renzora_viewport::external_runtime`) and VR needs an OpenXR device. A
    // browser tab has neither, so the web editor doesn't offer them.
    //
    // Viewport and Simulate both run in-process and work unchanged — which is
    // the whole reason play mode needed no porting for the web build.
    let mut choices = vec![(
        PlayLaunchChoice::Viewport,
        "frame-corners",
        renzora::lang::t_or("shell.play_target.viewport", "Viewport"),
    )];
    #[cfg(not(target_arch = "wasm32"))]
    {
        choices.push((
            PlayLaunchChoice::Window,
            "app-window",
            renzora::lang::t_or("shell.play_target.runtime_window", "Window"),
        ));
        choices.push((
            PlayLaunchChoice::Vr,
            "virtual-reality",
            renzora::lang::t_or("shell.play_target.vr", "VR Headset"),
        ));
    }
    choices.push((
        PlayLaunchChoice::Simulate,
        "flask",
        renzora::lang::t_or("common.simulate", "Simulate"),
    ));

    let mut rows = Vec::new();
    for (choice, icon_name, label) in choices {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                PlayTargetOption { choice },
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new("play-target-option"),
            ))
            .id();
        renzora_ember::reactive::tracked::bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                rgb(renzora_ember::theme::hover_bg())
            }
            _ => Color::NONE,
        });
        let ic = glyph(commands, icon_name, text_muted(), 12.0);
        commands.entity(ic).insert((
            PlayTargetOptionIcon { choice },
            bevy::ui::FocusPolicy::Pass,
        ));
        let t = commands
            .spawn((
                Text::new(label),
                ui_font(font, 12.0),
                TextColor(rgb(text_primary())),
                bevy::ui::FocusPolicy::Pass,
            ))
            .id();
        commands.entity(row).add_children(&[ic, t]);
        rows.push(row);
    }
    commands.entity(panel).add_children(&rows);

    let trigger = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(2.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            Popup { panel, open: false },
            PlayTargetCaret,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("play-target-caret"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, trigger, move |w| {
        match w.get::<Interaction>(trigger) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, 0.09)
            }
            _ => Color::NONE,
        }
    });
    let caret = glyph(commands, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(trigger).add_children(&[caret, panel]);
    trigger
}

/// Pick a play-target row → write the launch mode, persist the viewport/window
/// half of it, close the menu. Simulate is a session-only choice layered on
/// top: it doesn't touch the persisted viewport-vs-window preference, so
/// dropping back out of Simulate restores whichever of the two was saved.
pub(crate) fn play_target_option_click(
    opts: Query<(&Interaction, &PlayTargetOption), Changed<Interaction>>,
    mut settings: Option<ResMut<renzora_editor_framework::EditorSettings>>,
    carets: Query<Entity, (With<PlayTargetCaret>, With<Popup>)>,
    mut commands: Commands,
) {
    for (interaction, opt) in &opts {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(s) = settings.as_mut() {
            match opt.choice {
                PlayLaunchChoice::Simulate => s.play_launch_simulate = true,
                PlayLaunchChoice::Vr => {
                    s.play_launch_simulate = false;
                    s.play_launch_vr = true;
                    let _ = renzora::save_play_vr(true);
                }
                PlayLaunchChoice::Viewport | PlayLaunchChoice::Window => {
                    s.play_launch_simulate = false;
                    s.play_launch_vr = false;
                    let _ = renzora::save_play_vr(false);
                    let runtime_window = opt.choice == PlayLaunchChoice::Window;
                    s.external_play_window = runtime_window;
                    let _ = renzora::save_play_runtime_window(runtime_window);
                }
            }
        }
        for caret in &carets {
            renzora_ember::widgets::close_popup(&mut commands, caret);
        }
    }
}

/// Keep each play-target row's leading glyph in sync with the current launch
/// mode: the selected row shows a green check, the others show their own icons.
pub(crate) fn update_play_target_menu(
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    mut icons: Query<(&mut renzora_ember::icons::Icon, &PlayTargetOptionIcon)>,
) {
    let Some(settings) = settings else { return };
    let current = PlayLaunchChoice::current(&settings);
    let green = theme
        .map(|t| {
            let [r, g, b, _] = t.active_theme.semantic.success.to_array();
            Color::srgb_u8(r, g, b)
        })
        .unwrap_or_else(|| rgb(play_green()));
    for (mut icon, opt) in &mut icons {
        let (name, color) = if opt.choice == current {
            ("check", green)
        } else {
            (opt.choice.icon(), rgb(text_muted()))
        };
        if icon.name != name {
            icon.name = name.to_string();
            icon.resolved = false;
        }
        if icon.color != Some(color) {
            icon.color = Some(color);
            icon.resolved = false;
        }
    }
}

/// Simulate's accent colour (blue) — distinct from Play's green so the two
/// launch modes read apart at a glance on the Play button.
const SIM_BLUE: (u8, u8, u8) = (86, 169, 247);
