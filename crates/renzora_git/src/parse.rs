//! Parsers for the machine-readable output of the `git` commands in [`crate::cli`].
//!
//! Everything here is a pure function over a `&str`, which is deliberate: this is
//! the only part of the feature that can be wrong in a way no amount of clicking
//! reveals, so it is the part that carries the tests. The format strings the
//! parsers expect live beside them ([`STATUS_ARGS`], [`LOG_FORMAT`],
//! [`REF_FORMAT`]) rather than in the caller, because a parser and the flags that
//! produce its input are one decision and drift apart the moment they're split.
//!
//! # Why the NUL-separated forms
//!
//! Every one of these could be read in a friendlier format, and each friendlier
//! format is wrong for at least one real path. `git status --porcelain=v1` and
//! plain `git log` quote and escape paths with non-ASCII or special characters,
//! so a parser has to un-quote them — and get the escaping rules exactly right,
//! for paths a game project genuinely has (`assets/café/…`). Asking for `-z`
//! instead makes git emit paths verbatim and separate records with NUL, a byte no
//! path can contain. Nothing needs un-escaping because nothing was escaped.
//!
//! The trade is that field counts become load-bearing: with a raw path in the
//! record, "split on spaces and take the last field" breaks on
//! `assets/my level.bsn`. So the entry parsers use `splitn` with the *exact*
//! field count of each record shape, which leaves any spaces in the trailing path
//! untouched. Those counts, and the record shapes below, were read off the real
//! `git` binary (2.54) rather than from the docs — including the two that are
//! easy to get wrong: a rename entry stores its old path as a *separate* NUL
//! record after the new one, and an unmerged entry carries four mode fields where
//! an ordinary one carries three.

/// Args for the status read, paired with [`parse_status`].
///
/// `--branch` adds the `# branch.*` headers (without it there is no way to know
/// the branch, or how far ahead/behind it is). `-z` is what makes paths verbatim.
pub const STATUS_ARGS: &[&str] = &["status", "--porcelain=v2", "--branch", "-z"];

/// `git log` format paired with [`parse_log`]. Fields are separated by US
/// (`\x1f`) and commits by NUL (`git log -z`) — neither can appear in a subject,
/// a name or a ref, so no field needs escaping or quoting.
pub const LOG_FORMAT: &str = "%H%x1f%h%x1f%an%x1f%at%x1f%D%x1f%P%x1f%s";

/// `git for-each-ref` format paired with [`parse_refs`]. Fields are NUL, records
/// newline — a ref name can contain neither.
pub const REF_FORMAT: &str = "%(refname)%00%(objectname:short)%00%(upstream:short)%00%(HEAD)";

// ── Status ───────────────────────────────────────────────────────────────────

/// What happened to one file, in one of the two columns git reports.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Change {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    TypeChanged,
    Untracked,
    /// A merge left this file with conflict markers.
    Conflicted,
}

impl Change {
    /// The single letter git itself uses, for the badge on a file row.
    pub fn letter(self) -> char {
        match self {
            Change::Added => 'A',
            Change::Modified => 'M',
            Change::Deleted => 'D',
            Change::Renamed => 'R',
            Change::Copied => 'C',
            Change::TypeChanged => 'T',
            Change::Untracked => 'U',
            Change::Conflicted => '!',
        }
    }

    /// Human name, for tooltips and confirmation prompts.
    pub fn label(self) -> &'static str {
        match self {
            Change::Added => "added",
            Change::Modified => "modified",
            Change::Deleted => "deleted",
            Change::Renamed => "renamed",
            Change::Copied => "copied",
            Change::TypeChanged => "type changed",
            Change::Untracked => "untracked",
            Change::Conflicted => "conflicted",
        }
    }

    /// Decode one column of a porcelain-v2 `XY` pair. `.` means "unchanged in
    /// this column", which is an absence rather than a kind of change.
    fn from_code(c: u8) -> Option<Change> {
        match c {
            b'A' => Some(Change::Added),
            b'M' => Some(Change::Modified),
            b'D' => Some(Change::Deleted),
            b'R' => Some(Change::Renamed),
            b'C' => Some(Change::Copied),
            b'T' => Some(Change::TypeChanged),
            _ => None,
        }
    }
}

/// One changed path. A file can appear in both columns at once — stage an edit,
/// then edit it again, and it is `M` staged *and* `M` unstaged — so this is one
/// entry with two independent slots rather than two entries.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StatusEntry {
    /// Repo-relative, forward slashes, exactly as git wrote it. For an untracked
    /// *directory* git collapses the contents and this ends in `/`.
    pub path: String,
    /// Where a rename came from, for `R` entries.
    pub orig_path: Option<String>,
    /// HEAD → index: what committing right now would record.
    pub index: Option<Change>,
    /// Index → working tree: what is edited but not staged.
    pub worktree: Option<Change>,
}

impl StatusEntry {
    pub fn is_staged(&self) -> bool {
        self.index.is_some()
    }

    /// Unstaged *or* conflicted. A conflict is reported in the worktree slot
    /// because that is where the markers are and where the work has to happen.
    pub fn is_unstaged(&self) -> bool {
        self.worktree.is_some()
    }

    pub fn is_conflicted(&self) -> bool {
        self.worktree == Some(Change::Conflicted) || self.index == Some(Change::Conflicted)
    }

    /// `true` for a path git has never tracked — the one case where "discard"
    /// means *delete the file* rather than *restore it*, so the distinction has
    /// to survive all the way to the confirmation prompt.
    pub fn is_untracked(&self) -> bool {
        self.worktree == Some(Change::Untracked)
    }

    /// Display name: the trailing path segment, or the whole path for an
    /// untracked directory (whose trailing segment is empty).
    pub fn file_name(&self) -> &str {
        let trimmed = self.path.trim_end_matches('/');
        match trimmed.rsplit_once('/') {
            Some((_, name)) if !name.is_empty() => name,
            _ => trimmed,
        }
    }

    /// The directory the path sits in, `""` at the repo root.
    pub fn dir(&self) -> &str {
        let trimmed = self.path.trim_end_matches('/');
        trimmed.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
    }
}

/// What HEAD points at.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum Head {
    /// On a branch, by name.
    Branch(String),
    /// Detached — sitting directly on a commit. Committing here is a good way to
    /// lose work, so the panel says so loudly.
    Detached,
    /// No HEAD to speak of: the status could not be read at all. The default,
    /// so a `RepoStatus` that was never populated cannot claim to be on a branch.
    #[default]
    Unknown,
}

/// A parsed `git status`.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct RepoStatus {
    pub head: Head,
    /// Full HEAD oid, or `None` in a repo with no commits yet.
    pub oid: Option<String>,
    /// `true` for a freshly-`git init`ed repo: the branch exists but has no
    /// commits, so there is nothing to diff against, log, or reset to. Several
    /// commands fail outright in this state rather than returning empty.
    pub unborn: bool,
    /// The configured upstream (e.g. `origin/main`), if the branch has one. When
    /// this is `None`, a push has to create it (`--set-upstream`).
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub entries: Vec<StatusEntry>,
}

impl RepoStatus {
    /// The branch name, or `None` when detached / unknown.
    pub fn branch(&self) -> Option<&str> {
        match &self.head {
            Head::Branch(b) => Some(b.as_str()),
            _ => None,
        }
    }

    /// Short label for the branch chip: the branch name, or an abbreviated oid
    /// when detached.
    pub fn head_label(&self) -> String {
        match &self.head {
            Head::Branch(b) => b.clone(),
            Head::Detached => match &self.oid {
                Some(oid) => format!("detached @ {}", &oid[..oid.len().min(8)]),
                None => "detached".to_string(),
            },
            Head::Unknown => "unknown".to_string(),
        }
    }

    pub fn staged(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| e.is_staged())
    }

    pub fn unstaged(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| e.is_unstaged())
    }

    pub fn conflicts(&self) -> impl Iterator<Item = &StatusEntry> {
        self.entries.iter().filter(|e| e.is_conflicted())
    }

    pub fn has_conflicts(&self) -> bool {
        self.entries.iter().any(|e| e.is_conflicted())
    }

    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Parse `git status --porcelain=v2 --branch -z`.
///
/// Unrecognised records are skipped rather than treated as errors: git may add
/// header kinds, and a panel that refuses to show a status because it met an
/// unfamiliar `# branch.something` line would be worse than one that shows the
/// rest.
pub fn parse_status(raw: &str) -> RepoStatus {
    let mut out = RepoStatus::default();
    // A trailing NUL leaves an empty final record; skipping empties also keeps a
    // stray separator from being read as a record.
    let mut records = raw.split('\0').filter(|r| !r.is_empty());

    while let Some(rec) = records.next() {
        if let Some(rest) = rec.strip_prefix("# ") {
            parse_header(rest, &mut out);
        } else if let Some(rest) = rec.strip_prefix("1 ") {
            out.entries.extend(parse_ordinary(rest));
        } else if let Some(rest) = rec.strip_prefix("2 ") {
            // A rename's OLD path is its own record, immediately after this one.
            // Consuming it here is what keeps it from being parsed as an entry.
            let orig = records.next().map(str::to_string);
            out.entries.extend(parse_renamed(rest, orig));
        } else if let Some(rest) = rec.strip_prefix("u ") {
            out.entries.extend(parse_unmerged(rest));
        } else if let Some(path) = rec.strip_prefix("? ") {
            out.entries.push(StatusEntry {
                path: path.to_string(),
                orig_path: None,
                index: None,
                worktree: Some(Change::Untracked),
            });
        }
        // `! ` (ignored) never appears: `--ignored` is not requested.
    }

    out.entries.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn parse_header(rest: &str, out: &mut RepoStatus) {
    let Some((key, value)) = rest.split_once(' ') else {
        return;
    };
    match key {
        "branch.oid" => {
            // A repo with no commits reports the literal `(initial)`.
            if value == "(initial)" {
                out.unborn = true;
            } else {
                out.oid = Some(value.to_string());
            }
        }
        "branch.head" => {
            out.head = if value == "(detached)" {
                Head::Detached
            } else {
                Head::Branch(value.to_string())
            };
        }
        "branch.upstream" => out.upstream = Some(value.to_string()),
        "branch.ab" => {
            // `+N -M`. Absent entirely when there is no upstream.
            for part in value.split_whitespace() {
                match part.split_at(1) {
                    ("+", n) => out.ahead = n.parse().unwrap_or(0),
                    ("-", n) => out.behind = n.parse().unwrap_or(0),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// Split the `XY` pair into (index, worktree) changes.
fn split_xy(xy: &str) -> (Option<Change>, Option<Change>) {
    let b = xy.as_bytes();
    let x = b.first().copied().and_then(Change::from_code);
    let y = b.get(1).copied().and_then(Change::from_code);
    (x, y)
}

/// `1 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>` — 8 fields, path last.
fn parse_ordinary(rest: &str) -> Option<StatusEntry> {
    let mut f = rest.splitn(8, ' ');
    let xy = f.next()?;
    // sub, mH, mI, mW, hH, hI — carried for shape only.
    for _ in 0..6 {
        f.next()?;
    }
    let path = f.next()?;
    let (index, worktree) = split_xy(xy);
    Some(StatusEntry {
        path: path.to_string(),
        orig_path: None,
        index,
        worktree,
    })
}

/// `2 <XY> <sub> <mH> <mI> <mW> <hH> <hI> <Xscore> <path>` — 9 fields; the old
/// path arrived as the following NUL record and is passed in as `orig`.
fn parse_renamed(rest: &str, orig: Option<String>) -> Option<StatusEntry> {
    let mut f = rest.splitn(9, ' ');
    let xy = f.next()?;
    // sub, mH, mI, mW, hH, hI, Xscore.
    for _ in 0..7 {
        f.next()?;
    }
    let path = f.next()?;
    let (index, worktree) = split_xy(xy);
    Some(StatusEntry {
        path: path.to_string(),
        orig_path: orig,
        index,
        worktree,
    })
}

/// `u <XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>` — 10 fields. Note the
/// *four* mode fields (one per merge stage plus the worktree), where an ordinary
/// entry has three; counting three here would take a mode for the path.
fn parse_unmerged(rest: &str) -> Option<StatusEntry> {
    let mut f = rest.splitn(10, ' ');
    // XY here describes which side changed (`UU`, `AA`, `DU`, …). Which one it is
    // does not change what the user must do — resolve the file — so it collapses
    // to `Conflicted` rather than being decoded into eight near-identical states.
    f.next()?;
    for _ in 0..8 {
        f.next()?;
    }
    let path = f.next()?;
    Some(StatusEntry {
        path: path.to_string(),
        orig_path: None,
        index: None,
        worktree: Some(Change::Conflicted),
    })
}

// ── Log ──────────────────────────────────────────────────────────────────────

/// One commit from the history list.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Commit {
    pub oid: String,
    pub short: String,
    pub author: String,
    /// Author date, Unix seconds.
    pub timestamp: i64,
    /// Refs pointing here (`main`, `origin/main`, tags), already stripped of the
    /// `HEAD -> ` prefix git puts on the checked-out one.
    pub refs: Vec<String>,
    pub parents: Vec<String>,
    pub subject: String,
    /// This commit is where HEAD is.
    pub is_head: bool,
}

impl Commit {
    /// A merge has more than one parent. Worth showing, because reverting one is
    /// a different (and much more surprising) operation than reverting a normal
    /// commit — `git revert` refuses without `-m`.
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }
}

/// Parse `git log -z --format=`[`LOG_FORMAT`].
pub fn parse_log(raw: &str) -> Vec<Commit> {
    raw.split('\0')
        .filter(|r| !r.is_empty())
        .filter_map(|rec| {
            let mut f = rec.split('\x1f');
            let oid = f.next()?.to_string();
            let short = f.next()?.to_string();
            let author = f.next()?.to_string();
            let timestamp = f.next()?.trim().parse().unwrap_or(0);
            let decoration = f.next()?;
            let parents = f.next()?;
            // The subject is last and may itself contain anything except NUL and
            // US, so take the remainder rather than one more field.
            let subject = f.next()?.to_string();

            let mut is_head = false;
            let refs: Vec<String> = decoration
                .split(", ")
                .filter(|r| !r.is_empty())
                .map(|r| match r.strip_prefix("HEAD -> ") {
                    Some(branch) => {
                        is_head = true;
                        branch.to_string()
                    }
                    // Bare `HEAD` (detached) is a position, not a ref worth
                    // listing as a chip, but it still marks this commit.
                    None if r == "HEAD" => {
                        is_head = true;
                        r.to_string()
                    }
                    None => r.to_string(),
                })
                .collect();

            Some(Commit {
                oid,
                short,
                author,
                timestamp,
                refs,
                parents: parents.split_whitespace().map(str::to_string).collect(),
                subject,
                is_head,
            })
        })
        .collect()
}

// ── Refs ─────────────────────────────────────────────────────────────────────

/// A local or remote-tracking branch.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BranchRef {
    /// `refs/heads/main`, `refs/remotes/origin/main`.
    pub full: String,
    /// The name to hand back to git: `main`, `origin/main`.
    pub name: String,
    pub short_oid: String,
    pub upstream: Option<String>,
    /// This is the checked-out branch.
    pub is_head: bool,
    /// A remote-tracking ref. Checking one out detaches HEAD, so the panel
    /// offers "create local branch from here" instead of a plain switch.
    pub remote: bool,
}

/// Parse `git for-each-ref --format=`[`REF_FORMAT`]` refs/heads refs/remotes`.
///
/// `refs/remotes/<remote>/HEAD` is dropped: it is a symbolic alias for whatever
/// the remote's default branch is, so listing it shows the same branch twice
/// under two names, one of which cannot be checked out meaningfully.
pub fn parse_refs(raw: &str) -> Vec<BranchRef> {
    raw.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut f = line.split('\0');
            let full = f.next()?.to_string();
            let short_oid = f.next()?.to_string();
            let upstream = f.next()?;
            let head = f.next().unwrap_or("");

            let (name, remote) = if let Some(n) = full.strip_prefix("refs/heads/") {
                (n.to_string(), false)
            } else if let Some(n) = full.strip_prefix("refs/remotes/") {
                if n.ends_with("/HEAD") {
                    return None;
                }
                (n.to_string(), true)
            } else {
                return None;
            };

            Some(BranchRef {
                full,
                name,
                short_oid,
                upstream: (!upstream.is_empty()).then(|| upstream.to_string()),
                is_head: head.trim() == "*",
                remote,
            })
        })
        .collect()
}

// ── Formatting ───────────────────────────────────────────────────────────────

/// Coarse "how long ago", for a history row.
///
/// Deliberately one unit and never negative-looking: a commit whose author date
/// is slightly in the future (clock skew between machines, which is normal in a
/// shared repo) reads as "just now" rather than "in -3 minutes".
pub fn relative_time(now: i64, then: i64) -> String {
    let secs = now.saturating_sub(then);
    if secs <= 45 {
        return "just now".to_string();
    }
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    // Averaged, because a calendar month is not a fixed length and this is a
    // one-glance label, not a date.
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;
    let (n, unit) = match secs {
        s if s < HOUR => (s / MINUTE, "minute"),
        s if s < DAY => (s / HOUR, "hour"),
        s if s < MONTH => (s / DAY, "day"),
        s if s < YEAR => (s / MONTH, "month"),
        s => (s / YEAR, "year"),
    };
    let n = n.max(1);
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

/// Truncate to `max` characters (not bytes — a `&str[..n]` split mid-codepoint
/// panics, and commit subjects are full of non-ASCII), appending an ellipsis.
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the NUL-separated record stream git actually emits, so the tests
    /// exercise the same splitting the real output goes through.
    fn nul(records: &[&str]) -> String {
        let mut s = String::new();
        for r in records {
            s.push_str(r);
            s.push('\0');
        }
        s
    }

    // ── status: headers ──────────────────────────────────────────────────────

    #[test]
    fn a_clean_branch_with_an_upstream_parses_its_tracking_counts() {
        let s = parse_status(&nul(&[
            "# branch.oid d96d8527bc70a9f84c0d944225aed44f776dcac0",
            "# branch.head main",
            "# branch.upstream origin/main",
            "# branch.ab +3 -2",
        ]));
        assert_eq!(s.head, Head::Branch("main".into()));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (3, 2));
        assert!(s.is_clean());
        assert!(!s.unborn);
    }

    /// A branch with no upstream emits neither `branch.upstream` nor
    /// `branch.ab`. Defaulting ahead/behind to 0 is what lets the push button
    /// decide it needs `--set-upstream` from `upstream.is_none()` alone.
    #[test]
    fn a_branch_without_an_upstream_has_no_tracking_counts() {
        let s = parse_status(&nul(&["# branch.oid abc123", "# branch.head feature"]));
        assert_eq!(s.head, Head::Branch("feature".into()));
        assert_eq!(s.upstream, None);
        assert_eq!((s.ahead, s.behind), (0, 0));
    }

    /// A fresh `git init` reports `(initial)` where an oid goes. Every
    /// history/diff command fails in that state, so it must be detectable
    /// without running one.
    #[test]
    fn a_repo_with_no_commits_is_unborn_and_has_no_oid() {
        let s = parse_status(&nul(&["# branch.oid (initial)", "# branch.head main"]));
        assert!(s.unborn);
        assert_eq!(s.oid, None);
        assert_eq!(s.head, Head::Branch("main".into()));
    }

    #[test]
    fn a_detached_head_is_not_read_as_a_branch_named_detached() {
        let s = parse_status(&nul(&[
            "# branch.oid fa79d3ef0c61dbac08f8808c99d0ca271ac6ca50",
            "# branch.head (detached)",
        ]));
        assert_eq!(s.head, Head::Detached);
        assert_eq!(s.branch(), None);
        assert_eq!(s.head_label(), "detached @ fa79d3ef");
    }

    #[test]
    fn an_unrecognised_header_is_skipped_rather_than_failing_the_parse() {
        let s = parse_status(&nul(&[
            "# branch.head main",
            "# branch.somethingnew whatever",
            "1 .M N... 100644 100644 100644 aaa bbb file.txt",
        ]));
        assert_eq!(s.head, Head::Branch("main".into()));
        assert_eq!(s.entries.len(), 1);
    }

    // ── status: entries ──────────────────────────────────────────────────────

    /// The two `XY` columns are independent: `.M` is unstaged-only, `M.` is
    /// staged-only, `MM` is both. Collapsing them loses the distinction the
    /// stage/unstage buttons act on.
    #[test]
    fn the_two_status_columns_map_to_staged_and_unstaged_independently() {
        let s = parse_status(&nul(&[
            "1 .M N... 100644 100644 100644 aaa bbb unstaged.txt",
            "1 M. N... 100644 100644 100644 aaa bbb staged.txt",
            "1 MM N... 100644 100644 100644 aaa bbb both.txt",
        ]));
        let by = |name: &str| s.entries.iter().find(|e| e.path == name).unwrap().clone();

        let u = by("unstaged.txt");
        assert_eq!((u.index, u.worktree), (None, Some(Change::Modified)));
        assert!(u.is_unstaged() && !u.is_staged());

        let st = by("staged.txt");
        assert_eq!((st.index, st.worktree), (Some(Change::Modified), None));
        assert!(st.is_staged() && !st.is_unstaged());

        let b = by("both.txt");
        assert!(b.is_staged() && b.is_unstaged());
    }

    /// A rename's old path is a separate NUL record. If it were not consumed as
    /// part of the rename it would be parsed as its own entry, so the panel
    /// would list a phantom unchanged file with the pre-rename name.
    #[test]
    fn a_rename_takes_its_old_path_from_the_following_record() {
        let s = parse_status(&nul(&[
            "# branch.head main",
            "2 R. N... 100644 100644 100644 ce01 ce01 R100 renamed.txt",
            "a.txt",
        ]));
        assert_eq!(s.entries.len(), 1, "the old path must not become an entry");
        let e = &s.entries[0];
        assert_eq!(e.path, "renamed.txt");
        assert_eq!(e.orig_path.as_deref(), Some("a.txt"));
        assert_eq!(e.index, Some(Change::Renamed));
    }

    /// A renamed file can also be edited afterwards (`RM`), which is the case
    /// where miscounting the rename's fields shows up as a path of `R100`.
    #[test]
    fn a_renamed_then_edited_file_keeps_both_columns_and_the_real_path() {
        let s = parse_status(&nul(&[
            "2 RM N... 100644 100644 100644 ce01 ce01 R100 renamed.txt",
            "a.txt",
        ]));
        let e = &s.entries[0];
        assert_eq!(e.path, "renamed.txt");
        assert_eq!(e.index, Some(Change::Renamed));
        assert_eq!(e.worktree, Some(Change::Modified));
    }

    /// An unmerged entry has FOUR mode fields where an ordinary one has three.
    /// Counting three would return a mode as the path — and the resulting
    /// "conflicted file `100644`" is exactly the sort of thing that then gets
    /// passed to `git checkout --`.
    #[test]
    fn an_unmerged_entry_accounts_for_its_extra_mode_field() {
        let s = parse_status(&nul(&[
            "# branch.head main",
            "u UU N... 100644 100644 100644 100644 df96 ba29 2299 c.txt",
        ]));
        assert_eq!(s.entries.len(), 1);
        let e = &s.entries[0];
        assert_eq!(e.path, "c.txt");
        assert!(e.is_conflicted());
        assert!(s.has_conflicts());
        assert_eq!(s.conflicts().count(), 1);
    }

    #[test]
    fn untracked_paths_are_reported_as_untracked_not_added() {
        let s = parse_status(&nul(&["? new.txt", "? some/dir/"]));
        assert_eq!(s.entries.len(), 2);
        for e in &s.entries {
            assert_eq!(e.worktree, Some(Change::Untracked));
            assert!(e.is_untracked());
            // Untracked is not staged — nothing about it is in the index yet.
            assert!(!e.is_staged());
        }
    }

    /// The whole reason for `-z`: a raw path with spaces in it. Splitting the
    /// record on every space would truncate this to `assets/my`.
    #[test]
    fn a_path_containing_spaces_survives_intact() {
        let s = parse_status(&nul(&[
            "1 .M N... 100644 100644 100644 aaa bbb assets/my level 2.bsn",
        ]));
        assert_eq!(s.entries[0].path, "assets/my level 2.bsn");
    }

    #[test]
    fn a_path_containing_non_ascii_is_not_escaped_or_mangled() {
        let s = parse_status(&nul(&[
            "1 .M N... 100644 100644 100644 aaa bbb assets/café/ñ.png",
        ]));
        assert_eq!(s.entries[0].path, "assets/café/ñ.png");
    }

    #[test]
    fn entries_are_sorted_by_path_so_the_list_does_not_reshuffle() {
        let s = parse_status(&nul(&[
            "? zebra.txt",
            "1 .M N... 100644 100644 100644 a b middle.txt",
            "? alpha.txt",
        ]));
        let paths: Vec<&str> = s.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["alpha.txt", "middle.txt", "zebra.txt"]);
    }

    #[test]
    fn empty_output_parses_to_an_unknown_head_rather_than_panicking() {
        let s = parse_status("");
        assert_eq!(s.head, Head::Unknown);
        assert!(s.is_clean());
    }

    /// A truncated record (a killed `git`, a partial pipe read) must drop that
    /// entry, not take a mode field as a path.
    #[test]
    fn a_truncated_entry_record_is_dropped() {
        let s = parse_status(&nul(&["# branch.head main", "1 .M N... 100644"]));
        assert!(s.entries.is_empty());
        assert_eq!(s.head, Head::Branch("main".into()));
    }

    // ── entry display helpers ────────────────────────────────────────────────

    #[test]
    fn a_file_name_is_the_last_segment_and_a_directory_keeps_its_own() {
        let e = |p: &str| StatusEntry {
            path: p.to_string(),
            orig_path: None,
            index: None,
            worktree: None,
        };
        assert_eq!(e("assets/tex/foo.png").file_name(), "foo.png");
        assert_eq!(e("assets/tex/foo.png").dir(), "assets/tex");
        assert_eq!(e("root.txt").file_name(), "root.txt");
        assert_eq!(e("root.txt").dir(), "");
        // Collapsed untracked directory: the trailing slash must not produce an
        // empty name.
        assert_eq!(e("crates/renzora_collab/").file_name(), "renzora_collab");
        assert_eq!(e("crates/renzora_collab/").dir(), "crates");
    }

    // ── log ──────────────────────────────────────────────────────────────────

    fn log_record(fields: &[&str]) -> String {
        format!("{}\0", fields.join("\x1f"))
    }

    #[test]
    fn a_commit_parses_every_field_it_displays() {
        let raw = log_record(&[
            "d96d8527bc70a9f84c0d944225aed44f776dcac0",
            "d96d8527",
            "Renzora Engine",
            "1786928257",
            "HEAD -> main, origin/main, tag: v1",
            "2f2a35e0 cc8e6cf7",
            "Merge pull request #89 from saki2fifty/fix/object-snapping",
        ]);
        let commits = parse_log(&raw);
        assert_eq!(commits.len(), 1);
        let c = &commits[0];
        assert_eq!(c.oid, "d96d8527bc70a9f84c0d944225aed44f776dcac0");
        assert_eq!(c.short, "d96d8527");
        assert_eq!(c.author, "Renzora Engine");
        assert_eq!(c.timestamp, 1786928257);
        assert_eq!(c.parents, vec!["2f2a35e0", "cc8e6cf7"]);
        assert_eq!(c.subject, "Merge pull request #89 from saki2fifty/fix/object-snapping");
        assert!(c.is_merge(), "two parents is a merge");
    }

    /// `%D` writes the checked-out branch as `HEAD -> main`. Showing that
    /// verbatim as a ref chip is wrong twice: the arrow is noise, and `main`
    /// would be missing from the ref list.
    #[test]
    fn the_head_decoration_marks_the_commit_and_yields_a_bare_branch_name() {
        let c = &parse_log(&log_record(&[
            "aaa", "aaa", "A", "1", "HEAD -> main, origin/main", "", "subject",
        ]))[0];
        assert!(c.is_head);
        assert_eq!(c.refs, vec!["main", "origin/main"]);
    }

    /// Detached HEAD decorates as a bare `HEAD`, which still has to mark the
    /// row — that is the only way the panel can show where you are.
    #[test]
    fn a_detached_head_decoration_still_marks_the_commit() {
        let c = &parse_log(&log_record(&["aaa", "aaa", "A", "1", "HEAD", "", "s"]))[0];
        assert!(c.is_head);
    }

    #[test]
    fn an_undecorated_root_commit_has_no_refs_no_parents_and_is_not_a_merge() {
        let c = &parse_log(&log_record(&["aaa", "aaa", "A", "1", "", "", "first"]))[0];
        assert!(c.refs.is_empty());
        assert!(c.parents.is_empty());
        assert!(!c.is_merge());
        assert!(!c.is_head);
    }

    /// A subject can contain anything a shell or a commit template allows,
    /// including the `, ` that separates refs and the spaces that separate
    /// parents. It is the last field precisely so none of that matters.
    #[test]
    fn a_subject_containing_separators_is_not_split() {
        let subject = "fix: a, b and c — see #12";
        let c = &parse_log(&log_record(&["a", "a", "A", "1", "", "", subject]))[0];
        assert_eq!(c.subject, subject);
    }

    #[test]
    fn multiple_commits_parse_in_order() {
        let raw = format!(
            "{}{}",
            log_record(&["a", "a", "A", "2", "", "b", "newer"]),
            log_record(&["b", "b", "B", "1", "", "", "older"])
        );
        let commits = parse_log(&raw);
        assert_eq!(
            commits.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
    }

    #[test]
    fn an_empty_log_is_an_empty_list() {
        assert!(parse_log("").is_empty());
    }

    #[test]
    fn a_truncated_commit_record_is_dropped_rather_than_half_parsed() {
        assert!(parse_log("aaa\x1fbbb\0").is_empty());
    }

    // ── refs ─────────────────────────────────────────────────────────────────

    #[test]
    fn local_and_remote_branches_are_distinguished_and_shortened() {
        let raw = "refs/heads/main\0d96d8527\0origin/main\0*\n\
                   refs/heads/feature\0c98c2df9\0\0 \n\
                   refs/remotes/origin/main\0d96d8527\0\0 \n";
        let refs = parse_refs(raw);
        assert_eq!(refs.len(), 3);

        let main = &refs[0];
        assert_eq!(main.name, "main");
        assert_eq!(main.upstream.as_deref(), Some("origin/main"));
        assert!(main.is_head);
        assert!(!main.remote);

        let feature = &refs[1];
        assert_eq!(feature.name, "feature");
        assert_eq!(feature.upstream, None, "an empty field is no upstream");
        assert!(!feature.is_head);

        let remote = &refs[2];
        assert_eq!(remote.name, "origin/main", "remotes keep their remote prefix");
        assert!(remote.remote);
        assert!(!remote.is_head);
    }

    /// `refs/remotes/origin/HEAD` is a symbolic alias for the remote's default
    /// branch. Listed, it duplicates that branch under a name that cannot be
    /// checked out meaningfully.
    #[test]
    fn the_remote_head_alias_is_not_listed_as_a_branch() {
        let refs = parse_refs("refs/remotes/origin/HEAD\0d96d8527\0\0 \n");
        assert!(refs.is_empty());
    }

    /// A branch legitimately named `.../HEAD-something` must not be caught by
    /// the alias filter.
    #[test]
    fn only_an_exact_head_alias_is_filtered() {
        let refs = parse_refs("refs/remotes/origin/HEADROOM\0aaa\0\0 \n");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "origin/HEADROOM");
    }

    #[test]
    fn a_branch_name_containing_slashes_keeps_them() {
        let refs = parse_refs("refs/heads/fix/object-snapping\0c98c2df9\0\0 \n");
        assert_eq!(refs[0].name, "fix/object-snapping");
    }

    #[test]
    fn a_tag_ref_is_not_mistaken_for_a_branch() {
        assert!(parse_refs("refs/tags/v1.0\0aaa\0\0 \n").is_empty());
    }

    // ── formatting ───────────────────────────────────────────────────────────

    #[test]
    fn relative_time_picks_one_coarse_unit() {
        let now = 1_000_000_000;
        assert_eq!(relative_time(now, now), "just now");
        assert_eq!(relative_time(now, now - 30), "just now");
        assert_eq!(relative_time(now, now - 60), "1 minute ago");
        assert_eq!(relative_time(now, now - 5 * 60), "5 minutes ago");
        assert_eq!(relative_time(now, now - 3 * 3600), "3 hours ago");
        assert_eq!(relative_time(now, now - 2 * 86400), "2 days ago");
        assert_eq!(relative_time(now, now - 40 * 86400), "1 month ago");
        assert_eq!(relative_time(now, now - 400 * 86400), "1 year ago");
    }

    /// Clocks in a shared repo disagree, so a commit can be authored "later"
    /// than the machine reading it. That must not render as a negative age.
    #[test]
    fn a_future_timestamp_reads_as_just_now() {
        let now = 1_000_000_000;
        assert_eq!(relative_time(now, now + 10_000), "just now");
    }

    /// Truncating a subject on a byte boundary panics on any multi-byte
    /// character — and commit subjects are full of them.
    #[test]
    fn truncate_counts_characters_not_bytes() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("ééééééééé", 5), "éééé…");
        // No panic is the assertion here.
        let _ = truncate("日本語のコミットメッセージ", 4);
    }

    #[test]
    fn truncate_at_the_exact_limit_does_not_add_an_ellipsis() {
        assert_eq!(truncate("12345", 5), "12345");
    }
}
