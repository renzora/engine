//! The right rail: what the build will be, and what stands in its way.
//!
//! The old dialog answered "will Export work?" with one bit, the Export button's
//! colour, and left the reason somewhere in one of five tabs: a template line in
//! Packaging, an empty folder field in Output, a warning under the plugin grid.
//! A greyed-out button with no reason is the most common question this dialog
//! got. So every condition is now a [`Check`] with a sentence and, where there is
//! one, a button that fixes it, all in one list beside the form.
//!
//! [`preflight`] is also what [`can_export`](super::frame::can_export) asks, so
//! the list and the button cannot disagree: the button is grey exactly when the
//! list holds a blocking item.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;

use crate::download::DownloadProgress;
use crate::overlay::{release_fetch_pending, ExportOverlayState, PackagingMode, PluginLinkMode};
use crate::templates::{Platform, TemplateManager};

use super::settings::lean_source_available;
use super::widgets::{ca, cursor, hairline, mode_label, txt};
use super::{CheckAction, CheckActionBtn, AMBER, GREEN, RED};

/// How much a check matters.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(super) enum CheckKind {
    /// The export cannot run until this is fixed.
    Error,
    /// Something the export is waiting on (a lookup, a download). Blocks too,
    /// but it is not the user's mistake, so it is not drawn red.
    Pending,
    /// The export will run, but probably not the way you meant.
    Warn,
    /// Confirmation that a requirement is met.
    Ok,
}

impl CheckKind {
    pub(super) fn blocks(self) -> bool {
        matches!(self, CheckKind::Error | CheckKind::Pending)
    }

    fn colour(self) -> (u8, u8, u8) {
        match self {
            CheckKind::Error => RED,
            CheckKind::Pending => text_muted(),
            CheckKind::Warn => AMBER,
            CheckKind::Ok => GREEN,
        }
    }

    fn icon(self) -> &'static str {
        match self {
            CheckKind::Error => "x-circle",
            CheckKind::Pending => "hourglass-medium",
            CheckKind::Warn => "warning",
            CheckKind::Ok => "check-circle",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(super) struct Check {
    pub(super) kind: CheckKind,
    pub(super) text: String,
    pub(super) action: Option<(CheckAction, String)>,
}

impl Check {
    fn new(kind: CheckKind, text: String) -> Self {
        Self { kind, text, action: None }
    }

    fn with(mut self, action: CheckAction, label_key: &str) -> Self {
        self.action = Some((action, renzora::lang::t(label_key)));
        self
    }
}

fn t_platform(key: &str, p: Platform) -> String {
    renzora::lang::t(key).replace("{platform}", p.display_name())
}

fn t_n(key: &str, n: usize) -> String {
    renzora::lang::t(key).replace("{n}", &n.to_string())
}

/// Can a lean build be made for this platform from here at all?
pub(super) fn lean_possible(p: Platform) -> bool {
    crate::docker::lean_supported(p) && lean_source_available()
}

/// Every check for the current configuration, most serious first.
pub(super) fn preflight(w: &Rx) -> Vec<Check> {
    let Some(s) = w.get_resource::<ExportOverlayState>() else { return Vec::new() };
    let p = s.platform;
    let web = matches!(p, Platform::WebWasm32);
    let lean = s.packaging_mode == PackagingMode::LeanSingleBinary;
    let source = lean_source_available();
    let can_lean = crate::docker::lean_supported(p) && source;
    let mut out = Vec::new();

    // ── Is there a runtime to ship? ─────────────────────────────────────────
    if lean {
        if !crate::docker::lean_supported(p) {
            out.push(Check::new(CheckKind::Error, t_platform("export.check.lean_unsupported", p)));
        } else if !source {
            let c = Check::new(CheckKind::Error, renzora::lang::t("export.packaging.needs_source"));
            out.push(if s.release_info.is_some() {
                c.with(CheckAction::DownloadSource, "export.check.action.download")
            } else if s.release_fetch_error.is_some() {
                c.with(CheckAction::RetryRelease, "export.check.action.retry")
            } else {
                c
            });
        } else {
            out.push(Check::new(
                CheckKind::Ok,
                renzora::lang::t(if web { "export.check.toolchain_web" } else { "export.check.toolchain" }),
            ));
        }
        // A cross-platform lean build compiles in a container. Not blocking
        // here: `probe_docker` only knows once it has run, and the export
        // itself reports a missing daemon with the same words.
        if let Some(status) = &s.docker {
            match status {
                crate::docker::DockerStatus::Ready => {}
                crate::docker::DockerStatus::NotInstalled => {
                    out.push(Check::new(CheckKind::Warn, renzora::lang::t("export.docker.not_installed")))
                }
                crate::docker::DockerStatus::NotRunning(_) => {
                    out.push(Check::new(CheckKind::Warn, renzora::lang::t("export.check.docker_not_running")))
                }
            }
        }
    } else {
        let installed = w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p));
        let downloading = matches!(&s.download_status, Some((dp, DownloadProgress::Fetching(_))) if *dp == p);
        if installed {
            out.push(Check::new(CheckKind::Ok, t_platform("export.check.template_installed", p)));
        } else if downloading {
            out.push(Check::new(CheckKind::Pending, t_platform("export.check.template_downloading", p)));
        } else if s.release_fetch_error.is_some() {
            out.push(
                Check::new(CheckKind::Error, renzora::lang::t("export.check.github_unreachable"))
                    .with(CheckAction::RetryRelease, "export.check.action.retry"),
            );
        } else if let Some(info) = &s.release_info {
            let c = Check::new(CheckKind::Error, t_platform("export.check.template_missing", p));
            out.push(if info.available_platforms.contains(&p) {
                c.with(CheckAction::DownloadTemplate, "export.check.action.download")
            } else {
                c.with(CheckAction::InstallTemplate, "export.check.action.install")
            });
        } else if release_fetch_pending(s) || !s.release_fetch_started {
            out.push(Check::new(CheckKind::Pending, renzora::lang::t("export.check.template_looking")));
        }
    }

    // ── Where it goes ───────────────────────────────────────────────────────
    if s.output_dir.trim().is_empty() {
        out.push(
            Check::new(CheckKind::Error, renzora::lang::t("export.check.no_folder"))
                .with(CheckAction::ChooseFolder, "export.check.action.choose"),
        );
    } else {
        out.push(Check::new(CheckKind::Ok, renzora::lang::t("export.check.folder_set")));
    }

    // ── Will it be what you meant? ──────────────────────────────────────────
    let plugins = s.selected_plugins.len();
    if web && !lean && plugins > 0 {
        let c = Check::new(CheckKind::Warn, t_n("export.check.web_plugins_dropped", plugins));
        out.push(if can_lean { c.with(CheckAction::UseLean, "export.check.action.use_lean") } else { c });
    }
    if !web && !lean && plugins > 0 && s.plugin_link_mode == PluginLinkMode::LinkIn {
        let c = Check::new(CheckKind::Warn, renzora::lang::t("export.check.link_needs_lean"));
        out.push(if can_lean { c.with(CheckAction::UseLean, "export.check.action.use_lean") } else { c });
    }
    if lean {
        let switched_off = s
            .detected_capabilities
            .iter()
            .filter(|(id, on)| **on && !s.capabilities.get(*id).copied().unwrap_or(false))
            .count();
        if switched_off > 0 {
            out.push(
                Check::new(CheckKind::Warn, t_n("export.check.features_off", switched_off))
                    .with(CheckAction::ReviewFeatures, "export.check.action.review"),
            );
        }
    } else if can_lean {
        out.push(
            Check::new(CheckKind::Warn, renzora::lang::t("export.check.testing_mode"))
                .with(CheckAction::UseLean, "export.check.action.use_lean"),
        );
    }
    let stale = w
        .get_resource::<renzora::PluginUpdates>()
        .map(|u| u.entries.iter().filter(|e| s.selected_plugins.contains(&e.id)).count())
        .unwrap_or(0);
    if stale > 0 {
        out.push(
            Check::new(CheckKind::Warn, t_n("export.check.plugin_updates", stale))
                .with(CheckAction::ReviewPlugins, "export.check.action.review"),
        );
    }

    out.sort_by_key(|c| c.kind);
    out
}

/// Whether anything in [`preflight`] stops the export.
pub(super) fn blocking_count(w: &Rx) -> usize {
    preflight(w).iter().filter(|c| c.kind.blocks()).count()
}

pub(super) fn build_summary_rail(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let rail = commands
        .spawn((
            Node {
                width: Val::Px(260.0),
                flex_shrink: 0.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(8.0),
                border: UiRect::left(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(ca(0, 0, 0, 40)),
            BorderColor::all(hairline()),
        ))
        .id();

    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();

    let h1 = caption(commands, fonts, &renzora::lang::t("export.summary.this_build"));
    commands.entity(body).add_child(h1);
    let rows: [(&str, SummaryValue); 6] = [
        ("export.summary.platform", |s| s.platform.display_name().to_string()),
        ("export.summary.build", |s| mode_label(s.platform, s.packaging_mode)),
        ("export.summary.engine", engine_summary),
        ("export.summary.plugins", plugin_summary),
        ("export.summary.files", files_summary),
        ("export.summary.folder", folder_summary),
    ];
    for (key, value) in rows {
        let row = commands
            .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(10.0), justify_content: JustifyContent::SpaceBetween, ..default() })
            .id();
        let k = txt(commands, fonts, &renzora::lang::t(key), 11.5, text_muted());
        let v = commands
            .spawn((
                Text::new(String::new()),
                ui_font(&fonts.ui, 11.5),
                TextColor(rgb(text_primary())),
                bevy::text::TextLayout::justify(bevy::text::Justify::Right),
                Node { flex_shrink: 1.0, min_width: Val::Px(0.0), ..default() },
            ))
            .id();
        bind_text(commands, v, move |w| w.get_resource::<ExportOverlayState>().map(value).unwrap_or_default());
        commands.entity(row).add_children(&[k, v]);
        commands.entity(body).add_child(row);
    }

    let h2 = caption(commands, fonts, &renzora::lang::t("export.summary.before_export"));
    commands.entity(h2).insert(Node { margin: UiRect::top(Val::Px(12.0)), ..default() });
    commands.entity(body).add_child(h2);
    let list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    keyed_list(commands, list, checks_snapshot);
    commands.entity(body).add_child(list);

    // Scrolled: a long check list on a short window must not push the release
    // line off the bottom of the dialog.
    let scroll = renzora_ember::widgets::scroll_view(commands, body);
    commands.entity(rail).add_child(scroll);

    // Release line. It used to sit under the preset list, where the raw error
    // text of a failed lookup was the last thing in the sidebar. The failure is
    // now a check with a Retry button; this line only says where templates come
    // from.
    let foot = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::top(Val::Px(10.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(hairline()),
        ))
        .id();
    let dot = commands
        .spawn((Node { width: Val::Px(7.0), height: Val::Px(7.0), flex_shrink: 0.0, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(text_muted()))))
        .id();
    bind_bg(commands, dot, |w| {
        let s = w.get_resource::<ExportOverlayState>();
        if s.is_some_and(|s| s.release_fetch_error.is_some()) {
            rgb(RED)
        } else if s.is_some_and(|s| s.release_info.is_some()) {
            rgb(GREEN)
        } else {
            rgb(text_muted())
        }
    });
    let rel = txt(commands, fonts, "", 11.0, text_muted());
    bind_text(commands, rel, |w| {
        let Some(s) = w.get_resource::<ExportOverlayState>() else { return String::new() };
        if s.release_fetch_error.is_some() {
            renzora::lang::t("export.status.release_unavailable")
        } else if let Some(info) = &s.release_info {
            // Say when this is the nightly fallback rather than a release for
            // this exact version: "where did this runtime come from?" should be
            // answerable without reading the source.
            let key = if info.is_fallback { "export.status.nightly" } else { "export.status.latest" };
            format!("{} {}", renzora::lang::t(key), info.tag_name)
        } else {
            renzora::lang::t("export.status.loading_release")
        }
    });
    commands.entity(foot).add_children(&[dot, rel]);
    commands.entity(rail).add_child(foot);
    rail
}

/// Reads one summary value out of the export state.
type SummaryValue = fn(&ExportOverlayState) -> String;

/// A small upper-case label over a rail group.
fn caption(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    txt(commands, fonts, &label.to_uppercase(), 10.5, text_muted())
}

fn engine_summary(s: &ExportOverlayState) -> String {
    if s.packaging_mode != PackagingMode::LeanSingleBinary {
        return renzora::lang::t("export.summary.engine_all");
    }
    let total = crate::capabilities::CAPABILITIES.len();
    let on = crate::capabilities::CAPABILITIES
        .iter()
        .filter(|c| s.capabilities.get(c.id).copied().unwrap_or(false))
        .count();
    renzora::lang::t("export.summary.engine_count")
        .replace("{n}", &on.to_string())
        .replace("{total}", &total.to_string())
}

fn plugin_summary(s: &ExportOverlayState) -> String {
    let n = s.selected_plugins.len();
    if n == 0 {
        return renzora::lang::t("common.none");
    }
    let web = matches!(s.platform, Platform::WebWasm32);
    let lean = s.packaging_mode == PackagingMode::LeanSingleBinary;
    let key = if web && !lean {
        "export.summary.plugins_dropped"
    } else if web || (lean && s.plugin_link_mode == PluginLinkMode::LinkIn) {
        "export.summary.plugins_linked"
    } else {
        "export.summary.plugins_files"
    };
    t_n(key, n)
}

fn files_summary(s: &ExportOverlayState) -> String {
    match &s.included_files {
        None => renzora::lang::t("export.summary.files_auto"),
        Some(set) => renzora::lang::t("export.files.count")
            .replace("{n}", &set.len().to_string())
            .replace("{total}", &s.project_files.len().to_string()),
    }
}

fn folder_summary(s: &ExportOverlayState) -> String {
    let dir = s.output_dir.trim();
    if dir.is_empty() {
        return renzora::lang::t("export.summary.not_set");
    }
    // The last component is what tells two output folders apart; the full path
    // would be clipped to its drive letter in a column this narrow.
    std::path::Path::new(dir)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.to_string())
}

fn checks_snapshot(world: &Rx) -> KeyedSnapshot {
    let checks = preflight(world);
    let mut seen: std::collections::HashMap<(CheckKind, Option<CheckAction>), u32> = Default::default();
    let items = checks
        .iter()
        .map(|c| {
            // Keyed by what the check is about, so a check whose text changes
            // (a count going from 2 to 3) rebuilds in place rather than moving.
            // Two checks can share a kind and a fix (both "Use lean build"
            // warnings), so the occurrence number keeps their keys apart.
            let action = c.action.as_ref().map(|(a, _)| *a);
            let nth = seen.entry((c.kind, action)).or_insert(0);
            *nth += 1;
            let mut k = std::collections::hash_map::DefaultHasher::new();
            action.hash(&mut k);
            c.kind.hash(&mut k);
            nth.hash(&mut k);
            if c.action.is_none() {
                c.text.hash(&mut k);
            }
            let mut h = std::collections::hash_map::DefaultHasher::new();
            c.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot { items, build: Box::new(move |c, f, i| check_row(c, f, &checks[i])) }
}

fn check_row(commands: &mut Commands, fonts: &EmberFonts, check: &Check) -> Entity {
    let fg = check.kind.colour();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(9.0),
                padding: UiRect::all(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(ca(fg.0, fg.1, fg.2, 16)),
            BorderColor::all(ca(fg.0, fg.1, fg.2, 60)),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, check.kind.icon(), fg, 14.0);
    let col = commands
        .spawn(Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    let t = txt(commands, fonts, &check.text, 11.5, text_primary());
    commands.entity(col).add_child(t);
    if let Some((action, label)) = &check.action {
        let btn = commands
            .spawn((
                Node {
                    align_self: AlignSelf::FlexStart,
                    height: Val::Px(24.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(9.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(ca(fg.0, fg.1, fg.2, 110)),
                UiInteraction::default(),
                CheckActionBtn(*action),
                cursor(),
            ))
            .id();
        bind_bg(commands, btn, move |w| {
            if matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered)) {
                ca(255, 255, 255, 14)
            } else {
                Color::NONE
            }
        });
        let bt = commands
            .spawn((Text::new(label.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(fg)), PASS_THROUGH, bevy::text::TextLayout::no_wrap()))
            .id();
        commands.entity(btn).add_child(bt);
        commands.entity(col).add_child(btn);
    }
    commands.entity(row).add_children(&[ic, col]);
    row
}
