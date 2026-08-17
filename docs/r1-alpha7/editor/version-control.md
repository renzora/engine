# Version Control (Git)

The **Git** panel is a git client for the open project: it tracks the project's history, lets you go back to an earlier version, branch and merge, and push to or pull from a remote — without leaving the editor.

Open it from the Add-Panel `+` picker (category **Version Control**), or `Ctrl+P` → "Git".

## What you need

Git itself, on your `PATH`. The panel runs the real `git` command rather than reimplementing it, which means everything you have already set up keeps working:

- Your credentials — SSH keys, Windows Credential Manager, the `gh` credential helper.
- Your config — `user.name`, `commit.gpgsign`, `core.autocrlf`, `.gitattributes`.
- Git LFS, and any hooks the repository has.

A commit made from this panel is indistinguishable from one made in a terminal, in the same repository, by the same person.

If git is not installed the panel says so and does nothing else. Install it from [git-scm.com](https://git-scm.com/downloads) and restart the editor.

## Starting from nothing

Open the panel in a project that is not version-controlled yet and it offers one button: **Initialize Repository**. That creates the repository on a `main` branch and writes a starter `.gitignore` that excludes the editor's generated caches.

That `.gitignore` matters more than it sounds. Opening a project even once fills `.cache/thumbnails/` with generated images — potentially thousands of files. Without the ignore rules, your first commit would contain all of them, and noticing after you have pushed means rewriting history. The generated file covers:

| Ignored | Why |
|---|---|
| `.cache/`, `.thumbs/` | Thumbnail and import caches, regenerated on demand |
| `target/`, `*.rpak` | Build and export output |
| `last_crash.txt`, `*.log` | Crash reports and logs |
| `.DS_Store`, `Thumbs.db`, `desktop.ini` | OS clutter |

If the project already has a `.gitignore`, it is left exactly as it is.

> **Exports** usually land outside the project. If you point an export at a folder *inside* it, add that folder to `.gitignore` yourself.

## The toolbar

The top row is always where you are and what the remote thinks:

- **The branch name**, or `detached @ <hash>` in amber when you are not on a branch (see [Going back](#going-back)).
- **`↑3 ↓2`** — commits you have that the remote does not, and vice versa. Nothing is shown when you are in sync.
- **Fetch** — update your view of the remote. It changes no files and is always safe.
- **Pull** — bring the remote's commits into your branch.
- **Push** — send yours. On a branch that has never been pushed this also sets up tracking, so a first push just works.
- **Refresh** — re-read the repository now.

The panel re-reads the repository every few seconds while it is visible, so changes made outside the editor show up on their own. While a hidden tab, it reads nothing at all.

Only one git operation runs at a time. While one is in flight the buttons dim and the toolbar says what is happening ("Pushing…").

## Changes — staging and committing

The **Changes** view lists what has changed, in up to three sections:

- **Conflicts**, when a merge stopped — always first, because nothing else can be finished until they are dealt with.
- **Staged** — what committing right now would record.
- **Changes** — edited but not staged.

A file edited *and* staged appears in both, which is not a bug: the two rows do different things. Unstaging the top one keeps your later edit; discarding the bottom one throws it away.

Each row has:

| Control | Does |
|---|---|
| Click the row | Show the diff |
| **+** | Stage this file |
| **−** | Unstage this file |
| **✓** | On a conflicted file: mark it resolved and stage it |
| **↺** | Discard this change — **cannot be undone** |

The letter badge is git's own: `A`dded, `M`odified, `D`eleted, `R`enamed, `U`ntracked, and `!` for a conflict.

Type a message in the box at the bottom and press **Commit**. The button says how many files it will commit. Tick **Amend** to replace the previous commit instead of adding one — useful for fixing a message or a file you forgot.

## History — reading the log

The **History** view lists recent commits, newest first, with the branch and tag labels that point at each one. Click a commit to expand its actions:

| Action | What it does | Reversible? |
|---|---|---|
| **View changes** | Show the diff that commit introduced | — |
| **Branch from here** | Start a branch at this commit and switch to it | Yes |
| **Check out** | Put your files back as they were here, detaching HEAD | Yes |
| **Revert** | Add a new commit undoing this one | Yes |
| **Reset here (keep changes)** | Move the branch back; later work returns as uncommitted changes | Yes |
| **Reset here (discard changes)** | Move the branch back and throw the later work away | **No** for anything uncommitted |

### Going back

There are three ways back, and they differ in what survives:

**Revert** is the safe one. It adds a commit that undoes an old one, so history keeps both and reverting the revert puts it back. Use it for something already shared.

**Check out** shows you an old version without changing any branch. HEAD becomes *detached* — you are on a commit, not a branch — and the toolbar turns amber to say so. Look around, then switch back to a branch to return. A commit made while detached belongs to no branch and is easy to lose, so use **Branch from here** if you want to keep working from that point.

**Reset** moves the branch itself. The "keep changes" form un-commits, returning the work to your working tree. The "discard changes" form deletes it. Committed work can usually still be recovered from `git reflog`; uncommitted work cannot be recovered by anything.

## Branches

The **Branches** view lists local and remote-tracking branches.

- Click a local branch to switch to it.
- Click a **remote** branch to create a local branch that tracks it, and switch to that — checking a remote ref out directly would detach HEAD, which is never what clicking a branch means.
- **Merge** brings another branch's commits into the current one.
- The **trash** icon deletes a local branch. Git refuses if the branch holds commits that are not merged anywhere else, and the panel does not offer to force it — merge it first, or use `git branch -D` in a terminal if you really mean it.

Type a name at the top and press **Create & switch** for a new branch from the current commit.

## Conflicts

When a merge cannot combine both sides it stops, leaves conflict markers in the affected files, and the panel shows a **Conflicts** section.

To resolve one: open the file (the Code panel works), pick what the merged result should be, delete the `<<<<<<<`/`=======`/`>>>>>>>` markers, then press **✓** on its row to mark it resolved. When every conflict is staged, commit.

Or press **Abort merge** to put everything back the way it was before the merge started.

## Pull is fast-forward only

**Pull** refuses when your branch and the remote have both moved on, rather than quietly creating a merge commit or rebasing. If that happens, **Fetch**, then go to Branches and **Merge** `origin/<branch>` — the same result, but you chose it.

## Your open scene follows the working tree

This is the part worth understanding, because it protects you from a real way to lose work.

Checking out a branch, pulling, merging, reverting, resetting or discarding a file can all rewrite `scenes/level.bsn` on disk — while the editor is holding the *previous* version of that scene as live entities. Without anything else happening, the scene on your screen would be a version that exists nowhere else, and your next `Ctrl+S` would write it back over what git just checked out.

So after any operation that can touch your files, the editor checks whether the open scene actually changed on disk and reloads it if so, telling you it did. Two consequences:

- **Save before switching branches.** Unsaved editor changes to a scene are lost when it reloads — git never knew about them, so the reload has nothing to preserve.
- If you check out a revision from before the scene existed, the editor **keeps it open** and warns you rather than closing it, so you can still save it somewhere else.

During play mode, nothing is reloaded — the running game is left alone.

## Confirmations

Operations git cannot undo are behind a red confirmation that names what will be lost: discarding changes, a hard reset, deleting a branch, aborting a merge. Operations git *can* undo — a revert, a soft reset, a checkout — ask too, but in the ordinary colour, because a red button on everything trains you to click through it.

## The status bar

The status bar carries the current branch, the number of changed files, the ahead/behind counts and a conflict warning — so the state is visible without the panel open.

## Not (yet) in the panel

Stashing, rebasing, cherry-picking, tagging, submodules, and editing remotes. Use a terminal for those — the panel picks up whatever you do there on its next refresh. The [Terminal](terminal.md) panel is a convenient place for it.

## See also

- [Panels & Windows](panels-windows.md) — arranging and tearing off panels.
- [Terminal](terminal.md) — a shell inside the editor, for the git commands this panel does not cover.
- [Undo & Redo](undo-redo.md) — undo covers *editor* actions; git covers committed history. They are separate.
- [Collaboration](collaboration.md) — live multi-user editing, which is a different thing from version control.
