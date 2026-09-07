//! The **About Renzora Engine** overlay — reached from the brand mark in the top
//! bar, or from `Help ▸ About`.
//!
//! A splash-style modal that states what the engine is, shows the current
//! version, and credits the people and the projects it is built on: the
//! [contributors](crate::contributors) to this repository, fetched from GitHub,
//! and every vendored third-party crate under `crates/`, each linking out to its
//! upstream repository so the original authors get the visible attribution they
//! are owed.
//!
//! It is deliberately **darker than the editor around it** and built from its
//! own parts rather than the standard dialog chrome. Every other overlay is a
//! dialog you are trying to get through; this one is the product's face, and it
//! is the only place in the editor where that is worth a bespoke layout.
//!
//! The overlay is spawned by [`process_about_request`] reacting to a
//! [`ShowAboutRequested`] resource that the menu item inserts (the same
//! resource-flag → system pattern the exit prompt uses, so the menu closure
//! stays a one-liner). Dismissal (Escape / backdrop click / the title ×) is
//! handled for free by ember's generic `overlay_dismiss`, which despawns the
//! `Overlay` root that `overlay_val_parts` tags — [`AboutRoot`] is only an extra
//! marker so we don't stack a second copy on repeat clicks.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_with};
use renzora_ember::theme::{
    accent, border, card_bg, divider, faint_bg, placeholder, rgb, text_muted, text_primary,
    window_bg,
};
use renzora_ember::widgets::{
    overlay_val_parts, scroll_view, FileImageWanted, FileImages, WebImageWanted, WebImages,
};

use crate::contributors::{Contributor, Contributors};

/// Version line for the About modal. `renzora::version::display()` renders the
/// docs/release scheme (`r1-alphaN`) rather than the crate's semver — and appends
/// the nightly date or `(dev)` when this isn't a tagged release, so a bug report
/// from a nightly names the exact build.
fn version_line() -> String {
    renzora::version::display()
}

/// One-paragraph "what is this" blurb shown under the title.
const DESCRIPTION: &str = "A fully featured modular 2D and 3D game engine powered by Bevy, with an \
extensible plugin system.";

/// The repository the brand mark and the contributor section link to.
const REPO_URL: &str = "https://github.com/renzora/engine";

/// How many contributors the section shows before it stops. The API hands back
/// up to a hundred and the row would keep growing; this is a credit, not a
/// leaderboard, and the overflow is named as a count instead.
const MAX_CONTRIBUTORS: usize = 18;

/// Modal size. Wider than the 560px this was, because the credits are a
/// two-column grid now and two columns of 260px is what the tiles want; taller
/// because the header and the contributor strip are above them.
const WIDTH: f32 = 720.0;
const HEIGHT: f32 = 640.0;

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

/// A clickable tile, chip or button carrying the URL it opens.
#[derive(Component)]
pub(crate) struct AboutCreditLink(String);

/// The contributor strip, carrying the section it lives in so
/// [`about_contributors_fill`] can reveal it once there is something to show.
#[derive(Component)]
pub(crate) struct AboutContributorStrip {
    section: Entity,
}

/// Open the About overlay when [`ShowAboutRequested`] is present (once).
pub(crate) fn process_about_request(
    req: Option<Res<ShowAboutRequested>>,
    fonts: Option<Res<EmberFonts>>,
    open: Query<(), With<AboutRoot>>,
    mut contributors: ResMut<Contributors>,
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
    // Ask GitHub on the first open, not at startup: one request for a dialog
    // most sessions never open. The section fills itself in when the answer
    // lands (`about_contributors_fill`), so the first open is not the one that
    // misses out.
    contributors.start();
    spawn_about(&mut commands, &fonts, &contributors);
}

/// Open a credit row's repo in the browser on click.
pub(crate) fn about_credit_click(q: Query<(&Interaction, &AboutCreditLink), Changed<Interaction>>) {
    for (interaction, link) in &q {
        if *interaction == Interaction::Pressed {
            crate::open_url(&link.0);
        }
    }
}

/// Lift a tile or chip out of the card while the cursor is on it.
///
/// A binding rather than the `Changed<Interaction>` system this used to be: the
/// resting fill is no longer transparent, so the "not hovered" branch has a real
/// colour to restore, and a theme change has to move both. A binding re-reads
/// the palette; a system that wrote a colour once did not.
fn hover_lift(commands: &mut Commands, e: Entity) {
    bind_bg(commands, e, move |w| match w.get::<Interaction>(e) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(card_bg()),
        _ => rgb(faint_bg()),
    });
}

/// Build the centered About modal: the brand header, the contributor strip, and
/// the grid of upstream projects.
fn spawn_about(commands: &mut Commands, fonts: &EmberFonts, contributors: &Contributors) {
    let parts = overlay_val_parts(
        commands,
        fonts,
        "About",
        Val::Px(WIDTH),
        Val::Px(HEIGHT),
        true,
    );
    commands.entity(parts.root).insert(AboutRoot);
    // Darker than the editor around it, and darker than every other overlay:
    // the card is `window_bg` (the chrome shade, a step under a panel) instead
    // of `popup_bg`, and the title bar drops its own fill so the header runs
    // straight into the body as one surface.
    commands
        .entity(parts.card)
        .insert(BackgroundColor(rgb(window_bg())));
    commands
        .entity(parts.titlebar)
        .insert(BackgroundColor(Color::NONE));

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            row_gap: Val::Px(14.0),
            padding: UiRect {
                left: Val::Px(20.0),
                right: Val::Px(20.0),
                top: Val::Px(4.0),
                bottom: Val::Px(18.0),
            },
            ..default()
        })
        .id();

    let header = brand_header(commands, fonts);
    let desc = commands
        .spawn((
            Text::new(DESCRIPTION),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let people = contributors_section(commands, fonts, contributors);
    let projects = projects_section(commands, fonts);

    commands
        .entity(body)
        .add_children(&[header, desc, people, projects]);
    commands.entity(parts.content).add_child(body);
}

/// The mark, the name, the version, and a link to the repository.
fn brand_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .id();

    let mark = brand_mark(commands, fonts, 54.0);

    let text = commands
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    let name = commands
        .spawn((
            Text::new("Renzora Engine"),
            ui_font(&fonts.ui, 24.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let version = commands
        .spawn((
            Text::new(format!("Version {}", version_line())),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(accent())),
        ))
        .id();
    commands.entity(text).add_children(&[name, version]);

    let repo = link_button(commands, fonts, "github-logo", "GitHub", REPO_URL);

    commands.entity(row).add_children(&[mark, text, repo]);
    row
}

/// The engine's own icon at `size`, with a glyph standing in where the file is
/// not there.
///
/// The same `resources/icon.png` beside the executable that the top bar's mark
/// and the splash's title bar use, through ember's on-disk image cache — the
/// `AssetServer` resolves against the open *project*'s assets, and this is the
/// engine's own art. A bare `cargo run` stages no icon, which is what the glyph
/// is for.
fn brand_mark(commands: &mut Commands, fonts: &EmberFonts, size: f32) -> Entity {
    let frame = commands
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("about-brand-mark"),
        ))
        .id();

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = crate::top_menu::brand_icon_path() {
        let img = commands
            .spawn((
                ImageNode::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    // Revealed once the decode lands, so a blank `ImageNode`
                    // never flashes as a white square.
                    display: Display::None,
                    ..default()
                },
                FocusPolicy::Pass,
                FileImageWanted(path.clone()),
            ))
            .id();
        bind_with(
            commands,
            img,
            move |w| w.get_resource::<FileImages>().and_then(|c| c.get(&path)),
            reveal_image,
        );
        commands.entity(frame).add_child(img);
        return frame;
    }

    let glyph = icon_text(commands, &fonts.phosphor, "cube", accent(), size * 0.6);
    commands.entity(glyph).insert(FocusPolicy::Pass);
    commands.entity(frame).add_child(glyph);
    frame
}

/// Swap a bound `Handle<Image>` into an `ImageNode` and reveal it. Shared by the
/// brand mark and the contributor avatars, which differ only in which cache they
/// read.
fn reveal_image(w: &mut World, e: Entity, handle: &Option<Handle<Image>>) {
    let Some(h) = handle else { return };
    if let Some(mut n) = w.get_mut::<ImageNode>(e) {
        if n.image != *h {
            n.image = h.clone();
        }
    }
    if let Some(mut node) = w.get_mut::<Node>(e) {
        node.display = Display::Flex;
    }
}

/// A section heading: small, muted, letter-spaced by being upper-cased. The two
/// sections below are told apart by these rather than by rules, so the modal
/// stays a single dark surface instead of a stack of boxes.
fn section_heading(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_uppercase()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let rule = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id();
    commands.entity(row).add_children(&[text, rule]);
    row
}

/// The people who built this one, as a wrapping strip of avatars.
///
/// Built empty when the fetch has not landed yet and filled by
/// [`about_contributors_fill`] the moment it does, so the first open of the
/// session is not the one that misses out. An editor with no network never
/// fills it, and the section hides itself rather than sitting there empty.
fn contributors_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    contributors: &Contributors,
) -> Entity {
    let section = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                // Hidden until there is something in it: an empty heading over
                // an empty strip is worse than no section.
                display: if contributors.list.is_empty() {
                    Display::None
                } else {
                    Display::Flex
                },
                ..default()
            },
            Name::new("about-contributors"),
        ))
        .id();
    let heading = section_heading(commands, fonts, "Contributors");
    let strip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                ..default()
            },
            AboutContributorStrip { section },
            Name::new("about-contributor-strip"),
        ))
        .id();
    commands.entity(section).add_children(&[heading, strip]);
    fill_strip(commands, fonts, strip, &contributors.list);
    section
}

/// Spawn one chip per contributor into `strip`. Split out because it runs both
/// at build time and again from [`about_contributors_fill`] when the fetch
/// lands after the overlay is already open.
fn fill_strip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    strip: Entity,
    list: &[Contributor],
) {
    let chips: Vec<Entity> = list
        .iter()
        .take(MAX_CONTRIBUTORS)
        .map(|c| contributor_chip(commands, fonts, c))
        .collect();
    commands.entity(strip).add_children(&chips);
    if list.len() > MAX_CONTRIBUTORS {
        let more = commands
            .spawn((
                Text::new(format!("+{} more", list.len() - MAX_CONTRIBUTORS)),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_muted())),
                Node {
                    margin: UiRect::left(Val::Px(4.0)),
                    ..default()
                },
            ))
            .id();
        commands.entity(strip).add_child(more);
    }
}

/// One contributor: a round avatar with the login beside it, opening their
/// GitHub profile on click.
fn contributor_chip(commands: &mut Commands, fonts: &EmberFonts, c: &Contributor) -> Entity {
    const AVATAR: f32 = 22.0;

    let chip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::new(Val::Px(4.0), Val::Px(9.0), Val::Px(4.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(AVATAR / 2.0 + 4.0)),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            Interaction::default(),
            AboutCreditLink(c.html_url.clone()),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new(format!("about-contributor:{}", c.login)),
        ))
        .id();
    hover_lift(commands, chip);

    // The avatar frame doubles as the placeholder: a filled circle that the
    // image covers once it arrives, so a slow download leaves a chip that looks
    // deliberate rather than broken.
    let frame = commands
        .spawn((
            Node {
                width: Val::Px(AVATAR),
                height: Val::Px(AVATAR),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(AVATAR / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
            FocusPolicy::Pass,
        ))
        .id();
    let url = c.avatar_url.clone();
    let img = commands
        .spawn((
            ImageNode::default(),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                ..default()
            },
            FocusPolicy::Pass,
            WebImageWanted(url.clone()),
        ))
        .id();
    bind_with(
        commands,
        img,
        move |w| w.get_resource::<WebImages>().and_then(|c| c.get(&url)),
        reveal_image,
    );
    commands.entity(frame).add_child(img);

    let name = commands
        .spawn((
            Text::new(&c.login),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(chip).add_children(&[frame, name]);
    chip
}

/// The upstream projects, as a scrolling two-column grid of tiles.
///
/// A grid rather than the list this was: fifteen full-width rows made a modal
/// that was mostly scrollbar, and the rows carried a repo link and two lines of
/// text in a space wide enough for a paragraph. Two columns fit the same credits
/// in half the height, and a tile is the shape the content already had.
fn projects_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let section = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let heading = section_heading(commands, fonts, "Built with");

    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                display: Display::Grid,
                grid_template_columns: vec![bevy::ui::RepeatedGridTrack::flex(2, 1.0)],
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("about-projects"),
        ))
        .id();
    let tiles: Vec<Entity> = CREDITS
        .iter()
        .map(|c| credit_tile(commands, fonts, c))
        .collect();
    commands.entity(grid).add_children(&tiles);
    // `scroll_view`, not `scroll_area`: the flex-filling variant. The grid takes
    // whatever height the modal has left and scrolls past that, so the credits
    // never decide how tall the window is — a capped `scroll_area` would size
    // itself to its content and push the modal around instead.
    let scroll = scroll_view(commands, grid);

    commands.entity(section).add_children(&[heading, scroll]);
    section
}

/// A small link button: glyph + label in a bordered pill.
fn link_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    url: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            AboutCreditLink(url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new(format!("about-link:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| match w.get::<Interaction>(btn) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(faint_bg()),
        _ => Color::NONE,
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, text]);
    btn
}

/// Fill the contributor strip once the GitHub fetch lands, for an About overlay
/// that was opened before the answer arrived.
///
/// Keyed on the strip having no children rather than on a "done" flag: the
/// overlay is despawned and rebuilt on every open, so the only state that
/// survives is the resource, and "has this one been filled" is a question the
/// tree already answers.
pub(crate) fn about_contributors_fill(
    contributors: Res<Contributors>,
    fonts: Option<Res<EmberFonts>>,
    strips: Query<(Entity, &AboutContributorStrip, Option<&Children>)>,
    mut commands: Commands,
) {
    if contributors.list.is_empty() {
        return;
    }
    let Some(fonts) = fonts else { return };
    for (strip, marker, children) in &strips {
        if children.is_some_and(|c| !c.is_empty()) {
            continue;
        }
        fill_strip(&mut commands, &fonts, strip, &contributors.list);
        commands
            .entity(marker.section)
            .entry::<Node>()
            .and_modify(|mut n| n.display = Display::Flex);
    }
}

/// One project tile: the name with a GitHub glyph beside it, the description
/// under it, and the author under that. The whole tile is the click target
/// (carrying [`AboutCreditLink`]).
///
/// The description wraps to as many lines as it needs and the tiles are laid out
/// on a grid, so a long one makes its whole row taller rather than being cut
/// off. Fifteen credits is a fixed list; it is not worth truncating.
fn credit_tile(commands: &mut Commands, fonts: &EmberFonts, c: &Credit) -> Entity {
    let tile = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(faint_bg())),
            BorderColor::all(Color::NONE),
            Interaction::default(),
            AboutCreditLink(c.url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new(format!("about-credit:{}", c.name)),
        ))
        .id();
    hover_lift(commands, tile);

    let head = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let name = commands
        .spawn((
            Text::new(c.name),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    let spacer = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(6.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "github-logo", text_muted(), 13.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    commands.entity(head).add_children(&[name, spacer, icon]);

    let desc = commands
        .spawn((
            Text::new(c.desc),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    let by = commands
        .spawn((
            Text::new(format!("by {}", c.by)),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(placeholder())),
            FocusPolicy::Pass,
        ))
        .id();

    commands.entity(tile).add_children(&[head, desc, by]);
    tile
}
