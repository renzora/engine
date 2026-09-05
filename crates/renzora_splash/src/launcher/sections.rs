//! The dashboard's pages, and the registry other crates add pages to.
//!
//! The splash used to be one screen — recents, two buttons, some links — so
//! there was nothing to navigate and nothing to extend. A dashboard has pages,
//! and the moment it does, the question is *who owns them*. It cannot be this
//! crate alone: the marketplace page needs the catalogue client, the installer
//! and the session, all of which live in `renzora_marketplace`, and
//! `renzora_splash` is a dependency of the runtime — pulling the storefront
//! (with the import pipeline, the audio decoder and `rfd` behind it) down here
//! to draw one page would put all of it in the shipped game binary.
//!
//! So the dependency runs the other way. This module owns the rail, the page
//! host and nothing else; a crate that *can* depend on splash calls
//! [`register_splash_section`] from its own `Plugin::build` and hands over a
//! builder. The built-in pages (Projects, Changelog) register through exactly
//! the same door, so there is one path and not a privileged one plus a public
//! one.

use std::sync::Arc;

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_text_color};
use renzora_ember::reactive::Rx;

use super::style::*;

/// Builds a page body. Handed `Commands` and the loaded fonts, it returns the
/// root node the host adopts.
///
/// Called when the page is first shown and again every time the user comes back
/// to it, so anything live inside must be a reactive binding (`bind_*` /
/// `keyed_list`) rather than a value read at build time — see the recents list
/// in [`super::projects`] for the pattern.
pub type SectionBuilder = Arc<dyn Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync>;

/// One page of the splash dashboard.
pub struct SplashSection {
    /// Stable identity: what [`ActiveSection`] holds, and what a caller passes to
    /// open its own page.
    pub id: &'static str,
    /// Phosphor icon name for the rail row.
    pub icon: &'static str,
    /// Rail label. A `String`, not `&'static str`, so a page can build a
    /// translated label with `renzora::lang::t()`.
    pub label: String,
    /// Rail order, low first. The built-in pages claim 0 and 80, leaving room
    /// for registrations from elsewhere to land between them.
    pub order: i32,
    pub build: SectionBuilder,
}

impl SplashSection {
    pub fn new(
        id: &'static str,
        icon: &'static str,
        label: impl Into<String>,
        order: i32,
        build: impl Fn(&mut Commands, &EmberFonts) -> Entity + Send + Sync + 'static,
    ) -> Self {
        Self { id, icon, label: label.into(), order, build: Arc::new(build) }
    }
}

/// Every registered page, rail order.
#[derive(Resource, Default)]
pub struct SplashSections(Vec<SplashSection>);

impl SplashSections {
    /// Add a page, or replace one already registered under the same `id`.
    ///
    /// Replacing rather than appending matters because a plugin may be added to
    /// the `App` more than once across a hot reload, and two rails rows opening
    /// the same page is a worse outcome than the second registration winning.
    pub fn add(&mut self, section: SplashSection) {
        match self.0.iter().position(|s| s.id == section.id) {
            Some(i) => self.0[i] = section,
            None => self.0.push(section),
        }
        self.0.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.label.cmp(&b.label)));
    }

    pub fn iter(&self) -> impl Iterator<Item = &SplashSection> {
        self.0.iter()
    }

    pub fn get(&self, id: &str) -> Option<&SplashSection> {
        self.0.iter().find(|s| s.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The page to show when [`ActiveSection`] names one that isn't registered —
    /// a plugin that used to provide it is gone, or the id was simply wrong.
    fn first_id(&self) -> Option<&'static str> {
        self.0.first().map(|s| s.id)
    }
}

/// Register a dashboard page from a plugin's `build`.
///
/// Order-independent with respect to `SplashPlugin`: both sides `init_resource`,
/// which leaves an existing registry alone, so it does not matter which plugin
/// is added first.
pub fn register_splash_section(app: &mut App, section: SplashSection) {
    app.init_resource::<SplashSections>();
    app.world_mut().resource_mut::<SplashSections>().add(section);
}

/// The page currently on screen.
#[derive(Resource)]
pub struct ActiveSection(pub String);

impl Default for ActiveSection {
    fn default() -> Self {
        Self(super::projects::SECTION_ID.to_string())
    }
}

/// The node a page's body is built into. `shown` is the id currently in it, so
/// the rebuild is a no-op on every frame the selection has not changed.
#[derive(Component)]
pub(crate) struct SectionHost {
    shown: Option<String>,
}

/// A rail row; pressing it selects `0`.
#[derive(Component, Clone)]
pub(crate) struct NavRow(String);

// ── Rail ─────────────────────────────────────────────────────────────────────

/// A rail row's contents, read out of the registry before the spawn begins.
///
/// `manage_splash` is exclusive and can see the registry; `spawn_splash` has
/// only `Commands` and cannot. Rather than defer the rows through another queue,
/// the ids/icons/labels are lifted out first and handed down.
pub(crate) type RailEntry = (&'static str, &'static str, String);

/// Every registered page as a [`RailEntry`], rail order.
pub(crate) fn rail_entries(world: &World) -> Vec<RailEntry> {
    world
        .get_resource::<SplashSections>()
        .map(|s| s.iter().map(|s| (s.id, s.icon, s.label.clone())).collect())
        .unwrap_or_default()
}

/// The navigation rail: one row per registered page, then the account and
/// language controls pinned to the bottom.
///
/// Rows are built once here rather than through a keyed list, because every
/// registration happens in `Plugin::build` — the registry cannot change after
/// the app has started, so there is nothing for a reactive list to react to.
pub(crate) fn build_rail(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entries: &[RailEntry],
) -> Entity {
    let rail = commands
        .spawn((
            Node {
                width: Val::Px(RAIL_W),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(3.0),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rail_bg()),
            BorderColor::all(border_soft()),
            // The rail is a surface, not a drag handle: a press on the gap
            // between two rows should do nothing, not pick the window up.
            FocusPolicy::Block,
            Name::new("splash-rail"),
        ))
        .id();

    let mut rows: Vec<Entity> = Vec::new();
    rows.push(rail_heading(commands, fonts, "Dashboard"));
    for (id, icon, label) in entries {
        rows.push(nav_row(commands, fonts, id, icon, label));
    }

    // Pushes everything after it to the bottom of the rail.
    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, FocusPolicy::Pass))
        .id();
    rows.push(spacer);
    rows.push(super::account::build_account_row(commands, fonts));
    rows.push(super::account::build_language_picker(commands, fonts));

    commands.entity(rail).add_children(&rows);
    rail
}

fn rail_heading(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_uppercase()),
            ui_font(&fonts.ui, 9.5),
            TextColor(c(104, 112, 132)),
            Node { margin: UiRect::new(Val::Px(8.0), Val::Px(0.0), Val::Px(4.0), Val::Px(4.0)), ..default() },
            FocusPolicy::Pass,
        ))
        .id()
}

/// One rail row: icon, label, and an accent bar down the left when it is the
/// active page.
pub(crate) fn nav_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: &str,
    icon: &str,
    label: &str,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                padding: UiRect::horizontal(Val::Px(9.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FocusPolicy::Block,
            NavRow(id.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("splash-nav-row"),
        ))
        .id();

    let owned = id.to_string();
    let active_id = owned.clone();
    bind_bg(commands, row, move |w| {
        if is_active(w, &active_id) {
            ca(110, 150, 255, 34)
        } else if is_hovered(w, row) {
            panel_hover()
        } else {
            Color::NONE
        }
    });

    let glyph = icon_text(commands, &fonts.phosphor, icon, ICON_MUTED, 15.0);
    commands.entity(glyph).insert(FocusPolicy::Pass);
    let glyph_id = owned.clone();
    bind_text_color(commands, glyph, move |w| {
        if is_active(w, &glyph_id) {
            accent()
        } else {
            text_muted()
        }
    });

    let txt = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    let txt_id = owned;
    bind_text_color(commands, txt, move |w| {
        if is_active(w, &txt_id) {
            text()
        } else {
            text_muted()
        }
    });

    commands.entity(row).add_children(&[glyph, txt]);
    row
}

fn is_active(w: &Rx, id: &str) -> bool {
    w.get_resource::<ActiveSection>().is_some_and(|a| a.0 == id)
}

// ── Page host ────────────────────────────────────────────────────────────────

/// The node a page is built into: everything left of the rail, between the title
/// bar and the status strip.
///
/// **No percentage height, and `min_*: 0` on both axes.** A percentage resolves
/// against the parent's height, which in a flex row is only definite *after* the
/// row has been sized — and a page whose content is taller than the window then
/// sizes the row instead of the other way round. That is not hypothetical: the
/// Changelog page (a dozen releases of notes) pushed the account block, the
/// language picker and the whole status strip off the bottom of the window, and
/// left the page unscrollable because nothing was ever overflowing. Cross-axis
/// stretch gives this node the row's height without asking for it, and the
/// zeroed minimums are what let flexbox shrink it to that height rather than to
/// its content.
pub(crate) fn build_page_host(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(surface()),
            // A page's own background is not a drag handle either.
            FocusPolicy::Block,
            SectionHost { shown: None },
            Name::new("splash-page-host"),
        ))
        .id()
}

/// Swap the host's contents when the selected page changes.
///
/// Exclusive because building a page needs the registry (a `World` resource) and
/// the fonts, and because tearing the old page down has to happen in the same
/// pass — two systems would leave a frame with both pages, or neither.
pub(crate) fn rebuild_section(world: &mut World) {
    let mut q = world.query::<(Entity, &SectionHost)>();
    let Some((host, shown)) = q.iter(world).map(|(e, h)| (e, h.shown.clone())).next() else {
        return;
    };
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };

    // An `ActiveSection` naming a page nobody registered falls back to the first
    // one rather than leaving the dashboard blank.
    let wanted = {
        let sections = world.resource::<SplashSections>();
        let active = world.resource::<ActiveSection>().0.clone();
        if sections.get(&active).is_some() {
            Some(active)
        } else {
            sections.first_id().map(str::to_string)
        }
    };
    let Some(wanted) = wanted else { return };
    if shown.as_deref() == Some(wanted.as_str()) {
        return;
    }
    let Some(build) = world.resource::<SplashSections>().get(&wanted).map(|s| s.build.clone())
    else {
        return;
    };

    let existing: Vec<Entity> = world
        .get::<Children>(host)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for child in existing {
            commands.entity(child).despawn();
        }
        let body = build(&mut commands, &fonts);
        commands.entity(host).add_child(body);
    }
    queue.apply(world);

    if let Some(mut h) = world.get_mut::<SectionHost>(host) {
        h.shown = Some(wanted);
    }
}

pub(crate) fn nav_click(
    q: Query<(&Interaction, &NavRow), Changed<Interaction>>,
    mut active: ResMut<ActiveSection>,
) {
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed && active.0 != row.0 {
            active.0 = row.0.clone();
        }
    }
}
