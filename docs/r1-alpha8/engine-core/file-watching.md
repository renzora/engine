# File Watching

One watcher over the open project, and one message every crate can read.

When a file under the project changes, `renzora_project_watch` publishes a
`ProjectFileChanged` message. Anything that wants to react to a file being
saved, added, renamed or deleted reads that message. No feature runs its own
watcher, no feature polls, and nothing walks the project on a timer.

## Reading changes

```rust
use bevy::prelude::*;
use renzora::core::project_files::{AssetKind, ProjectFileChanged};

fn reload_my_thing(mut changes: MessageReader<ProjectFileChanged>) {
    for change in changes.read() {
        if change.kind != AssetKind::Particle || !change.is_live() {
            continue;
        }
        info!("reload {}", change.relative);
    }
}
```

Every message carries:

| Field | What it is |
|---|---|
| `change` | `Added`, `Modified`, `Removed`, or `Renamed { from }` |
| `path` | Absolute path, which is what `std::fs` wants |
| `relative` | Project-relative and forward-slashed, the key `ContentProblems`, the asset registry and `AssetServer` all use |
| `kind` | `AssetKind` classification by extension |
| `is_dir` | Whether the path is (or was) a directory |

Two helpers save a repeated mistake. `is_live()` is true for added, modified and
renamed and false for removed, which is the condition nearly every hot-reload
wants; writing it out by hand is how one of them ends up silently not handling
`Added`. `has_extension("rs")` is a case-insensitive extension test.

## What reacts today

| File | What happens |
|---|---|
| `.rs` script | recompiles and swaps the live image |
| `.bsn` scene | the open scene reloads, unless you have unsaved changes |
| `.particle` | every entity using the effect rebuilds |
| `.html` template | every canvas using it rebuilds |
| anything deleted | a warning naming the entities that still reference it |
| textures, models, audio | `AssetServer` reloads them |

The particle and template reloads existed already, wired to editor-initiated
saves. Both were fed from the watcher rather than reimplemented, so an external
edit takes the identical path a save does.

## Recognising your own writes

The watcher cannot tell "someone edited this in another program" from "we just
saved it" — both are a write. Anything that reacts to a change by reloading has
to recognise the echo of its own save, or pressing Ctrl+S makes the editor
reload what it just wrote and throw away selection and undo for nothing.

`SelfWrites` answers that. Record the bytes after writing a file **and after
reading one**, then check before reacting:

```rust
world.get_resource_or_insert_with(SelfWrites::default)
     .record(path, bytes);
// ...later, on a change event:
if self_writes.matches(&path, &bytes) { return; }  // our own echo
```

A content hash rather than a timestamp, deliberately: timestamps depend on clock
resolution and on how long a save takes, which is the kind of race that works on
one machine and not another.

## Deletions are a question, not a fact

Many editors save by writing a temporary file and renaming it over the original,
so an ordinary save can surface as a removal followed by an addition. **Never
warn the user from a `Removed` alone.** Confirm it first:

```rust
if !change.is_live() && change.still_missing() {
    // now it is really gone
}
```

## What is not reported

**Scratch files from atomic saves.** Almost nothing writes a file in place: the
safe way to save is to write a temporary file beside the target and rename it
over the top. The watcher sees every step, and a real save looks like this:

```text
added    particles/x.particle.tmp.20124.8ed8f24b3cfd   ← suppressed
modified particles/x.particle.tmp.20124.8ed8f24b3cfd   ← suppressed
removed  particles/x.particle
renamed  ...tmp... -> particles/x.particle
```

The first two are dropped by `is_transient`. They help nobody and actively hurt:
the scratch file exists for milliseconds, is half-written for most of them, and
classifies as `Other` because the real extension is in the *middle* of the name.
A consumer that reacted by parsing it would read a truncated file.

The `removed` and `renamed` pair on the real file still comes through. That is
why `still_missing()` exists — a save briefly looks exactly like a deletion.

Nothing inside `target/`, `.git/`, `.renzora/`, `node_modules/`, `dist/`,
`.svn/`, `.hg/`, or any directory whose name starts with a dot. This is not a
performance tweak: `target/` alone can hold hundreds of thousands of files that
churn during a build, and a script or shader rebuild writing into it would feed
its own output back in as change events.

Bevy's `.meta` sidecars are also not published. Renzora writes none, and the
asset server is handed them directly.

## Asset hot-reload

The watcher forwards asset-shaped changes into `AssetServer`, so a texture,
model or audio file that something holds a `Handle` for reloads on save with no
extra work.

This did not happen before. Renzora registers a custom default asset source, and
it never supplied a watcher, so Bevy's `file_watcher` feature was compiled into
every desktop build and never ran.

## Why not just use Bevy's watcher?

Bevy has the plumbing but not the product. `FileWatcher` and `AssetSourceEvent`
are public, and this crate uses both. What Bevy does not offer is a way to
*observe* the stream: the events go into a single-consumer channel that
`handle_internal_asset_events` drains, and they only ever become reloads for
assets already loaded. A file no `Handle` points at, which includes every `.rs`
script, is dropped.

So Renzora drives the watcher and Bevy listens. The asset source captures Bevy's
sender (`renzora_engine::setup_asset_reader`), this crate owns the watcher, and
both get what they need from one `notify` backend.

## Timing

Changes are debounced for 100ms and published in `PreUpdate`, so a consumer
reading in `Update` sees them on the same frame they arrive.

The debounce is what makes one save one message. Without it a single Ctrl+S in
an external editor arrives as a create, one or more writes, and a close, and a
script would be recompiled several times over.

## Bursts

`ProjectFileChanged` is a buffered `Message` rather than an observer `Event`,
because the bursts are the hard part: a `git checkout` or a batch export rewrites
thousands of paths at once. Buffering lets a consumer coalesce the whole burst
into one response. Prefer collecting what changed in the loop and acting once:

```rust
fn relist(mut changes: MessageReader<ProjectFileChanged>, mut dirty: Local<bool>) {
    for change in changes.read() {
        if change.kind == AssetKind::Texture {
            *dirty = true;
        }
    }
    if std::mem::take(&mut *dirty) {
        // one re-list for the whole burst, not one per file
    }
}
```

## Scope

`ProjectWatchPlugin` is editor-scope. A shipped game reads its assets from a
`.rpak` and has nothing to hot-reload.

A runtime-scope crate that reads `ProjectFileChanged` must therefore call
`app.add_message::<ProjectFileChanged>()` itself. `add_message` is idempotent, so
the duplicate registration is free, and without it an exported game panics on its
first frame reading a message nobody registered. `renzora_rust_script` does this
for exactly that reason.

## Is any of this polling?

Not for the disk. The chain is push all the way down to the frame loop:

1. The kernel notifies on change. `ReadDirectoryChangesW` on Windows, `inotify`
   on Linux, `FSEvents` on macOS. A background thread **blocks** until the kernel
   wakes it, and costs nothing while it waits.
2. That thread debounces and pushes into a channel.
3. One system calls `try_recv` once per frame.

Step 3 is the only per-frame work, and it is an atomic load on an in-memory
queue: no syscall, no disk, no directory read. It cannot block, because a system
that waits stalls the frame, and the editor has to keep drawing whether or not
anyone is editing files.

That is the difference that mattered. The old model asked the disk "has anything
changed?" on a timer, which on a project of a few thousand files meant thousands
of syscalls twice a second to hear "no" almost every time. The new one is told.

## Apply the change, don't go and look again

The event names exactly which file changed. A consumer that responds by
re-reading the project has thrown that away and does work proportional to the
size of the project to learn something it was already told. For a `git pull`
that adds twenty scripts, that is twenty whole-project walks.

Keep a sorted list and splice it:

```rust
fn apply(&mut self, key: &str, value: &Path, exists: bool) -> bool {
    let list = Arc::make_mut(&mut self.entries);
    match (list.binary_search_by(|(k, _)| k.as_str().cmp(key)), exists) {
        (Err(at), true) => { list.insert(at, (key.into(), value.into())); true }
        (Ok(at), false) => { list.remove(at); true }
        _ => false,
    }
}
```

Two things to get right, because both fail quietly:

- **Derive the key exactly as the opening walk does.** `ScriptIndex` uses native
  separators and `MaterialIndex` forward-slashes them. Use the wrong one and the
  binary search looks in the wrong place, so the same file lands in the list
  twice.
- **Respect whatever the walk skips.** `find_material_files` stops at a depth
  bound, so accepting a deeper file incrementally puts an entry in the list that
  disappears the next time the project opens.

`MaterialIndex` and `ScriptIndex` are both this shape, and
`incremental_matches_a_full_walk` is the test that holds them to it.

## Watching something outside the project

`ExtraWatchRoots` adds a directory to the watch set. Events from it carry that
directory as their `root`, so `relative` is relative to it and not to the
project.

```rust
let mut roots = app.world_mut()
    .get_resource_or_insert_with(ExtraWatchRoots::default);
roots.add(languages_dir);
```

The bar is high, and "my feature reads files from there" is not it. This is for
directories belonging to the **engine install** rather than the user's project,
where there is no project-relative path to speak of. `renzora_lang`'s
`languages/` folder, which sits beside the executable, is the case it was built
for.

Anything under the project is already covered, including several things that
look like engine directories and are not: `themes/` and `fonts/` are both
`<project>/...`, and registering them would watch them twice.

Only the project's own events are forwarded to `AssetServer`. Its paths resolve
against the project asset source, so handing it a path from another root would
make it look for that path inside the project.

## A reactive snapshot is not free

`keyed_list` runs its snapshot **every frame**. A snapshot that touches the disk
is therefore a per-frame filesystem read for as long as the panel is open, and it
will not show up in any search for a timer or a throttle. The Scenes panel did
exactly this: `read_dir` of `<project>/scenes`, every frame, on the main thread.

When a snapshot is more than cheap field reads, use `keyed_list_tokened` and let
the token answer "could this have changed?" from a counter the watcher moves:

```rust
keyed_list_tokened(commands, list, scenes_token, scenes_snapshot);
```

`ScenesRevision` and `ThemeFilesRevision` are both that counter. A counter rather
than an mtime, because nothing compares it to a clock: it only has to be
*different* after an edit, and a counter is that without a syscall.

## Gated consumers need somewhere to record staleness

A `MessageReader` in a system with a `run_if` advances no cursor while the
condition is false, and messages are dropped after two frames. A panel that only
reads changes while it is open will therefore miss everything that happened while
it was closed.

Split it: an ungated system reads the messages and sets a `dirty` flag, and the
gated system does the expensive work when it sees the flag. Reading the buffer
costs nanoseconds; it is the response that is expensive. `ScriptIndex::dirty` and
`MaterialIndex::dirty` are both this pattern.

## Finding files that were already there

A file event can only tell you about a file that changed while you were
listening. Anything that needs to know what was already on disk when the project
opened still has to look once, gated on `CurrentProject::is_changed()`, and that
walk belongs on a task pool rather than in a system.

Two project walks were separately measured costing **110ms and 130ms in a single
frame** on a project with a few thousand files. Both were fixed the same way,
months apart, because "did a file change?" had no single owner. It has one now.
