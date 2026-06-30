//! The **About Renzora Engine** overlay — reached from `Help ▸ About`.
//!
//! A splash-style modal that states what the engine is, shows the current
//! version, and — the part that earns this its own module — credits every
//! vendored third-party "community" crate the engine is built on, each row
//! linking out to that project's upstream repository so the original authors
//! get the visible attribution they're owed.
//!
//! The overlay is spawned by [`process_about_request`] reacting to a
//! [`ShowAboutRequested`] resource that the Help menu item inserts (the same
//! resource-flag → system pattern the exit prompt uses, so the menu closure
//! stays a one-liner). Dismissal (Escape / backdrop click / the title ×) is
//! handled for free by ember's generic `overlay_dismiss`, which despawns the
//! `Overlay` root that [`overlay_sized`] tags — [`AboutRoot`] is only an extra
//! marker so we don't stack a second copy on repeat clicks.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{accent, card_bg, divider, rgb, text_muted, text_primary};
use renzora_ember::widgets::{overlay_sized, scroll_area};

/// Current dev version. Kept as the docs/release scheme (`r1-alphaN`) rather
/// than the crate's semver, since that's what we show users everywhere else.
const VERSION: &str = "r1-alpha7";

/// One-paragraph "what is this" blurb shown under the title.
const DESCRIPTION: &str = "Renzora is an open-source, Bevy-powered game engine and editor. It ships as a \
single binary that runs as the editor when the editor bundle is present and as your game when it \
isn't, built on a modular dlopen plugin system with Lua and Rhai scripting.";

/// A credited upstream project: display name, author, one-line description, and
/// the repo the row links to.
struct Credit {
    name: &'static str,
    by: &'static str,
    desc: &'static str,
    url: &'static str,
}

/// The vendored community crates that live under `crates/` (everything not
/// prefixed `renzora_`), plus Bevy itself as the foundation. In-house helpers
/// (`dynamic_plugin_loader`, `mcp_server_plugin`, `websocket_plugin`) are
/// omitted — they're ours, not third-party attributions. Authors/URLs were
/// taken from each crate's `Cargo.toml`/`LICENSE`; keep this in sync when a
/// vendored crate is added or removed.
const CREDITS: &[Credit] = &[
    Credit {
        name: "Bevy",
        by: "the Bevy contributors",
        desc: "The game engine and ECS that Renzora is built on",
        url: "https://github.com/bevyengine/bevy",
    },
    Credit {
        name: "Bevy Solari",
        by: "jms55",
        desc: "Real-time hardware-raytraced global illumination",
        url: "https://jms55.github.io/",
    },
    Credit {
        name: "Avian Physics",
        by: "Joona Aalto",
        desc: "ECS-driven 2D & 3D physics engine",
        url: "https://github.com/avianphysics/avian",
    },
    Credit {
        name: "bevy_hanabi",
        by: "Jérôme Humbert (djeedai)",
        desc: "GPU-accelerated particle effects",
        url: "https://github.com/djeedai/bevy_hanabi",
    },
    Credit {
        name: "bevy_heavy",
        by: "Joona Aalto",
        desc: "Mass-property computation for geometric primitives",
        url: "https://github.com/Jondolf/bevy_heavy",
    },
    Credit {
        name: "bevy_hui",
        by: "Lorenz Mielke",
        desc: "Pseudo-HTML UI templating",
        url: "https://github.com/Lommix/bevy_hui",
    },
    Credit {
        name: "bevy_mod_outline",
        by: "komadori",
        desc: "Mesh outlining plugin",
        url: "https://github.com/komadori/bevy_mod_outline",
    },
    Credit {
        name: "bevy_oxr",
        by: "awtterpip & the Bevy XR community",
        desc: "OpenXR / WebXR support",
        url: "https://github.com/awtterpip/bevy_oxr",
    },
    Credit {
        name: "bevy_procedural_tree",
        by: "Affinator",
        desc: "Procedurally generated 3D trees",
        url: "https://github.com/Affinator/bevy_procedural_tree",
    },
    Credit {
        name: "bevy_silk",
        by: "Félix de Maneville",
        desc: "Verlet cloth physics",
        url: "https://github.com/ManevilleF/bevy_silk",
    },
    Credit {
        name: "bevy_transform_interpolation",
        by: "Joona Aalto",
        desc: "Transform interpolation for fixed timesteps",
        url: "https://github.com/Jondolf/bevy_transform_interpolation",
    },
    Credit {
        name: "bvh2d",
        by: "François Mockers",
        desc: "Fast 2D bounding-volume hierarchy (SAH)",
        url: "https://github.com/mockersf/bvh2d",
    },
    Credit {
        name: "polyanya",
        by: "François Mockers (vleue)",
        desc: "Compromise-free any-angle pathfinding",
        url: "https://github.com/vleue/polyanya",
    },
    Credit {
        name: "vleue_navigator",
        by: "François Mockers (vleue)",
        desc: "Navigation-mesh plugin",
        url: "https://github.com/vleue/vleue_navigator",
    },
    Credit {
        name: "glam_matrix_extras",
        by: "Joona Aalto",
        desc: "Matrix types & utilities for glam",
        url: "https://github.com/Jondolf/glam_matrix_extras",
    },
    Credit {
        name: "Kira",
        by: "Tesselode (Andrew Minnich)",
        desc: "Audio playback and mixing backend",
        url: "https://github.com/tesselode/kira",
    },
    Credit {
        name: "Tracy Profiler",
        by: "Bartosz Taudul",
        desc: "Real-time frame and CPU profiler",
        url: "https://github.com/wolfpld/tracy",
    },
];

/// Set by the Help menu item; consumed by [`process_about_request`].
#[derive(Resource)]
pub(crate) struct ShowAboutRequested;

/// Backdrop root of the About overlay — used only as an "already open" guard.
#[derive(Component)]
pub(crate) struct AboutRoot;

/// A clickable credit row carrying the repository URL it opens.
#[derive(Component)]
pub(crate) struct AboutCreditLink(String);

/// Open the About overlay when [`ShowAboutRequested`] is present (once).
pub(crate) fn process_about_request(
    req: Option<Res<ShowAboutRequested>>,
    fonts: Option<Res<EmberFonts>>,
    open: Query<(), With<AboutRoot>>,
    mut commands: Commands,
) {
    if req.is_none() {
        return;
    }
    commands.remove_resource::<ShowAboutRequested>();
    // Already showing, or we can't render text yet — ignore.
    if !open.is_empty() {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    spawn_about(&mut commands, &fonts);
}

/// Open a credit row's repo in the browser on click.
pub(crate) fn about_credit_click(q: Query<(&Interaction, &AboutCreditLink), Changed<Interaction>>) {
    for (interaction, link) in &q {
        if *interaction == Interaction::Pressed {
            crate::open_url(&link.0);
        }
    }
}

/// Highlight a credit row while hovered (the rows are otherwise transparent).
pub(crate) fn about_credit_hover(
    mut q: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<AboutCreditLink>)>,
) {
    for (interaction, mut bg) in &mut q {
        *bg = match *interaction {
            Interaction::Hovered | Interaction::Pressed => BackgroundColor(rgb(card_bg())),
            Interaction::None => BackgroundColor(Color::NONE),
        };
    }
}

/// Build the centered About modal: title + version, blurb, then the scrollable
/// credits list.
fn spawn_about(commands: &mut Commands, fonts: &EmberFonts) {
    let (root, content) =
        overlay_sized(commands, fonts, "About Renzora Engine", 560.0, 600.0, true);
    commands.entity(root).insert(AboutRoot);

    // Padded column inside the card's flexible content area.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            row_gap: Val::Px(12.0),
            padding: UiRect::all(Val::Px(18.0)),
            ..default()
        })
        .id();

    let title = commands
        .spawn((
            Text::new("Renzora Engine"),
            ui_font(&fonts.ui, 22.0),
            TextColor(rgb(text_primary())),
        ))
        .id();

    let version = commands
        .spawn((
            Text::new(format!("Version {VERSION}")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(accent())),
        ))
        .id();

    let desc = commands
        .spawn((
            Text::new(DESCRIPTION),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_muted())),
        ))
        .id();

    let rule = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id();

    let heading = commands
        .spawn((
            Text::new("Built with these open-source community projects"),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
        ))
        .id();

    // Credit rows in a column, wrapped in a height-capped scroll viewport.
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let rows: Vec<Entity> = CREDITS.iter().map(|c| credit_row(commands, fonts, c)).collect();
    commands.entity(list).add_children(&rows);
    let scroll = scroll_area(commands, list, 320.0);

    commands
        .entity(body)
        .add_children(&[title, version, desc, rule, heading, scroll]);
    commands.entity(content).add_child(body);
}

/// One credit row: name + "desc · by author" on the left, a GitHub glyph on the
/// right. The whole row is the click target (carrying [`AboutCreditLink`]).
fn credit_row(commands: &mut Commands, fonts: &EmberFonts, c: &Credit) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            AboutCreditLink(c.url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new(format!("about-credit:{}", c.name)),
        ))
        .id();

    let info = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let name = commands
        .spawn((
            Text::new(c.name),
            ui_font(&fonts.ui, 13.5),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    let sub = commands
        .spawn((
            Text::new(format!("{} · by {}", c.desc, c.by)),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(info).add_children(&[name, sub]);

    let icon = icon_text(commands, &fonts.phosphor, "github-logo", text_muted(), 16.0);
    commands.entity(icon).insert(FocusPolicy::Pass);

    commands.entity(row).add_children(&[info, icon]);
    row
}
