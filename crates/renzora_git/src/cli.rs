//! Every git operation, as a call to the `git` binary.
//!
//! # Why the CLI and not libgit2
//!
//! Linking `git2` would have been the obvious choice and is the wrong one here,
//! for reasons that are all about the half of this feature that talks to a
//! remote. `git` on the user's PATH already resolves their credentials — SSH
//! agent, Windows Credential Manager, the `gh` helper, a `credential.helper` line
//! in a config three levels up — and libgit2 resolves none of it. It hands you a
//! callback and expects you to reimplement the lookup, per platform, per
//! transport. Push and pull are what the user asked for, and the CLI gets them
//! for free while the library makes them the hardest part.
//!
//! The same argument repeats for everything else the binary carries and the
//! library does not: `.gitattributes` filters, Git LFS (a game project's textures
//! are very often behind it), hooks, `commit.gpgsign`, `core.autocrlf`,
//! `include.path`. Any of those silently not applying would make the editor's
//! commits differ from the same user's commits from a terminal, in the same repo.
//!
//! It also costs nothing at build time. `git2` means `libgit2-sys`, a C
//! dependency compiled per target — and this crate has to keep compiling for
//! every platform the editor targets, wasm included (where it installs nothing
//! but must still build).
//!
//! The price is a hard dependency on git being installed, which is why
//! [`version`] exists and why the panel has a first-class "git not found" state
//! instead of a stream of failures.
//!
//! # Two things that are easy to get wrong on Windows
//!
//! Both are handled once, in [`command`], so no call site has to remember:
//!
//! 1. **A console window flashes** for every spawn unless `CREATE_NO_WINDOW` is
//!    set. The panel refreshes on a timer, so this is not a one-off blink; it is
//!    a window popping over the editor every few seconds.
//! 2. **`git` can block forever waiting to be typed at.** With no tty it may
//!    still prompt for a username on stdin and simply never return, hanging the
//!    worker thread for the rest of the session. `GIT_TERMINAL_PROMPT=0` turns
//!    that into a prompt fast failure. Note it does *not* disable credential
//!    helpers, which is the point: Credential Manager still shows its dialog and
//!    a push to a fresh remote still works.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::parse::{self, BranchRef, Commit, RepoStatus};

/// How long a network operation may run before it is killed.
///
/// Unreachable hosts do eventually fail on their own, but "eventually" is a TCP
/// timeout, and an editor that says *Pushing…* for twenty minutes is
/// indistinguishable from one that is broken. The cap is generous because the
/// legitimate slow case is real: a first clone-sized push over a poor link.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);

/// How often to check on a running child while waiting for it.
const POLL: Duration = Duration::from_millis(25);

/// A `git` invocation configured the way every call here needs it.
fn command(repo: Option<&Path>, args: &[&str]) -> Command {
    let mut cmd = Command::new("git");
    if let Some(repo) = repo {
        // `-C` rather than `current_dir`, so the path git reports in its own
        // error messages is the one we asked about.
        cmd.arg("-C").arg(repo);
    }
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // See the module doc: without this, git can wait on stdin forever.
        .env("GIT_TERMINAL_PROMPT", "0")
        // Some helpers escalate to an editor (a merge commit message, a rebase
        // todo). There is no terminal to host one, so refuse rather than hang.
        .env("GIT_EDITOR", "true");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW` — no console window for the child.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd
}

/// Turn a finished `git` into stdout, or its stderr as the error.
///
/// Git writes the useful part of a failure to stderr and exits non-zero, so the
/// message the user sees is git's own — better than anything this crate could
/// paraphrase, and it matches what they'd get in a terminal.
fn finish(out: std::process::Output, args: &[&str]) -> Result<String, String> {
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if !stderr.is_empty() {
        return Err(stderr);
    }
    // Some failures say nothing on stderr and put it on stdout (or say nothing at
    // all). Never return an empty error — a silent failure reads as a hang.
    let stdout = stdout.trim();
    if !stdout.is_empty() {
        return Err(stdout.to_string());
    }
    Err(format!(
        "`git {}` failed with {}",
        args.join(" "),
        out.status
    ))
}

/// Map a spawn failure to something that names the actual problem.
fn spawn_error(e: std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        return "`git` was not found on your PATH.".to_string();
    }
    format!("Could not run `git`: {e}")
}

/// Run a git command to completion. For local operations only — a network one
/// can hang, so it goes through [`run_networked`].
fn run(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = command(Some(repo), args).output().map_err(spawn_error)?;
    finish(out, args)
}

/// A read-only git command.
///
/// `GIT_OPTIONAL_LOCKS=0` stops the read from taking the index lock to refresh
/// its cache. Without it, the panel's periodic `git status` contends with
/// whatever the user is doing in a terminal in the same repo — each side
/// intermittently failing to take a lock the other holds, for no reason, because
/// this side only ever wanted to read.
fn run_read(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = command(Some(repo), args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .map_err(spawn_error)?;
    finish(out, args)
}

/// Run a git command that talks to a remote, killing it at [`NETWORK_TIMEOUT`].
///
/// Polling `try_wait` is what makes the timeout possible — `wait` would block
/// past it — but polling alone is not enough, because the child's pipes have to
/// keep being drained while we wait. A pipe holds tens of kilobytes; a child that
/// fills one blocks writing, forever, and `try_wait` reports it as still running.
/// The result is a hang that looks exactly like the network stall the timeout was
/// added for, and it is reachable: a fetch that prunes many refs, or any git
/// configured to force `--progress`, writes plenty to stderr.
///
/// So each pipe gets a thread that reads it to EOF. Killing the child closes
/// both, the threads finish on their own, and the output is whatever arrived
/// before the kill — which for a timeout is the part worth reporting anyway.
fn run_networked(repo: &Path, args: &[&str]) -> Result<String, String> {
    use std::io::Read;

    let mut child = command(Some(repo), args).spawn().map_err(spawn_error)?;

    // `take` leaves `None` behind, so `wait_with_output` (which would also try to
    // read these) is deliberately not used below.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut buf);
            }
            buf
        })
    };
    let stdout = drain(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let stderr = drain(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() >= NETWORK_TIMEOUT {
                    let _ = child.kill();
                    timed_out = true;
                    // Still wait: the kill is asynchronous, and the reader
                    // threads only reach EOF once the process is gone.
                    break child.wait().map_err(|e| format!("Could not wait for `git`: {e}"))?;
                }
                std::thread::sleep(POLL);
            }
            Err(e) => return Err(format!("Could not wait for `git`: {e}")),
        }
    };

    // A panicked reader thread means no output, not a failed command.
    let out = std::process::Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    };

    if timed_out {
        let detail = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let mut msg = format!(
            "`git {}` did not finish within {}s and was stopped. \
             Check the remote is reachable and your credentials are set up.",
            args.join(" "),
            NETWORK_TIMEOUT.as_secs()
        );
        if !detail.is_empty() {
            msg.push_str("\n\nGit said:\n");
            msg.push_str(&detail);
        }
        return Err(msg);
    }
    finish(out, args)
}

// ── Discovery ────────────────────────────────────────────────────────────────

/// The installed git's version string, or why it could not be run.
///
/// Checked once at startup so "git is not installed" is a state the panel can
/// show, rather than something the user infers from every button failing.
pub fn version() -> Result<String, String> {
    let out = command(None, &["--version"]).output().map_err(spawn_error)?;
    Ok(finish(out, &["--version"])?.trim().to_string())
}

/// The root of the repository containing `dir`, or `None` if it is not in one.
///
/// This is the enclosing repo, which is not always the project folder: a project
/// kept inside a larger checkout resolves to that checkout's root, and every
/// operation then covers the whole thing. That is what git itself would do, but
/// it is surprising enough that the panel displays the root it found rather than
/// assuming it is the project directory.
pub fn discover_root(dir: &Path) -> Option<PathBuf> {
    let out = run_read(dir, &["rev-parse", "--show-toplevel"]).ok()?;
    let line = out.lines().next()?.trim();
    if line.is_empty() {
        return None;
    }
    Some(PathBuf::from(line))
}

/// Create a repository in `dir` (with `main` as the initial branch, rather than
/// whatever this git's compiled-in default is).
pub fn init(dir: &Path) -> Result<String, String> {
    // `--initial-branch` needs git 2.28; fall back for older ones rather than
    // refusing to initialise at all.
    match run(dir, &["init", "--initial-branch=main"]) {
        Ok(s) => Ok(s),
        Err(_) => run(dir, &["init"]),
    }
}

/// The generated caches and build output a renzora project should not commit.
///
/// Written on init because the alternative is worse than it sounds: the first
/// commit of a project that has been opened even once would otherwise include
/// `.cache/thumbnails/`, which is a few thousand generated PNGs. Discovering that
/// after pushing is a rewrite-history problem, and it is entirely avoidable here.
const GITIGNORE: &str = "\
# Generated by the Renzora editor — safe to edit.

# Thumbnail and import caches. Regenerated on demand from the assets themselves.
.cache/
# Legacy thumbnail cache location.
.thumbs/

# Build and export output.
target/
*.rpak

# Crash reports and logs.
last_crash.txt
*.log

# OS and editor cruft.
.DS_Store
Thumbs.db
desktop.ini

# Exported builds usually land outside the project. If you point an export at a
# folder inside it, add that folder here.
";

/// Write a starter `.gitignore` if the project does not already have one.
///
/// Never overwrites: a project that already has one has an author's intent in it,
/// and silently replacing that would be worse than adding nothing. Failure is not
/// propagated — a repository without a `.gitignore` is still a working repository,
/// and failing the whole "Initialize" over it would be disproportionate.
pub fn bootstrap_gitignore(dir: &Path) -> bool {
    let path = dir.join(".gitignore");
    if path.exists() {
        return false;
    }
    std::fs::write(&path, GITIGNORE).is_ok()
}

// ── Reads ────────────────────────────────────────────────────────────────────

pub fn status(root: &Path) -> Result<RepoStatus, String> {
    Ok(parse::parse_status(&run_read(root, parse::STATUS_ARGS)?))
}

/// The last `limit` commits on the current history.
///
/// An unborn repo (fresh `git init`, nothing committed) makes this fail — there
/// is no HEAD to walk — so the caller checks [`RepoStatus::unborn`] first and
/// treats it as an empty history rather than an error.
pub fn log(root: &Path, limit: usize) -> Result<Vec<Commit>, String> {
    let format = format!("--format={}", parse::LOG_FORMAT);
    let limit = format!("-n{limit}");
    let raw = run_read(root, &["log", "-z", &format, &limit])?;
    Ok(parse::parse_log(&raw))
}

pub fn refs(root: &Path) -> Result<Vec<BranchRef>, String> {
    let format = format!("--format={}", parse::REF_FORMAT);
    let raw = run_read(
        root,
        &["for-each-ref", &format, "refs/heads", "refs/remotes"],
    )?;
    Ok(parse::parse_refs(&raw))
}

/// Configured remote names (`origin`, …). Empty means nothing to push to, which
/// is the difference between "Push failed" and "there is no remote yet".
pub fn remotes(root: &Path) -> Result<Vec<String>, String> {
    Ok(run_read(root, &["remote"])?
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// A unified diff for one path.
///
/// `staged` picks which of the two comparisons the panel is showing: the index
/// against HEAD, or the working tree against the index. An untracked file is in
/// neither, so it has no diff git will produce — `--no-index` against `/dev/null`
/// is how its whole content is shown as an addition.
pub fn diff(root: &Path, path: &str, staged: bool, untracked: bool) -> Result<String, String> {
    if untracked {
        // Exits 1 because the two sides differ, which `finish` would read as a
        // failure — so the "error" is the diff. Nothing else writes to stdout on
        // this path, which is what makes reading it back safe.
        let args = ["diff", "--no-index", "--", "/dev/null", path];
        let out = command(Some(root), &args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .output()
            .map_err(spawn_error)?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if !stdout.trim().is_empty() {
            return Ok(stdout);
        }
        return finish(out, &args);
    }
    let mut args = vec!["diff"];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(path);
    run_read(root, &args)
}

/// The diff a single commit introduced, with a summary line per file.
pub fn show(root: &Path, rev: &str) -> Result<String, String> {
    run_read(root, &["show", "--stat", "--patch", rev])
}

// ── Staging ──────────────────────────────────────────────────────────────────

/// Build an argument list ending in `-- <paths>`.
///
/// The `--` is not optional politeness: a path that looks like a flag, or that
/// happens to match a branch name (`assets/main`), is otherwise ambiguous and git
/// resolves it the other way.
fn with_paths<'a>(head: &[&'a str], paths: &'a [String]) -> Vec<&'a str> {
    let mut args: Vec<&str> = head.to_vec();
    args.push("--");
    args.extend(paths.iter().map(String::as_str));
    args
}

pub fn stage(root: &Path, paths: &[String]) -> Result<String, String> {
    // `--all` so a deletion stages as a deletion. Plain `git add <deleted path>`
    // is a no-op in older git, which silently leaves the deletion unstaged.
    run(root, &with_paths(&["add", "--all"], paths))
}

/// Remove paths from the index, leaving the working tree alone.
///
/// `unborn` selects a different command rather than a flag, because with no HEAD
/// there is nothing for `restore --staged` to restore *from* and it fails
/// outright. This is the normal state right after "Initialize Repository", so it
/// is not an edge case the user has to work to reach.
pub fn unstage(root: &Path, paths: &[String], unborn: bool) -> Result<String, String> {
    if unborn {
        return run(root, &with_paths(&["rm", "--cached", "-r", "--quiet"], paths));
    }
    run(root, &with_paths(&["restore", "--staged"], paths))
}

/// Throw away changes to tracked paths — staged, unstaged, or both.
///
/// Irreversible: the discarded content was never committed, so no part of git
/// can bring it back. Every route to here is behind a confirmation.
pub fn discard_tracked(root: &Path, paths: &[String]) -> Result<String, String> {
    run(
        root,
        &with_paths(&["restore", "--staged", "--worktree"], paths),
    )
}

/// Delete untracked paths. `-d` includes directories, which is what an untracked
/// entry usually is (git collapses untracked directories in its status output).
pub fn delete_untracked(root: &Path, paths: &[String]) -> Result<String, String> {
    run(root, &with_paths(&["clean", "-fd"], paths))
}

// ── History ──────────────────────────────────────────────────────────────────

/// Counter making each commit-message file name unique within this process.
///
/// The name used to be the process id alone, which collided the moment two commits
/// overlapped in one process: both wrote the same path, the first one's cleanup
/// deleted it, and the second `git commit` either failed to read it or — worse —
/// committed with the *other* message. Caught by the integration tests, where
/// several commits genuinely do run at once.
static COMMIT_MESSAGE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Commit the index. `amend` replaces the previous commit instead of adding one.
///
/// The message goes through a temporary file rather than `-m`: a multi-line
/// message with blank lines survives intact, and nothing has to be escaped for a
/// shell that isn't involved. `--cleanup=strip` gives it the same treatment
/// git gives a message typed into an editor (trailing whitespace and comment
/// lines removed), so a commit made here looks like one made from a terminal.
pub fn commit(root: &Path, message: &str, amend: bool) -> Result<String, String> {
    // Process id, a per-process counter, and the clock. The counter is what makes
    // two concurrent commits in one process safe; the clock covers the case of a
    // second editor that the OS handed a recycled process id.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let file = std::env::temp_dir().join(format!(
        "renzora-git-commit-{}-{}-{nanos}.txt",
        std::process::id(),
        COMMIT_MESSAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    std::fs::write(&file, message)
        .map_err(|e| format!("Could not write the commit message to a temporary file: {e}"))?;
    let file_arg = file.to_string_lossy().to_string();
    let mut args = vec!["commit", "--cleanup=strip", "--file", file_arg.as_str()];
    if amend {
        args.push("--amend");
    }
    let result = run(root, &args);
    // Best-effort: leaving it behind is harmless, and failing the commit that
    // already succeeded because cleanup did not would be worse.
    let _ = std::fs::remove_file(&file);
    result
}

/// Move HEAD (and optionally the working tree) to `rev`.
///
/// `hard` discards uncommitted work irreversibly; the soft form keeps it, so
/// only the hard one is treated as destructive by the caller.
pub fn reset(root: &Path, rev: &str, hard: bool) -> Result<String, String> {
    let mode = if hard { "--hard" } else { "--mixed" };
    run(root, &["reset", mode, rev])
}

/// Create a commit that undoes `rev`, leaving history intact.
///
/// `--no-edit` because there is no terminal to host the message editor. A merge
/// commit needs `-m 1` to say which parent's changes to undo; without it git
/// refuses rather than guessing, and "revert" on a merge row would just fail.
pub fn revert(root: &Path, rev: &str, is_merge: bool) -> Result<String, String> {
    let mut args = vec!["revert", "--no-edit"];
    if is_merge {
        args.push("-m");
        args.push("1");
    }
    args.push(rev);
    run(root, &args)
}

/// Switch to an existing branch, or check out a commit (which detaches HEAD).
pub fn checkout(root: &Path, rev: &str) -> Result<String, String> {
    run(root, &["checkout", rev])
}

/// Create a branch at `start` (HEAD when `None`) and switch to it.
pub fn create_branch(root: &Path, name: &str, start: Option<&str>) -> Result<String, String> {
    let mut args = vec!["checkout", "-b", name];
    if let Some(start) = start {
        args.push(start);
    }
    run(root, &args)
}

/// Delete a local branch. Without `force`, git refuses when the branch holds
/// commits that are not merged anywhere else — which is the check worth keeping.
pub fn delete_branch(root: &Path, name: &str, force: bool) -> Result<String, String> {
    let flag = if force { "-D" } else { "-d" };
    run(root, &["branch", flag, name])
}

/// Merge `rev` into the current branch.
///
/// `--no-edit` accepts the generated merge message. A conflict is *not* an error
/// to hide: git exits non-zero and leaves the tree in a conflicted state, which
/// the next status read picks up, so the caller reports it as a conflict to
/// resolve rather than a failure that changed nothing.
pub fn merge(root: &Path, rev: &str) -> Result<String, String> {
    run(root, &["merge", "--no-edit", rev])
}

/// Abandon an in-progress merge and return to the pre-merge state.
pub fn merge_abort(root: &Path) -> Result<String, String> {
    run(root, &["merge", "--abort"])
}

// ── Remotes ──────────────────────────────────────────────────────────────────

/// Update remote-tracking refs without touching the working tree. The safe
/// remote operation: it can only ever add information.
pub fn fetch(root: &Path) -> Result<String, String> {
    run_networked(root, &["fetch", "--all", "--prune"])
}

/// Fetch and integrate. `--ff-only` is deliberate: a pull that silently creates
/// a merge commit, or rebases, is a surprise, and both are recoverable-but-
/// confusing from a GUI. If it cannot fast-forward, the caller says so and points
/// at Merge, which is the explicit version of the same thing.
pub fn pull(root: &Path) -> Result<String, String> {
    run_networked(root, &["pull", "--ff-only"])
}

/// Push the current branch. `set_upstream` is for a branch that has none yet —
/// without it git refuses and explains how, which is a worse first push than
/// simply doing it.
pub fn push(root: &Path, set_upstream: bool, branch: Option<&str>) -> Result<String, String> {
    let mut args = vec!["push"];
    if set_upstream {
        args.push("--set-upstream");
        // `origin` by name: a repo with several remotes has no unambiguous
        // default, and the caller only enables this when exactly one exists.
        args.push("origin");
        if let Some(branch) = branch {
            args.push(branch);
        }
    }
    run_networked(root, &args)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--` has to separate the command from the paths, or a path that reads as a
    /// revision (`assets/main`) is resolved as one.
    #[test]
    fn path_arguments_are_separated_from_the_command_by_a_double_dash() {
        let paths = vec!["assets/main".to_string(), "a b.txt".to_string()];
        let args = with_paths(&["add", "--all"], &paths);
        assert_eq!(args, vec!["add", "--all", "--", "assets/main", "a b.txt"]);
    }

    #[test]
    fn a_path_list_with_no_paths_still_ends_in_a_double_dash() {
        let args = with_paths(&["restore", "--staged"], &[]);
        assert_eq!(args, vec!["restore", "--staged", "--"]);
    }
}
