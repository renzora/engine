//! Two unrelated plugin views that happen to share a word.
//!
//! [`plugins_section`] is the editor's own control over which plugins load at
//! all — a grid of cards, one per installed plugin, ending the Editor page.
//! [`tab_plugins`] is the opposite direction: it renders the settings a loaded
//! plugin *contributed*, one plugin at a time, selected from the sidebar.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::settings_sections::SettingsSectionRegistry;
use renzora_ember::theme::*;
use renzora_ember::widgets::{section, toggle_switch};

use crate::lang::tr;
use crate::rows::{focus_hide, note_row};
use crate::state::A_TEAL;

/// The Plugins "tab" now shows a SINGLE plugin's section — the one selected in
/// the sidebar (`active_sub`), defaulting to the first registered section.
/// Each plugin is its own sidebar category, so this never lists them all.
pub(crate) fn tab_plugins(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    sections: Option<&SettingsSectionRegistry>,
    active_sub: Option<&str>,
) {
    let entries = sections.map(|s| s.0.as_slice()).unwrap_or_default();
    if entries.is_empty() {
        let lbl = commands
            .spawn((
                Text::new(tr("settings.hint.no_plugins")),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_muted())),
                Node {
                    margin: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
            ))
            .id();
        commands.entity(col).add_child(lbl);
        return;
    }
    // Render the selected section (or the first if nothing's selected yet).
    let entry = active_sub
        .and_then(|id| entries.iter().find(|e| e.id == id))
        .unwrap_or(&entries[0]);
    let (sec, body) = section(commands, fonts, &entry.icon, &entry.title, A_TEAL);
    commands.entity(col).add_child(sec);
    let content = (entry.build)(commands, fonts);
    commands.entity(body).add_child(content);
}

/// Every plugin the engine found this launch, as a grid of cards with a switch
/// each.
///
/// # Why the list is not a `read_dir`
///
/// "Is this a plugin?" has a non-obvious answer, twice over: a standalone plugin
/// is a library exporting one specific symbol and not a proc-macro dylib, a
/// native plugin is a *directory* containing `src/lib.rs`, and both loaders also
/// decline entries for reasons of their own — wrong scope for this binary,
/// already linked in, an ABI too old. A panel that scans for itself drifts from
/// the engine the first time either rule moves, and then shows a list that is
/// confidently wrong.
///
/// So both loaders report into [`renzora::PluginInventory`] as they run and this
/// renders that. It reads only contract-crate types, which is why the settings
/// crate needs no dependency on either loader.
///
/// # Why a grid
///
/// The population is a few dozen at most, each with a short name and a one-line
/// status, and the question being asked is "which of these is on?" — a scanning
/// question, not a reading one. A single tall column makes that a scroll; cards
/// put the whole set in view at once.
pub(crate) fn plugins_section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    focus: Option<&str>,
) {
    let (sec, body) = section(commands, fonts, "puzzle-piece", &tr("settings.cat.plugins"), A_TEAL);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "plugins");
    note_row(commands, fonts, body, &tr("settings.hint.plugins_restart"));

    // The grid itself. `keyed_list` spawns each card straight into this
    // container, so the wrapping lives here rather than in the card builder.
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(8.0)),
                ..default()
            },
            Name::new("plugin-grid"),
        ))
        .id();
    renzora_ember::reactive::tracked::keyed_list(commands, grid, plugin_cards);
    commands.entity(body).add_child(grid);
}

/// One card's worth of data, lifted out of the world so the build closure owns
/// it — the builder runs later, with only `Commands`.
#[derive(Clone)]
struct PluginCard {
    id: String,
    kind: String,
    enabled: bool,
    status: String,
    /// Whether `status` describes something wrong, which decides its colour.
    problem: bool,
}

fn plugin_cards(rx: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    use renzora_ember::reactive::KeyedSnapshot;

    let empty = || KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c: &mut Commands, _: &EmberFonts, _| c.spawn(Node::default()).id()),
    };
    let Some(inventory) = rx.get_resource::<renzora::PluginInventory>() else {
        return empty();
    };
    // Read even when nothing is disabled, so the binding subscribes to it and a
    // toggle repaints the card it just changed.
    let disabled = rx.get_resource::<renzora::DisabledPlugins>();

    let cards: Vec<PluginCard> = inventory
        .sorted()
        .into_iter()
        .map(|e| {
            let enabled = !disabled.map(|d| d.contains(&e.id)).unwrap_or(false);
            let (status, problem) = match &e.state {
                // Both halves matter: "Active" is this launch, the switch is
                // intent for the next one. A plugin that is running but toggled
                // off has to say so, or the panel looks like it did nothing.
                renzora::PluginState::Loaded if enabled => (tr("settings.plugin.active"), false),
                renzora::PluginState::Loaded => (tr("settings.plugin.until_restart"), false),
                renzora::PluginState::Disabled if enabled => {
                    (tr("settings.plugin.on_restart"), false)
                }
                renzora::PluginState::Disabled => (tr("settings.plugin.disabled"), false),
                renzora::PluginState::Skipped(why) => (why.clone(), false),
                // A compile error is dozens of lines of rustc output; the whole
                // thing is in the Console, and a card eight pixels tall gets the
                // first line.
                renzora::PluginState::Failed(why) => {
                    (why.lines().next().unwrap_or(why).to_string(), true)
                }
            };
            PluginCard {
                id: e.id.clone(),
                kind: e.kind.label().to_string(),
                enabled,
                status,
                problem,
            }
        })
        .collect();

    if cards.is_empty() {
        let none = tr("settings.hint.no_installed_plugins");
        return KeyedSnapshot {
            items: vec![(0, 0)],
            build: Box::new(move |c: &mut Commands, f: &EmberFonts, _| {
                c.spawn((
                    Text::new(none.clone()),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(text_muted())),
                ))
                .id()
            }),
        };
    }

    // Keyed by identity, hashed on everything drawn — so a card rebuilds when
    // its switch or its status changes, and not otherwise.
    let items: Vec<(u64, u64)> = cards
        .iter()
        .map(|c| {
            (
                hash_str(&format!("{}:{}", c.kind, c.id)),
                hash_str(&format!("{}{}", c.enabled, c.status)),
            )
        })
        .collect();

    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| plugin_card(c, f, &cards[i])),
    }
}

fn plugin_card(commands: &mut Commands, fonts: &EmberFonts, card: &PluginCard) -> Entity {
    let root = commands
        .spawn((
            Node {
                // Four columns, and a percentage basis is what pins it there.
                // 22% × 4 = 88%, leaving 12% for the three 8px gaps at any
                // realistic panel width — so four fit on a row and a fifth
                // cannot, whatever the settings pane is resized to.
                //
                // A pixel basis was the previous attempt and is why this comment
                // exists: `flex_basis` is what the wrap decision measures, so a
                // fixed 170 px gives four columns only at the widths that happen
                // to divide that way, and three-and-a-gap everywhere else. The
                // ragged empty column it replaced (a fixed 210 px card) was the
                // same problem one step earlier.
                //
                // `flex_grow` still shares the leftover space, so a row fills the
                // panel rather than leaving the remainder at its right edge.
                flex_basis: Val::Percent(22.0),
                flex_grow: 1.0,
                // Without this a long plugin name pushes the card wider than its
                // share and the row wraps one card early.
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                // The card is the clipping boundary. A `Failed` status is the
                // first line of rustc output, which is arbitrarily long, and a
                // long plugin id is nearly as bad — either would otherwise run
                // out over the neighbouring card.
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Name::new("plugin-card"),
        ))
        .id();

    // Artwork first, so the grid reads as a shelf of things rather than a list
    // of switches. A plugin without a `thumbnail.jpg` gets its kind's glyph on
    // the same tinted square, which keeps every card the same shape — a card
    // that collapsed to text when art was missing would make the grid ragged,
    // and most plugins do not ship art.
    let thumb = renzora_ember::widgets::file_image_tile(
        commands,
        fonts,
        renzora::core::plugin_thumbnail_path(&card.id).unwrap_or_default(),
        "puzzle-piece",
        placeholder(),
        10.0,
    );

    // The name gets its own full-width line, and the switch moves to a footer
    // beside the kind. They shared a row while this was a text card; once the
    // artwork went in above them, the switch left too narrow a column for a name
    // like `chromatic_aberration`, which ran off the card. Pinning the switch to
    // the end of the footer also puts every card's control in the same place.
    let foot = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    // A marker plus an explicit click system, NOT `bind_2way`.
    //
    // The two-way binding is right for a settings toggle built once, and wrong
    // here for a specific reason: it writes the model whenever the widget's
    // `Bound` disagrees with the getter, and this getter reads the very resource
    // the setter writes, inside a reactive list whose snapshot also reads it. A
    // switch that gets flipped by anything other than a deliberate click — and
    // in Bevy 0.19 `FocusPolicy` defaults to `Pass`, so a press reaches every
    // node under the pointer — is then indistinguishable from the user asking
    // for it, and the write persists to disk immediately.
    //
    // That is not hypothetical: the first version of this panel disabled 70 of
    // 74 installed plugins by itself. A marker read on a real press transition
    // has one write path and no loop.
    let sw = toggle_switch(commands, card.enabled);
    commands.entity(sw).insert((
        PluginToggle { id: card.id.clone() },
        // The switch must swallow its own press rather than let it pass through
        // to whatever is behind it, for the same reason.
        FocusPolicy::Block,
    ));

    // `width: 100%` as well as `no_wrap`: a no-wrap text node sizes itself to its
    // content, so there is nothing for `clip` to clip against without one.
    let name = commands
        .spawn((
            Text::new(card.id.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();

    let kind = commands
        .spawn((
            Text::new(card.kind.clone()),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
        ))
        .id();
    commands.entity(foot).add_children(&[kind, sw]);
    let status = commands
        .spawn((
            Text::new(card.status.clone()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(if card.problem { warn_amber() } else { text_muted() })),
            // One line: a compile failure's first rustc line is long enough to
            // stretch the card several rows tall and make the grid ragged. The
            // whole message is in the Console, which is where it belongs.
            bevy::text::TextLayout::no_wrap(),
            Node { width: Val::Percent(100.0), min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
        ))
        .id();

    commands.entity(root).add_children(&[thumb, name, status, foot]);
    root
}

/// Marks a plugin card's switch with the plugin it controls.
#[derive(Component)]
pub(crate) struct PluginToggle {
    id: String,
}

/// Flip a plugin on or off, and persist it.
///
/// # Why this waits for a RELEASE
///
/// Almost every click handler in the editor fires on `Interaction::Pressed`, and
/// for a button in a short list that is fine. This one writes a file that decides
/// what loads at the next launch, and its widgets sit in a list seventy rows
/// long — so the cost of a spurious press is not a stray click, it is an editor
/// that comes back with most of its plugins missing. That is not hypothetical:
/// the first version of this panel disabled 70 of 74 installed plugins by itself.
///
/// So a toggle needs a press **and** a release on the same switch. Anything that
/// makes a switch read `Pressed` in passing — a drag across the list, a press
/// leaking through a node that did not block it (`FocusPolicy` defaults to `Pass`
/// in Bevy 0.19) — never reaches the release half on that same entity, and is
/// dropped.
///
/// `Changed<Interaction>` on top of that, so a held press is one event rather
/// than one per frame.
pub(crate) fn plugin_toggle_click(
    changed: Query<(Entity, &Interaction, &PluginToggle), Changed<Interaction>>,
    mut armed: Local<Option<Entity>>,
    mut disabled: Option<ResMut<renzora::DisabledPlugins>>,
) {
    for (entity, interaction, toggle) in &changed {
        match interaction {
            // Arm. Nothing is written yet.
            Interaction::Pressed => *armed = Some(entity),
            // Released while still over the switch it was pressed on — a click.
            Interaction::Hovered if *armed == Some(entity) => {
                *armed = None;
                let Some(disabled) = disabled.as_mut() else {
                    continue;
                };
                // The switch's own `switch_interact` has already flipped the
                // visual, so reading `Bound` here would race it. The resource is
                // the single source of truth and the card rebuilds from it.
                let enable = disabled.contains(&toggle.id);
                if !disabled.set_enabled(&toggle.id, enable) {
                    continue;
                }
                // Persisted immediately rather than when the overlay closes. The
                // whole point of this switch is what happens at the NEXT launch,
                // and an editor that crashed before a deferred save would
                // silently discard the one instruction the user gave it.
                if let Err(e) = renzora::save_disabled_plugins(&disabled.0) {
                    warn!("[plugins] could not save the disabled-plugin list: {e}");
                }
            }
            // Left the switch, or released elsewhere. Disarm rather than carry
            // the press to whatever the pointer lands on next.
            _ => {
                if *armed == Some(entity) {
                    *armed = None;
                }
            }
        }
    }
}

fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}
