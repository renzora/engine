//! The viewport shading switch — wireframe / solid / material / rendered.
//!
//! Four buttons centred on the viewport's top edge. They are the four ways you
//! look at a scene while building it, under the names every 3D tool gives them.
//!
//! Centred rather than tucked into the top-right corner, which is where they
//! started: the axis gizmo already lives there and the two collided. The top
//! edge's middle is the one part of the viewport frame nothing else claims.
//!
//! # Presets, not a fifth piece of state
//!
//! Each button *writes* the existing switches — `RenderToggles` and
//! `VisualizationMode` — and the highlight is derived by comparing the current
//! settings back against each preset. Storing the chosen mode as its own field
//! would have been simpler to write and wrong to live with: the Display
//! dropdown and Settings edit the same switches, so the stored mode would go
//! stale the moment anyone touched one, and the viewport would claim to be in a
//! mode it had left.
//!
//! Derived, a hand-tweaked combination lights no button at all, which is the
//! honest answer: you are not in any of the four.
//!
//! # A ladder: each mode adds one thing to the one below it
//!
//! - **Wireframe** — topology. Edges only, mesh fill off.
//! - **Solid** — adds form: the [`Matcap`](VisualizationMode::Matcap) clay,
//!   lights fixed to the camera, curvature shaded. No materials, no scene
//!   lighting, which is what you want while modeling.
//! - **Material** — adds materials and scene lighting, shadows included. Your
//!   objects, lit, against a flat background.
//! - **Rendered** — adds the world around them: sky, atmosphere, clouds.
//!
//! Material used to be "everything except shadows", which made it and Rendered
//! nearly the same picture — one soft difference, and the sky dominating both.
//! Moving the environment onto the rung between them gives each step a single
//! obvious thing it turns on, and gives Material a use Rendered does not cover:
//! judging materials and lighting without a sky lighting and colouring them.
//!
//! # The environment is part of a mode, not a consequence of one
//!
//! The bottom three suppress the sky through
//! [`EnvironmentSuppressed`](renzora::core::EnvironmentSuppressed), and Rendered
//! restores it. That flag is *the* thing separating Material from Rendered,
//! whose render toggles are otherwise identical, so it is a switch these buttons
//! write and compare like any other — not something derived from the toggles
//! afterwards, which could not tell those two apart.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::viewport_types::{RenderToggles, ViewportSettings, VisualizationMode};
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::theme::{accent, hover_bg, panel_bg, rgb};
use renzora_ember::widgets::OverlaySurface;

const BTN: f32 = 30.0;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Shading {
    Wireframe,
    Solid,
    Material,
    Rendered,
}

impl Shading {
    const ALL: [Shading; 4] = [
        Self::Wireframe,
        Self::Solid,
        Self::Material,
        Self::Rendered,
    ];

    /// The glyph on the button.
    ///
    /// These are *views*, so each icon has to say what you would be looking at,
    /// never what you would be doing. Material was a paint brush, which is a
    /// tool: it reads as "edit materials" next to a Materials panel that
    /// genuinely does that. A half-lit ball is the shading it actually turns on.
    fn icon(self) -> &'static str {
        match self {
            Self::Wireframe => "polygon",
            Self::Solid => "sphere",
            Self::Material => "circle-half-tilt",
            Self::Rendered => "sun",
        }
    }

    /// Does this mode show the scene's environment?
    ///
    /// Only the top of the ladder does, and that is the whole of what separates
    /// Rendered from Material: their render toggles are identical, so this is
    /// part of a mode's identity rather than something read off the toggles.
    fn shows_environment(self) -> bool {
        matches!(self, Self::Rendered)
    }

    /// The switch positions this mode stands for.
    fn preset(self) -> (RenderToggles, VisualizationMode) {
        let base = RenderToggles {
            textures: true,
            wireframe: false,
            lighting: true,
            shadows: true,
            mesh: true,
        };
        match self {
            Self::Wireframe => (
                RenderToggles {
                    wireframe: true,
                    mesh: false,
                    ..base
                },
                VisualizationMode::None,
            ),
            // Matcap rather than "textures off": an untextured surface under
            // scene lighting still changes as you orbit, and reading form is the
            // whole reason to be in this mode.
            Self::Solid => (base, VisualizationMode::Matcap),
            // Deliberately the same switches. The two differ by the environment
            // alone, which `shows_environment` carries and `current` compares.
            Self::Material | Self::Rendered => (base, VisualizationMode::None),
        }
    }

    /// Which mode the viewport is currently in, if it is in one of them.
    ///
    /// `environment` is whether the sky is showing, which is a switch in its own
    /// right: without it Material and Rendered are the same settings and the
    /// first of the two would always win.
    fn current(settings: &ViewportSettings, environment: bool) -> Option<Shading> {
        Self::ALL.into_iter().find(|mode| {
            let (toggles, viz) = mode.preset();
            toggles == settings.render_toggles
                && viz == settings.visualization_mode
                && mode.shows_environment() == environment
        })
    }
}

fn resting() -> Color {
    let (r, g, b) = panel_bg();
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 0.55)
}

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        OnEnter(renzora_editor_framework::SplashState::Editor),
        align_environment_to_restored_settings,
    );
    app.add_systems(
        Update,
        (
            shading_clicks,
            shading_visuals,
            restore_environment_in_play_mode,
        )
            .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
}

/// Build the strip, centred on the viewport's top edge.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // A full-width row that centres the strip, so the buttons stay centred
    // whatever their number without anyone hand-computing half their width.
    //
    // The wrapper must not swallow the pointer: it spans the whole top edge,
    // and marking *that* as an overlay surface would kill camera orbit and
    // box-select across the full width of the viewport. Only the strip inside
    // it claims the cursor.
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(8.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
            Name::new("shading-overlay-wrap"),
        ))
        .id();

    let strip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(1.0),
                ..default()
            },
            RelativeCursorPosition::default(),
            // Hovering the strip must suppress viewport hover, or the camera
            // orbits and box-select arms under the buttons.
            OverlaySurface,
            Name::new("shading-overlay"),
        ))
        .id();
    commands.entity(wrap).add_child(strip);

    let buttons: Vec<Entity> = Shading::ALL
        .into_iter()
        .map(|mode| shading_btn(commands, fonts, mode))
        .collect();
    commands.entity(strip).add_children(&buttons);

    // Gone in play mode: the point of play mode is to see the game, and a
    // shading switch is an authoring control.
    renzora_ember::reactive::tracked::bind_display(commands, wrap, |w| {
        !w.get_resource::<renzora::core::PlayModeState>()
            .map(|p| p.is_in_play_mode())
            .unwrap_or(false)
    });
    wrap
}

fn shading_btn(commands: &mut Commands, fonts: &EmberFonts, mode: Shading) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(BTN),
                height: Val::Px(BTN),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(resting()),
            Interaction::default(),
            mode,
            Name::new("shading-btn"),
        ))
        .id();
    let g = icon_text(commands, &fonts.phosphor, mode.icon(), (235, 235, 240), 15.0);
    // Clicks must reach the button, not stop at the glyph in its dead centre —
    // the same trap the nav buttons hit.
    commands.entity(g).insert(bevy::picking::Pickable::IGNORE);
    commands.entity(b).add_child(g);
    b
}

/// Accent the active mode, wash on hover.
pub(crate) fn shading_visuals(
    settings: Option<Res<ViewportSettings>>,
    suppressed: Option<Res<renzora::core::EnvironmentSuppressed>>,
    mut buttons: Query<(&Shading, &Interaction, &mut BackgroundColor)>,
) {
    let environment = !suppressed.as_deref().is_some_and(|s| s.active);
    let active = settings
        .as_deref()
        .and_then(|s| Shading::current(s, environment));
    for (mode, interaction, mut bg) in &mut buttons {
        bg.0 = if active == Some(*mode) {
            rgb(accent())
        } else if *interaction == Interaction::Hovered {
            rgb(hover_bg())
        } else {
            resting()
        };
    }
}

/// Line the environment up with the render toggles the project restored, once,
/// as the editor opens.
///
/// `ViewportSettings` is saved with the project and `EnvironmentSuppressed` is
/// not, so a project last left in Wireframe would otherwise reopen showing bare
/// wireframes against a full sky, with no button lit to say what mode that is.
///
/// Wireframe and Solid are identifiable from their toggles alone. Material and
/// Rendered are not — that is exactly what the flag is for — so the ambiguous
/// pair resolves to Rendered, the mode a project that never touched this switch
/// has always been in.
pub(crate) fn align_environment_to_restored_settings(
    settings: Option<Res<ViewportSettings>>,
    suppressed: Option<ResMut<renzora::core::EnvironmentSuppressed>>,
) {
    let (Some(settings), Some(mut suppressed)) = (settings, suppressed) else {
        return;
    };
    let want = matches!(
        Shading::current(&settings, false),
        Some(Shading::Wireframe) | Some(Shading::Solid)
    );
    if suppressed.active != want {
        suppressed.active = want;
    }
}

/// The environment comes back in play mode, and the shading mode is returned on
/// the way out.
///
/// The switch is an authoring control and hides itself in play mode, but the
/// flag it wrote does not un-write itself: pressing Play from Wireframe would
/// otherwise run the game under a bare background, with the one control that
/// explains it no longer on screen. Stashing the value rather than just clearing
/// it means leaving play mode puts you back in the mode you were working in.
pub(crate) fn restore_environment_in_play_mode(
    play: Option<Res<renzora::core::PlayModeState>>,
    suppressed: Option<ResMut<renzora::core::EnvironmentSuppressed>>,
    mut stashed: Local<Option<bool>>,
) {
    let Some(mut suppressed) = suppressed else {
        return;
    };
    let playing = play.is_some_and(|p| p.is_in_play_mode());
    match (playing, *stashed) {
        (true, None) => {
            *stashed = Some(suppressed.active);
            if suppressed.active {
                suppressed.active = false;
            }
        }
        (false, Some(was)) => {
            *stashed = None;
            if suppressed.active != was {
                suppressed.active = was;
            }
        }
        _ => {}
    }
}

/// Apply a mode on click.
///
/// Writes the environment alongside the render toggles, because it is one of the
/// switches a mode stands for — see [`Shading::shows_environment`]. Nothing else
/// in the editor writes that flag, so there is no one to fight over it with.
pub(crate) fn shading_clicks(
    buttons: Query<(&Shading, &Interaction), Changed<Interaction>>,
    settings: Option<ResMut<ViewportSettings>>,
    suppressed: Option<ResMut<renzora::core::EnvironmentSuppressed>>,
) {
    let Some(mut settings) = settings else { return };
    let mut suppressed = suppressed;
    for (mode, interaction) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (toggles, viz) = mode.preset();
        settings.render_toggles = toggles;
        settings.visualization_mode = viz;
        if let Some(suppressed) = suppressed.as_mut() {
            let want = !mode.shows_environment();
            if suppressed.active != want {
                suppressed.active = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every preset must be recognised by `current`, or the button you just
    /// pressed would not light up.
    #[test]
    fn each_preset_round_trips() {
        for mode in Shading::ALL {
            let (toggles, viz) = mode.preset();
            let settings = ViewportSettings {
                render_toggles: toggles,
                visualization_mode: viz,
                ..default()
            };
            assert_eq!(
                Shading::current(&settings, mode.shows_environment()),
                Some(mode),
                "{mode:?} did not recognise its own preset"
            );
        }
    }

    /// The four must be distinguishable from each other, or two buttons would
    /// light at once and clicking between them would look broken. The
    /// environment counts: it is what tells Material and Rendered apart.
    #[test]
    fn the_presets_are_distinct() {
        for a in Shading::ALL {
            for b in Shading::ALL {
                if a == b {
                    continue;
                }
                assert_ne!(
                    (a.preset(), a.shows_environment()),
                    (b.preset(), b.shows_environment()),
                    "{a:?} and {b:?} are the same view"
                );
            }
        }
    }

    /// Only the top of the ladder shows the environment. A wireframe over a lit
    /// sky is the bug this pairing exists to prevent.
    #[test]
    fn only_rendered_shows_the_environment() {
        assert!(!Shading::Wireframe.shows_environment());
        assert!(!Shading::Solid.shows_environment());
        assert!(!Shading::Material.shows_environment());
        assert!(Shading::Rendered.shows_environment());
    }

    /// Material and Rendered are the same render settings, told apart by the
    /// world alone. If that ever stops being true the two buttons collapse into
    /// one view, which is what this arrangement replaced.
    #[test]
    fn material_and_rendered_differ_only_by_the_world() {
        assert_eq!(Shading::Material.preset(), Shading::Rendered.preset());

        let (toggles, viz) = Shading::Rendered.preset();
        let settings = ViewportSettings {
            render_toggles: toggles,
            visualization_mode: viz,
            ..default()
        };
        assert_eq!(
            Shading::current(&settings, true),
            Some(Shading::Rendered),
            "with the world on, these settings are Rendered"
        );
        assert_eq!(
            Shading::current(&settings, false),
            Some(Shading::Material),
            "with the world off, the same settings are Material"
        );
    }

    /// A hand-tweaked combination belongs to no mode, and says so rather than
    /// claiming the nearest one.
    #[test]
    fn a_custom_combination_lights_nothing() {
        let (mut toggles, _) = Shading::Rendered.preset();
        toggles.textures = false;
        let settings = ViewportSettings {
            render_toggles: toggles,
            visualization_mode: VisualizationMode::None,
            ..default()
        };
        assert_eq!(Shading::current(&settings, true), None);
        assert_eq!(Shading::current(&settings, false), None);
    }
}
