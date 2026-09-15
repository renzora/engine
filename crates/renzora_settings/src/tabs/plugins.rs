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
/// "Is this a plugin?" has a non-obvious answer: a *directory* whose manifest
/// declares a `dylib`, or one holding nothing but a prebuilt `build/`. The
/// loader also declines entries for reasons of its own — wrong scope for this
/// binary, no shared engine image, a build that failed. A panel that scans for
/// itself drifts from the engine the first time a rule moves, and then shows a
/// list that is confidently wrong.
///
/// So the loader reports into [`renzora::PluginInventory`] as it runs and this
/// renders that. It reads only contract-crate types, which is why the settings
/// crate needs no dependency on the loader.
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

    // "Where is that folder?" is a real question, not a convenience. On macOS
    // the writable plugins root is under `~/Library/Application Support`, and
    // `~/Library` carries the `hidden` flag — so the directory the empty-state
    // hint tells people to drop a plugin into is one Finder will not show them.
    // A button is the only answer that works on every platform without the
    // panel explaining a different path for each.
    let open_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            padding: UiRect::horizontal(Val::Px(8.0)),
            ..default()
        })
        .id();
    let open = renzora_ember::widgets::icon_label_button(
        commands,
        fonts,
        "folder-open",
        &tr("settings.plugin.open_folder"),
    );
    commands.entity(open).insert((OpenPluginsFolder, FocusPolicy::Block));
    commands.entity(open_row).add_child(open);
    commands.entity(body).add_child(open_row);

    // The grid itself. `keyed_list` spawns each card straight into this
    // container, so the wrapping lives here rather than in the card builder.
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                // Percent, so it adds up with the cards' percentage basis to
                // exactly four per row at every width (see `plugin_card`). A
                // pixel gap beside a percentage card is arithmetic that only
                // works out at one panel width.
                column_gap: Val::Percent(1.0),
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
    enabled: bool,
    status: String,
    /// Whether `status` describes something wrong, which decides its colour.
    problem: bool,
    /// Whether this plugin can be deleted — false for one sealed inside the
    /// macOS `.app`, which is read-only in every sense that matters (see
    /// `renzora::core::delete_plugin`). Decided once here rather than in the
    /// card builder, which has no world to ask.
    removable: bool,
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
            // Asked here rather than in the card builder, which has no world —
            // and answered by the same rule `delete_plugin` enforces, so a
            // button is never offered for something that would be refused.
            let removable = renzora::core::plugin_is_removable(&e.id);
            PluginCard { id: e.id.clone(), enabled, status, problem, removable }
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
                hash_str(&c.id),
                hash_str(&format!("{}{}{}", c.enabled, c.status, c.removable)),
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
                // Four columns, and a percentage basis is what pins it there:
                // 24% × 4 = 96%, leaving the three 1% gaps (the row sets them in
                // percent too, so this arithmetic holds at any panel width) and
                // 1% of slack against sub-pixel rounding. Four fit on a row and
                // a fifth cannot, whatever the settings pane is resized to.
                //
                // A pixel basis was the previous attempt and is why this comment
                // exists: `flex_basis` is what the wrap decision measures, so a
                // fixed 170 px gives four columns only at the widths that happen
                // to divide that way, and three-and-a-gap everywhere else. The
                // ragged empty column it replaced (a fixed 210 px card) was the
                // same problem one step earlier.
                //
                // **No `flex_grow`.** It shared the leftover space so a row would
                // fill the panel, which is right for a full row and wrong for the
                // last one: flex distributes that space per *line*, so a line
                // holding one card gave it the whole width. The grid ended in a
                // single card four times the size of every other. A card is a
                // fixed share of the row now, and a short last row simply stops
                // early — which is what a grid does.
                flex_basis: Val::Percent(24.0),
                flex_grow: 0.0,
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
    // of switches. A plugin without artwork gets a glyph on the same
    // tinted square, which keeps every card the same shape — a card that
    // collapsed to text when art was missing would make the grid ragged, and
    // most plugins do not ship art.
    let thumb = renzora_ember::widgets::file_image_tile(
        commands,
        fonts,
        renzora::core::plugin_thumbnail_path(&card.id).unwrap_or_default(),
        "puzzle-piece",
        placeholder(),
        10.0,
    );

    // The name gets its own full-width line, and the switch moves to a footer
    // below it. They shared a row while this was a text card; once the artwork
    // went in above them, the switch left too narrow a column for a name like
    // `chromatic_aberration`, which ran off the card. Pinning the switch to the
    // end of the footer also puts every card's control in the same place.
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

    // A bare spacer, so the switch sits at the end of the footer on every card
    // whatever else the row holds. See the note above on why a fixed position
    // matters more than the couple of pixels it costs.
    let spacer = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() })
        .id();

    // Delete sits before the switch, so the control every card has is in the
    // same place whether or not this one can be removed — a plugin sealed in the
    // macOS bundle gets no button, and a switch that shifted sideways to fill the
    // gap would make the grid read as two different kinds of card.
    let mut children = vec![spacer];
    if card.removable {
        let del = renzora_ember::widgets::icon_button(commands, fonts, "trash");
        commands.entity(del).insert((
            PluginDelete { id: card.id.clone() },
            // Same reason as the switch: a press that passes through to whatever
            // is behind it is indistinguishable from one aimed at this button,
            // and this one deletes a directory.
            FocusPolicy::Block,
        ));
        children.push(del);
    }
    children.push(sw);
    commands.entity(foot).add_children(&children);
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

/// Marks a card's delete button with the plugin it would remove.
#[derive(Component)]
pub(crate) struct PluginDelete {
    id: String,
}

/// Marks the section's "open the plugins folder" button.
#[derive(Component)]
pub(crate) struct OpenPluginsFolder;

/// The two buttons on the confirmation dialog, and the plugin they answer for.
#[derive(Component)]
pub(crate) struct PluginDeleteConfirm {
    id: String,
    /// Whether this is the button that goes through with it.
    confirm: bool,
}

/// Open the plugins folder in the OS file manager.
///
/// Fires on a press rather than a press-and-release: unlike the toggle and the
/// delete button, the worst a spurious one can do is open a window.
pub(crate) fn open_plugins_folder_click(
    q: Query<&Interaction, (With<OpenPluginsFolder>, Changed<Interaction>)>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    match renzora::core::plugins_dir() {
        Some(dir) => renzora::core::reveal_in_explorer(&dir),
        None => warn!("[plugins] could not work out where the plugins folder is"),
    }
}

/// Ask before deleting a plugin.
///
/// Press **and** release on the same button, for the reason
/// [`plugin_toggle_click`] spells out at length — these sit in the same long
/// list of cards, and the consequence here is worse than a wrong toggle.
///
/// The click only opens a dialog; nothing is removed until its confirm button is
/// clicked. That is not ceremony: the directory holds the plugin's *source*, a
/// marketplace plugin is re-downloadable but a hand-written one is not, and
/// there is no undo for a `remove_dir_all`.
pub(crate) fn plugin_delete_click(
    changed: Query<(Entity, &Interaction, &PluginDelete), Changed<Interaction>>,
    mut armed: Local<Option<Entity>>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
) {
    for (entity, interaction, del) in &changed {
        match interaction {
            Interaction::Pressed => *armed = Some(entity),
            Interaction::Hovered if *armed == Some(entity) => {
                *armed = None;
                let Some(fonts) = fonts.as_ref() else {
                    continue;
                };
                // The overlay root is not kept: `plugin_delete_confirm_click`
                // closes whatever is open, the same way Escape and the backdrop
                // already do, so nothing needs to remember which one this was.
                let (_, buttons) = renzora_ember::widgets::confirm_dialog(
                    &mut commands,
                    fonts,
                    &tr("settings.plugin.delete_title"),
                    // Names the plugin and says what deleting does and does not
                    // do — it frees the disk now and changes what the next
                    // launch loads, but the running code cannot be withdrawn.
                    tr("settings.plugin.delete_body").replace("{id}", &del.id),
                    380.0,
                    190.0,
                    &[&tr("common.cancel"), &tr("settings.plugin.delete_confirm")],
                );
                // `confirm_dialog` accents the LAST label, which is the
                // destructive one by its own convention — and Escape, the
                // backdrop and the × all cancel, so the safe answer needs no aim.
                for (i, b) in buttons.iter().enumerate() {
                    commands.entity(*b).insert(PluginDeleteConfirm {
                        id: del.id.clone(),
                        confirm: i + 1 == buttons.len(),
                    });
                }
            }
            _ => {
                if *armed == Some(entity) {
                    *armed = None;
                }
            }
        }
    }
}

/// Carry out — or call off — a delete the dialog asked about.
///
/// A plain `Pressed` is enough here: this button exists only inside a modal the
/// user deliberately opened, so there is no long list for a stray press to
/// arrive from.
pub(crate) fn plugin_delete_confirm_click(
    q: Query<(&Interaction, &PluginDeleteConfirm), Changed<Interaction>>,
    overlays: Query<Entity, With<renzora_ember::widgets::Overlay>>,
    mut inventory: Option<ResMut<renzora::PluginInventory>>,
    mut disabled: Option<ResMut<renzora::DisabledPlugins>>,
    mut commands: Commands,
) {
    for (interaction, answer) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if answer.confirm {
            delete_one(answer, inventory.as_deref_mut(), disabled.as_deref_mut());
        }
        // Either way the dialog is done with. Despawning every overlay matches
        // what `overlay_dismiss` does for Escape and the backdrop — this is the
        // only one open, because it is modal.
        for overlay in &overlays {
            commands.entity(overlay).despawn();
        }
    }
}

/// The delete itself, lifted out so the click system stays about the click.
fn delete_one(
    answer: &PluginDeleteConfirm,
    inventory: Option<&mut renzora::PluginInventory>,
    disabled: Option<&mut renzora::DisabledPlugins>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = renzora::core::delete_plugin(&answer.id) {
        // Left in the inventory on failure, so the card stays and the user can
        // see the plugin is still there rather than watching it vanish from a
        // list while its directory sits on disk.
        warn!("[plugins] {e}");
        return;
    }
    // Drop the card. The grid is a reactive list over this resource, so removing
    // the entry is what makes the card go away — there is no separate refresh.
    if let Some(inventory) = inventory {
        inventory.entries.retain(|e| e.id != answer.id);
    }
    // And forget any disable preference for it, so a plugin reinstalled later
    // under the same name does not come back mysteriously switched off.
    if let Some(disabled) = disabled {
        if disabled.set_enabled(&answer.id, true) {
            if let Err(e) = renzora::save_disabled_plugins(&disabled.0) {
                warn!("[plugins] could not save the disabled-plugin list: {e}");
            }
        }
    }
    info!("[plugins] deleted `{}`; it stops loading at the next launch", answer.id);
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
