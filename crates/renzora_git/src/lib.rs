//! Version control for the open project — a git client in the editor.
//!
//! Stage and commit changes, read the history, go back to an earlier commit,
//! branch and merge, and fetch/pull/push to a remote. Everything runs against the
//! `git` binary on the user's PATH (see [`cli`] for why that, and not `libgit2`),
//! on a worker thread (see [`job`] for why never inline).
//!
//! # Scope
//!
//! `Editor`. A shipped game has no version control to do, and the whole crate
//! compiles to nothing on wasm — a browser tab cannot spawn a process. The plugin
//! type still exists there, because the generated plugin list names it
//! unconditionally; it just installs nothing.
//!
//! # The three things this feature has to get right
//!
//! Parsing and process-spawning are the bulk of the code but not the risk. These
//! are:
//!
//! 1. **The open scene must follow the working tree.** Git rewriting
//!    `scenes/level.bsn` while the editor holds the old version live means the next
//!    save destroys what was just checked out. [`reload`] is the whole answer, and
//!    it is why every job declares [`job::Job::changes_worktree`].
//! 2. **Destructive operations must be confirmed, and must say what they destroy.**
//!    Discarding a file and resetting hard are unrecoverable *by git* — the content
//!    was never committed, so no reflog brings it back. Those go through
//!    [`Confirm`] with the file count and consequence spelled out. Operations git
//!    can undo (a revert, a soft reset) do not need the same ceremony and don't get
//!    it.
//! 3. **Nothing that happens repeatedly may block the frame.** Every operation
//!    and every repository read goes to a worker thread — see [`job`]. The two
//!    exceptions are the one-shot probes ([`probe_git`], [`probe_repo`]): each
//!    spawns a short `git` once, at startup and when a project opens, where the
//!    editor is already loading. Putting those on a thread would buy a frame that
//!    nobody is looking at, at the cost of a second polling path.
//!
//! # Reading state
//!
//! One [`GitState`] holds a whole-repository [`job::Snapshot`] read in a single
//! pass, so the branch, the file list and the history can never disagree with each
//! other. `revision` ticks on every change and is what the panel's reactive lists
//! use as a cheap change token, instead of re-deriving their contents every frame.

#[cfg(not(target_arch = "wasm32"))]
mod cli;
#[cfg(not(target_arch = "wasm32"))]
pub mod job;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub mod parse;
#[cfg(not(target_arch = "wasm32"))]
mod reload;

use bevy::prelude::*;

/// The dock panel's id, shared by the registration and the content builder.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const PANEL_ID: &str = "git";

/// Which of the panel's three lists is showing.
///
/// Lives here rather than in [`native`] because [`GitState`] is what the panel's
/// reactive snapshot reads, and splitting the view across two resources would mean
/// the list could rebuild for one without seeing a change in the other.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum View {
    /// Working-tree changes, staging, and the commit box.
    #[default]
    Changes,
    /// The commit log, and the ways back to an earlier one.
    History,
    /// Local and remote branches: switch, create, merge, delete.
    Branches,
}

/// A pending confirmation for an operation that can lose work.
///
/// Held in [`GitState`] rather than being a modal the UI owns, so the decision of
/// *what needs confirming* lives beside the operations instead of being spread
/// across click handlers where one could quietly forget.
#[cfg(not(target_arch = "wasm32"))]
pub struct Confirm {
    pub title: String,
    /// What will happen, in full. Never elided: this is the only place the user
    /// finds out whether the thing they are about to lose is recoverable.
    pub body: String,
    pub action_label: String,
    /// Draws the action in red. Reserved for operations git cannot undo — using it
    /// for merely-significant ones would train the user to click through it.
    pub danger: bool,
    pub job: job::Job,
}

/// Everything the panel reads and the workers write.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
pub struct GitState {
    /// `None` until probed; `Err` when `git` could not be run at all, which the
    /// panel shows as its own state rather than as a failed operation.
    pub git_version: Option<Result<String, String>>,
    /// The enclosing repository's root. `None` means the project is not in a repo
    /// yet — the only state where the Initialize button appears.
    pub root: Option<std::path::PathBuf>,
    /// Whether a repo lookup has happened for the current project. Cleared when
    /// the project changes so the next frame re-probes.
    probed_project: Option<std::path::PathBuf>,
    /// The last complete read of the repository.
    pub snapshot: job::Snapshot,
    /// Which list the panel is showing.
    pub view: View,
    /// The last operation's failure, shown as a dismissible banner. Kept in state
    /// (as well as toasted) because a push rejection is several lines long and
    /// worth re-reading.
    pub error: Option<String>,
    pub runner: job::Runner,
    /// Ticks on every observable change. The panel's reactive lists compare this
    /// instead of rebuilding their snapshots every frame.
    pub revision: u64,
    /// Queued request from the UI. The UI cannot start a job itself: dispatch
    /// needs `&World` to fingerprint the open scene first (see [`reload`]), so it
    /// happens in one exclusive system rather than at every click site.
    pending: Option<job::Job>,
    /// The open scene as it was before the in-flight job started.
    watch: Option<reload::SceneWatch>,
    /// Diff viewer contents, and what it is a diff of.
    pub diff_title: Option<String>,
    pub diff_text: Option<String>,
    /// Commit box: replace the last commit instead of adding one.
    pub amend: bool,
    /// The history row expanded into a detail view.
    pub selected_commit: Option<String>,
    /// A confirmation the user has not answered yet.
    pub confirm: Option<Confirm>,
    /// When the periodic refresh last ran.
    last_refresh: Option<std::time::Instant>,
}

#[cfg(not(target_arch = "wasm32"))]
impl GitState {
    /// Is git usable at all?
    pub fn git_available(&self) -> bool {
        matches!(self.git_version, Some(Ok(_)))
    }

    /// Why git could not be used, if that's the situation.
    pub fn git_error(&self) -> Option<&str> {
        match &self.git_version {
            Some(Err(e)) => Some(e.as_str()),
            _ => None,
        }
    }

    /// The project is in a repository and we can talk to it.
    pub fn ready(&self) -> bool {
        self.git_available() && self.root.is_some()
    }

    /// Note that something the panel draws has changed.
    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Can a new operation start right now?
    ///
    /// Both halves matter. `busy` is the running job; `pending` is one already
    /// queued this frame but not yet dispatched — two buttons pressed in the same
    /// frame is rare but reachable, and a caller that checked only `busy` would
    /// think its request was accepted when [`Self::request`] had dropped it.
    pub fn can_start(&self) -> bool {
        !self.runner.busy() && self.pending.is_none()
    }

    /// Ask for `job` to run. Ignored unless [`Self::can_start`] — the button that
    /// leads here is disabled then, so this is the backstop, not the gate.
    ///
    /// Requests are not queued on purpose: a queued push behind a failed pull
    /// would run against a state the user never saw and did not agree to.
    pub fn request(&mut self, job: job::Job) {
        if !self.can_start() {
            return;
        }
        self.pending = Some(job);
        self.touch();
    }

    /// Ask for confirmation before running `job`.
    pub fn ask(&mut self, confirm: Confirm) {
        self.confirm = Some(confirm);
        self.touch();
    }

    /// Run what the confirmation was guarding.
    pub fn confirm_accept(&mut self) {
        if let Some(confirm) = self.confirm.take() {
            self.request(confirm.job);
        }
        self.touch();
    }

    pub fn confirm_cancel(&mut self) {
        self.confirm = None;
        self.touch();
    }

    /// The label for the busy indicator, or `None` when idle.
    pub fn progress(&self) -> Option<&'static str> {
        self.runner.current().map(job::Job::progress_label)
    }

    pub fn clear_error(&mut self) {
        self.error = None;
        self.touch();
    }

    /// Switch list. Clears the expanded commit, which belongs to the view being
    /// left and would otherwise reappear expanded on the way back.
    pub fn set_view(&mut self, view: View) {
        if self.view == view {
            return;
        }
        self.view = view;
        self.selected_commit = None;
        self.touch();
    }

    /// Expand a history row, or collapse it if it is already the expanded one.
    pub fn toggle_commit(&mut self, oid: &str) {
        self.selected_commit = match self.selected_commit.as_deref() {
            Some(current) if current == oid => None,
            _ => Some(oid.to_string()),
        };
        self.touch();
    }

    /// Whether enough time has passed to re-read the repository.
    ///
    /// Only consulted while the panel is visible, which is what keeps a
    /// backgrounded editor from spawning a `git status` every few seconds forever.
    pub fn due_for_refresh(&self, interval: std::time::Duration) -> bool {
        if !self.can_start() {
            return false;
        }
        match self.last_refresh {
            Some(at) => at.elapsed() >= interval,
            None => true,
        }
    }

    pub fn close_diff(&mut self) {
        self.diff_title = None;
        self.diff_text = None;
        self.touch();
    }
}

/// Version control for the open project.
#[derive(Default)]
pub struct GitPlugin;

impl Plugin for GitPlugin {
    fn build(&self, _app: &mut App) {
        info!("[editor] GitPlugin");
        #[cfg(not(target_arch = "wasm32"))]
        {
            use renzora::RenzoraShellExt;
            _app.init_resource::<GitState>()
                .register_shell_panel(
                    PANEL_ID,
                    renzora::lang::t_or("git.panel.title", "Git"),
                    "git-branch",
                    // Its own category rather than "Tools": version control is not
                    // a tool you reach for inside a task, it is a thing the whole
                    // project has, and the Add-Panel picker groups by this.
                    renzora::lang::t_or("git.panel.category", "Version Control"),
                )
                .add_systems(Startup, probe_git)
                // Deliberately NOT panel-gated. A push has to finish whether or not
                // the user is still looking at the panel, and the repo lookup has
                // to notice a project opening before the panel is ever shown.
                .add_systems(
                    Update,
                    (probe_repo, dispatch_pending, poll_jobs)
                        .chain()
                        .run_if(renzora::core::not_in_play_mode),
                );
            native::register(_app);
        }
    }
}

/// Find out once whether `git` can be run.
///
/// A missing git is a first-class state, not a failure: the panel explains what to
/// install instead of surfacing "program not found" from whatever the user clicked.
#[cfg(not(target_arch = "wasm32"))]
fn probe_git(mut state: ResMut<GitState>) {
    state.git_version = Some(cli::version());
    if let Some(Err(e)) = &state.git_version {
        warn!("[git] git is not available: {e}");
    }
    state.touch();
}

/// Locate the repository containing the open project, re-probing when the project
/// changes (opening a second project must not inherit the first one's repo).
#[cfg(not(target_arch = "wasm32"))]
fn probe_repo(mut state: ResMut<GitState>, project: Option<Res<renzora::core::CurrentProject>>) {
    let Some(project) = project else {
        return;
    };
    if state.probed_project.as_deref() == Some(project.path.as_path()) {
        return;
    }
    if !state.git_available() {
        // Still record the project, so this doesn't re-run every frame; the probe
        // is re-armed if git itself is re-probed.
        state.probed_project = Some(project.path.clone());
        return;
    }

    let root = cli::discover_root(&project.path);
    state.probed_project = Some(project.path.clone());
    state.root = root.clone();
    state.snapshot = job::Snapshot::default();
    state.error = None;
    state.last_refresh = None;
    state.touch();

    match root {
        Some(root) => {
            info!("[git] repository at {}", root.display());
            state.request(job::Job::Refresh);
        }
        None => info!(
            "[git] {} is not inside a git repository",
            project.path.display()
        ),
    }
}

/// Start the queued job, fingerprinting the open scene first when the job could
/// rewrite it.
///
/// Exclusive because the fingerprint needs to read the world's scene state, and
/// this is the one place a job can start — so there is no route to a working-tree
/// change that skips the watch.
#[cfg(not(target_arch = "wasm32"))]
fn dispatch_pending(world: &mut World) {
    let Some(pending) = world
        .get_resource_mut::<GitState>()
        .and_then(|mut s| s.pending.take())
    else {
        return;
    };

    // `Init` runs in the project directory: there is no repo root yet, and
    // creating one is the point. Every other job runs at the repo root.
    let dir = if pending == job::Job::Init {
        world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.path.clone())
    } else {
        world.resource::<GitState>().root.clone()
    };
    let Some(dir) = dir else {
        // Nothing to run against. Dropping the request is right: the buttons that
        // produce one are not reachable in this state.
        return;
    };

    let watch = pending
        .changes_worktree()
        .then(|| reload::watch_open_scene(world))
        .flatten();

    let mut state = world.resource_mut::<GitState>();
    state.watch = watch;
    if state.runner.start(dir, pending) {
        state.error = None;
    }
    state.touch();
}

/// Collect a finished job: adopt its snapshot, report it, and reconcile the scene.
#[cfg(not(target_arch = "wasm32"))]
fn poll_jobs(world: &mut World) {
    let Some(polled) = world
        .get_resource_mut::<GitState>()
        .and_then(|mut s| s.runner.poll())
    else {
        return;
    };

    let report = match polled {
        Ok(report) => report,
        // The worker died without answering. Surfaced rather than swallowed: the
        // slot has already cleared, so the panel would otherwise just look idle
        // and the operation would appear never to have been asked for.
        Err(e) => {
            let mut state = world.resource_mut::<GitState>();
            state.error = Some(e.clone());
            state.watch = None;
            state.touch();
            toast(world, Level::Error, e);
            return;
        }
    };

    // Snapshot first, so anything reading state during the reload below sees the
    // post-operation repository rather than the pre-operation one.
    {
        let mut state = world.resource_mut::<GitState>();
        if let Some(snapshot) = report.snapshot.clone() {
            state.snapshot = snapshot;
            state.last_refresh = Some(std::time::Instant::now());
        }
        if let Some(text) = report.text.clone() {
            state.diff_text = Some(text);
        }
        state.touch();
    }

    let watch = world.resource_mut::<GitState>().watch.take();

    match &report.outcome {
        Ok(_) => {
            // `Init` ran in the project directory because there was no repo root to
            // run in. Now there is one, so re-arm the lookup — until it runs,
            // `root` is still `None` and the panel would keep offering to
            // initialise a repository that already exists.
            if report.job == job::Job::Init {
                world.resource_mut::<GitState>().probed_project = None;
            }
            if let Some(message) = report.job.success_message(report.snapshot.as_ref()) {
                toast(world, Level::Success, message);
            }
        }
        Err(e) => {
            let message = explain(&report.job, e, report.snapshot.as_ref());
            let mut state = world.resource_mut::<GitState>();
            state.error = Some(message.clone());
            state.touch();
            toast(world, Level::Error, first_line(&message));
        }
    }

    // After reporting, and regardless of the outcome: a conflicting merge fails
    // *and* rewrites files.
    if let Some(watch) = watch {
        match reload::reconcile(world, &watch) {
            reload::Reconciled::Unchanged => {}
            reload::Reconciled::Reloaded(path) => {
                let name = file_name(&path);
                info!("[git] reloaded {name} after {:?}", report.job);
                toast(world, Level::Info, format!("Reloaded {name} from disk"));
            }
            reload::Reconciled::Vanished(path) => {
                let name = file_name(&path);
                // Left loaded on purpose — see `Reconciled::Vanished`.
                toast(
                    world,
                    Level::Warning,
                    format!(
                        "{name} does not exist at this revision. The editor still has it open — \
                         save it somewhere else if you want to keep it."
                    ),
                );
            }
        }
    }
}

/// Add context git cannot know it should have added.
///
/// Git's own message is always kept — it is more precise than any paraphrase and
/// matches what a terminal would say. This only *prepends* the editor-side reason
/// when the failure has one, for the cases where git's advice is right but points
/// somewhere the user cannot go from here.
#[cfg(not(target_arch = "wasm32"))]
fn explain(job: &job::Job, error: &str, snapshot: Option<&job::Snapshot>) -> String {
    let hint = match job {
        // The one failure that is expected rather than exceptional: `pull` is
        // `--ff-only`, so diverged branches stop here by design.
        job::Job::Pull if error.contains("Not possible to fast-forward") || error.contains("diverging") => Some(
            "Your branch and the remote have both moved on, so this cannot be a simple \
             fast-forward. Fetch, then merge the remote branch to combine them."
                .to_string(),
        ),
        job::Job::Push { .. } if error.contains("rejected") => Some(
            "The remote has commits you do not. Pull (or fetch and merge) first, then push."
                .to_string(),
        ),
        job::Job::Push { .. } if snapshot.is_some_and(|s| !s.has_remote()) => Some(
            "This repository has no remote configured, so there is nowhere to push to. \
             Add one with `git remote add origin <url>`."
                .to_string(),
        ),
        job::Job::Merge(_) if snapshot.is_some_and(|s| s.status.has_conflicts()) => Some(
            "The merge stopped with conflicts. Resolve the conflicted files, stage them, \
             and commit — or abort the merge to go back."
                .to_string(),
        ),
        // Git names the files it would overwrite, which is the useful part; what it
        // cannot say is that the editor is holding one of them.
        job::Job::Checkout(_) if error.contains("would be overwritten") => Some(
            "Commit, stash or discard your local changes to these files first."
                .to_string(),
        ),
        _ => None,
    };
    match hint {
        Some(hint) => format!("{hint}\n\nGit said:\n{error}"),
        None => error.to_string(),
    }
}

/// The first line of a message, for a toast (which has no room for a paragraph —
/// the banner in the panel keeps the whole thing).
#[cfg(not(target_arch = "wasm32"))]
fn first_line(message: &str) -> String {
    message.lines().next().unwrap_or(message).to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(not(target_arch = "wasm32"))]
enum Level {
    Info,
    Success,
    Warning,
    Error,
}

/// Report to the editor's toast queue, and to the console so it is still findable
/// after the toast fades.
#[cfg(not(target_arch = "wasm32"))]
fn toast(world: &mut World, level: Level, message: impl Into<String>) {
    let message = message.into();
    match level {
        Level::Error => renzora::core::console_log::console_error("Git", message.clone()),
        Level::Warning => renzora::core::console_log::console_warn("Git", message.clone()),
        Level::Success => renzora::core::console_log::console_success("Git", message.clone()),
        Level::Info => renzora::core::console_log::console_info("Git", message.clone()),
    }
    let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() else {
        return;
    };
    match level {
        Level::Info => toasts.info(message),
        Level::Success => toasts.success(message),
        Level::Warning => toasts.warning(message),
        Level::Error => toasts.error(message),
    }
}

renzora::add!(GitPlugin, Editor);

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::job::Job;

    #[test]
    fn a_request_is_ignored_while_a_job_is_in_flight() {
        let mut state = GitState::default();
        state.request(Job::Refresh);
        assert_eq!(state.pending, Some(Job::Refresh));
        // A second request must not replace the first, or a click during an
        // operation silently changes what runs.
        state.request(Job::Fetch);
        assert_eq!(state.pending, Some(Job::Refresh));
    }

    #[test]
    fn every_request_bumps_the_revision_so_the_panel_redraws() {
        let mut state = GitState::default();
        let before = state.revision;
        state.request(Job::Refresh);
        assert_ne!(state.revision, before);
    }

    /// Cancelling must not run the job it was guarding — this is the whole point
    /// of the confirmation.
    #[test]
    fn cancelling_a_confirmation_runs_nothing() {
        let mut state = GitState::default();
        state.ask(Confirm {
            title: "Discard".into(),
            body: "b".into(),
            action_label: "Discard".into(),
            danger: true,
            job: Job::Discard {
                tracked: vec!["a".into()],
                untracked: vec![],
            },
        });
        assert!(state.confirm.is_some());
        state.confirm_cancel();
        assert!(state.confirm.is_none());
        assert_eq!(state.pending, None, "cancel must not queue the job");
    }

    #[test]
    fn accepting_a_confirmation_queues_exactly_the_guarded_job() {
        let mut state = GitState::default();
        let job = Job::Reset {
            rev: "abc".into(),
            hard: true,
        };
        state.ask(Confirm {
            title: "Reset".into(),
            body: "b".into(),
            action_label: "Reset".into(),
            danger: true,
            job: job.clone(),
        });
        state.confirm_accept();
        assert!(state.confirm.is_none());
        assert_eq!(state.pending, Some(job));
    }

    /// Until git has been probed the panel must not offer operations, and a
    /// project outside a repo must not look ready either.
    #[test]
    fn readiness_needs_both_git_and_a_repository() {
        let mut state = GitState::default();
        assert!(!state.git_available() && !state.ready());

        state.git_version = Some(Err("not found".into()));
        assert!(!state.ready());
        assert_eq!(state.git_error(), Some("not found"));

        state.git_version = Some(Ok("git version 2.54.0".into()));
        assert!(state.git_available());
        assert!(!state.ready(), "git alone is not enough — there must be a repo");
        assert_eq!(state.git_error(), None);

        state.root = Some(std::path::PathBuf::from("/repo"));
        assert!(state.ready());
    }

    /// A toast has one line of room; the banner keeps the rest.
    #[test]
    fn a_multi_line_error_is_reduced_to_its_first_line_for_a_toast() {
        assert_eq!(first_line("first\nsecond\nthird"), "first");
        assert_eq!(first_line("only"), "only");
        assert_eq!(first_line(""), "");
    }

    // ── failure explanations ─────────────────────────────────────────────────

    /// Git's own wording is the precise part and must survive; the hint is what
    /// this crate adds on top.
    #[test]
    fn an_explained_failure_keeps_gits_original_message() {
        let git = "fatal: Not possible to fast-forward, aborting.";
        let out = explain(&Job::Pull, git, None);
        assert!(out.contains(git), "git's message must be kept verbatim");
        assert!(out.contains("fast-forward"));
        assert!(out.contains("merge"), "and point at the way forward");
    }

    /// A `--ff-only` pull failing on a diverged branch is the expected outcome of
    /// a deliberate choice, so it gets an explanation rather than a raw fatal.
    #[test]
    fn a_diverged_pull_is_explained_rather_than_passed_through_raw() {
        let out = explain(&Job::Pull, "hint: diverging branches", None);
        assert!(out.starts_with("Your branch and the remote"));
    }

    #[test]
    fn a_rejected_push_says_to_pull_first() {
        let out = explain(
            &Job::Push {
                set_upstream: false,
                branch: None,
            },
            "! [rejected] main -> main (fetch first)",
            None,
        );
        assert!(out.contains("Pull"));
    }

    /// An unrecognised failure is passed through untouched — inventing an
    /// explanation for something unknown is worse than showing git's own words.
    #[test]
    fn an_unrecognised_failure_is_passed_through_unchanged() {
        let git = "error: something nobody anticipated";
        assert_eq!(explain(&Job::Fetch, git, None), git);
        assert_eq!(explain(&Job::Stage(vec![]), git, None), git);
    }

    /// A merge that conflicts is not a failure to hide: git exits non-zero but the
    /// tree really did change, and the user needs to know what to do next.
    #[test]
    fn a_conflicting_merge_is_explained_as_something_to_resolve() {
        let snapshot = job::Snapshot {
            status: parse::RepoStatus {
                entries: vec![parse::StatusEntry {
                    path: "c.txt".into(),
                    orig_path: None,
                    index: None,
                    worktree: Some(parse::Change::Conflicted),
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        let out = explain(
            &Job::Merge("side".into()),
            "Automatic merge failed; fix conflicts",
            Some(&snapshot),
        );
        assert!(out.contains("conflicts"));
        assert!(out.contains("abort"), "the way back has to be named");
    }
}
