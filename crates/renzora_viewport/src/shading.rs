//! The viewport shading switch — wireframe / solid / material / rendered.
//!
//! One dropdown on the toolbar, immediately left of Maximize. Its trigger shows
//! the current mode's icon; the menu lists the four ways you look at a scene
//! while building it, under the names every 3D tool gives them.
//!
//! They floated over the scene once, then sat on the bar as four buttons. Four
//! buttons were a lot of the bar for a switch you set and leave, and they were
//! what pushed the bar onto a second line first, so they fold into one trigger
//! now. They are still *view* state rather than a session action, which is why
//! the dropdown sits beside Maximize on the right of the bar rather than among
//! the draggable tool groups on the left.
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
//! [`EnvironmentSuppressed`](renzora_render_api::EnvironmentSuppressed), and Rendered
//! restores it. That flag is *the* thing separating Material from Rendered,
//! whose render toggles are otherwise identical, so it is a switch these buttons
//! write and compare like any other — not something derived from the toggles
//! afterwards, which could not tell those two apart.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_viewport_api::{RenderToggles, ViewportSettings, VisualizationMode};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{rgb, tab_active, text_muted, text_primary};
use renzora_ember::widgets::{popup_anchor, popup_panel, HoverTooltip, Popup};
use renzora_ember::UiInteraction;

use crate::tool_buttons::SIDE_ICON;
use crate::toolbar::display::DisplayTrigger;

/// Row and trigger height, matching the toolbar's other dropdowns.
const BTN_H: f32 = 22.0;

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

    /// The row's label in the menu.
    fn label(self) -> String {
        let (key, fallback) = match self {
            Self::Wireframe => ("viewport.shading.wireframe", "Wireframe"),
            Self::Solid => ("viewport.shading.solid", "Solid"),
            Self::Material => ("viewport.shading.material", "Material"),
            Self::Rendered => ("viewport.shading.rendered", "Rendered"),
        };
        renzora_lang_api::t_or(key, fallback)
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

/// A menu row's icon and label, so [`shading_visuals`] can recolour them: both
/// follow the theme, and both flip to the active foreground on the accent fill.
#[derive(Component, Clone, Copy)]
pub(crate) struct ShadingRow {
    glyph: Entity,
    label: Entity,
}

/// The dropdown's trigger, pointing at its glyph so [`shading_visuals`] can
/// swap it to the current mode's icon.
#[derive(Component, Clone, Copy)]
pub(crate) struct ShadingMenu {
    glyph: Entity,
}

/// What the trigger shows when the switches match none of the four modes: the
/// Display dropdown's sliders, since a hand-tuned mix is what that menu makes.
const CUSTOM_ICON: &str = "faders";

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

/// Build the shading dropdown for the viewport toolbar.
///
/// Mounted by [`crate::toolbar`] into the right-hand cluster next to Maximize.
/// The trigger is the toolbar's icon-and-caret dropdown shape, tagged
/// `DisplayTrigger` so it themes with the options menu beside it and needs no
/// background system of its own. The panel is right-aligned because the
/// trigger is near the right edge of the window.
pub(crate) fn build_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let rows: Vec<Entity> = Shading::ALL
        .into_iter()
        .map(|mode| shading_row(commands, fonts, mode))
        .collect();
    let panel = popup_panel(commands, &rows);
    // The standard popup is sized for the switch panels; four short rows do not
    // need 220px. Patched onto the panel's own `Node` so its styling stays.
    commands.queue(move |world: &mut World| {
        if let Some(mut node) = world.get_mut::<Node>(panel) {
            node.min_width = Val::Px(150.0);
        }
    });

    let glyph = icon_text(
        commands,
        &fonts.phosphor,
        Shading::Rendered.icon(),
        text_muted(),
        15.0,
    );
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 8.0);
    let trigger = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(BTN_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            UiInteraction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            HoverTooltip::new(renzora_lang_api::t_or("viewport.shading.title", "Shading")),
            Popup::new(panel),
            DisplayTrigger,
            ShadingMenu { glyph },
            Name::new("shading-menu"),
        ))
        .id();
    commands.entity(trigger).add_children(&[glyph, caret]);
    let anchor = popup_anchor(commands, trigger, panel);

    // Gone in play mode: the point of play mode is to see the game, and a
    // shading switch is an authoring control.
    renzora_ember::reactive::tracked::bind_display(commands, anchor, |w| {
        !w.get_resource::<renzora_project_api::PlayModeState>()
            .map(|p| p.is_in_play_mode())
            .unwrap_or(false)
    });
    anchor
}

/// One mode in the menu: its icon, then its name.
fn shading_row(commands: &mut Commands, fonts: &EmberFonts, mode: Shading) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, mode.icon(), text_muted(), SIDE_ICON);
    let label = commands
        .spawn((
            Text::new(mode.label()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    // Clicks must reach the row, not stop at the glyph or the text on it.
    for e in [glyph, label] {
        commands.entity(e).insert(bevy::picking::Pickable::IGNORE);
    }
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            mode,
            ShadingRow { glyph, label },
            Name::new("shading-row"),
        ))
        .id();
    commands.entity(row).add_children(&[glyph, label]);
    row
}

/// Accent the active mode's row, wash on hover, and show the active mode's icon
/// on the trigger.
///
/// Fill, glyph and label all come from the theme, matching `tool_buttons`'s
/// scheme so the lit row reads the way a lit tool button does. A hand-tweaked
/// combination lights no row and puts [`CUSTOM_ICON`] on the trigger, rather
/// than claiming the nearest mode.
pub(crate) fn shading_visuals(
    settings: Option<Res<ViewportSettings>>,
    suppressed: Option<Res<renzora_render_api::EnvironmentSuppressed>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    mut rows: Query<(&Shading, &UiInteraction, &ShadingRow, &mut BackgroundColor)>,
    menus: Query<&ShadingMenu>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let Some(theme) = theme else { return };
    let col = |c: renzora_theme::ThemeColor| {
        let [r, g, b, _a] = c.to_array();
        Color::srgb_u8(r, g, b)
    };
    let accent = col(theme.active_theme.semantic.accent);
    let hovered = col(theme.active_theme.widgets.hovered_bg);
    let fg_active = col(theme.active_theme.widgets.active_fg);
    let icon_inactive = col(theme.active_theme.text.secondary);
    let label_inactive = col(theme.active_theme.text.primary);

    let environment = !suppressed.as_deref().is_some_and(|s| s.active);
    let active = settings
        .as_deref()
        .and_then(|s| Shading::current(s, environment));
    for (mode, interaction, row, mut bg) in &mut rows {
        let is_active = active == Some(*mode);
        let want = if is_active {
            accent
        } else if *interaction == UiInteraction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
        for (e, inactive) in [(row.glyph, icon_inactive), (row.label, label_inactive)] {
            let want = if is_active { fg_active } else { inactive };
            if let Ok((_, mut tc)) = texts.get_mut(e) {
                if tc.0 != want {
                    tc.0 = want;
                }
            }
        }
    }

    let icon = active.map_or(CUSTOM_ICON, Shading::icon);
    let Some(ch) = icon_glyph(icon) else { return };
    for menu in &menus {
        if let Ok((mut text, mut tc)) = texts.get_mut(menu.glyph) {
            if text.0.chars().ne(std::iter::once(ch)) {
                text.0 = ch.to_string();
            }
            if tc.0 != icon_inactive {
                tc.0 = icon_inactive;
            }
        }
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
    suppressed: Option<ResMut<renzora_render_api::EnvironmentSuppressed>>,
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
    play: Option<Res<renzora_project_api::PlayModeState>>,
    suppressed: Option<ResMut<renzora_render_api::EnvironmentSuppressed>>,
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
///
/// The menu closes on a pick. Ember's outside-click dismiss deliberately leaves
/// a popup open when the click lands inside it, which suits the switch panels;
/// this is a one-of-four choice, so it closes like the view-angle menu does.
pub(crate) fn shading_clicks(
    buttons: Query<(&Shading, &UiInteraction), Changed<UiInteraction>>,
    mut menus: Query<&mut Popup, With<ShadingMenu>>,
    mut nodes: Query<&mut Node>,
    settings: Option<ResMut<ViewportSettings>>,
    suppressed: Option<ResMut<renzora_render_api::EnvironmentSuppressed>>,
) {
    let Some(mut settings) = settings else { return };
    let mut suppressed = suppressed;
    for (mode, interaction) in &buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        for mut popup in &mut menus {
            popup.open = false;
            if let Ok(mut n) = nodes.get_mut(popup.panel) {
                n.display = Display::None;
            }
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
