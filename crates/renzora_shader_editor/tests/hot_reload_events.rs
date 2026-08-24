//! Reading a watched `.wgsl` must not look like an edit — the resolver reads
//! every file it re-resolves, so an access that counts as a change never stops.

use std::time::{Duration, Instant};

use notify_debouncer_full::{
    new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use renzora_shader_editor::hot_reload::is_content_change;

const DEBOUNCE: Duration = Duration::from_millis(200);

struct Watched {
    dir: std::path::PathBuf,
    rx: std::sync::mpsc::Receiver<DebounceEventResult>,
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

fn watch(test_name: &str) -> Watched {
    let dir = std::env::temp_dir().join(format!(
        "renzora_hot_reload_{}_{test_name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("m.wgsl"), "// original\n").unwrap();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(DEBOUNCE, None, move |result| {
        let _ = tx.send(result);
    })
    .unwrap();
    debouncer.watch(&dir, RecursiveMode::Recursive).unwrap();
    // The watch is registered on a background thread; an event raised before it
    // is in place is not delivered.
    std::thread::sleep(DEBOUNCE * 2);
    Watched {
        dir,
        rx,
        _debouncer: debouncer,
    }
}

impl Watched {
    /// Everything the watcher delivered within `window`, split by whether the
    /// filter calls it a content change.
    fn drain(&self, window: Duration) -> (Vec<std::path::PathBuf>, usize) {
        let deadline = Instant::now() + window;
        let (mut changed, mut ignored) = (Vec::new(), 0);
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Ok(Ok(events)) = self.rx.recv_timeout(left) else {
                continue;
            };
            for event in events {
                if is_content_change(&event.kind) {
                    changed.extend(event.paths.iter().cloned());
                } else {
                    ignored += 1;
                }
            }
        }
        (changed, ignored)
    }
}

impl Drop for Watched {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn reading_a_watched_file_is_not_a_change() {
    let w = watch("read");
    let path = w.dir.join("m.wgsl");
    for _ in 0..5 {
        let _ = std::fs::read_to_string(&path).unwrap();
        std::thread::sleep(DEBOUNCE / 2);
    }

    let (changed, ignored) = w.drain(DEBOUNCE * 4);
    assert!(
        changed.is_empty(),
        "reading a file must not register as an edit, got {changed:?}"
    );
    // Without this the test proves nothing: it also passes on a platform
    // that reports no read events at all, while the loop it guards against
    // lives on.
    assert!(
        ignored > 0,
        "this platform reported no access events, so the filter went untested"
    );
}

#[test]
fn writing_a_watched_file_is_a_change() {
    let w = watch("write");
    let path = w.dir.join("m.wgsl");
    std::fs::write(&path, "// edited\n").unwrap();

    let (changed, _) = w.drain(DEBOUNCE * 6);
    assert!(
        changed.contains(&path),
        "an actual edit must still come through, got {changed:?}"
    );
}
