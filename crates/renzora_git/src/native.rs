//! The Git panel (ember / bevy_ui).
//!
//! A toolbar that says where you are and talks to the remote, three lists behind a
//! segmented switch (Changes / History / Branches), and a commit box under the
//! first of them. Two overlays sit on top when needed: a confirmation, and a diff
//! viewer.
//!
//! # How it stays cheap
//!
//! The panel is built **once**, when its tab is first activated, and never rebuilt
//! wholesale. Everything after is one of two things:
//!
//! - The three lists are a single [`keyed_list_tokened`] over an [`Item`] enum, so
//!   a refresh reuses every row whose content hash is unchanged. The token is
//!   `GitState::revision`, a counter — so the frames where nothing happened cost
//!   one integer comparison, not a rebuild of the item list. That matters here
//!   because a repo with a hundred changed files would otherwise re-derive and
//!   re-hash a hundred items every frame to discover that none of them moved.
//! - The toolbar's labels are `bind_*` effects, which are value-diffed and so only
//!   write when the value actually changes.
//!
//! The periodic re-read is registered through `PanelScope::systems`, so it stops
//! entirely while the tab is hidden — an editor with the Git panel in a background
//! tab spawns no `git` at all.
//!
//! # One click handler
//!
//! Every interactive row and button carries a [`GitAction`], and a single system
//! reads them. That is deliberate rather than tidy: it is the one place that
//! decides what needs confirming, so a new destructive action cannot be added
//! somewhere that forgot to ask.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{
    bind_bg, bind_display, bind_text, bind_text_color, bind_with, keyed_list_tokened,
};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{scroll_view_keyed, textarea, EmberTextInput};

use crate::job::Job;
use crate::parse::{self, Change, Commit, StatusEntry};
use crate::{Confirm, GitState, View, PANEL_ID};

/// How often the panel re-reads the repository while it is visible.
///
/// A compromise, and the reason it is not shorter: files change on disk from
/// outside the editor (a terminal, another tool), so the panel has to poll to
/// notice. But each poll is a process spawn plus a full `git status`, so polling at
/// frame rate — or even once a second — is a real cost on a large project for
/// information that changes a few times an hour. Anything the *editor* does
/// refreshes immediately regardless, because those go through a job.
const REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(4);

const ROW_H: f32 = 24.0;
const HOVER_A: f32 = 14.0 / 255.0;
const SELECTED_A: f32 = 40.0 / 255.0;

/// Status letter colours. Green for what is going in, red for what is going away,
/// amber for what needs a decision.
const GREEN: (u8, u8, u8) = (89, 191, 115);
const AMBER: (u8, u8, u8) = (242, 166, 64);
const RED: (u8, u8, u8) = (239, 68, 68);
const BLUE: (u8, u8, u8) = (96, 165, 250);

fn tr(key: &str, default: &str) -> String {
    renzora::lang::t_or(key, default)
}

/// Colour with an explicit alpha — ember's theme exposes only opaque `rgb`, and
/// these tints sit over whatever is behind them.
fn ca((r, g, b): (u8, u8, u8), a: f32) -> Color {
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
}

/// The badge colour for a change kind.
fn change_tint(c: Change) -> (u8, u8, u8) {
    match c {
        Change::Added | Change::Untracked => GREEN,
        Change::Modified | Change::TypeChanged => AMBER,
        Change::Deleted => RED,
        Change::Renamed | Change::Copied => BLUE,
        Change::Conflicted => RED,
    }
}

// ── Actions ──────────────────────────────────────────────────────────────────

/// What clicking a row or button does.
///
/// The destructive variants are named for what they destroy rather than for the
/// git command they run, so the match in [`git_click`] reads as a list of
/// consequences and a missing confirmation is visible.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Action {
    SetView(View),
    Refresh,
    /// Create the repository, and a starter `.gitignore` with it.
    Init,

    Stage(Vec<String>),
    Unstage(Vec<String>),
    /// Irreversible. Split by tracked-ness so the prompt can say whether the file
    /// is being restored or deleted.
    DiscardPrompt {
        tracked: Vec<String>,
        untracked: Vec<String>,
        what: String,
    },
    ShowDiff {
        path: String,
        staged: bool,
        untracked: bool,
    },
    Commit,
    ToggleAmend,

    Fetch,
    Pull,
    Push,

    /// Expand/collapse a history row.
    SelectCommit(String),
    ShowCommit(String),
    /// Detaches HEAD — reversible, but a state worth explaining first.
    CheckoutCommitPrompt(String),
    /// Adds a commit that undoes one. Fully reversible.
    RevertPrompt {
        oid: String,
        short: String,
        is_merge: bool,
    },
    /// Un-commits, keeping the changes in the working tree.
    ResetMixedPrompt(String),
    /// Irreversible for anything uncommitted.
    ResetHardPrompt(String),
    /// Start a branch at a commit and switch to it.
    BranchFromCommit(String),

    CheckoutBranch(String),
    MergePrompt(String),
    DeleteBranchPrompt(String),
    /// Create a local branch tracking a remote one and switch to it.
    TrackRemote(String),
    /// Create a branch from the name typed into the New Branch field.
    CreateBranchFromInput,

    AbortMergePrompt,
    ClearError,
    ConfirmAccept,
    ConfirmCancel,
}

/// Marks an entity as clickable, with what it does.
#[derive(Component, Clone)]
struct GitAction(Action);

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct CommitMessageInput;
#[derive(Component)]
struct NewBranchInput;
/// The commit box, hidden on the views it does not belong to.
#[derive(Component)]
struct CommitBox;
#[derive(Component)]
struct ConfirmRoot;
#[derive(Component)]
struct DiffRoot;
#[derive(Component)]
struct AmendBox;

// ── The item list ────────────────────────────────────────────────────────────

/// One row of whichever list is showing.
///
/// A single flattened enum across all three views, so the panel needs one reactive
/// list rather than three that would each have to be shown and hidden.
#[derive(Clone, PartialEq)]
enum Item {
    Header {
        label: String,
        /// A trailing button on the header (Stage all / Unstage all / Discard all).
        action: Option<(String, Action, bool)>,
    },
    Hint(String),
    Empty {
        icon: &'static str,
        title: String,
        subtitle: String,
    },
    /// A changed file.
    File {
        name: String,
        dir: String,
        badge: char,
        tint: (u8, u8, u8),
        tooltip: String,
        /// Clicking the row shows its diff.
        open: Action,
        /// The +/− button: stage it, or unstage it.
        toggle: (&'static str, Action),
        /// The discard button. Absent for a staged-only entry, which has nothing
        /// in the working tree to throw away.
        discard: Option<Action>,
    },
    Commit {
        short: String,
        subject: String,
        author: String,
        when: String,
        refs: Vec<String>,
        is_head: bool,
        is_merge: bool,
        selected: bool,
        action: Action,
    },
    /// The actions for the expanded history row.
    CommitActions {
        oid: String,
        short: String,
        is_merge: bool,
    },
    Branch {
        name: String,
        short_oid: String,
        upstream: Option<String>,
        is_head: bool,
        remote: bool,
    },
    /// The name field + Create button.
    NewBranch,
}

/// Content hash — must cover every field that changes what a row renders, or an
/// updated row keeps its old contents.
fn hash_item(it: &Item) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match it {
        Item::Header { label, action } => {
            (0u8, label, action.as_ref().map(|(l, a, d)| (l, format!("{a:?}"), d))).hash(&mut h);
        }
        Item::Hint(s) => (1u8, s).hash(&mut h),
        Item::Empty {
            icon,
            title,
            subtitle,
        } => (2u8, icon, title, subtitle).hash(&mut h),
        Item::File {
            name,
            dir,
            badge,
            tint,
            tooltip,
            open,
            toggle,
            discard,
        } => (
            3u8,
            name,
            dir,
            badge,
            tint,
            tooltip,
            format!("{open:?}"),
            toggle.0,
            format!("{:?}", toggle.1),
            discard.as_ref().map(|a| format!("{a:?}")),
        )
            .hash(&mut h),
        Item::Commit {
            short,
            subject,
            author,
            when,
            refs,
            is_head,
            is_merge,
            selected,
            // Included even though `short` is derived from the same oid: this is
            // what the row *does* when clicked, and a reused row carrying the
            // previous commit's action would send "Reset here" to the wrong one.
            action,
        } => (
            4u8,
            short,
            subject,
            author,
            when,
            refs,
            is_head,
            is_merge,
            selected,
            format!("{action:?}"),
        )
            .hash(&mut h),
        Item::CommitActions {
            oid,
            short,
            is_merge,
        } => (5u8, oid, short, is_merge).hash(&mut h),
        Item::Branch {
            name,
            short_oid,
            upstream,
            is_head,
            remote,
        } => (6u8, name, short_oid, upstream, is_head, remote).hash(&mut h),
        Item::NewBranch => 7u8.hash(&mut h),
    }
    h.finish()
}

/// A file row's badge, colour and available actions.
///
/// `staged` picks which of the entry's two columns this row is showing. The same
/// file appears twice when it is staged *and* edited again, and the two rows do
/// different things — so the row is derived from (entry, column), not from the
/// entry alone.
fn file_item(entry: &StatusEntry, staged: bool) -> Item {
    let change = if staged {
        entry.index.unwrap_or(Change::Modified)
    } else {
        entry.worktree.unwrap_or(Change::Modified)
    };
    let path = entry.path.clone();
    let conflicted = entry.is_conflicted();
    let untracked = entry.is_untracked();

    let mut tooltip = match &entry.orig_path {
        Some(from) => format!("{} ({} from {from})", entry.path, change.label()),
        None => format!("{} ({})", entry.path, change.label()),
    };
    if conflicted {
        tooltip.push_str(" — resolve, then stage to mark it done");
    }

    // A conflicted file is staged to *mark it resolved*, which is the same command
    // with a very different meaning, so it gets its own label.
    let toggle = if staged {
        ("minus", Action::Unstage(vec![path.clone()]))
    } else if conflicted {
        ("check", Action::Stage(vec![path.clone()]))
    } else {
        ("plus", Action::Stage(vec![path.clone()]))
    };

    // Only the working-tree column has something to discard. Discarding from the
    // staged column would also throw away the unstaged edit, which the row does
    // not represent and the user did not point at.
    let discard = (!staged).then(|| Action::DiscardPrompt {
        tracked: if untracked { vec![] } else { vec![path.clone()] },
        untracked: if untracked { vec![path.clone()] } else { vec![] },
        what: entry.file_name().to_string(),
    });

    Item::File {
        name: entry.file_name().to_string(),
        dir: entry.dir().to_string(),
        badge: change.letter(),
        tint: change_tint(change),
        tooltip,
        open: Action::ShowDiff {
            path,
            staged,
            untracked,
        },
        toggle,
        discard,
    }
}

/// Flatten the current state into the list the panel draws.
fn build_items(state: &GitState, now: i64) -> Vec<Item> {
    if !state.git_available() {
        return vec![Item::Empty {
            icon: "warning-circle",
            title: tr("git.empty.no_git.title", "Git is not available"),
            subtitle: match state.git_error() {
                Some(e) => format!("{e}\n\nInstall git and restart the editor."),
                None => tr("git.empty.no_git.checking", "Checking…"),
            },
        }];
    }
    if state.root.is_none() {
        return vec![
            Item::Empty {
                icon: "git-branch",
                title: tr("git.empty.no_repo.title", "Not a git repository"),
                subtitle: tr(
                    "git.empty.no_repo.subtitle",
                    "Track this project's history, go back to earlier versions, and share it \
                     with a remote.",
                ),
            },
            Item::Header {
                label: String::new(),
                action: Some((
                    tr("git.action.init", "Initialize Repository"),
                    Action::Init,
                    false,
                )),
            },
        ];
    }

    match state.view {
        View::Changes => changes_items(state),
        View::History => history_items(state, now),
        View::Branches => branch_items(state),
    }
}

fn changes_items(state: &GitState) -> Vec<Item> {
    let status = &state.snapshot.status;
    let mut items = Vec::new();

    let staged: Vec<&StatusEntry> = status.staged().collect();
    let unstaged: Vec<&StatusEntry> = status.unstaged().collect();

    if staged.is_empty() && unstaged.is_empty() {
        return vec![Item::Empty {
            icon: "check-circle",
            title: tr("git.empty.clean.title", "Nothing to commit"),
            subtitle: tr(
                "git.empty.clean.subtitle",
                "Every change in this project is committed.",
            ),
        }];
    }

    // Conflicts first and on their own: nothing else in the panel can be finished
    // until they are dealt with, and burying them under "Changes" hides that.
    let conflicts: Vec<&StatusEntry> = unstaged.iter().copied().filter(|e| e.is_conflicted()).collect();
    if !conflicts.is_empty() {
        items.push(Item::Header {
            label: format!(
                "{} ({})",
                tr("git.section.conflicts", "Conflicts"),
                conflicts.len()
            ),
            action: Some((
                tr("git.action.abort_merge", "Abort merge"),
                Action::AbortMergePrompt,
                true,
            )),
        });
        items.push(Item::Hint(tr(
            "git.hint.conflicts",
            "Edit each file to resolve it, then mark it resolved with ✓.",
        )));
        for entry in &conflicts {
            items.push(file_item(entry, false));
        }
    }

    if !staged.is_empty() {
        let paths: Vec<String> = staged.iter().map(|e| e.path.clone()).collect();
        items.push(Item::Header {
            label: format!(
                "{} ({})",
                tr("git.section.staged", "Staged"),
                staged.len()
            ),
            action: Some((
                tr("git.action.unstage_all", "Unstage all"),
                Action::Unstage(paths),
                false,
            )),
        });
        for entry in &staged {
            items.push(file_item(entry, true));
        }
    }

    let plain: Vec<&StatusEntry> = unstaged.iter().copied().filter(|e| !e.is_conflicted()).collect();
    if !plain.is_empty() {
        let paths: Vec<String> = plain.iter().map(|e| e.path.clone()).collect();
        let (tracked, untracked): (Vec<String>, Vec<String>) = plain
            .iter()
            .map(|e| (e.path.clone(), e.is_untracked()))
            .fold((Vec::new(), Vec::new()), |(mut t, mut u), (p, is_untracked)| {
                if is_untracked {
                    u.push(p);
                } else {
                    t.push(p);
                }
                (t, u)
            });
        items.push(Item::Header {
            label: format!(
                "{} ({})",
                tr("git.section.changes", "Changes"),
                plain.len()
            ),
            action: Some((
                tr("git.action.stage_all", "Stage all"),
                Action::Stage(paths),
                false,
            )),
        });
        items.push(Item::Header {
            label: String::new(),
            action: Some((
                tr("git.action.discard_all", "Discard all"),
                Action::DiscardPrompt {
                    tracked,
                    untracked,
                    what: format!("{} files", plain.len()),
                },
                true,
            )),
        });
        for entry in &plain {
            items.push(file_item(entry, false));
        }
    }

    items
}

fn history_items(state: &GitState, now: i64) -> Vec<Item> {
    let snapshot = &state.snapshot;
    if snapshot.status.unborn {
        return vec![Item::Empty {
            icon: "git-commit",
            title: tr("git.empty.unborn.title", "No commits yet"),
            subtitle: tr(
                "git.empty.unborn.subtitle",
                "Stage your project's files and make the first commit to start its history.",
            ),
        }];
    }
    if snapshot.log.is_empty() {
        return vec![Item::Empty {
            icon: "git-commit",
            title: tr("git.empty.no_history.title", "No history"),
            subtitle: tr("git.empty.no_history.subtitle", "Nothing has been committed."),
        }];
    }

    let mut items = Vec::new();
    for commit in &snapshot.log {
        let selected = state.selected_commit.as_deref() == Some(commit.oid.as_str());
        items.push(commit_item(commit, now, selected));
        if selected {
            items.push(Item::CommitActions {
                oid: commit.oid.clone(),
                short: commit.short.clone(),
                is_merge: commit.is_merge(),
            });
        }
    }
    if snapshot.log.len() >= crate::job::LOG_LIMIT {
        // Said out loud rather than left to look like the whole history: a silent
        // cap reads as "this project has 200 commits".
        items.push(Item::Hint(format!(
            "Showing the most recent {} commits.",
            crate::job::LOG_LIMIT
        )));
    }
    items
}

fn commit_item(commit: &Commit, now: i64, selected: bool) -> Item {
    Item::Commit {
        short: commit.short.clone(),
        subject: parse::truncate(&commit.subject, 72),
        author: commit.author.clone(),
        when: parse::relative_time(now, commit.timestamp),
        refs: commit.refs.clone(),
        is_head: commit.is_head,
        is_merge: commit.is_merge(),
        selected,
        action: Action::SelectCommit(commit.oid.clone()),
    }
}

fn branch_items(state: &GitState) -> Vec<Item> {
    let snapshot = &state.snapshot;
    let mut items = vec![
        Item::Header {
            label: tr("git.section.new_branch", "New branch"),
            action: None,
        },
        Item::NewBranch,
    ];

    let local: Vec<_> = snapshot.local_branches().collect();
    items.push(Item::Header {
        label: format!("{} ({})", tr("git.section.local", "Local"), local.len()),
        action: None,
    });
    if local.is_empty() {
        items.push(Item::Hint(tr(
            "git.hint.no_local_branches",
            "No branches yet — the first commit creates one.",
        )));
    }
    for branch in local {
        items.push(Item::Branch {
            name: branch.name.clone(),
            short_oid: branch.short_oid.clone(),
            upstream: branch.upstream.clone(),
            is_head: branch.is_head,
            remote: false,
        });
    }

    let remote: Vec<_> = snapshot.remote_branches().collect();
    if !remote.is_empty() {
        items.push(Item::Header {
            label: format!("{} ({})", tr("git.section.remote", "Remote"), remote.len()),
            action: None,
        });
        items.push(Item::Hint(tr(
            "git.hint.remote_branches",
            "Switching to a remote branch creates a local one that tracks it.",
        )));
        for branch in remote {
            items.push(Item::Branch {
                name: branch.name.clone(),
                short_oid: branch.short_oid.clone(),
                upstream: None,
                is_head: false,
                remote: true,
            });
        }
    }

    items
}

// ── Row builders ─────────────────────────────────────────────────────────────

fn text_node(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Text::new(s),
            ui_font(&fonts.ui, size),
            TextColor(rgb(color)),
        ))
        .id()
}

/// A small text button. `danger` draws it red — reserved for actions git cannot
/// undo.
fn small_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    action: Action,
    danger: bool,
) -> Entity {
    let hue = if danger { RED } else { accent() };
    let button = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(ca(hue, 0.16)),
            Interaction::default(),
            GitAction(action),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            // Without this the press also reaches the row behind the button, so
            // "Discard all" on a header would additionally open the header's row.
            bevy::ui::FocusPolicy::Block,
            Name::new("git-button"),
        ))
        .id();
    bind_bg(commands, button, move |world| {
        match world.get::<Interaction>(button) {
            Some(Interaction::Hovered) => ca(hue, 0.30),
            Some(Interaction::Pressed) => ca(hue, 0.42),
            _ => ca(hue, 0.16),
        }
    });
    let label = text_node(commands, fonts, label, 11.0, hue);
    commands.entity(button).add_child(label);
    button
}

/// A square icon button, for the per-row stage / unstage / discard controls.
fn icon_action(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    hue: (u8, u8, u8),
    action: Action,
    tooltip: &str,
) -> Entity {
    let button = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            GitAction(action),
            renzora_ember::widgets::HoverTooltip::new(tooltip),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            bevy::ui::FocusPolicy::Block,
            Name::new("git-icon-action"),
        ))
        .id();
    bind_bg(commands, button, move |world| {
        match world.get::<Interaction>(button) {
            Some(Interaction::Hovered) => ca(hue, 0.26),
            Some(Interaction::Pressed) => ca(hue, 0.38),
            _ => Color::NONE,
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, hue, 12.0);
    commands.entity(button).add_child(glyph);
    button
}

/// A row that highlights on hover and dispatches `action` when clicked.
fn clickable_row(commands: &mut Commands, action: Option<Action>, selected: bool) -> Entity {
    let base = if selected {
        rgb(accent()).with_alpha(SELECTED_A)
    } else {
        Color::NONE
    };
    let mut row = commands.spawn((
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(ROW_H),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
            column_gap: Val::Px(6.0),
            ..default()
        },
        BackgroundColor(base),
        Name::new("git-row"),
    ));
    if let Some(action) = action {
        row.insert((
            Interaction::default(),
            GitAction(action),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ));
    }
    let row = row.id();
    bind_bg(commands, row, move |world| {
        match world.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) if !selected => {
                Color::srgba(1.0, 1.0, 1.0, HOVER_A)
            }
            _ => base,
        }
    });
    row
}

fn spacer(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id()
}

fn build_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    action: &Option<(String, Action, bool)>,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                column_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(if label.is_empty() {
                Color::NONE
            } else {
                rgb(header_bg())
            }),
            Name::new("git-section"),
        ))
        .id();
    let mut children = vec![];
    if !label.is_empty() {
        children.push(text_node(commands, fonts, label, 11.0, text_muted()));
    }
    children.push(spacer(commands));
    if let Some((button_label, action, danger)) = action {
        children.push(small_button(
            commands,
            fonts,
            button_label,
            action.clone(),
            *danger,
        ));
    }
    commands.entity(row).add_children(&children);
    row
}

fn build_hint(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let t = text_node(commands, fonts, text, 11.0, placeholder());
    commands.entity(row).add_child(t);
    row
}

fn build_empty(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    title: &str,
    subtitle: &str,
) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(8.0),
            padding: UiRect::all(Val::Px(24.0)),
            ..default()
        })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, placeholder(), 30.0);
    let t = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let s = text_node(commands, fonts, subtitle, 11.0, text_muted());
    commands.entity(root).add_children(&[ic, t, s]);
    root
}

/// A one-letter status badge in the change's colour.
fn badge(commands: &mut Commands, fonts: &EmberFonts, letter: char, tint: (u8, u8, u8)) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(ca(tint, 0.20)),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(letter.to_string()),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(tint)),
        ))
        .id();
    commands.entity(box_e).add_child(t);
    box_e
}

#[allow(clippy::too_many_arguments)]
fn build_file(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    dir: &str,
    letter: char,
    tint: (u8, u8, u8),
    tooltip: &str,
    open: &Action,
    toggle: &(&'static str, Action),
    discard: &Option<Action>,
) -> Entity {
    let row = clickable_row(commands, Some(open.clone()), false);
    commands
        .entity(row)
        .insert(renzora_ember::widgets::HoverTooltip::new(tooltip));

    let mut children = vec![badge(commands, fonts, letter, tint)];
    children.push(text_node(commands, fonts, name, 12.0, text_primary()));
    if !dir.is_empty() {
        // The directory is context, not identity — muted and after the name, the
        // way a file list reads.
        children.push(text_node(
            commands,
            fonts,
            &parse::truncate(dir, 40),
            10.0,
            placeholder(),
        ));
    }
    children.push(spacer(commands));

    let (icon, action) = toggle;
    let stage_hue = if *icon == "minus" { AMBER } else { GREEN };
    let stage_tip = match *icon {
        "minus" => tr("git.tip.unstage", "Unstage"),
        // Staging a conflicted file is a claim that it is resolved, so say that
        // rather than "Stage".
        "check" => tr("git.tip.resolved", "Mark resolved and stage"),
        _ => tr("git.tip.stage", "Stage"),
    };
    children.push(icon_action(
        commands,
        fonts,
        icon,
        stage_hue,
        action.clone(),
        &stage_tip,
    ));
    if let Some(discard) = discard {
        children.push(icon_action(
            commands,
            fonts,
            "arrow-counter-clockwise",
            RED,
            discard.clone(),
            &tr("git.tip.discard", "Discard this change"),
        ));
    }
    commands.entity(row).add_children(&children);
    row
}

/// A ref chip (`main`, `origin/main`) on a history row.
fn ref_chip(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let hue = if label.contains('/') { placeholder() } else { GREEN };
    let chip = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(ca(hue, 0.18)),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(parse::truncate(label, 24)),
            ui_font(&fonts.ui, 9.0),
            TextColor(rgb(hue)),
        ))
        .id();
    commands.entity(chip).add_child(t);
    chip
}

#[allow(clippy::too_many_arguments)]
fn build_commit(
    commands: &mut Commands,
    fonts: &EmberFonts,
    short: &str,
    subject: &str,
    author: &str,
    when: &str,
    refs: &[String],
    is_head: bool,
    is_merge: bool,
    selected: bool,
    action: &Action,
) -> Entity {
    let row = clickable_row(commands, Some(action.clone()), selected);
    // A commit row is two stacked lines rather than one, so turn the shared row into
    // a column — by modifying its `Node`, which keeps the width, minimum height and
    // hover behaviour every other row shares.
    commands.entity(row).entry::<Node>().and_modify(|mut n| {
        n.flex_direction = FlexDirection::Column;
        n.align_items = AlignItems::Stretch;
        n.padding = UiRect::axes(Val::Px(8.0), Val::Px(4.0));
        n.row_gap = Val::Px(2.0);
        n.column_gap = Val::Px(0.0);
    });

    // Top line: the graph marker, the subject, and where refs point.
    let top = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let marker = icon_text(
        commands,
        &fonts.phosphor,
        if is_merge { "git-merge" } else { "git-commit" },
        if is_head { accent() } else { placeholder() },
        12.0,
    );
    let subject_node = text_node(
        commands,
        fonts,
        subject,
        12.0,
        if is_head { text_primary() } else { value_text() },
    );
    let mut top_children = vec![marker, subject_node];
    for r in refs {
        top_children.push(ref_chip(commands, fonts, r));
    }
    commands.entity(top).add_children(&top_children);

    // Bottom line: who and when, plus the short hash to identify it by.
    let meta = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            padding: UiRect::left(Val::Px(18.0)),
            ..default()
        })
        .id();
    let hash = commands
        .spawn((
            Text::new(short.to_string()),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(placeholder())),
        ))
        .id();
    let who = text_node(
        commands,
        fonts,
        &format!("{} · {}", parse::truncate(author, 24), when),
        10.0,
        placeholder(),
    );
    commands.entity(meta).add_children(&[hash, who]);

    commands.entity(row).add_children(&[top, meta]);
    row
}

/// The expanded history row's actions — every way back to this commit, in
/// increasing order of consequence.
fn build_commit_actions(
    commands: &mut Commands,
    fonts: &EmberFonts,
    oid: &str,
    short: &str,
    is_merge: bool,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(4.0),
                padding: UiRect::new(Val::Px(26.0), Val::Px(8.0), Val::Px(4.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(accent()).with_alpha(SELECTED_A * 0.6)),
            Name::new("git-commit-actions"),
        ))
        .id();

    let children = vec![
        small_button(
            commands,
            fonts,
            &tr("git.action.view_diff", "View changes"),
            Action::ShowCommit(oid.to_string()),
            false,
        ),
        small_button(
            commands,
            fonts,
            &tr("git.action.branch_here", "Branch from here"),
            Action::BranchFromCommit(oid.to_string()),
            false,
        ),
        small_button(
            commands,
            fonts,
            &tr("git.action.checkout", "Check out"),
            Action::CheckoutCommitPrompt(oid.to_string()),
            false,
        ),
        small_button(
            commands,
            fonts,
            &tr("git.action.revert", "Revert"),
            Action::RevertPrompt {
                oid: oid.to_string(),
                short: short.to_string(),
                is_merge,
            },
            false,
        ),
        small_button(
            commands,
            fonts,
            &tr("git.action.reset_mixed", "Reset here (keep changes)"),
            Action::ResetMixedPrompt(oid.to_string()),
            false,
        ),
        small_button(
            commands,
            fonts,
            &tr("git.action.reset_hard", "Reset here (discard changes)"),
            Action::ResetHardPrompt(oid.to_string()),
            true,
        ),
    ];
    commands.entity(root).add_children(&children);
    root
}

fn build_branch(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    short_oid: &str,
    upstream: &Option<String>,
    is_head: bool,
    remote: bool,
) -> Entity {
    // The checked-out branch is not clickable as a switch — there is nowhere to go.
    let action = if is_head {
        None
    } else if remote {
        Some(Action::TrackRemote(name.to_string()))
    } else {
        Some(Action::CheckoutBranch(name.to_string()))
    };
    let row = clickable_row(commands, action, is_head);

    let mut children = vec![icon_text(
        commands,
        &fonts.phosphor,
        if remote { "cloud-arrow-down" } else { "git-branch" },
        if is_head { accent() } else { placeholder() },
        12.0,
    )];
    children.push(text_node(
        commands,
        fonts,
        name,
        12.0,
        if is_head { text_primary() } else { value_text() },
    ));
    if let Some(upstream) = upstream {
        children.push(ref_chip(commands, fonts, upstream));
    }
    children.push(spacer(commands));
    children.push(
        commands
            .spawn((
                Text::new(short_oid.to_string()),
                ui_font(&fonts.mono, 10.0),
                TextColor(rgb(placeholder())),
            ))
            .id(),
    );

    // A local branch that is not the current one can be merged in or deleted.
    if !remote && !is_head {
        children.push(small_button(
            commands,
            fonts,
            &tr("git.action.merge", "Merge"),
            Action::MergePrompt(name.to_string()),
            false,
        ));
        children.push(icon_action(
            commands,
            fonts,
            "trash",
            RED,
            Action::DeleteBranchPrompt(name.to_string()),
            &tr("git.tip.delete_branch", "Delete this branch"),
        ));
    }
    commands.entity(row).add_children(&children);
    row
}

fn build_new_branch(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let input = renzora_ember::widgets::text_input(
        commands,
        &fonts.ui,
        &tr("git.placeholder.branch", "new-branch-name"),
        "",
    );
    commands.entity(input).insert(NewBranchInput);
    // Widen it to fill the row by *modifying* the widget's own `Node`, not by
    // replacing it. `text_input` sets `overflow: clip()` (its caret math assumes a
    // single line) and a padding the caret offsets are measured from — replacing
    // the component drops both and puts the caret in the wrong place.
    renzora_ember::inspector::fill_control(commands, input);
    let create = small_button(
        commands,
        fonts,
        &tr("git.action.create_branch", "Create & switch"),
        Action::CreateBranchFromInput,
        false,
    );
    commands.entity(row).add_children(&[input, create]);
    row
}

fn build_item(commands: &mut Commands, fonts: &EmberFonts, it: &Item) -> Entity {
    match it {
        Item::Header { label, action } => build_header(commands, fonts, label, action),
        Item::Hint(s) => build_hint(commands, fonts, s),
        Item::Empty {
            icon,
            title,
            subtitle,
        } => build_empty(commands, fonts, icon, title, subtitle),
        Item::File {
            name,
            dir,
            badge: letter,
            tint,
            tooltip,
            open,
            toggle,
            discard,
        } => build_file(
            commands, fonts, name, dir, *letter, *tint, tooltip, open, toggle, discard,
        ),
        Item::Commit {
            short,
            subject,
            author,
            when,
            refs,
            is_head,
            is_merge,
            selected,
            action,
        } => build_commit(
            commands, fonts, short, subject, author, when, refs, *is_head, *is_merge, *selected,
            action,
        ),
        Item::CommitActions {
            oid,
            short,
            is_merge,
        } => build_commit_actions(commands, fonts, oid, short, *is_merge),
        Item::Branch {
            name,
            short_oid,
            upstream,
            is_head,
            remote,
        } => build_branch(commands, fonts, name, short_oid, upstream, *is_head, *remote),
        Item::NewBranch => build_new_branch(commands, fonts),
    }
}

/// Wall-clock seconds, for the relative timestamps.
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The keyed-list snapshot. Index-keyed and content-hashed: the list reshuffles
/// wholesale when the view changes (rare) and otherwise only individual rows move.
fn list_snapshot(world: &Rx) -> KeyedSnapshot {
    let data: Vec<Item> = match world.get_resource::<GitState>() {
        Some(state) => build_items(state, now_secs()),
        None => Vec::new(),
    };
    let items: Vec<(u64, u64)> = data
        .iter()
        .enumerate()
        .map(|(i, it)| (i as u64, hash_item(it)))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| build_item(c, f, &data[i])),
    }
}

// ── Toolbar ──────────────────────────────────────────────────────────────────

/// Where you are and what the remote thinks, plus the four buttons that change it.
fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            Name::new("git-toolbar"),
        ))
        .id();

    let branch_icon = icon_text(commands, &fonts.phosphor, "git-branch", accent(), 13.0);
    let branch = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text(commands, branch, |world| {
        world
            .get_resource::<GitState>()
            .map(|s| {
                if s.root.is_none() {
                    tr("git.toolbar.no_repo", "No repository")
                } else {
                    s.snapshot.status.head_label()
                }
            })
            .unwrap_or_default()
    });
    // Detached HEAD reads as a normal branch name unless it is coloured as the
    // exception it is — committing there leaves work on no branch at all.
    bind_text_color(commands, branch, |world| {
        let detached = world
            .get_resource::<GitState>()
            .is_some_and(|s| s.snapshot.status.head == parse::Head::Detached);
        if detached {
            rgb(AMBER)
        } else {
            rgb(text_primary())
        }
    });

    // Tracking counts. Hidden when both are zero, so a synced branch is quiet.
    let tracking = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 10.0),
            TextColor(rgb(placeholder())),
        ))
        .id();
    bind_text(commands, tracking, |world| {
        let Some(state) = world.get_resource::<GitState>() else {
            return String::new();
        };
        let s = &state.snapshot.status;
        match (s.ahead, s.behind) {
            (0, 0) => String::new(),
            (a, 0) => format!("↑{a}"),
            (0, b) => format!("↓{b}"),
            (a, b) => format!("↑{a} ↓{b}"),
        }
    });

    // Busy label, in place of nothing happening — the buttons stay put rather than
    // being replaced by a spinner that moves the layout.
    let busy = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(accent())),
        ))
        .id();
    bind_text(commands, busy, |world| {
        world
            .get_resource::<GitState>()
            .and_then(|s| s.progress())
            .unwrap_or("")
            .to_string()
    });

    let gap = spacer(commands);
    let fetch = toolbar_button(commands, fonts, "arrows-clockwise", &tr("git.tip.fetch", "Fetch from remote"), Action::Fetch);
    let pull = toolbar_button(commands, fonts, "cloud-arrow-down", &tr("git.tip.pull", "Pull from remote"), Action::Pull);
    let push = toolbar_button(commands, fonts, "cloud-arrow-up", &tr("git.tip.push", "Push to remote"), Action::Push);
    let refresh = toolbar_button(commands, fonts, "arrow-counter-clockwise", &tr("git.tip.refresh", "Re-read the repository"), Action::Refresh);

    commands.entity(bar).add_children(&[
        branch_icon, branch, tracking, busy, gap, fetch, pull, push, refresh,
    ]);
    bar
}

/// A toolbar icon button, greyed out and inert while a job is running or there is
/// no repository.
fn toolbar_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    tooltip: &str,
    action: Action,
) -> Entity {
    let remote_only = matches!(action, Action::Fetch | Action::Pull | Action::Push);
    let button = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            GitAction(action),
            renzora_ember::widgets::HoverTooltip::new(tooltip),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            bevy::ui::FocusPolicy::Block,
            Name::new("git-toolbar-button"),
        ))
        .id();
    bind_bg(commands, button, move |world| {
        if !enabled(world, remote_only) {
            return Color::NONE;
        }
        match world.get::<Interaction>(button) {
            Some(Interaction::Hovered) => ca(accent(), 0.24),
            Some(Interaction::Pressed) => ca(accent(), 0.36),
            _ => Color::NONE,
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, accent(), 13.0);
    // The one signal that a button is unavailable: the icon dims. The click handler
    // enforces it too — a disabled look that still works is worse than either.
    bind_text_color(commands, glyph, move |world| {
        if enabled(world, remote_only) {
            rgb(accent())
        } else {
            rgb(placeholder()).with_alpha(0.5)
        }
    });
    commands.entity(button).add_child(glyph);
    button
}

/// Can an operation be started right now? `remote` additionally requires a remote
/// to be configured.
fn enabled(world: &Rx<'_>, remote: bool) -> bool {
    world.get_resource::<GitState>().is_some_and(|s| {
        s.ready() && s.can_start() && (!remote || s.snapshot.has_remote())
    })
}

// ── Banners ──────────────────────────────────────────────────────────────────

/// The error banner. Shown only when there is an error, and dismissible — a push
/// rejection is several lines and worth re-reading, so it does not fade like the
/// toast that accompanied it.
fn build_error_banner(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let banner = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                column_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::left(Val::Px(2.0)),
                display: Display::None,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(ca(RED, 0.12)),
            BorderColor::all(rgb(RED)),
            Name::new("git-error"),
        ))
        .id();
    bind_display(commands, banner, |world| {
        world
            .get_resource::<GitState>()
            .is_some_and(|s| s.error.is_some())
    });

    let icon = icon_text(commands, &fonts.phosphor, "warning-circle", RED, 13.0);
    let message = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    bind_text(commands, message, |world| {
        world
            .get_resource::<GitState>()
            .and_then(|s| s.error.clone())
            .unwrap_or_default()
    });
    let dismiss = icon_action(
        commands,
        fonts,
        "x",
        placeholder(),
        Action::ClearError,
        &tr("git.tip.dismiss", "Dismiss"),
    );
    commands.entity(banner).add_children(&[icon, message, dismiss]);
    banner
}

// ── Commit box ───────────────────────────────────────────────────────────────

fn build_commit_box(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::top(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            CommitBox,
            Name::new("git-commit-box"),
        ))
        .id();
    // Only meaningful on the Changes view: the commit box acts on the index, and
    // showing it under the history list would suggest it commits *there*.
    bind_display(commands, root, |world| {
        world
            .get_resource::<GitState>()
            .is_some_and(|s| s.view == View::Changes && s.ready())
    });

    let input = textarea(
        commands,
        &fonts.ui,
        &tr("git.placeholder.commit", "Commit message"),
        "",
    );
    commands.entity(input).insert(CommitMessageInput);
    // Same reasoning as the branch field: widen the widget's own `Node` rather than
    // replacing it, so its padding and border survive.
    commands.entity(input).entry::<Node>().and_modify(|mut n| {
        n.width = Val::Percent(100.0);
        n.min_width = Val::Px(0.0);
    });

    let controls = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    // Amend, as a checkbox row: replacing the previous commit is a different
    // operation from adding one and should not be hidden in a menu.
    let amend_row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                ..default()
            },
            Interaction::default(),
            GitAction(Action::ToggleAmend),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(tr(
                "git.tip.amend",
                "Replace the last commit instead of adding a new one",
            )),
            Name::new("git-amend"),
        ))
        .id();
    let check = commands
        .spawn((
            Node {
                width: Val::Px(13.0),
                height: Val::Px(13.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            AmendBox,
        ))
        .id();
    bind_bg(commands, check, |world| {
        let on = world
            .get_resource::<GitState>()
            .is_some_and(|s| s.amend);
        if on {
            rgb(accent())
        } else {
            Color::NONE
        }
    });
    let amend_label = text_node(commands, fonts, &tr("git.label.amend", "Amend"), 11.0, text_muted());
    commands.entity(amend_row).add_children(&[check, amend_label]);

    // Says what will happen, including the count — "Commit 3 files" is a much
    // better button than "Commit" when the staged set is not on screen.
    let commit_button = small_button(
        commands,
        fonts,
        &tr("git.action.commit", "Commit"),
        Action::Commit,
        false,
    );
    bind_with(
        commands,
        commit_button,
        |world| {
            let Some(state) = world.get_resource::<GitState>() else {
                return (false, String::new());
            };
            let staged = state.snapshot.status.staged().count();
            let label = if state.amend {
                tr("git.action.amend", "Amend last commit")
            } else if staged == 0 {
                tr("git.action.commit", "Commit")
            } else if staged == 1 {
                tr("git.action.commit_one", "Commit 1 file")
            } else {
                format!("{} {staged} {}", tr("git.action.commit_verb", "Commit"), tr("git.action.files", "files"))
            };
            (can_commit(world), label)
        },
        |world, entity, (enabled, label)| {
            // The label lives on the button's single text child.
            let child = world
                .get::<Children>(entity)
                .and_then(|c| c.iter().next());
            if let Some(child) = child {
                if let Some(mut text) = world.get_mut::<Text>(child) {
                    text.0.clone_from(label);
                }
                let color = if *enabled {
                    rgb(accent())
                } else {
                    rgb(placeholder()).with_alpha(0.5)
                };
                if let Some(mut c) = world.get_mut::<TextColor>(child) {
                    c.0 = color;
                }
            }
        },
    );

    let gap = spacer(commands);
    commands
        .entity(controls)
        .add_children(&[amend_row, gap, commit_button]);
    commands.entity(root).add_children(&[input, controls]);
    root
}

/// Is there anything to commit, and can we?
///
/// Amending does not need a staged file — rewording the last commit is a normal
/// thing to want — but a fresh commit does, and an unborn repo cannot amend
/// anything because there is no previous commit.
fn can_commit(world: &Rx<'_>) -> bool {
    world.get_resource::<GitState>().is_some_and(|s| {
        if !s.ready() || !s.can_start() {
            return false;
        }
        if s.amend {
            return !s.snapshot.status.unborn;
        }
        s.snapshot.status.staged().count() > 0
    })
}

// ── View switch ──────────────────────────────────────────────────────────────

fn build_view_tabs(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            Name::new("git-views"),
        ))
        .id();
    let children: Vec<Entity> = [
        (View::Changes, "git.view.changes", "Changes", "git-diff"),
        (View::History, "git.view.history", "History", "clock-counter-clockwise"),
        (View::Branches, "git.view.branches", "Branches", "git-branch"),
    ]
    .into_iter()
    .map(|(view, key, label, icon)| view_tab(commands, fonts, view, &tr(key, label), icon))
    .collect();
    commands.entity(row).add_children(&children);
    row
}

fn view_tab(
    commands: &mut Commands,
    fonts: &EmberFonts,
    view: View,
    label: &str,
    icon: &str,
) -> Entity {
    let tab = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            GitAction(Action::SetView(view)),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            bevy::ui::FocusPolicy::Block,
            Name::new("git-view-tab"),
        ))
        .id();
    let active = move |world: &Rx<'_>| {
        world
            .get_resource::<GitState>()
            .is_some_and(|s| s.view == view)
    };
    bind_bg(commands, tab, move |world| {
        if active(world) {
            ca(accent(), 0.20)
        } else {
            match world.get::<Interaction>(tab) {
                Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                    Color::srgba(1.0, 1.0, 1.0, HOVER_A)
                }
                _ => Color::NONE,
            }
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    bind_text_color(commands, glyph, move |world| {
        if active(world) {
            rgb(accent())
        } else {
            rgb(text_muted())
        }
    });
    let text = text_node(commands, fonts, label, 11.0, text_muted());
    bind_text_color(commands, text, move |world| {
        if active(world) {
            rgb(text_primary())
        } else {
            rgb(text_muted())
        }
    });
    commands.entity(tab).add_children(&[glyph, text]);
    tab
}

// ── Panel assembly ───────────────────────────────────────────────────────────

fn build_panel(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                ..default()
            },
            Name::new("git-panel"),
        ))
        .id();

    let toolbar = build_toolbar(commands, fonts);
    let error = build_error_banner(commands, fonts);
    let tabs = build_view_tabs(commands, fonts);

    // The list scrolls; the toolbar, tabs and commit box do not. `min_height: 0`
    // on the scroll host is what lets it actually shrink inside the flex column
    // instead of pushing the commit box off the bottom.
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                padding: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            Name::new("git-list"),
        ))
        .id();
    // Keyed on `revision`, so a frame in which nothing changed does not rebuild the
    // item list at all — see the module doc.
    keyed_list_tokened(
        commands,
        list,
        |world: &Rx<'_>| {
            world
                .get_resource::<GitState>()
                .map(|s| s.revision)
                .unwrap_or(0)
        },
        list_snapshot,
    );
    // The wrapper's own `Node` is deliberately left alone. It already carries
    // exactly what is wanted here (`width: 100%`, `flex_grow: 1`, `min_height: 0`)
    // *plus* three things that are not obvious and break if replaced:
    // `position_type: Relative`, which is what the absolutely-positioned scrollbar
    // anchors to (without it the bar positions against the panel root and lands in
    // the wrong place); `overflow: clip()`, because the inner viewport is what
    // scrolls, not this; and `flex_basis: 0`, without which the content's height
    // wins and the commit box below gets pushed off the bottom.
    let scroll = scroll_view_keyed(commands, list, "git-panel");

    let commit_box = build_commit_box(commands, fonts);

    commands
        .entity(root)
        .add_children(&[toolbar, error, tabs, scroll, commit_box]);
    root
}

// ── Registration ─────────────────────────────────────────────────────────────

pub(crate) fn register(app: &mut App) {
    app.register_panel_content(PANEL_ID, false, build_panel)
        // Panel-gated: no reason to poll git for a list nobody is looking at.
        .systems(Update, (git_click, auto_refresh))
        // NOT gated. The confirmation and the diff viewer are overlays that live
        // outside the panel's dock pane, and a confirmation left on screen after
        // the tab is switched away would be unanswerable and would block the
        // next operation forever.
        .always(Update, (manage_confirm, manage_diff, overlay_click));
    // The branch and dirty-file count belong in the status bar too: it is the one
    // piece of git state worth knowing without opening the panel.
    use renzora::RenzoraShellExt;
    app.register_shell_status_item(renzora::ShellStatusItem {
        id: "git",
        align: renzora::ShellStatusAlign::Left,
        order: 40,
        render: status_segments,
    });
}

/// The status-bar chip: branch, tracking counts, and how many files are changed.
fn status_segments(world: &World) -> Vec<renzora::ShellStatusSegment> {
    let Some(state) = world.get_resource::<GitState>() else {
        return Vec::new();
    };
    // Nothing to say when the project is not version-controlled — an empty chip is
    // better than one reading "no repository" on every project that has none.
    if !state.ready() {
        return Vec::new();
    }
    let status = &state.snapshot.status;
    let mut segments = vec![renzora::ShellStatusSegment::new(
        "git-branch",
        status.head_label(),
        if status.head == parse::Head::Detached {
            [AMBER.0, AMBER.1, AMBER.2]
        } else {
            let c = text_muted();
            [c.0, c.1, c.2]
        },
    )];
    let changed = status.entries.len();
    if changed > 0 {
        segments.push(renzora::ShellStatusSegment::new(
            "git-diff",
            format!("{changed}"),
            [AMBER.0, AMBER.1, AMBER.2],
        ));
    }
    match (status.ahead, status.behind) {
        (0, 0) => {}
        (a, b) => segments.push(renzora::ShellStatusSegment::new(
            "",
            match (a, b) {
                (a, 0) => format!("↑{a}"),
                (0, b) => format!("↓{b}"),
                (a, b) => format!("↑{a} ↓{b}"),
            },
            {
                let c = placeholder();
                [c.0, c.1, c.2]
            },
        )),
    }
    if status.has_conflicts() {
        segments.push(renzora::ShellStatusSegment::new(
            "warning-circle",
            tr("git.status.conflicts", "conflicts"),
            [RED.0, RED.1, RED.2],
        ));
    }
    segments
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Re-read the repository periodically while the panel is visible.
fn auto_refresh(mut state: ResMut<GitState>) {
    if !state.ready() || !state.due_for_refresh(REFRESH_INTERVAL) {
        return;
    }
    state.request(Job::Refresh);
}

/// Everything the panel can do, in one place. See the module doc for why.
fn git_click(
    clicks: Query<(&Interaction, &GitAction), Changed<Interaction>>,
    mut state: ResMut<GitState>,
    mut inputs: Query<(&mut EmberTextInput, Has<CommitMessageInput>, Has<NewBranchInput>)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for (interaction, action) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Built per click so the query borrows do not have to outlive the loop.
        let mut take = |commit_box: bool| take_input(&mut inputs, &mut texts, commit_box);
        dispatch(&action.0, &mut state, &mut take);
    }
}

/// Clicks on the two overlays, which sit outside the panel's pane and so are not
/// covered by the panel-gated handler.
fn overlay_click(
    clicks: Query<(&Interaction, &GitAction), Changed<Interaction>>,
    mut state: ResMut<GitState>,
) {
    for (interaction, action) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match &action.0 {
            Action::ConfirmAccept => state.confirm_accept(),
            Action::ConfirmCancel => state.confirm_cancel(),
            // Everything else belongs to the panel's own handler. Both systems see
            // every click, so acting on more than these here would run a panel
            // action twice.
            _ => {}
        }
    }
}

/// Read a text input's value and clear it.
///
/// The widget only repaints its display text on a keystroke, so clearing `value`
/// alone leaves the sent text on screen — the placeholder has to be restored by
/// hand, the same way the AI chat panel does it.
fn take_input(
    inputs: &mut Query<(&mut EmberTextInput, Has<CommitMessageInput>, Has<NewBranchInput>)>,
    texts: &mut Query<(&mut Text, &mut TextColor)>,
    commit_box: bool,
) -> Option<String> {
    for (mut input, is_commit, is_branch) in inputs.iter_mut() {
        let wanted = if commit_box { is_commit } else { is_branch };
        if !wanted {
            continue;
        }
        let value = input.value.trim().to_string();
        if value.is_empty() {
            return None;
        }
        input.value.clear();
        input.caret_index = 0;
        if let Ok((mut text, mut color)) = texts.get_mut(input.text_entity) {
            text.0.clone_from(&input.placeholder);
            color.0 = rgb(text_muted());
        }
        return Some(value);
    }
    None
}

/// Decide what an action does.
///
/// `take_field` reads (and clears) one of the two text fields: `true` for the
/// commit message, `false` for the new-branch name. Injected rather than read from
/// the world here so this — the function that decides what needs confirming — can
/// be tested without a UI, which is the only way the "must ask first" tests below
/// can cover every action.
fn dispatch(
    action: &Action,
    state: &mut GitState,
    take_field: &mut dyn FnMut(bool) -> Option<String>,
) {
    // View switching and dismissal are always available; everything that runs git
    // is not. Checking once here is what makes the dimmed buttons honest.
    match action {
        Action::SetView(view) => return state.set_view(*view),
        Action::ClearError => return state.clear_error(),
        Action::ToggleAmend => {
            state.amend = !state.amend;
            state.touch();
            return;
        }
        Action::SelectCommit(oid) => return state.toggle_commit(oid),
        _ => {}
    }

    // Not just "a job is running": a job queued by another click this same frame
    // has not started yet, and starting past it would silently drop one of the two.
    if !state.can_start() {
        return;
    }
    // `Init` is the one operation that runs without a repository — it creates one.
    if !state.ready() && *action != Action::Init {
        return;
    }
    // The same gate the toolbar dims these buttons with. Enforced here as well
    // because a dimmed button that still works is worse than either a working one
    // or a disabled one — and "push failed" is a much worse answer than a button
    // that visibly cannot be pressed.
    if matches!(action, Action::Fetch | Action::Pull | Action::Push)
        && !state.snapshot.has_remote()
    {
        return;
    }

    match action {
        // Handled above.
        Action::SetView(_)
        | Action::ClearError
        | Action::ToggleAmend
        | Action::SelectCommit(_) => {}

        // Owned by `overlay_click`, deliberately not here. Both systems observe
        // every click, so handling these in both would answer one confirmation
        // twice — and the confirmation has to be answerable while the panel's tab
        // is hidden, which only `overlay_click` (ungated) can do.
        Action::ConfirmAccept | Action::ConfirmCancel => {}

        Action::Init => state.request(Job::Init),
        Action::Refresh => state.request(Job::Refresh),
        Action::Stage(paths) => state.request(Job::Stage(paths.clone())),
        Action::Unstage(paths) => state.request(Job::Unstage(paths.clone())),

        // ── Irreversible ────────────────────────────────────────────────────
        Action::DiscardPrompt {
            tracked,
            untracked,
            what,
        } => {
            // The two halves destroy differently, and the prompt has to say which:
            // a tracked file is restored to its committed state, an untracked one is
            // deleted outright.
            let mut body = String::new();
            if !tracked.is_empty() {
                body.push_str(&format!(
                    "{} will go back to its last committed state.\n",
                    plural(tracked.len(), "file", "files")
                ));
            }
            if !untracked.is_empty() {
                body.push_str(&format!(
                    "{} will be deleted from disk — git has never tracked them, so there is no \
                     copy anywhere.\n",
                    plural(untracked.len(), "file", "files")
                ));
            }
            body.push_str("\nThis cannot be undone, by the editor or by git.");
            state.ask(Confirm {
                title: format!("Discard changes to {what}?"),
                body,
                action_label: tr("git.action.discard", "Discard"),
                danger: true,
                job: Job::Discard {
                    tracked: tracked.clone(),
                    untracked: untracked.clone(),
                },
            });
        }
        Action::ResetHardPrompt(oid) => state.ask(Confirm {
            title: tr("git.confirm.reset_hard.title", "Discard everything after this commit?"),
            body: format!(
                "The branch will move back to {}, and every change after it — committed or \
                 not — will be gone from your working tree.\n\nUncommitted changes cannot be \
                 recovered. Commits can, from `git reflog`, for a while.",
                short_of(oid)
            ),
            action_label: tr("git.action.reset_hard_short", "Reset and discard"),
            danger: true,
            job: Job::Reset {
                rev: oid.clone(),
                hard: true,
            },
        }),

        // ── Significant but recoverable ─────────────────────────────────────
        Action::ResetMixedPrompt(oid) => state.ask(Confirm {
            title: tr("git.confirm.reset_mixed.title", "Move the branch back to this commit?"),
            body: format!(
                "The branch will point at {}, and everything committed after it will come back \
                 as uncommitted changes in your working tree. Nothing is deleted.",
                short_of(oid)
            ),
            action_label: tr("git.action.reset_mixed_short", "Move branch"),
            danger: false,
            job: Job::Reset {
                rev: oid.clone(),
                hard: false,
            },
        }),
        Action::CheckoutCommitPrompt(oid) => state.ask(Confirm {
            title: tr("git.confirm.checkout.title", "Check out this commit?"),
            body: format!(
                "Your project files will be replaced with their contents at {}, and HEAD will \
                 be detached — you will not be on a branch.\n\nAny commit you make there \
                 belongs to no branch and is easy to lose. To keep working from this point, \
                 use \"Branch from here\" instead. To come back, switch to a branch.",
                short_of(oid)
            ),
            action_label: tr("git.action.checkout_short", "Check out"),
            danger: false,
            job: Job::Checkout(oid.clone()),
        }),
        Action::RevertPrompt {
            oid,
            short,
            is_merge,
        } => state.ask(Confirm {
            title: format!("Revert {short}?"),
            body: if *is_merge {
                format!(
                    "This adds a new commit undoing everything {short} merged in. History is \
                     kept — reverting the revert puts it back.\n\nThis is a merge commit, so \
                     the changes from the branch that was merged are the ones undone."
                )
            } else {
                format!(
                    "This adds a new commit that undoes the changes in {short}. Nothing is \
                     removed from history, and reverting the revert puts it back."
                )
            },
            action_label: tr("git.action.revert_short", "Revert"),
            danger: false,
            job: Job::Revert {
                rev: oid.clone(),
                is_merge: *is_merge,
            },
        }),
        Action::MergePrompt(name) => state.ask(Confirm {
            title: format!("Merge {name} into {}?", current_branch(state)),
            body: format!(
                "Commits from {name} will be combined into {}. If the same lines changed on \
                 both sides, the merge stops with conflicts for you to resolve — you can abort \
                 it then.",
                current_branch(state)
            ),
            action_label: tr("git.action.merge_short", "Merge"),
            danger: false,
            job: Job::Merge(name.clone()),
        }),
        Action::DeleteBranchPrompt(name) => state.ask(Confirm {
            title: format!("Delete branch {name}?"),
            body: tr(
                "git.confirm.delete_branch.body",
                "The branch label is removed. Its commits are kept if they are merged \
                 somewhere else — and if they are not, git refuses and says so rather than \
                 losing them.",
            ),
            action_label: tr("git.action.delete_short", "Delete"),
            danger: true,
            job: Job::DeleteBranch {
                name: name.clone(),
                // Never forced from here. A branch holding unmerged commits is
                // exactly the one where a single click must not be enough.
                force: false,
            },
        }),
        Action::AbortMergePrompt => state.ask(Confirm {
            title: tr("git.confirm.abort_merge.title", "Abort the merge?"),
            body: tr(
                "git.confirm.abort_merge.body",
                "Your files go back to how they were before the merge started. Any conflict \
                 resolution you have done is discarded.",
            ),
            action_label: tr("git.action.abort_short", "Abort merge"),
            danger: true,
            job: Job::MergeAbort,
        }),

        // ── Safe ────────────────────────────────────────────────────────────
        Action::Fetch => state.request(Job::Fetch),
        Action::Pull => state.request(Job::Pull),
        Action::Push => {
            let set_upstream = state.snapshot.push_needs_upstream();
            let branch = state.snapshot.status.branch().map(str::to_string);
            state.request(Job::Push {
                set_upstream,
                branch,
            });
        }
        Action::CheckoutBranch(name) => state.request(Job::Checkout(name.clone())),
        Action::TrackRemote(name) => {
            // `origin/feature` → a local `feature` tracking it. Checking the remote
            // ref out directly would detach HEAD, which is never what clicking a
            // branch means.
            let local = name.split_once('/').map(|(_, rest)| rest).unwrap_or(name);
            state.request(Job::CreateBranch {
                name: local.to_string(),
                start: Some(name.clone()),
            });
        }
        Action::BranchFromCommit(oid) => {
            // Named after the commit so it is identifiable, and unique enough not
            // to collide with an existing branch.
            let name = format!("from-{}", short_of(oid));
            state.request(Job::CreateBranch {
                name,
                start: Some(oid.clone()),
            });
        }
        Action::CreateBranchFromInput => match take_field(false) {
            Some(name) => state.request(Job::CreateBranch { name, start: None }),
            None => {
                state.error = Some(tr(
                    "git.error.empty_branch",
                    "Enter a name for the new branch first.",
                ));
                state.touch();
            }
        },
        Action::ShowDiff {
            path,
            staged,
            untracked,
        } => {
            state.diff_title = Some(path.clone());
            state.diff_text = None;
            state.request(Job::Diff {
                path: path.clone(),
                staged: *staged,
                untracked: *untracked,
            });
        }
        Action::ShowCommit(oid) => {
            state.diff_title = Some(short_of(oid));
            state.diff_text = None;
            state.request(Job::Show(oid.clone()));
        }
        Action::Commit => {
            let amend = state.amend;
            let staged = state.snapshot.status.staged().count();
            if !amend && staged == 0 {
                return;
            }
            let Some(message) = take_field(true) else {
                state.error = Some(tr(
                    "git.error.empty_message",
                    "Enter a commit message first.",
                ));
                state.touch();
                return;
            };
            state.request(Job::Commit { message, amend });
            state.amend = false;
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Abbreviate an oid for display. Git's own default is 7–8 characters.
fn short_of(oid: &str) -> String {
    oid.chars().take(8).collect()
}

fn current_branch(state: &GitState) -> String {
    state
        .snapshot
        .status
        .branch()
        .unwrap_or("the current branch")
        .to_string()
}

// ── Overlays ─────────────────────────────────────────────────────────────────

/// Spawn/despawn the confirmation to match [`GitState::confirm`].
///
/// The `spawned` local is the same guard the update dialog needs: ember's generic
/// overlay dismissal can despawn the root without this crate knowing, so "state
/// says visible, no root exists" is ambiguous between "not built yet" and "just
/// dismissed". Without the flag the second case looks like the first and the
/// modal reopens every frame.
fn manage_confirm(world: &mut World, mut spawned: Local<bool>) {
    let wanted = world
        .get_resource::<GitState>()
        .is_some_and(|s| s.confirm.is_some());
    let existing: Vec<Entity> = world
        .query_filtered::<Entity, With<ConfirmRoot>>()
        .iter(world)
        .collect();

    match (wanted, existing.is_empty(), *spawned) {
        (true, true, false) => {
            let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
                return;
            };
            // Cloned out of state because building needs `&mut Commands` (and so
            // `&mut World`) while reading the confirmation.
            let Some((title, body, label, danger)) =
                world.get_resource::<GitState>().and_then(|s| {
                    s.confirm.as_ref().map(|c| {
                        (
                            c.title.clone(),
                            c.body.clone(),
                            c.action_label.clone(),
                            c.danger,
                        )
                    })
                })
            else {
                return;
            };
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, world);
                spawn_confirm(&mut commands, &fonts, &title, &body, &label, danger);
            }
            queue.apply(world);
            *spawned = true;
        }
        // Dismissed by ember (Escape / backdrop). Treated as a cancel: an
        // unanswered confirmation must not stay pending, or it silently blocks the
        // next one.
        (true, true, true) => {
            *spawned = false;
            if let Some(mut state) = world.get_resource_mut::<GitState>() {
                state.confirm_cancel();
            }
        }
        (false, false, _) => {
            for e in existing {
                world.entity_mut(e).despawn();
            }
            *spawned = false;
        }
        _ => {}
    }
}

fn spawn_confirm(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    body: &str,
    action_label: &str,
    danger: bool,
) {
    let (root, content) = renzora_ember::widgets::overlay_sized(commands, fonts, title, 460.0, 260.0, true);
    commands.entity(root).insert(ConfirmRoot);

    let panel = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            row_gap: Val::Px(14.0),
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        })
        .id();

    let message = commands
        .spawn((
            Text::new(body.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();

    let buttons = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let gap = spacer(commands);
    // Cancel first in the tab order and on the left of the pair: the safe choice
    // should be the one a stray Return does not hit.
    let cancel = small_button(
        commands,
        fonts,
        &tr("git.action.cancel", "Cancel"),
        Action::ConfirmCancel,
        false,
    );
    let accept = small_button(commands, fonts, action_label, Action::ConfirmAccept, danger);
    commands.entity(buttons).add_children(&[gap, cancel, accept]);

    commands.entity(panel).add_children(&[message, buttons]);
    commands.entity(content).add_child(panel);
}

/// Spawn/despawn the diff viewer to match [`GitState::diff_title`].
fn manage_diff(world: &mut World, mut spawned: Local<bool>) {
    let wanted = world
        .get_resource::<GitState>()
        .is_some_and(|s| s.diff_title.is_some());
    let existing: Vec<Entity> = world
        .query_filtered::<Entity, With<DiffRoot>>()
        .iter(world)
        .collect();

    match (wanted, existing.is_empty(), *spawned) {
        (true, true, false) => {
            let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
                return;
            };
            let title = world
                .get_resource::<GitState>()
                .and_then(|s| s.diff_title.clone())
                .unwrap_or_default();
            let mut queue = bevy::ecs::world::CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, world);
                spawn_diff(&mut commands, &fonts, &title);
            }
            queue.apply(world);
            *spawned = true;
        }
        (true, true, true) => {
            *spawned = false;
            if let Some(mut state) = world.get_resource_mut::<GitState>() {
                state.close_diff();
            }
        }
        (false, false, _) => {
            for e in existing {
                world.entity_mut(e).despawn();
            }
            *spawned = false;
        }
        _ => {}
    }
}

/// Diff colours, applied per line. A diff is unreadable without them.
fn diff_line_color(line: &str) -> (u8, u8, u8) {
    // Order matters: the `+++`/`---` file headers start with the same characters
    // as added/removed lines and must be matched first, or every diff opens with a
    // green and a red line that are not changes at all.
    if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") || line.starts_with("index ") {
        return placeholder();
    }
    if line.starts_with("@@") {
        return BLUE;
    }
    match line.as_bytes().first() {
        Some(b'+') => GREEN,
        Some(b'-') => RED,
        _ => text_muted(),
    }
}

fn spawn_diff(commands: &mut Commands, fonts: &EmberFonts, title: &str) {
    let (root, content) =
        renzora_ember::widgets::overlay_sized(commands, fonts, title, 780.0, 560.0, true);
    commands.entity(root).insert(DiffRoot);

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .id();

    // The lines are a keyed list over the loaded text, so the viewer shows a
    // "Loading…" line first and fills in when the worker answers, without the
    // overlay being rebuilt.
    // No `width: 100%`: a both-axis scroll view needs its content to size to its
    // natural extent so the horizontal axis can actually overflow. `min_width`
    // keeps it filling the viewport when the diff is narrow.
    let lines = commands
        .spawn((
            Node {
                min_width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("git-diff-lines"),
        ))
        .id();
    keyed_list_tokened(
        commands,
        lines,
        |world: &Rx<'_>| {
            // The text arrives once, so its length is enough of a token — and it
            // avoids re-hashing several thousand diff lines every frame.
            world
                .get_resource::<GitState>()
                .and_then(|s| s.diff_text.as_ref().map(|t| t.len() as u64))
                .unwrap_or(0)
        },
        diff_snapshot,
    );
    // Both axes: diff lines are `no_wrap`, so long ones have to scroll sideways
    // rather than be cut off. Not the keyed variant — the overlay is spawned fresh
    // each time it opens, so there is no scroll position worth restoring, and
    // there is no keyed both-axis view anyway.
    let scroll = renzora_ember::widgets::scroll_view_xy(commands, lines);

    commands.entity(body).add_child(scroll);
    commands.entity(content).add_child(body);
}

/// Cap on rendered diff lines.
///
/// One row is one entity with a text layout, and a diff of a whole imported model
/// can be hundreds of thousands of lines — enough to stall the editor for minutes
/// building UI nobody will scroll through. The cap is stated in the last row rather
/// than silently applied.
const MAX_DIFF_LINES: usize = 4000;

fn diff_snapshot(world: &Rx) -> KeyedSnapshot {
    let text = world
        .get_resource::<GitState>()
        .and_then(|s| s.diff_text.clone());
    let mut lines: Vec<String> = match &text {
        None => vec![tr("git.diff.loading", "Loading…")],
        Some(t) if t.trim().is_empty() => {
            vec![tr("git.diff.empty", "No changes to show.")]
        }
        Some(t) => t.lines().take(MAX_DIFF_LINES).map(str::to_string).collect(),
    };
    if let Some(t) = &text {
        let total = t.lines().count();
        if total > MAX_DIFF_LINES {
            lines.push(String::new());
            lines.push(format!(
                "… {} more lines not shown (showing the first {MAX_DIFF_LINES}).",
                total - MAX_DIFF_LINES
            ));
        }
    }

    let items: Vec<(u64, u64)> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            line.hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| {
            let line = &lines[i];
            commands
                .spawn((
                    Text::new(line.clone()),
                    ui_font(&fonts.mono, 11.0),
                    TextColor(rgb(diff_line_color(line))),
                    // Diffs are pre-formatted: wrapping a long line would break the
                    // column alignment that makes one readable. It scrolls instead.
                    bevy::text::TextLayout::no_wrap(),
                ))
                .id()
        }),
    }
}

#[cfg(test)]
impl GitState {
    fn pending_for_test(&self) -> Option<Job> {
        self.pending.clone()
    }
}

/// Run an action through [`dispatch`] with both text fields empty.
///
/// Empty is the interesting default: it is what an untouched commit box looks like,
/// and the case that must not produce an empty commit.
#[cfg(test)]
fn route(action: &Action, state: &mut GitState) {
    route_typed(action, state, None);
}

/// Run an action through [`dispatch`] with `typed` in whichever field is read.
#[cfg(test)]
fn route_typed(action: &Action, state: &mut GitState, typed: Option<&str>) {
    let typed = typed.map(str::to_string);
    let mut take = |_commit_box: bool| typed.clone();
    dispatch(action, state, &mut take);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::Snapshot;
    use crate::parse::{Head, RepoStatus};

    fn entry(path: &str, index: Option<Change>, worktree: Option<Change>) -> StatusEntry {
        StatusEntry {
            path: path.to_string(),
            orig_path: None,
            index,
            worktree,
        }
    }

    /// A ready repository on `main`, with one remote — the ordinary case. Tests
    /// that care about having no remote clear it explicitly.
    fn state_with(entries: Vec<StatusEntry>) -> GitState {
        GitState {
            git_version: Some(Ok("git version 2.54.0".into())),
            root: Some(std::path::PathBuf::from("/repo")),
            snapshot: Snapshot {
                remotes: vec!["origin".into()],
                status: RepoStatus {
                    head: Head::Branch("main".into()),
                    oid: Some("abcdef1234".into()),
                    entries,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Git is installed but the project is not in a repository — the state that
    /// offers "Initialize Repository".
    fn state_without_repo() -> GitState {
        GitState {
            git_version: Some(Ok("git version 2.54.0".into())),
            ..Default::default()
        }
    }

    /// Compact rendering, so a test can assert on the list without matching
    /// ten-field struct variants inline.
    fn sketch(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|it| match it {
                Item::Header { label, action } => format!(
                    "header:{label}{}",
                    match action {
                        Some((l, _, danger)) => format!("[{l}{}]", if *danger { "!" } else { "" }),
                        None => String::new(),
                    }
                ),
                Item::Hint(_) => "hint".to_string(),
                Item::Empty { title, .. } => format!("empty:{title}"),
                Item::File {
                    name,
                    badge,
                    toggle,
                    discard,
                    ..
                } => format!(
                    "file:{name}/{badge}/{}{}",
                    toggle.0,
                    if discard.is_some() { "+discard" } else { "" }
                ),
                Item::Commit { short, .. } => format!("commit:{short}"),
                Item::CommitActions { short, .. } => format!("actions:{short}"),
                Item::Branch { name, is_head, .. } => {
                    format!("branch:{name}{}", if *is_head { "*" } else { "" })
                }
                Item::NewBranch => "new-branch".to_string(),
            })
            .collect()
    }

    // ── the empty states, which are also the error states ────────────────────

    /// Git missing is a state of its own, not a failed operation. Getting this
    /// wrong means every button reports "program not found" instead.
    #[test]
    fn a_missing_git_shows_its_own_state_and_no_operations() {
        let state = GitState {
            git_version: Some(Err("`git` was not found on your PATH.".into())),
            ..Default::default()
        };
        let items = build_items(&state, 0);
        assert_eq!(items.len(), 1);
        assert!(matches!(&items[0], Item::Empty { title, .. } if title.contains("not available")));
    }

    /// A project outside a repository gets exactly one action, and it is Init.
    #[test]
    fn a_project_outside_a_repo_is_offered_initialization_only() {
        let state = state_without_repo();
        let items = build_items(&state, 0);
        let actions: Vec<&Action> = items
            .iter()
            .filter_map(|it| match it {
                Item::Header {
                    action: Some((_, a, _)),
                    ..
                } => Some(a),
                _ => None,
            })
            .collect();
        assert_eq!(actions, vec![&Action::Init]);
    }

    #[test]
    fn a_clean_tree_says_there_is_nothing_to_commit() {
        let state = state_with(vec![]);
        assert_eq!(sketch(&build_items(&state, 0)), vec!["empty:Nothing to commit"]);
    }

    // ── the changes list ────────────────────────────────────────────────────

    /// A file staged *and* edited again is two rows that do different things.
    /// Collapsing it to one loses the ability to unstage without discarding.
    #[test]
    fn a_file_both_staged_and_edited_appears_in_both_sections() {
        let state = state_with(vec![entry(
            "a.txt",
            Some(Change::Modified),
            Some(Change::Modified),
        )]);
        let sketch = sketch(&build_items(&state, 0));
        assert_eq!(
            sketch,
            vec![
                "header:Staged (1)[Unstage all]",
                "file:a.txt/M/minus",
                "header:Changes (1)[Stage all]",
                "header:[Discard all!]",
                "file:a.txt/M/plus+discard",
            ]
        );
    }

    /// The staged row has no discard button: discarding would also throw away the
    /// unstaged edit, which that row does not represent.
    #[test]
    fn only_the_working_tree_row_can_discard() {
        let staged = file_item(&entry("a.txt", Some(Change::Modified), None), true);
        let unstaged = file_item(&entry("a.txt", None, Some(Change::Modified)), false);
        assert!(matches!(staged, Item::File { discard: None, .. }));
        assert!(matches!(unstaged, Item::File { discard: Some(_), .. }));
    }

    /// Untracked and tracked files are discarded by different commands with
    /// different consequences, and the action has to carry the distinction all the
    /// way to the confirmation.
    #[test]
    fn discarding_an_untracked_file_is_routed_as_a_deletion() {
        let item = file_item(&entry("new.txt", None, Some(Change::Untracked)), false);
        let Item::File {
            discard: Some(Action::DiscardPrompt {
                tracked,
                untracked,
                ..
            }),
            ..
        } = item
        else {
            panic!("an untracked file must offer a discard");
        };
        assert!(tracked.is_empty(), "it is not tracked, so nothing is restored");
        assert_eq!(untracked, vec!["new.txt"]);
    }

    #[test]
    fn discarding_a_tracked_file_is_routed_as_a_restore() {
        let item = file_item(&entry("a.txt", None, Some(Change::Modified)), false);
        let Item::File {
            discard: Some(Action::DiscardPrompt {
                tracked,
                untracked,
                ..
            }),
            ..
        } = item
        else {
            panic!("a tracked file must offer a discard");
        };
        assert_eq!(tracked, vec!["a.txt"]);
        assert!(untracked.is_empty(), "restoring must not delete anything");
    }

    /// "Discard all" spans both kinds, and each path must land in the right half —
    /// mixing them up would delete a tracked file or try to restore an untracked
    /// one.
    #[test]
    fn discard_all_splits_tracked_from_untracked() {
        let state = state_with(vec![
            entry("tracked.txt", None, Some(Change::Modified)),
            entry("new.txt", None, Some(Change::Untracked)),
        ]);
        let items = build_items(&state, 0);
        let discard = items
            .iter()
            .find_map(|it| match it {
                Item::Header {
                    action: Some((_, a @ Action::DiscardPrompt { .. }, _)),
                    ..
                } => Some(a),
                _ => None,
            })
            .expect("a discard-all action");
        let Action::DiscardPrompt {
            tracked, untracked, ..
        } = discard
        else {
            unreachable!()
        };
        assert_eq!(tracked, &vec!["tracked.txt".to_string()]);
        assert_eq!(untracked, &vec!["new.txt".to_string()]);
    }

    /// Conflicts come first and get their own section: nothing else can be
    /// finished until they are resolved, and burying them hides that.
    #[test]
    fn conflicts_are_listed_first_with_a_way_to_abort() {
        let state = state_with(vec![
            entry("z.txt", None, Some(Change::Modified)),
            entry("c.txt", None, Some(Change::Conflicted)),
        ]);
        let sketch = sketch(&build_items(&state, 0));
        assert!(sketch[0].starts_with("header:Conflicts"), "got {sketch:?}");
        assert!(sketch[0].contains("Abort merge"));
        assert!(sketch.iter().any(|s| s.starts_with("file:c.txt")));
        // And it is not also counted among the ordinary changes.
        assert!(sketch.iter().any(|s| s == "header:Changes (1)[Stage all]"));
    }

    /// Staging a conflicted file means "I have resolved this", so it gets a tick
    /// rather than a plus — the same command, a different claim.
    #[test]
    fn a_conflicted_file_is_staged_as_resolved() {
        let item = file_item(&entry("c.txt", None, Some(Change::Conflicted)), false);
        let Item::File { toggle, .. } = &item else {
            panic!("expected a file row")
        };
        assert_eq!(toggle.0, "check");
    }

    // ── the history list ────────────────────────────────────────────────────

    fn commit(oid: &str, subject: &str, parents: usize) -> Commit {
        Commit {
            oid: oid.to_string(),
            short: oid.chars().take(8).collect(),
            author: "A".into(),
            timestamp: 0,
            refs: vec![],
            parents: (0..parents).map(|i| format!("p{i}")).collect(),
            subject: subject.to_string(),
            is_head: false,
        }
    }

    #[test]
    fn an_unborn_repo_says_there_are_no_commits_yet() {
        let mut state = state_with(vec![]);
        state.view = View::History;
        state.snapshot.status.unborn = true;
        assert_eq!(
            sketch(&build_items(&state, 0)),
            vec!["empty:No commits yet"]
        );
    }

    /// The actions row belongs to the expanded commit and only to it — one open at
    /// a time, or "Reset here" appears six times with six different meanings.
    #[test]
    fn only_the_expanded_commit_shows_its_actions() {
        let mut state = state_with(vec![]);
        state.view = View::History;
        state.snapshot.log = vec![commit("aaaaaaaa11", "one", 1), commit("bbbbbbbb22", "two", 1)];
        state.selected_commit = Some("bbbbbbbb22".into());
        assert_eq!(
            sketch(&build_items(&state, 0)),
            vec!["commit:aaaaaaaa", "commit:bbbbbbbb", "actions:bbbbbbbb"]
        );
    }

    /// A merge commit needs `-m 1`, so the action has to know it is one or the
    /// revert simply fails.
    #[test]
    fn a_merge_commits_actions_carry_the_merge_flag() {
        let mut state = state_with(vec![]);
        state.view = View::History;
        state.snapshot.log = vec![commit("mmmmmmmm11", "merge", 2)];
        state.selected_commit = Some("mmmmmmmm11".into());
        let items = build_items(&state, 0);
        assert!(
            items
                .iter()
                .any(|it| matches!(it, Item::CommitActions { is_merge: true, .. })),
            "a two-parent commit must be flagged as a merge"
        );
    }

    /// A capped list that says nothing reads as the whole history.
    #[test]
    fn a_capped_history_says_it_is_capped() {
        let mut state = state_with(vec![]);
        state.view = View::History;
        state.snapshot.log = (0..crate::job::LOG_LIMIT)
            .map(|i| commit(&format!("{i:010}"), "s", 1))
            .collect();
        let items = build_items(&state, 0);
        assert!(matches!(items.last(), Some(Item::Hint(_))));
    }

    // ── the branches list ───────────────────────────────────────────────────

    fn branch(name: &str, is_head: bool, remote: bool) -> parse::BranchRef {
        parse::BranchRef {
            full: format!("refs/{name}"),
            name: name.to_string(),
            short_oid: "abc12345".into(),
            upstream: None,
            is_head,
            remote,
        }
    }

    #[test]
    fn branches_list_local_first_with_the_current_one_marked() {
        let mut state = state_with(vec![]);
        state.view = View::Branches;
        state.snapshot.refs = vec![
            branch("main", true, false),
            branch("feature", false, false),
            branch("origin/main", false, true),
        ];
        let sketch = sketch(&build_items(&state, 0));
        assert!(sketch.contains(&"branch:main*".to_string()));
        assert!(sketch.contains(&"branch:feature".to_string()));
        assert!(sketch.contains(&"new-branch".to_string()));
        let local_at = sketch.iter().position(|s| s == "branch:main*").unwrap();
        let remote_at = sketch.iter().position(|s| s == "branch:origin/main").unwrap();
        assert!(local_at < remote_at, "local branches come first");
    }

    // ── action routing ──────────────────────────────────────────────────────

    /// Checking out `origin/feature` directly detaches HEAD, which is never what
    /// clicking a branch means. It has to become a local branch that tracks it.
    #[test]
    fn switching_to_a_remote_branch_creates_a_local_tracking_branch() {
        let mut state = state_with(vec![]);
        let action = Action::TrackRemote("origin/feature".into());
        route(&action, &mut state);
        assert_eq!(
            state.pending_for_test(),
            Some(Job::CreateBranch {
                name: "feature".into(),
                start: Some("origin/feature".into()),
            })
        );
    }

    /// A remote branch whose name contains slashes keeps everything after the
    /// remote — `origin/fix/thing` is the local branch `fix/thing`.
    #[test]
    fn a_remote_branch_with_slashes_keeps_all_but_the_remote_name() {
        let mut state = state_with(vec![]);
        route(&Action::TrackRemote("origin/fix/snapping".into()), &mut state);
        assert_eq!(
            state.pending_for_test(),
            Some(Job::CreateBranch {
                name: "fix/snapping".into(),
                start: Some("origin/fix/snapping".into()),
            })
        );
    }

    /// Every destructive action must reach a confirmation rather than a job — this
    /// is the test that fails if a new one is wired straight through.
    #[test]
    fn destructive_actions_ask_before_they_run() {
        for action in [
            Action::DiscardPrompt {
                tracked: vec!["a".into()],
                untracked: vec![],
                what: "a".into(),
            },
            Action::ResetHardPrompt("abc".into()),
            Action::ResetMixedPrompt("abc".into()),
            Action::CheckoutCommitPrompt("abc".into()),
            Action::RevertPrompt {
                oid: "abc".into(),
                short: "abc".into(),
                is_merge: false,
            },
            Action::MergePrompt("side".into()),
            Action::DeleteBranchPrompt("side".into()),
            Action::AbortMergePrompt,
        ] {
            let mut state = state_with(vec![]);
            route(&action, &mut state);
            assert!(
                state.confirm.is_some(),
                "{action:?} must ask for confirmation"
            );
            assert_eq!(
                state.pending_for_test(),
                None,
                "{action:?} must not start a job before it is confirmed"
            );
        }
    }

    /// The unrecoverable ones must also *look* unrecoverable, and the merely
    /// significant ones must not — a red button on everything trains the user to
    /// click through it.
    #[test]
    fn only_unrecoverable_actions_are_marked_dangerous() {
        let danger_of = |action: Action| {
            let mut state = state_with(vec![]);
            route(&action, &mut state);
            state.confirm.as_ref().map(|c| c.danger)
        };
        assert_eq!(
            danger_of(Action::DiscardPrompt {
                tracked: vec!["a".into()],
                untracked: vec![],
                what: "a".into()
            }),
            Some(true)
        );
        assert_eq!(danger_of(Action::ResetHardPrompt("abc".into())), Some(true));
        // Recoverable: the changes come back as uncommitted work.
        assert_eq!(danger_of(Action::ResetMixedPrompt("abc".into())), Some(false));
        // Recoverable: adds a commit, removes nothing.
        assert_eq!(
            danger_of(Action::RevertPrompt {
                oid: "abc".into(),
                short: "abc".into(),
                is_merge: false
            }),
            Some(false)
        );
        assert_eq!(danger_of(Action::CheckoutCommitPrompt("abc".into())), Some(false));
    }

    /// Deleting a branch must never be forced from a single click: `-D` throws away
    /// unmerged commits, and git's refusal is the safety net.
    #[test]
    fn deleting_a_branch_is_never_forced() {
        let mut state = state_with(vec![]);
        route(&Action::DeleteBranchPrompt("side".into()), &mut state);
        let job = state.confirm.as_ref().map(|c| c.job.clone());
        assert_eq!(
            job,
            Some(Job::DeleteBranch {
                name: "side".into(),
                force: false
            })
        );
    }

    /// A discard prompt has to say which files are restored and which are deleted —
    /// it is the only place the user learns that an untracked file has no copy.
    #[test]
    fn a_discard_prompt_distinguishes_restoring_from_deleting() {
        let mut state = state_with(vec![]);
        route(
            &Action::DiscardPrompt {
                tracked: vec!["a.txt".into()],
                untracked: vec!["new.txt".into()],
                what: "2 files".into(),
            },
            &mut state,
        );
        let body = state.confirm.as_ref().unwrap().body.clone();
        assert!(body.contains("last committed state"), "got: {body}");
        assert!(body.contains("deleted from disk"), "got: {body}");
        assert!(body.contains("cannot be undone"));
    }

    /// The safe operations must not be gated behind a dialog — a fetch that needs
    /// confirming is a fetch nobody uses.
    #[test]
    fn safe_operations_run_without_asking() {
        for (action, expected) in [
            (Action::Fetch, Job::Fetch),
            (Action::Pull, Job::Pull),
            (Action::Refresh, Job::Refresh),
            (
                Action::CheckoutBranch("main".into()),
                Job::Checkout("main".into()),
            ),
            (Action::Stage(vec!["a".into()]), Job::Stage(vec!["a".into()])),
            (
                Action::Unstage(vec!["a".into()]),
                Job::Unstage(vec!["a".into()]),
            ),
        ] {
            let mut state = state_with(vec![]);
            route(&action, &mut state);
            assert!(state.confirm.is_none(), "{action:?} must not ask");
            assert_eq!(state.pending_for_test(), Some(expected));
        }
    }

    /// A push on a branch with no upstream has to create it, or git refuses and
    /// explains — which is a worse first push than simply doing it.
    #[test]
    fn a_first_push_sets_the_upstream() {
        let mut state = state_with(vec![]);
        route(&Action::Push, &mut state);
        assert_eq!(
            state.pending_for_test(),
            Some(Job::Push {
                set_upstream: true,
                branch: Some("main".into())
            })
        );
    }

    #[test]
    fn a_later_push_does_not_set_the_upstream_again() {
        let mut state = state_with(vec![]);
        state.snapshot.status.upstream = Some("origin/main".into());
        route(&Action::Push, &mut state);
        assert_eq!(
            state.pending_for_test(),
            Some(Job::Push {
                set_upstream: false,
                branch: Some("main".into())
            })
        );
    }

    /// Two buttons pressed in the same frame: the first queues a job, and the
    /// second must be refused rather than appear accepted and be dropped. Opening a
    /// diff is the case where that shows — it would sit on "Loading…" for a load
    /// that was never started.
    #[test]
    fn a_second_action_in_the_same_frame_is_refused() {
        let mut state = state_with(vec![]);
        route(&Action::Fetch, &mut state);
        assert_eq!(state.pending_for_test(), Some(Job::Fetch));
        route(
            &Action::ShowDiff {
                path: "a.txt".into(),
                staged: false,
                untracked: false,
            },
            &mut state,
        );
        assert_eq!(
            state.pending_for_test(),
            Some(Job::Fetch),
            "the queued job must not be replaced"
        );
        assert_eq!(
            state.diff_title, None,
            "and the viewer must not open for a load that never started"
        );
    }

    /// The remote buttons are dimmed when there is no remote; they must also be
    /// inert. A dimmed button that still works produces a confusing "push failed"
    /// for something that was never possible.
    #[test]
    fn remote_operations_do_nothing_when_there_is_no_remote() {
        for action in [Action::Fetch, Action::Pull, Action::Push] {
            let mut state = state_with(vec![]);
            state.snapshot.remotes.clear();
            route(&action, &mut state);
            assert_eq!(
                state.pending_for_test(),
                None,
                "{action:?} must not run without a remote"
            );
        }
        // And the local operations are unaffected by the same gate.
        let mut state = state_with(vec![]);
        state.snapshot.remotes.clear();
        route(&Action::Refresh, &mut state);
        assert_eq!(state.pending_for_test(), Some(Job::Refresh));
    }

    /// Nothing may start while a job is in flight — git would fail on `index.lock`
    /// and the failure would look like it belonged to whatever was clicked.
    #[test]
    fn no_action_starts_while_a_job_is_running() {
        let mut state = state_with(vec![]);
        state.runner.start(std::env::temp_dir(), Job::Fetch);
        route(&Action::Pull, &mut state);
        assert_eq!(state.pending_for_test(), None);
        route(&Action::ResetHardPrompt("abc".into()), &mut state);
        assert!(state.confirm.is_none(), "not even a prompt");
    }

    /// Switching views and dismissing things must keep working while a job runs —
    /// they touch no repository state, and freezing the UI during a push would be
    /// worse than useless.
    #[test]
    fn navigation_still_works_while_a_job_is_running() {
        let mut state = state_with(vec![]);
        state.error = Some("boom".into());
        state.runner.start(std::env::temp_dir(), Job::Fetch);
        route(&Action::SetView(View::History), &mut state);
        assert_eq!(state.view, View::History);
        route(&Action::ClearError, &mut state);
        assert!(state.error.is_none());
    }

    /// Committing with no message must not produce an empty commit — and must say
    /// why nothing happened.
    #[test]
    fn committing_without_a_message_reports_instead_of_committing() {
        let mut state = state_with(vec![entry("a.txt", Some(Change::Modified), None)]);
        // `route` has no text inputs, which is the same as an empty box.
        route(&Action::Commit, &mut state);
        assert_eq!(state.pending_for_test(), None);
        assert!(state.error.as_deref().is_some_and(|e| e.contains("message")));
    }

    /// Nothing staged means nothing to commit, and this must be checked before the
    /// message is consumed — otherwise the user's typing is thrown away.
    #[test]
    fn committing_with_nothing_staged_does_nothing() {
        let mut state = state_with(vec![entry("a.txt", None, Some(Change::Modified))]);
        route(&Action::Commit, &mut state);
        assert_eq!(state.pending_for_test(), None);
        assert!(state.error.is_none(), "an empty index is not an error to report");
    }

    /// The happy path: a staged file plus a message makes a commit, and the amend
    /// flag is consumed so the *next* commit is a normal one.
    #[test]
    fn committing_with_a_message_and_a_staged_file_commits() {
        let mut state = state_with(vec![entry("a.txt", Some(Change::Modified), None)]);
        route_typed(&Action::Commit, &mut state, Some("feat: a thing"));
        assert_eq!(
            state.pending_for_test(),
            Some(Job::Commit {
                message: "feat: a thing".into(),
                amend: false
            })
        );
    }

    /// Amending needs no staged file — rewording the last commit is a normal thing
    /// to want — and the flag must not persist into the next commit.
    #[test]
    fn amending_needs_no_staged_file_and_resets_afterwards() {
        let mut state = state_with(vec![]);
        state.amend = true;
        route_typed(&Action::Commit, &mut state, Some("reworded"));
        assert_eq!(
            state.pending_for_test(),
            Some(Job::Commit {
                message: "reworded".into(),
                amend: true
            })
        );
        assert!(!state.amend, "amend must not carry over to the next commit");
    }

    #[test]
    fn creating_a_branch_uses_the_typed_name() {
        let mut state = state_with(vec![]);
        route_typed(&Action::CreateBranchFromInput, &mut state, Some("feature/x"));
        assert_eq!(
            state.pending_for_test(),
            Some(Job::CreateBranch {
                name: "feature/x".into(),
                start: None
            })
        );
    }

    /// An empty name must not create a branch, and must say why nothing happened
    /// rather than looking like a dead button.
    #[test]
    fn creating_a_branch_with_no_name_reports_instead() {
        let mut state = state_with(vec![]);
        route(&Action::CreateBranchFromInput, &mut state);
        assert_eq!(state.pending_for_test(), None);
        assert!(state.error.is_some());
    }

    /// "Branch from here" names the branch after the commit, so two of them do not
    /// collide and the name says where it came from.
    #[test]
    fn branching_from_a_commit_names_the_branch_after_it() {
        let mut state = state_with(vec![]);
        route(&Action::BranchFromCommit("abcdef1234567890".into()), &mut state);
        assert_eq!(
            state.pending_for_test(),
            Some(Job::CreateBranch {
                name: "from-abcdef12".into(),
                start: Some("abcdef1234567890".into())
            })
        );
    }

    /// Opening a diff must clear the previous one, or the viewer shows the old
    /// file's contents under the new file's title until the worker answers.
    #[test]
    fn opening_a_diff_clears_the_previous_text() {
        let mut state = state_with(vec![]);
        state.diff_text = Some("old diff".into());
        route(
            &Action::ShowDiff {
                path: "a.txt".into(),
                staged: false,
                untracked: false,
            },
            &mut state,
        );
        assert_eq!(state.diff_title.as_deref(), Some("a.txt"));
        assert_eq!(state.diff_text, None, "stale text must not be shown");
        assert_eq!(
            state.pending_for_test(),
            Some(Job::Diff {
                path: "a.txt".into(),
                staged: false,
                untracked: false
            })
        );
    }

    /// The confirmation is answered by `overlay_click` alone. Both click systems
    /// see every press, so if the panel's handler answered it too, one click would
    /// answer it twice — and it has to stay answerable while the panel's tab is
    /// hidden, which only the ungated system can manage.
    #[test]
    fn the_panel_handler_does_not_answer_confirmations() {
        let mut state = state_with(vec![]);
        state.ask(Confirm {
            title: "t".into(),
            body: "b".into(),
            action_label: "go".into(),
            danger: true,
            job: Job::Fetch,
        });
        route(&Action::ConfirmAccept, &mut state);
        assert!(
            state.confirm.is_some(),
            "the panel handler must leave the confirmation for `overlay_click`"
        );
        assert_eq!(state.pending_for_test(), None);
    }

    #[test]
    fn toggling_amend_flips_it_and_redraws() {
        let mut state = state_with(vec![]);
        let revision = state.revision;
        route(&Action::ToggleAmend, &mut state);
        assert!(state.amend);
        assert_ne!(state.revision, revision);
        route(&Action::ToggleAmend, &mut state);
        assert!(!state.amend);
    }

    /// Clicking the expanded row again closes it, which is how a list of six
    /// actions gets out of the way.
    #[test]
    fn selecting_the_open_commit_collapses_it() {
        let mut state = state_with(vec![]);
        route(&Action::SelectCommit("abc".into()), &mut state);
        assert_eq!(state.selected_commit.as_deref(), Some("abc"));
        route(&Action::SelectCommit("abc".into()), &mut state);
        assert_eq!(state.selected_commit, None);
        route(&Action::SelectCommit("abc".into()), &mut state);
        route(&Action::SelectCommit("def".into()), &mut state);
        assert_eq!(state.selected_commit.as_deref(), Some("def"));
    }

    /// Changing view drops the expanded commit — it belongs to the list being left.
    #[test]
    fn changing_view_collapses_the_expanded_commit() {
        let mut state = state_with(vec![]);
        state.selected_commit = Some("abc".into());
        route(&Action::SetView(View::Branches), &mut state);
        assert_eq!(state.selected_commit, None);
    }

    /// Init is the one operation allowed without a repository — it is what creates
    /// one, so gating it on `ready()` would make the button do nothing.
    #[test]
    fn init_runs_without_a_repository_and_other_operations_do_not() {
        let mut state = state_without_repo();
        route(&Action::Fetch, &mut state);
        assert_eq!(state.pending_for_test(), None);
        route(&Action::Init, &mut state);
        assert_eq!(state.pending_for_test(), Some(Job::Init));
    }

    // ── diff rendering ──────────────────────────────────────────────────────

    /// `+++`/`---` are file headers, not added and removed lines. Matching them as
    /// changes opens every diff with a green line and a red line that are neither.
    #[test]
    fn diff_file_headers_are_not_coloured_as_changes() {
        assert_eq!(diff_line_color("+++ b/a.txt"), placeholder());
        assert_eq!(diff_line_color("--- a/a.txt"), placeholder());
        assert_eq!(diff_line_color("diff --git a/a.txt b/a.txt"), placeholder());
        assert_eq!(diff_line_color("index e69de29..0000000"), placeholder());
    }

    #[test]
    fn added_and_removed_lines_are_coloured_and_hunks_are_marked() {
        assert_eq!(diff_line_color("+added"), GREEN);
        assert_eq!(diff_line_color("-removed"), RED);
        assert_eq!(diff_line_color("@@ -1,2 +1,3 @@"), BLUE);
        assert_eq!(diff_line_color(" unchanged"), text_muted());
        assert_eq!(diff_line_color(""), text_muted());
    }

    // ── row hashing ─────────────────────────────────────────────────────────

    /// The list reuses rows whose hash is unchanged, so a hash that misses a field
    /// leaves an updated row rendering its old contents.
    #[test]
    fn the_item_hash_tracks_the_fields_a_row_renders() {
        let base = file_item(&entry("a.txt", None, Some(Change::Modified)), false);
        let key = hash_item(&base);
        assert_eq!(key, hash_item(&file_item(&entry("a.txt", None, Some(Change::Modified)), false)));
        // A different path, kind, or column all change what is drawn.
        assert_ne!(key, hash_item(&file_item(&entry("b.txt", None, Some(Change::Modified)), false)));
        assert_ne!(key, hash_item(&file_item(&entry("a.txt", None, Some(Change::Deleted)), false)));
        assert_ne!(key, hash_item(&file_item(&entry("a.txt", Some(Change::Modified), None), true)));
    }

    /// Expanding a commit changes its row's appearance, so the hash has to move or
    /// the highlight never appears.
    #[test]
    fn expanding_a_commit_changes_its_row_hash() {
        let c = commit("abcdef1234", "s", 1);
        assert_ne!(
            hash_item(&commit_item(&c, 0, false)),
            hash_item(&commit_item(&c, 0, true))
        );
    }

    /// The relative timestamp is rendered, so a row an hour later must not be
    /// reused with a stale "2 minutes ago".
    #[test]
    fn a_commits_row_hash_follows_its_relative_time() {
        let c = commit("abcdef1234", "s", 1);
        assert_ne!(
            hash_item(&commit_item(&c, 300, false)),
            hash_item(&commit_item(&c, 100_000, false))
        );
    }

    #[test]
    fn different_item_kinds_with_the_same_text_do_not_collide() {
        let header = hash_item(&Item::Header {
            label: "Staged".into(),
            action: None,
        });
        let hint = hash_item(&Item::Hint("Staged".into()));
        assert_ne!(header, hint);
    }

    /// A header's trailing button is part of what it renders — "Stage all" and
    /// "Unstage all" on the same label must not be reused for each other.
    #[test]
    fn a_headers_button_is_part_of_its_hash() {
        let with = |label: &str, action: Action| {
            hash_item(&Item::Header {
                label: "Changes (1)".into(),
                action: Some((label.into(), action, false)),
            })
        };
        assert_ne!(
            with("Stage all", Action::Stage(vec!["a".into()])),
            with("Unstage all", Action::Unstage(vec!["a".into()]))
        );
        // Same label, different paths: clicking it would act on the wrong files.
        assert_ne!(
            with("Stage all", Action::Stage(vec!["a".into()])),
            with("Stage all", Action::Stage(vec!["b".into()]))
        );
    }
}
