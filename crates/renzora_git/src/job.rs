//! One git operation, run off the main thread.
//!
//! Every operation here goes to a worker thread, including the reads. That is not
//! caution about `git status` being slow in the abstract — it is that this is a
//! renderer. A frame is 16 ms and a cold `git status` on a project with a few
//! thousand assets takes longer than that on its own, so doing it inline is a
//! visible stutter every time the panel refreshes. The network operations are not
//! even a question: a push is seconds at best.
//!
//! # One at a time
//!
//! [`Runner`] holds at most one job. This is not a simplification that gives
//! something up — git serialises anyway, on `index.lock`, and two operations
//! racing for it means one of them fails with a lock error that has nothing to do
//! with what the user did. Serialising here instead turns that into a disabled
//! button and an honest "Pushing…".
//!
//! # Why a mutation refreshes in the same hop
//!
//! A job that changes anything runs [`snapshot`] itself before reporting, rather
//! than the caller queueing a separate refresh. Two reasons, and the second is the
//! important one:
//!
//! 1. It removes a frame (often several) where the panel shows pre-operation
//!    state — long enough to read a stale "1 file staged" after committing it.
//! 2. **It happens even when the operation fails**, which is required for merge.
//!    A conflicting `git merge` exits non-zero *and* leaves the working tree
//!    conflicted, so treating a failure as "nothing changed" would leave the panel
//!    claiming a clean tree while the files on disk have conflict markers in them.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};

use crate::cli;
use crate::parse::{BranchRef, Commit, RepoStatus};

/// How much history to read. Deep enough that scrolling back through a real
/// project's work doesn't hit the end, shallow enough that the read stays cheap
/// on a repo with a decade of commits in it.
pub const LOG_LIMIT: usize = 200;

/// Everything the panel draws, read in one pass so the parts cannot disagree.
///
/// Read together deliberately: a status from before an operation beside a log from
/// after it renders a branch whose commit count contradicts its own history.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub status: RepoStatus,
    pub log: Vec<Commit>,
    pub refs: Vec<BranchRef>,
    pub remotes: Vec<String>,
}

impl Snapshot {
    /// Local branches, in the order git listed them (alphabetical).
    pub fn local_branches(&self) -> impl Iterator<Item = &BranchRef> {
        self.refs.iter().filter(|r| !r.remote)
    }

    pub fn remote_branches(&self) -> impl Iterator<Item = &BranchRef> {
        self.refs.iter().filter(|r| r.remote)
    }

    /// Whether a push needs to create the upstream ref.
    ///
    /// Only true when there is exactly one remote: `--set-upstream origin` names
    /// `origin` outright, and with two remotes configured there is no honest
    /// default to pick for the user.
    pub fn push_needs_upstream(&self) -> bool {
        self.status.upstream.is_none() && self.remotes.len() == 1
    }

    /// Is there a remote to talk to at all? Distinguishes "push failed" from
    /// "this repo has no remote yet", which need different things from the user.
    pub fn has_remote(&self) -> bool {
        !self.remotes.is_empty()
    }
}

/// Read the whole repository state.
///
/// The log read is skipped in an unborn repo rather than allowed to fail: with no
/// HEAD there is nothing to walk, and `git log` says so with an error. An empty
/// history is the truthful answer, and reporting the error instead would put a
/// scary message in front of every user who just pressed "Initialize Repository".
pub fn snapshot(root: &Path) -> Result<Snapshot, String> {
    let status = cli::status(root)?;
    let log = if status.unborn {
        Vec::new()
    } else {
        cli::log(root, LOG_LIMIT)?
    };
    Ok(Snapshot {
        status,
        log,
        refs: cli::refs(root)?,
        remotes: cli::remotes(root)?,
    })
}

/// A requested operation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Job {
    /// Re-read everything.
    Refresh,
    /// `git init` in the project directory — the only job that runs when there is
    /// no repository yet, so it takes the directory rather than a repo root.
    Init,
    Stage(Vec<String>),
    Unstage(Vec<String>),
    /// Split by tracked-ness because the two need different commands and mean
    /// different things: restoring a tracked file, versus deleting an untracked
    /// one that git has no copy of.
    Discard {
        tracked: Vec<String>,
        untracked: Vec<String>,
    },
    Commit {
        message: String,
        amend: bool,
    },
    Fetch,
    Pull,
    Push {
        set_upstream: bool,
        branch: Option<String>,
    },
    Checkout(String),
    CreateBranch {
        name: String,
        start: Option<String>,
    },
    DeleteBranch {
        name: String,
        force: bool,
    },
    Merge(String),
    MergeAbort,
    Revert {
        rev: String,
        is_merge: bool,
    },
    Reset {
        rev: String,
        hard: bool,
    },
    /// One file's diff, for the viewer.
    Diff {
        path: String,
        staged: bool,
        untracked: bool,
    },
    /// One commit's diff.
    Show(String),
}

impl Job {
    /// Present-tense label for the busy indicator.
    pub fn progress_label(&self) -> &'static str {
        match self {
            Job::Refresh => "Reading repository…",
            Job::Init => "Initializing repository…",
            Job::Stage(_) => "Staging…",
            Job::Unstage(_) => "Unstaging…",
            Job::Discard { .. } => "Discarding changes…",
            Job::Commit { amend: false, .. } => "Committing…",
            Job::Commit { amend: true, .. } => "Amending…",
            Job::Fetch => "Fetching…",
            Job::Pull => "Pulling…",
            Job::Push { .. } => "Pushing…",
            Job::Checkout(_) => "Switching…",
            Job::CreateBranch { .. } => "Creating branch…",
            Job::DeleteBranch { .. } => "Deleting branch…",
            Job::Merge(_) => "Merging…",
            Job::MergeAbort => "Aborting merge…",
            Job::Revert { .. } => "Reverting…",
            Job::Reset { .. } => "Resetting…",
            Job::Diff { .. } | Job::Show(_) => "Loading diff…",
        }
    }

    /// Reads nothing but repository state — no snapshot needed afterwards, and no
    /// reason to disturb the open scene.
    pub fn is_read_only(&self) -> bool {
        matches!(self, Job::Refresh | Job::Diff { .. } | Job::Show(_))
    }

    /// Can this rewrite files in the working tree?
    ///
    /// Drives the open-scene reconciliation in [`crate::reload`]. Deliberately
    /// broad: `Commit` is included because a commit hook can reformat a file, and
    /// a false positive costs one file fingerprint while a false negative leaves
    /// the editor holding a scene that no longer matches the disk.
    pub fn changes_worktree(&self) -> bool {
        !matches!(
            self,
            Job::Refresh
                | Job::Diff { .. }
                | Job::Show(_)
                | Job::Init
                | Job::Stage(_)
                | Job::Unstage(_)
                | Job::Fetch
                | Job::Push { .. }
        )
    }

    /// Past-tense confirmation, for the toast on success. `None` for jobs whose
    /// result is the panel visibly updating — announcing "Refreshed" every few
    /// seconds would be noise.
    pub fn success_message(&self, snapshot: Option<&Snapshot>) -> Option<String> {
        let count = |n: usize, one: &str, many: &str| {
            if n == 1 {
                format!("1 {one}")
            } else {
                format!("{n} {many}")
            }
        };
        Some(match self {
            Job::Refresh | Job::Diff { .. } | Job::Show(_) => return None,
            Job::Init => "Initialized a git repository".to_string(),
            Job::Stage(p) => format!("Staged {}", count(p.len(), "file", "files")),
            Job::Unstage(p) => format!("Unstaged {}", count(p.len(), "file", "files")),
            Job::Discard { tracked, untracked } => format!(
                "Discarded {}",
                count(tracked.len() + untracked.len(), "change", "changes")
            ),
            Job::Commit { amend: false, .. } => {
                // Naming the new commit is the useful part: it is what the user
                // would look for to undo this.
                match snapshot.and_then(|s| s.log.first()) {
                    Some(c) => format!("Committed {}", c.short),
                    None => "Committed".to_string(),
                }
            }
            Job::Commit { amend: true, .. } => "Amended the last commit".to_string(),
            Job::Fetch => "Fetched from remote".to_string(),
            Job::Pull => "Pulled from remote".to_string(),
            Job::Push { .. } => "Pushed to remote".to_string(),
            Job::Checkout(rev) => format!("Switched to {rev}"),
            Job::CreateBranch { name, .. } => format!("Created and switched to {name}"),
            Job::DeleteBranch { name, .. } => format!("Deleted branch {name}"),
            Job::Merge(rev) => format!("Merged {rev}"),
            Job::MergeAbort => "Aborted the merge".to_string(),
            Job::Revert { rev, .. } => format!("Reverted {rev}"),
            Job::Reset { rev, hard } => {
                if *hard {
                    format!("Reset the working tree to {rev}")
                } else {
                    format!("Moved HEAD to {rev}, keeping your changes")
                }
            }
        })
    }
}

/// What a finished job reports back.
pub struct JobReport {
    /// Echoed so the poller knows which job this is the answer to without having
    /// to track what it sent.
    pub job: Job,
    /// Git's own output on success, or its own error message on failure. Passing
    /// git's wording through is deliberate: it is better than any paraphrase, and
    /// it matches what the user would see in a terminal for the same operation.
    pub outcome: Result<String, String>,
    /// Fresh state, for every job that is not read-only (and for `Refresh`).
    /// `None` only when the snapshot read itself failed.
    pub snapshot: Option<Snapshot>,
    /// Diff text, for [`Job::Diff`] / [`Job::Show`].
    pub text: Option<String>,
}

/// Execute `job` on a worker thread. The receiver yields exactly one report.
pub fn spawn(root: PathBuf, job: Job) -> Receiver<JobReport> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let report = execute(&root, job);
        // A closed channel means the editor is shutting down (or the panel's
        // state was dropped). Nothing to report to and nothing to clean up.
        let _ = tx.send(report);
    });
    rx
}

fn execute(root: &Path, job: Job) -> JobReport {
    let mut text = None;

    let outcome = match &job {
        Job::Refresh => Ok(String::new()),
        Job::Init => {
            // Before `git init`, so the very first status already excludes the
            // caches rather than listing several thousand generated files as
            // untracked.
            cli::bootstrap_gitignore(root);
            cli::init(root)
        }
        Job::Stage(paths) => cli::stage(root, paths),
        // The unborn case needs a different command entirely, and the flag is not
        // knowable from the UI thread's stale snapshot by the time this runs — so
        // it is read here, on the worker, right before it is used.
        Job::Unstage(paths) => {
            let unborn = cli::status(root).map(|s| s.unborn).unwrap_or(false);
            cli::unstage(root, paths, unborn)
        }
        Job::Discard { tracked, untracked } => {
            let mut log = String::new();
            let mut result = Ok(());
            if !tracked.is_empty() {
                match cli::discard_tracked(root, tracked) {
                    Ok(out) => log.push_str(&out),
                    Err(e) => result = Err(e),
                }
            }
            // Attempted even if the restore failed: they are independent sets of
            // paths, and half-discarding without saying so is worse than doing
            // both and reporting the one that broke.
            if result.is_ok() && !untracked.is_empty() {
                match cli::delete_untracked(root, untracked) {
                    Ok(out) => log.push_str(&out),
                    Err(e) => result = Err(e),
                }
            }
            result.map(|()| log)
        }
        Job::Commit { message, amend } => cli::commit(root, message, *amend),
        Job::Fetch => cli::fetch(root),
        Job::Pull => cli::pull(root),
        Job::Push {
            set_upstream,
            branch,
        } => cli::push(root, *set_upstream, branch.as_deref()),
        Job::Checkout(rev) => cli::checkout(root, rev),
        Job::CreateBranch { name, start } => cli::create_branch(root, name, start.as_deref()),
        Job::DeleteBranch { name, force } => cli::delete_branch(root, name, *force),
        Job::Merge(rev) => cli::merge(root, rev),
        Job::MergeAbort => cli::merge_abort(root),
        Job::Revert { rev, is_merge } => cli::revert(root, rev, *is_merge),
        Job::Reset { rev, hard } => cli::reset(root, rev, *hard),
        Job::Diff {
            path,
            staged,
            untracked,
        } => match cli::diff(root, path, *staged, *untracked) {
            Ok(d) => {
                text = Some(d);
                Ok(String::new())
            }
            Err(e) => Err(e),
        },
        Job::Show(rev) => match cli::show(root, rev) {
            Ok(d) => {
                text = Some(d);
                Ok(String::new())
            }
            Err(e) => Err(e),
        },
    };

    // See the module doc: refreshed even on failure, because a failed merge is
    // exactly the case where the tree changed anyway. A diff/show is the one read
    // that changes nothing and so needs no re-read.
    let refreshed = if job.is_read_only() && job != Job::Refresh {
        None
    } else {
        snapshot(root).ok()
    };

    JobReport {
        job,
        outcome,
        snapshot: refreshed,
        text,
    }
}

/// Holds the in-flight job, if any.
///
/// Not a queue: see the module doc. A request arriving while one is running is
/// dropped by the caller, which checks [`Runner::busy`] before asking.
#[derive(Default)]
pub struct Runner {
    /// The `Mutex` is not for contention — only this system ever touches the
    /// receiver. It is because a bevy `Resource` must be `Sync` and
    /// `mpsc::Receiver` is not, which is the same reason the update dialog wraps
    /// its channel.
    active: Option<(Job, std::sync::Mutex<Receiver<JobReport>>)>,
}

impl Runner {
    /// Start `job` unless something is already running. Returns whether it did.
    pub fn start(&mut self, root: PathBuf, job: Job) -> bool {
        if self.active.is_some() {
            return false;
        }
        self.active = Some((job.clone(), std::sync::Mutex::new(spawn(root, job))));
        true
    }

    /// The running job, for the busy indicator.
    pub fn current(&self) -> Option<&Job> {
        self.active.as_ref().map(|(job, _)| job)
    }

    pub fn busy(&self) -> bool {
        self.active.is_some()
    }

    /// Collect the report if the worker has finished.
    ///
    /// A disconnected channel with no value means the worker died without
    /// reporting — a panic inside `git` handling, not a git failure. Clearing the
    /// slot is what stops that from wedging every later operation behind a job
    /// that will never finish.
    pub fn poll(&mut self) -> Option<Result<JobReport, String>> {
        // Received into a local first so the borrow of `active` is over before the
        // arms below clear it.
        let received = {
            let (_, rx) = self.active.as_ref()?;
            match rx.lock() {
                Ok(rx) => rx.try_recv(),
                // Poisoned means a panic happened while the lock was held. Nothing
                // here panics under it, but treating it as a dead worker beats
                // panicking a second time and taking the editor with it.
                Err(_) => Err(mpsc::TryRecvError::Disconnected),
            }
        };
        match received {
            Ok(report) => {
                self.active = None;
                Some(Ok(report))
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                let job = self.active.take().map(|(job, _)| job);
                Some(Err(format!(
                    "The git operation stopped unexpectedly{}.",
                    match job {
                        Some(job) => format!(" ({})", job.progress_label().trim_end_matches('…')),
                        None => String::new(),
                    }
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reconciliation in `crate::reload` hangs off this, so a job that can
    /// rewrite a scene file and reports otherwise leaves the editor holding a
    /// stale scene that `Ctrl+S` would write back over the new one.
    #[test]
    fn every_job_that_can_rewrite_a_file_says_so() {
        for job in [
            Job::Checkout("main".into()),
            Job::Pull,
            Job::Merge("main".into()),
            Job::MergeAbort,
            Job::Revert {
                rev: "abc".into(),
                is_merge: false,
            },
            Job::Reset {
                rev: "abc".into(),
                hard: true,
            },
            Job::Discard {
                tracked: vec!["a".into()],
                untracked: vec![],
            },
            Job::CreateBranch {
                name: "x".into(),
                start: None,
            },
            // A hook can reformat what it is given, so a commit counts too.
            Job::Commit {
                message: "m".into(),
                amend: false,
            },
        ] {
            assert!(job.changes_worktree(), "{job:?} must be treated as touching the working tree");
        }
    }

    /// The converse: staging only moves things into the index, and fetch only
    /// updates remote refs. Treating them as tree changes would re-fingerprint
    /// and potentially reload the open scene on every stage click.
    #[test]
    fn index_only_and_remote_only_jobs_do_not_claim_to_touch_the_tree() {
        for job in [
            Job::Refresh,
            Job::Stage(vec!["a".into()]),
            Job::Unstage(vec!["a".into()]),
            Job::Fetch,
            Job::Push {
                set_upstream: false,
                branch: None,
            },
            Job::Diff {
                path: "a".into(),
                staged: false,
                untracked: false,
            },
            Job::Show("abc".into()),
        ] {
            assert!(!job.changes_worktree(), "{job:?} does not change the working tree");
        }
    }

    #[test]
    fn only_the_three_pure_reads_are_read_only() {
        assert!(Job::Refresh.is_read_only());
        assert!(Job::Show("a".into()).is_read_only());
        assert!(Job::Diff {
            path: "a".into(),
            staged: false,
            untracked: false
        }
        .is_read_only());
        assert!(!Job::Fetch.is_read_only(), "fetch rewrites remote-tracking refs");
        assert!(!Job::Stage(vec![]).is_read_only());
    }

    /// A refresh whose result was already visible does not need announcing; an
    /// operation the user asked for does.
    #[test]
    fn reads_are_silent_and_operations_are_announced() {
        assert_eq!(Job::Refresh.success_message(None), None);
        assert_eq!(Job::Show("a".into()).success_message(None), None);
        assert!(Job::Pull.success_message(None).is_some());
        assert!(Job::Fetch.success_message(None).is_some());
    }

    #[test]
    fn counted_messages_are_singular_for_one_file() {
        assert_eq!(
            Job::Stage(vec!["a".into()]).success_message(None).unwrap(),
            "Staged 1 file"
        );
        assert_eq!(
            Job::Stage(vec!["a".into(), "b".into()])
                .success_message(None)
                .unwrap(),
            "Staged 2 files"
        );
    }

    /// A discard spans both sets, and the user cares about the total.
    #[test]
    fn a_discard_counts_tracked_and_untracked_together() {
        let job = Job::Discard {
            tracked: vec!["a".into()],
            untracked: vec!["b".into(), "c".into()],
        };
        assert_eq!(job.success_message(None).unwrap(), "Discarded 3 changes");
    }

    /// The two reset modes do opposite things to uncommitted work, so they must
    /// not report the same sentence.
    #[test]
    fn the_two_reset_modes_report_differently() {
        let hard = Job::Reset {
            rev: "abc".into(),
            hard: true,
        };
        let soft = Job::Reset {
            rev: "abc".into(),
            hard: false,
        };
        assert_ne!(hard.success_message(None), soft.success_message(None));
        assert!(soft.success_message(None).unwrap().contains("keeping"));
    }

    #[test]
    fn a_commit_names_the_commit_it_created_when_the_snapshot_has_it() {
        let snap = Snapshot {
            log: vec![Commit {
                oid: "abcdef".into(),
                short: "abcdef".into(),
                author: "A".into(),
                timestamp: 0,
                refs: vec![],
                parents: vec![],
                subject: "s".into(),
                is_head: true,
            }],
            ..Default::default()
        };
        let job = Job::Commit {
            message: "m".into(),
            amend: false,
        };
        assert_eq!(job.success_message(Some(&snap)).unwrap(), "Committed abcdef");
    }

    // ── push/upstream decisions ──────────────────────────────────────────────

    fn snap(upstream: Option<&str>, remotes: &[&str]) -> Snapshot {
        Snapshot {
            status: RepoStatus {
                upstream: upstream.map(str::to_string),
                ..Default::default()
            },
            remotes: remotes.iter().map(|r| r.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_branch_with_no_upstream_and_one_remote_pushes_with_set_upstream() {
        assert!(snap(None, &["origin"]).push_needs_upstream());
    }

    #[test]
    fn a_branch_that_already_tracks_does_not_set_upstream_again() {
        assert!(!snap(Some("origin/main"), &["origin"]).push_needs_upstream());
    }

    /// `--set-upstream origin` names one remote outright. With two configured
    /// there is no honest default, so the plain push runs and git explains.
    #[test]
    fn two_remotes_means_no_automatic_upstream_choice() {
        assert!(!snap(None, &["origin", "fork"]).push_needs_upstream());
    }

    /// No remote at all is a different message from a failed push, so it has to
    /// be distinguishable before anything is attempted.
    #[test]
    fn a_repo_with_no_remote_is_distinguishable_from_one_with_a_broken_remote() {
        assert!(!snap(None, &[]).has_remote());
        assert!(!snap(None, &[]).push_needs_upstream());
        assert!(snap(None, &["origin"]).has_remote());
    }

    #[test]
    fn branches_split_into_local_and_remote() {
        let s = Snapshot {
            refs: vec![
                BranchRef {
                    full: "refs/heads/main".into(),
                    name: "main".into(),
                    short_oid: "a".into(),
                    upstream: None,
                    is_head: true,
                    remote: false,
                },
                BranchRef {
                    full: "refs/remotes/origin/main".into(),
                    name: "origin/main".into(),
                    short_oid: "a".into(),
                    upstream: None,
                    is_head: false,
                    remote: true,
                },
            ],
            ..Default::default()
        };
        assert_eq!(s.local_branches().count(), 1);
        assert_eq!(s.remote_branches().count(), 1);
        assert_eq!(s.local_branches().next().unwrap().name, "main");
    }

    // ── Runner ───────────────────────────────────────────────────────────────

    /// The gate that keeps two operations from racing for `index.lock`.
    #[test]
    fn a_second_job_is_refused_while_one_is_running() {
        let mut runner = Runner::default();
        // A path that is not a repo: the job fails fast, which is all this needs.
        let root = std::env::temp_dir();
        assert!(runner.start(root.clone(), Job::Refresh));
        assert!(runner.busy());
        assert!(
            !runner.start(root, Job::Fetch),
            "a second job must be refused, not queued"
        );
        assert_eq!(runner.current(), Some(&Job::Refresh));
    }

    /// Polling an idle runner must not report a phantom failure.
    #[test]
    fn an_idle_runner_polls_to_nothing() {
        let mut runner = Runner::default();
        assert!(runner.poll().is_none());
        assert!(!runner.busy());
    }

    /// A real end-to-end pass through the worker: the slot has to clear, or every
    /// later operation is blocked behind a job that already finished.
    #[test]
    fn a_finished_job_clears_the_slot_and_reports_once() {
        let mut runner = Runner::default();
        runner.start(std::env::temp_dir(), Job::Refresh);
        let mut report = None;
        // The worker spawns a process; give it room without making the test slow
        // when it answers immediately.
        for _ in 0..400 {
            if let Some(r) = runner.poll() {
                report = Some(r);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(report.is_some(), "the worker never reported");
        assert!(!runner.busy(), "the slot must clear so later jobs can run");
        assert!(runner.poll().is_none(), "a report must not be delivered twice");
    }
}
