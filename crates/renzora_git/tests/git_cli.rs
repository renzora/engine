//! End-to-end tests against the real `git` binary.
//!
//! The unit tests in `parse` cover output shapes captured by hand; these check
//! that the shapes are the ones git actually produces *for the flags this crate
//! passes*. That gap is where the interesting failures live, and none of them are
//! visible to a parser test: a flag that means something slightly different than
//! expected, a command that needs a repo state it doesn't have, an output format
//! that changes between git versions. All of those parse perfectly into the wrong
//! answer.
//!
//! Everything runs through [`renzora_git::job`], the same path the panel uses, so a
//! test failing here means the panel is broken in the same way.
//!
//! These are skipped when `git` is not installed rather than failed — the crate
//! already treats a missing git as a supported state, so a machine without one is
//! not a broken checkout.

#![cfg(not(target_arch = "wasm32"))]

use std::path::{Path, PathBuf};
use std::process::Command;

use renzora_git::job::{self, Job, JobReport, Snapshot};
use renzora_git::parse::{Change, Head};

/// Is git usable? These tests are meaningless without it.
fn git_present() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Run a raw git command in `dir`, panicking with git's own message on failure.
///
/// Used only for *arranging* a scenario, never for the behaviour under test.
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("could not run git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A throwaway repository, removed when the test ends.
struct Repo {
    path: PathBuf,
}

impl Repo {
    /// An empty directory — not yet a repository, so `Job::Init` can be tested.
    fn bare_dir(name: &str) -> Repo {
        let path = std::env::temp_dir().join(format!(
            "renzora-git-it-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("could not create the test directory");
        Repo { path }
    }

    /// An initialised repository with an identity, so commits succeed.
    ///
    /// The identity is set on the repo rather than relying on a global config: a CI
    /// container has none, and `git commit` fails outright without one.
    fn new(name: &str) -> Repo {
        let repo = Repo::bare_dir(name);
        git(&repo.path, &["init", "--initial-branch=main"]);
        git(&repo.path, &["config", "user.email", "test@example.com"]);
        git(&repo.path, &["config", "user.name", "Test"]);
        // Keeps line-ending conversion from rewriting the fixtures underneath the
        // tests on Windows, where it would otherwise report files as modified
        // immediately after they were committed.
        git(&repo.path, &["config", "core.autocrlf", "false"]);
        repo
    }

    fn write(&self, name: &str, contents: &str) {
        let path = self.path.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.path.join(name)).unwrap()
    }

    fn exists(&self, name: &str) -> bool {
        self.path.join(name).exists()
    }

    /// Run a job to completion, the way the panel's poll loop would.
    fn run(&self, job: Job) -> JobReport {
        let rx = job::spawn(self.path.clone(), job);
        rx.recv_timeout(std::time::Duration::from_secs(60))
            .expect("the git worker did not report within 60s")
    }

    /// Run a job and require it to have succeeded.
    fn ok(&self, job: Job) -> Snapshot {
        let label = format!("{job:?}");
        let report = self.run(job);
        match &report.outcome {
            Ok(_) => {}
            Err(e) => panic!("{label} failed: {e}"),
        }
        report.snapshot.unwrap_or_else(|| panic!("{label} returned no snapshot"))
    }

    fn snapshot(&self) -> Snapshot {
        job::snapshot(&self.path).expect("could not read the repository")
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        // Best effort: a leftover temp directory is not worth failing a test over,
        // and on Windows a file can still be held briefly by a just-exited git.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Arrange a repo with one commit on `main`.
fn with_one_commit(name: &str) -> Repo {
    let repo = Repo::new(name);
    repo.write("scenes/level.bsn", "(entities: {})\n");
    git(&repo.path, &["add", "."]);
    git(&repo.path, &["commit", "-m", "first"]);
    repo
}

#[test]
fn a_fresh_init_reports_an_unborn_branch_and_writes_a_gitignore() {
    if !git_present() {
        return;
    }
    let repo = Repo::bare_dir("init");
    repo.write("project.toml", "name = \"test\"\n");

    let snapshot = repo.ok(Job::Init);

    assert!(
        snapshot.status.unborn,
        "a repository with no commits must report as unborn"
    );
    assert_eq!(snapshot.status.head, Head::Branch("main".into()));
    assert!(
        snapshot.log.is_empty(),
        "an unborn repo has no history, and reading it must not fail"
    );
    assert!(
        repo.exists(".gitignore"),
        "init must leave a starter .gitignore"
    );
    // The caches are the point of writing one: they are thousands of generated
    // files that must not reach a first commit.
    let ignore = repo.read(".gitignore");
    assert!(ignore.contains(".cache/"), "got: {ignore}");
    assert!(ignore.contains(".thumbs/"), "got: {ignore}");
}

/// An existing `.gitignore` is the author's; replacing it silently would lose
/// whatever was in it.
#[test]
fn init_does_not_overwrite_an_existing_gitignore() {
    if !git_present() {
        return;
    }
    let repo = Repo::bare_dir("init-keep");
    repo.write(".gitignore", "# mine\nsecret/\n");
    repo.ok(Job::Init);
    assert_eq!(repo.read(".gitignore"), "# mine\nsecret/\n");
}

/// The `.gitignore` has to actually work — a syntactically fine file that does not
/// match the cache directory would pass the test above and still let thousands of
/// generated files into the first commit.
#[test]
fn the_generated_gitignore_actually_excludes_the_caches() {
    if !git_present() {
        return;
    }
    let repo = Repo::bare_dir("init-effective");
    repo.write("project.toml", "name = \"test\"\n");
    repo.ok(Job::Init);
    repo.write(".cache/thumbnails/material/a.png", "x");
    repo.write("scenes/level.bsn", "(entities: {})\n");

    let snapshot = repo.snapshot();
    let paths: Vec<&str> = snapshot.status.entries.iter().map(|e| e.path.as_str()).collect();
    assert!(
        !paths.iter().any(|p| p.starts_with(".cache")),
        "the cache must be ignored, got: {paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.starts_with("scenes")),
        "real project files must still be listed, got: {paths:?}"
    );
}

#[test]
fn a_new_file_is_untracked_then_staged_then_committed() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("lifecycle");
    repo.write("scripts/player.lua", "-- hello\n");

    // Untracked: git has no copy, which is what makes "discard" a deletion.
    let snapshot = repo.snapshot();
    let entry = snapshot
        .status
        .entries
        .iter()
        .find(|e| e.path.starts_with("scripts"))
        .expect("the new file must be listed");
    assert!(entry.is_untracked());
    assert!(!entry.is_staged());

    // Staged.
    let snapshot = repo.ok(Job::Stage(vec!["scripts/player.lua".into()]));
    let entry = snapshot
        .status
        .entries
        .iter()
        .find(|e| e.path == "scripts/player.lua")
        .expect("the staged file must be listed");
    assert_eq!(entry.index, Some(Change::Added));
    assert!(!entry.is_unstaged());

    // Committed: the tree is clean and the history grew.
    let snapshot = repo.ok(Job::Commit {
        message: "feat: add the player script".into(),
        amend: false,
    });
    assert!(snapshot.status.is_clean(), "got: {:?}", snapshot.status.entries);
    assert_eq!(snapshot.log.len(), 2);
    assert_eq!(snapshot.log[0].subject, "feat: add the player script");
    assert!(snapshot.log[0].is_head);
}

/// A multi-line message with a blank line in it is the conventional form, and the
/// reason the message goes through a file instead of `-m`.
#[test]
fn a_multi_line_commit_message_survives_intact() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("multiline");
    repo.write("a.txt", "x\n");
    repo.ok(Job::Stage(vec!["a.txt".into()]));
    let snapshot = repo.ok(Job::Commit {
        message: "feat: the subject\n\nA body paragraph explaining why.\n".into(),
        amend: false,
    });
    assert_eq!(snapshot.log[0].subject, "feat: the subject");
}

#[test]
fn staging_then_unstaging_returns_the_file_to_the_working_tree() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("unstage");
    repo.write("scenes/level.bsn", "(entities: {edited})\n");

    let snapshot = repo.ok(Job::Stage(vec!["scenes/level.bsn".into()]));
    assert_eq!(snapshot.status.staged().count(), 1);

    let snapshot = repo.ok(Job::Unstage(vec!["scenes/level.bsn".into()]));
    assert_eq!(snapshot.status.staged().count(), 0);
    assert_eq!(snapshot.status.unstaged().count(), 1);
    // Unstaging must not touch the file itself.
    assert_eq!(repo.read("scenes/level.bsn"), "(entities: {edited})\n");
}

/// Unstaging in an unborn repo needs a different command: with no HEAD there is
/// nothing for `restore --staged` to restore from and it fails. This is the state
/// right after "Initialize Repository", so it is not an obscure edge case.
#[test]
fn unstaging_works_in_a_repository_with_no_commits() {
    if !git_present() {
        return;
    }
    let repo = Repo::new("unstage-unborn");
    repo.write("a.txt", "x\n");
    let snapshot = repo.ok(Job::Stage(vec!["a.txt".into()]));
    assert!(snapshot.status.unborn);
    assert_eq!(snapshot.status.staged().count(), 1);

    let snapshot = repo.ok(Job::Unstage(vec!["a.txt".into()]));
    assert_eq!(
        snapshot.status.staged().count(),
        0,
        "unstaging must work before the first commit"
    );
    assert!(repo.exists("a.txt"), "the file itself must survive");
}

/// The two halves of a discard are different commands with different
/// consequences, and both have to happen.
#[test]
fn discarding_restores_tracked_files_and_deletes_untracked_ones() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("discard");
    repo.write("scenes/level.bsn", "(entities: {ruined})\n");
    repo.write("junk.tmp", "garbage\n");

    let snapshot = repo.ok(Job::Discard {
        tracked: vec!["scenes/level.bsn".into()],
        untracked: vec!["junk.tmp".into()],
    });

    assert_eq!(
        repo.read("scenes/level.bsn"),
        "(entities: {})\n",
        "a tracked file goes back to its committed contents"
    );
    assert!(
        !repo.exists("junk.tmp"),
        "an untracked file is deleted — git has no copy to restore"
    );
    assert!(snapshot.status.is_clean(), "got: {:?}", snapshot.status.entries);
}

/// Discarding an untracked *directory* is the common shape, because git collapses
/// untracked directories in its status output — so the path handed back is the
/// directory, and `clean` needs `-d` to remove it.
#[test]
fn discarding_a_collapsed_untracked_directory_removes_it() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("discard-dir");
    repo.write("generated/a.txt", "x\n");
    repo.write("generated/b.txt", "y\n");

    let snapshot = repo.snapshot();
    let entry = snapshot
        .status
        .entries
        .iter()
        .find(|e| e.path.starts_with("generated"))
        .expect("the new directory must be listed");
    assert_eq!(
        entry.path, "generated/",
        "git collapses an untracked directory, trailing slash and all"
    );

    repo.ok(Job::Discard {
        tracked: vec![],
        untracked: vec![entry.path.clone()],
    });
    assert!(!repo.exists("generated"), "the directory must be gone");
}

#[test]
fn a_rename_is_reported_with_the_path_it_came_from() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("rename");
    git(&repo.path, &["mv", "scenes/level.bsn", "scenes/arena.bsn"]);

    let snapshot = repo.snapshot();
    let entry = snapshot
        .status
        .entries
        .iter()
        .find(|e| e.path == "scenes/arena.bsn")
        .expect("the renamed file must be listed under its new name");
    assert_eq!(entry.index, Some(Change::Renamed));
    assert_eq!(entry.orig_path.as_deref(), Some("scenes/level.bsn"));
    assert_eq!(
        snapshot.status.entries.len(),
        1,
        "the old path must not appear as a second entry, got: {:?}",
        snapshot.status.entries
    );
}

/// A path with a space in it is the case that a whitespace-splitting parser
/// truncates — and the truncated path is then what gets passed to `git restore`.
#[test]
fn a_path_with_spaces_round_trips_through_staging_and_committing() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("spaces");
    repo.write("scenes/my level 2.bsn", "(entities: {})\n");

    let snapshot = repo.snapshot();
    assert!(
        snapshot
            .status
            .entries
            .iter()
            .any(|e| e.path == "scenes/my level 2.bsn"),
        "got: {:?}",
        snapshot.status.entries
    );

    let snapshot = repo.ok(Job::Stage(vec!["scenes/my level 2.bsn".into()]));
    assert_eq!(snapshot.status.staged().count(), 1);
    let snapshot = repo.ok(Job::Commit {
        message: "add a level with spaces".into(),
        amend: false,
    });
    assert!(snapshot.status.is_clean());
}

#[test]
fn branching_switching_and_merging_moves_the_history() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("branch");

    // Create and switch.
    let snapshot = repo.ok(Job::CreateBranch {
        name: "feature".into(),
        start: None,
    });
    assert_eq!(snapshot.status.head, Head::Branch("feature".into()));
    assert!(snapshot.local_branches().any(|b| b.name == "feature" && b.is_head));

    // Commit on the branch.
    repo.write("scripts/new.lua", "-- new\n");
    repo.ok(Job::Stage(vec!["scripts/new.lua".into()]));
    repo.ok(Job::Commit {
        message: "on the branch".into(),
        amend: false,
    });

    // Back to main: the branch's file must be gone from the working tree, which is
    // exactly why the editor has to reload the open scene after a switch.
    let snapshot = repo.ok(Job::Checkout("main".into()));
    assert_eq!(snapshot.status.head, Head::Branch("main".into()));
    assert!(!repo.exists("scripts/new.lua"));
    assert_eq!(snapshot.log.len(), 1);

    // Merge it back.
    let snapshot = repo.ok(Job::Merge("feature".into()));
    assert!(repo.exists("scripts/new.lua"));
    assert!(!snapshot.status.has_conflicts());
    assert!(snapshot.log.iter().any(|c| c.subject == "on the branch"));
}

/// A conflicting merge fails *and* rewrites the working tree. Reporting the
/// failure without the refreshed state would leave the panel claiming a clean tree
/// over files that have conflict markers in them.
#[test]
fn a_conflicting_merge_fails_but_still_reports_the_conflicted_tree() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("conflict");
    repo.write("shared.txt", "base\n");
    git(&repo.path, &["add", "."]);
    git(&repo.path, &["commit", "-m", "base"]);

    git(&repo.path, &["checkout", "-b", "side"]);
    repo.write("shared.txt", "side\n");
    git(&repo.path, &["commit", "-am", "side"]);
    git(&repo.path, &["checkout", "main"]);
    repo.write("shared.txt", "main\n");
    git(&repo.path, &["commit", "-am", "main"]);

    let report = repo.run(Job::Merge("side".into()));
    assert!(report.outcome.is_err(), "a conflicting merge must report failure");

    let snapshot = report
        .snapshot
        .expect("a failed merge must STILL refresh — the tree changed");
    assert!(
        snapshot.status.has_conflicts(),
        "the conflict must be visible, got: {:?}",
        snapshot.status.entries
    );
    assert_eq!(snapshot.status.conflicts().count(), 1);
    assert_eq!(snapshot.status.conflicts().next().unwrap().path, "shared.txt");
    assert!(
        repo.read("shared.txt").contains("<<<<<<<"),
        "and the file really does have markers in it"
    );

    // Aborting puts it back.
    let snapshot = repo.ok(Job::MergeAbort);
    assert!(!snapshot.status.has_conflicts());
    assert_eq!(repo.read("shared.txt"), "main\n");
}

#[test]
fn checking_out_a_commit_detaches_head() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("detach");
    repo.write("second.txt", "x\n");
    git(&repo.path, &["add", "."]);
    git(&repo.path, &["commit", "-m", "second"]);

    let first = repo.snapshot().log.last().unwrap().oid.clone();
    let snapshot = repo.ok(Job::Checkout(first));
    assert_eq!(
        snapshot.status.head,
        Head::Detached,
        "checking out a commit must be reported as detached, not as a branch"
    );
    assert!(!repo.exists("second.txt"));
    assert!(snapshot.status.head_label().starts_with("detached @ "));
}

/// The three ways back, and the difference between them — which is the whole
/// reason the panel offers all three with different warnings.
#[test]
fn the_three_ways_back_differ_in_what_they_keep() {
    if !git_present() {
        return;
    }
    // Revert: adds a commit, keeps history.
    {
        let repo = with_one_commit("revert");
        repo.write("added.txt", "x\n");
        git(&repo.path, &["add", "."]);
        git(&repo.path, &["commit", "-m", "add a file"]);
        let target = repo.snapshot().log[0].oid.clone();

        let snapshot = repo.ok(Job::Revert {
            rev: target,
            is_merge: false,
        });
        assert!(!repo.exists("added.txt"), "the change is undone");
        assert_eq!(snapshot.log.len(), 3, "and history grew rather than shrank");
        assert!(snapshot.log.iter().any(|c| c.subject.contains("add a file")));
    }

    // Mixed reset: un-commits, keeps the changes as uncommitted work.
    {
        let repo = with_one_commit("reset-mixed");
        repo.write("added.txt", "x\n");
        git(&repo.path, &["add", "."]);
        git(&repo.path, &["commit", "-m", "add a file"]);

        let snapshot = repo.ok(Job::Reset {
            rev: "HEAD~1".into(),
            hard: false,
        });
        assert_eq!(snapshot.log.len(), 1, "the commit is gone");
        assert!(repo.exists("added.txt"), "but the work is not");
        assert!(!snapshot.status.is_clean());
    }

    // Hard reset: un-commits and throws the changes away.
    {
        let repo = with_one_commit("reset-hard");
        repo.write("added.txt", "x\n");
        git(&repo.path, &["add", "."]);
        git(&repo.path, &["commit", "-m", "add a file"]);

        let snapshot = repo.ok(Job::Reset {
            rev: "HEAD~1".into(),
            hard: true,
        });
        assert_eq!(snapshot.log.len(), 1);
        assert!(!repo.exists("added.txt"), "the work is gone too");
        assert!(snapshot.status.is_clean());
    }
}

/// Reverting a merge needs `-m 1`; without it git refuses outright, so the
/// merge flag has to reach the command.
#[test]
fn reverting_a_merge_commit_succeeds() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("revert-merge");
    git(&repo.path, &["checkout", "-b", "side"]);
    repo.write("from-side.txt", "x\n");
    git(&repo.path, &["add", "."]);
    git(&repo.path, &["commit", "-m", "side work"]);
    git(&repo.path, &["checkout", "main"]);
    // `--no-ff` forces a real merge commit even though it could fast-forward.
    git(&repo.path, &["merge", "--no-ff", "--no-edit", "side"]);

    let merge = repo.snapshot().log[0].clone();
    assert!(merge.is_merge(), "the fixture must produce a merge commit");

    repo.ok(Job::Revert {
        rev: merge.oid,
        is_merge: true,
    });
    assert!(
        !repo.exists("from-side.txt"),
        "reverting the merge undoes what it brought in"
    );
}

#[test]
fn a_branch_can_be_deleted_once_it_is_merged() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("delete-branch");
    git(&repo.path, &["branch", "spare"]);

    let snapshot = repo.ok(Job::DeleteBranch {
        name: "spare".into(),
        force: false,
    });
    assert!(!snapshot.local_branches().any(|b| b.name == "spare"));
}

/// Git refusing to delete a branch holding unmerged commits is the safety net the
/// panel relies on instead of offering a one-click force delete.
#[test]
fn deleting_an_unmerged_branch_is_refused_rather_than_losing_commits() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("delete-unmerged");
    git(&repo.path, &["checkout", "-b", "unmerged"]);
    repo.write("work.txt", "important\n");
    git(&repo.path, &["add", "."]);
    git(&repo.path, &["commit", "-m", "unmerged work"]);
    git(&repo.path, &["checkout", "main"]);

    let report = repo.run(Job::DeleteBranch {
        name: "unmerged".into(),
        force: false,
    });
    assert!(
        report.outcome.is_err(),
        "an unmerged branch must not be deleted without being forced"
    );
    let snapshot = report.snapshot.expect("a refreshed snapshot");
    assert!(
        snapshot.local_branches().any(|b| b.name == "unmerged"),
        "and the branch must still be there"
    );
}

/// A repo with no remote is a different situation from a broken one: nowhere to
/// push, as opposed to a push that failed.
#[test]
fn a_repository_with_no_remote_says_so_before_anything_is_attempted() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("no-remote");
    let snapshot = repo.snapshot();
    assert!(!snapshot.has_remote());
    assert!(
        !snapshot.push_needs_upstream(),
        "with no remote there is no upstream to guess at"
    );
    assert_eq!(snapshot.status.upstream, None);
    assert_eq!((snapshot.status.ahead, snapshot.status.behind), (0, 0));
}

/// The full remote round trip, against a local bare repo standing in for a
/// server. This is the part that cannot be checked by parsing alone: whether
/// `--set-upstream` is passed at the right moment, and whether the tracking
/// counts then read back correctly.
#[test]
fn a_first_push_sets_up_tracking_and_later_pushes_report_ahead_and_behind() {
    if !git_present() {
        return;
    }
    let remote = Repo::bare_dir("remote-bare");
    git(&remote.path, &["init", "--bare", "--initial-branch=main"]);

    let repo = with_one_commit("remote-work");
    git(
        &repo.path,
        &["remote", "add", "origin", &remote.path.to_string_lossy()],
    );

    // Before the first push there is no upstream, so one must be created.
    let snapshot = repo.snapshot();
    assert!(snapshot.has_remote());
    assert!(
        snapshot.push_needs_upstream(),
        "a branch with no upstream and one remote must push with --set-upstream"
    );

    let snapshot = repo.ok(Job::Push {
        set_upstream: true,
        branch: Some("main".into()),
    });
    assert_eq!(
        snapshot.status.upstream.as_deref(),
        Some("origin/main"),
        "the push must have established tracking"
    );
    assert_eq!((snapshot.status.ahead, snapshot.status.behind), (0, 0));
    assert!(
        !snapshot.push_needs_upstream(),
        "and a later push must not try to set it again"
    );

    // A local commit puts us ahead.
    repo.write("local.txt", "x\n");
    repo.ok(Job::Stage(vec!["local.txt".into()]));
    let snapshot = repo.ok(Job::Commit {
        message: "local work".into(),
        amend: false,
    });
    assert_eq!(
        (snapshot.status.ahead, snapshot.status.behind),
        (1, 0),
        "one unpushed commit means one ahead"
    );

    let snapshot = repo.ok(Job::Push {
        set_upstream: false,
        branch: Some("main".into()),
    });
    assert_eq!((snapshot.status.ahead, snapshot.status.behind), (0, 0));

    // Now make the remote move on, from a second clone, and check we read as
    // behind after a fetch — the state that makes the Pull button meaningful.
    let other = Repo::bare_dir("remote-other");
    git(
        &other.path,
        &["clone", &remote.path.to_string_lossy(), "."],
    );
    git(&other.path, &["config", "user.email", "b@example.com"]);
    git(&other.path, &["config", "user.name", "Other"]);
    other.write("theirs.txt", "y\n");
    git(&other.path, &["add", "."]);
    git(&other.path, &["commit", "-m", "their work"]);
    git(&other.path, &["push"]);

    let snapshot = repo.ok(Job::Fetch);
    assert_eq!(
        (snapshot.status.ahead, snapshot.status.behind),
        (0, 1),
        "a fetch must reveal the remote commit without applying it"
    );
    assert!(
        !repo.exists("theirs.txt"),
        "and fetch must not touch the working tree"
    );

    // Pull applies it. `--ff-only` can, because nothing diverged.
    let snapshot = repo.ok(Job::Pull);
    assert_eq!((snapshot.status.ahead, snapshot.status.behind), (0, 0));
    assert!(repo.exists("theirs.txt"), "pull must bring the file in");
    assert!(snapshot.log.iter().any(|c| c.subject == "their work"));
}

/// `pull` is deliberately `--ff-only`, so diverged branches stop rather than
/// silently creating a merge commit. That refusal is a designed outcome, and the
/// panel's advice depends on it happening.
#[test]
fn a_pull_on_a_diverged_branch_refuses_instead_of_merging_silently() {
    if !git_present() {
        return;
    }
    let remote = Repo::bare_dir("diverge-bare");
    git(&remote.path, &["init", "--bare", "--initial-branch=main"]);

    let repo = with_one_commit("diverge-work");
    git(
        &repo.path,
        &["remote", "add", "origin", &remote.path.to_string_lossy()],
    );
    repo.ok(Job::Push {
        set_upstream: true,
        branch: Some("main".into()),
    });

    // Their commit.
    let other = Repo::bare_dir("diverge-other");
    git(&other.path, &["clone", &remote.path.to_string_lossy(), "."]);
    git(&other.path, &["config", "user.email", "b@example.com"]);
    git(&other.path, &["config", "user.name", "Other"]);
    other.write("theirs.txt", "y\n");
    git(&other.path, &["add", "."]);
    git(&other.path, &["commit", "-m", "theirs"]);
    git(&other.path, &["push"]);

    // Our commit, on the same base — now the histories have diverged.
    repo.write("ours.txt", "x\n");
    repo.ok(Job::Stage(vec!["ours.txt".into()]));
    repo.ok(Job::Commit {
        message: "ours".into(),
        amend: false,
    });

    let report = repo.run(Job::Pull);
    assert!(
        report.outcome.is_err(),
        "a diverged pull must refuse rather than invent a merge commit"
    );
    // The user's own commit is untouched by the refusal.
    assert!(repo.exists("ours.txt"));

    // And the explicit route works: fetch, then merge.
    repo.ok(Job::Fetch);
    let snapshot = repo.ok(Job::Merge("origin/main".into()));
    assert!(repo.exists("ours.txt") && repo.exists("theirs.txt"));
    assert!(snapshot.log.iter().any(|c| c.subject == "theirs"));
}

/// Switching to a remote branch has to create a local one tracking it — checking
/// the remote ref out directly would detach HEAD.
#[test]
fn a_local_branch_created_from_a_remote_one_tracks_it() {
    if !git_present() {
        return;
    }
    let remote = Repo::bare_dir("track-bare");
    git(&remote.path, &["init", "--bare", "--initial-branch=main"]);

    let repo = with_one_commit("track-work");
    git(
        &repo.path,
        &["remote", "add", "origin", &remote.path.to_string_lossy()],
    );
    repo.ok(Job::Push {
        set_upstream: true,
        branch: Some("main".into()),
    });
    git(&repo.path, &["checkout", "-b", "feature"]);
    repo.ok(Job::Push {
        set_upstream: true,
        branch: Some("feature".into()),
    });
    git(&repo.path, &["checkout", "main"]);
    git(&repo.path, &["branch", "-D", "feature"]);
    repo.ok(Job::Fetch);

    // The remote branch is visible but there is no local one.
    let snapshot = repo.snapshot();
    assert!(snapshot.remote_branches().any(|b| b.name == "origin/feature"));
    assert!(!snapshot.local_branches().any(|b| b.name == "feature"));

    let snapshot = repo.ok(Job::CreateBranch {
        name: "feature".into(),
        start: Some("origin/feature".into()),
    });
    assert_eq!(
        snapshot.status.head,
        Head::Branch("feature".into()),
        "and it must be checked out, not just created"
    );
    assert_eq!(
        snapshot.status.upstream.as_deref(),
        Some("origin/feature"),
        "a branch created from a remote one must track it"
    );
}

/// Diffs feed the viewer, and the untracked case needs a different command
/// entirely — git will not diff a file it does not know about.
#[test]
fn diffs_are_produced_for_staged_unstaged_and_untracked_files() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("diff");

    // Unstaged.
    repo.write("scenes/level.bsn", "(entities: {edited})\n");
    let report = repo.run(Job::Diff {
        path: "scenes/level.bsn".into(),
        staged: false,
        untracked: false,
    });
    let text = report.text.expect("an unstaged diff");
    assert!(text.contains("-(entities: {})"), "got: {text}");
    assert!(text.contains("+(entities: {edited})"), "got: {text}");

    // Staged.
    repo.ok(Job::Stage(vec!["scenes/level.bsn".into()]));
    let report = repo.run(Job::Diff {
        path: "scenes/level.bsn".into(),
        staged: true,
        untracked: false,
    });
    let text = report.text.expect("a staged diff");
    assert!(text.contains("+(entities: {edited})"), "got: {text}");

    // Untracked: the whole file, as an addition.
    repo.write("brand-new.txt", "line one\nline two\n");
    let report = repo.run(Job::Diff {
        path: "brand-new.txt".into(),
        staged: false,
        untracked: true,
    });
    let text = report.text.expect("an untracked diff");
    assert!(text.contains("+line one"), "got: {text}");
    assert!(text.contains("+line two"), "got: {text}");
}

#[test]
fn a_commits_own_diff_can_be_read_back() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("show");
    repo.write("added.txt", "hello\n");
    repo.ok(Job::Stage(vec!["added.txt".into()]));
    let snapshot = repo.ok(Job::Commit {
        message: "add a greeting".into(),
        amend: false,
    });

    let report = repo.run(Job::Show(snapshot.log[0].oid.clone()));
    let text = report.text.expect("a commit diff");
    assert!(text.contains("add a greeting"), "got: {text}");
    assert!(text.contains("+hello"), "got: {text}");
}

#[test]
fn amending_replaces_the_last_commit_rather_than_adding_one() {
    if !git_present() {
        return;
    }
    let repo = with_one_commit("amend");
    let before = repo.snapshot().log.len();

    let snapshot = repo.ok(Job::Commit {
        message: "first, reworded".into(),
        amend: true,
    });
    assert_eq!(snapshot.log.len(), before, "amending must not add a commit");
    assert_eq!(snapshot.log[0].subject, "first, reworded");
}

/// A path outside any repository must be reported as such, not crash and not be
/// mistaken for an empty repo.
#[test]
fn a_directory_outside_a_repository_reads_as_no_repository() {
    if !git_present() {
        return;
    }
    let dir = Repo::bare_dir("not-a-repo");
    // `job::snapshot` on a non-repo fails; that failure is what the panel turns
    // into its "Initialize Repository" state.
    assert!(job::snapshot(&dir.path).is_err());
}
