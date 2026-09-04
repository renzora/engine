//! The **Plugins** tab: how plugins reach the shipped game, and which of them go.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display};
use renzora_ember::theme::*;
use renzora_ember::widgets::{radio_group, section, toggle_switch};

use crate::overlay::{ExportOverlayState, PackagingMode, PluginLinkMode};
use crate::templates::Platform;

use super::settings::{finish_tab, tab_panel};
use super::widgets::txt;
use super::AMBER;

pub(super) fn build_plugins_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let mut secs = Vec::new();
    let web = matches!(p, Platform::WebWasm32);

    // How the plugins get there: files beside the binary, or compiled into it.
    // Offered wherever a lean build is possible, since that is the only mode
    // that compiles anything to link into.
    if host {
        let (lsec, lbody) = section(commands, fonts, "link", &renzora::lang::t("export.section.plugin_link"), accent());
        if web {
            // No radio, because there is no choice to offer. A browser has no
            // `dlopen`, so a `plugins/` folder beside the bundle is never read —
            // the export links them in regardless, and showing a two-way control
            // whose first option silently does nothing would be worse than
            // showing none. (`renzora_plugin::host::loader`'s wasm shim is the
            // other end of the same fact: a wasm build gets its plugins linked
            // in or not at all.)
            let note = txt(commands, fonts, &renzora::lang::t("export.plugin_link.web_forced"), 11.0, text_muted());
            commands.entity(lbody).add_child(note);
            // …and linking in still needs the mode that compiles. On the web that
            // means the template mode ships no plugins at all, which is worth
            // saying where the plugin list is rather than only in the log.
            let warn = txt(commands, fonts, &renzora::lang::t("export.plugin_link.web_needs_lean"), 11.0, AMBER);
            bind_display(commands, warn, |w| {
                w.get_resource::<ExportOverlayState>()
                    .is_some_and(|s| s.packaging_mode != PackagingMode::LeanSingleBinary)
            });
            commands.entity(lbody).add_child(warn);
        } else {
            let files = renzora::lang::t("export.plugin_link.files");
            let linked = renzora::lang::t("export.plugin_link.linked");
            let labels: Vec<&str> = vec![files.as_str(), linked.as_str()];
            let radios = radio_group(commands, &fonts.ui, &labels, 0);
            bind_2way(
                commands,
                radios,
                |w| match w.resource::<ExportOverlayState>().plugin_link_mode {
                    PluginLinkMode::ShipFiles => 0usize,
                    PluginLinkMode::LinkIn => 1,
                },
                |w, v: &usize| {
                    w.resource_mut::<ExportOverlayState>().plugin_link_mode = match *v {
                        1 => PluginLinkMode::LinkIn,
                        _ => PluginLinkMode::ShipFiles,
                    };
                },
            );
            commands.entity(lbody).add_child(radios);
            let hint = txt(commands, fonts, &renzora::lang::t("export.plugin_link.hint"), 11.0, text_muted());
            commands.entity(lbody).add_child(hint);
            // Linking in needs something to compile into, and only the lean mode
            // compiles. Rather than disable the radio from the other tab (where
            // the reason would be invisible), say so — and only when it applies.
            let warn = txt(commands, fonts, &renzora::lang::t("export.plugin_link.needs_lean"), 11.0, AMBER);
            bind_display(commands, warn, |w| {
                w.get_resource::<ExportOverlayState>().is_some_and(|s| {
                    s.plugin_link_mode == PluginLinkMode::LinkIn
                        && s.packaging_mode != PackagingMode::LeanSingleBinary
                })
            });
            commands.entity(lbody).add_child(warn);
        }
        secs.push(lsec);
    }

    let (sec, body) = section(commands, fonts, "puzzle-piece", &renzora::lang::t("export.section.plugins"), accent());
    // A wrapping grid of thumbnail cards, matching Settings → Plugins. This was
    // a zebra-striped list of checkboxes: seventy identical rows in which the
    // only way to tell one plugin from another was to read it. The artwork does
    // that work, and the two panels now answer "which plugins?" the same way.
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(body).add_child(list);

    // Said only when it applies, like the Plugin Linking warning above it. The
    // native plugins in this list are real choices in a host copy-based export
    // and silently ignored in any other, so the list would otherwise be quietly
    // lying in exactly the configurations where it matters most.
    let native_note = txt(
        commands,
        fonts,
        &renzora::lang::t("export.plugins.native_host_only"),
        11.0,
        AMBER,
    );
    bind_display(commands, native_note, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| {
            !matches!(
                s.packaging_mode,
                PackagingMode::SeparateFiles | PackagingMode::SingleBinary
            ) || Platform::current() != Some(s.platform)
        })
    });
    commands.entity(body).add_child(native_note);

    // The same idea one step further: some plugins cannot be built for this
    // platform at all, and the grid below would otherwise let you tick one and
    // find out minutes into the compile. Named individually rather than left to
    // a general warning, because "audio is not coming" is the sentence someone
    // needs — a web game with no sound is a surprise worth having before the
    // build, not after it.
    //
    // Resolved once here rather than in a binding: it depends only on the
    // selected platform, and the tab is rebuilt when that changes.
    let blocked = crate::docker::rust_triple(p)
        .and_then(|triple| {
            crate::build::plugin_source_root()
                .map(|root| crate::build::unsupported_plugins_for(&root, triple))
        })
        .unwrap_or_default();
    if !blocked.is_empty() {
        let note = txt(
            commands,
            fonts,
            &format!(
                "{} {}",
                renzora::lang::t("export.plugins.unsupported_target"),
                blocked.join(", ")
            ),
            11.0,
            AMBER,
        );
        commands.entity(body).add_child(note);
    }

    // Filled by a command that can read the world (the plugin list is stable
    // after the scan).
    commands.queue(move |world: &mut World| {
        let plugins: Vec<(String, String)> = world.get_resource::<ExportOverlayState>().map(|s| s.available_plugins.iter().map(|p| (p.id.clone(), format!("{:?}", p.scope))).collect()).unwrap_or_default();
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut c = Commands::new(&mut queue, world);
            if plugins.is_empty() {
                let note = c.spawn((Text::new(renzora::lang::t("export.plugins.none")), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
                c.entity(list).add_child(note);
            }
            for (id, scope) in plugins.into_iter() {
                let card = c
                    .spawn((
                        Node {
                            // Four columns, expressed as a percentage basis
                            // rather than a pixel one. 22% × 4 = 88%, and the
                            // three 8px gaps between them fit in the remaining
                            // 12% at any realistic panel width — so four wrap
                            // onto a row and a fifth cannot, whatever the dialog
                            // is resized to. `flex_grow` then shares the leftover
                            // space so the row still fills edge to edge. A pixel
                            // basis would give four columns at exactly one width.
                            flex_basis: Val::Percent(22.0),
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::all(Val::Px(9.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            // The card is the clipping boundary for a long plugin
                            // name. Without it `chromatic_aberration` ran out
                            // past the card's edge and over its neighbour.
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(rgb(card_bg())),
                    ))
                    .id();

                let thumb = renzora_ember::widgets::file_image_tile(
                    &mut c,
                    &fonts,
                    renzora::core::plugin_thumbnail_path(&id).unwrap_or_default(),
                    "puzzle-piece",
                    text_muted(),
                    10.0,
                );

                // The name gets the card's full width on its own line. It used to
                // share a row with the switch, which left a narrow column for a
                // name like `chromatic_aberration` and pushed it off the card.
                // `width: 100%` matters as much as `no_wrap` here: a no-wrap text
                // node sizes itself to its content, so clipping it needs a width
                // to clip against.
                let name = c
                    .spawn((
                        Text::new(id.clone()),
                        ui_font(&fonts.ui, 11.5),
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

                // Footer: scope on the left, the switch pinned right. The switch
                // is a fixed 28px, so putting it at the end of a row the name no
                // longer competes for keeps every card's control in the same
                // place — a column of switches you can run your eye down.
                let foot = c
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .id();
                let scope_t = c
                    .spawn((
                        Text::new(scope),
                        ui_font(&fonts.ui, 9.0),
                        TextColor(rgb(text_muted())),
                        bevy::text::TextLayout::no_wrap(),
                        Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
                    ))
                    .id();

                // A switch, not a checkbox: this is "ship it / don't", which is
                // an on-off state rather than an item ticked off a list, and it
                // matches the switch the Settings panel uses for the same
                // decision about the same plugins.
                let sw = toggle_switch(&mut c, true);
                // Bevy 0.19 defaults `FocusPolicy` to `Pass`, so a switch that
                // does not block hands its press to everything behind it.
                c.entity(sw).insert(FocusPolicy::Block);
                let id2 = id.clone();
                bind_2way(&mut c, sw, move |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.selected_plugins.contains(&id2)), {
                    let id3 = id.clone();
                    move |w, v: &bool| {
                        if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                            if *v { s.selected_plugins.insert(id3.clone()); } else { s.selected_plugins.remove(&id3); }
                        }
                    }
                });
                c.entity(foot).add_children(&[scope_t, sw]);

                c.entity(card).add_children(&[thumb, name, foot]);
                c.entity(list).add_child(card);
            }
        }
        queue.apply(world);
    });
    secs.push(sec);
    finish_tab(commands, panel, &secs, tab_max);
    panel
}
