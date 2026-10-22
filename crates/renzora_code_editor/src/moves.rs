//! Keep open files in step with an asset that moves.
//!
//! A move rewrites the paths in every script on disk (see the asset browser's
//! `references`), but a script open here is a buffer, not the file. Left alone,
//! the buffer still holds the old path, and the next Ctrl+S writes it straight
//! back over the fixed file. A tab whose own file moved would also go on saving
//! to the path it was opened from, recreating the file in its old place.
//!
//! So each open file gets the same rewrite its file on disk got, unsaved edits
//! and all: a tab with changes keeps them, with the new path among them.

use bevy::prelude::*;
use renzora::core::{AssetPathChanged, CurrentProject};

use crate::state::CodeEditorState;

pub(crate) fn follow_asset_moves(
    trigger: On<AssetPathChanged>,
    project: Option<Res<CurrentProject>>,
    state: Option<ResMut<CodeEditorState>>,
    mut commands: Commands,
) {
    let (Some(project), Some(mut state)) = (project, state) else {
        return;
    };
    let ev = trigger.event().clone();
    let mut any = false;

    for file in state.bypass_change_detection().open_files.iter_mut() {
        if let Some(new_rel) = ev.rewrite(&project.make_asset_relative(&file.path)) {
            file.path = project.path.join(new_rel);
            if let Some(name) = file.path.file_name().and_then(|n| n.to_str()) {
                file.name = name.to_string();
            }
            any = true;
        }
        if let Some(updated) = ev.rewrite_text(&file.content) {
            file.content = updated;
            any = true;
        }
        // Folded regions are cut out of `content` until unfolded or saved, so
        // they need the rewrite as much as the visible text does.
        for text in file.folds.values_mut() {
            if let Some(updated) = ev.rewrite_text(text) {
                *text = updated;
                any = true;
            }
        }
    }
    if !any {
        return;
    }
    state.set_changed();

    // The tab on screen is drawn from the widget's own copy of the text, which
    // would push the old path back on the next keystroke.
    commands.queue(move |world: &mut World| {
        renzora_ember::widgets::rewrite_code_lines(world, &|line| ev.rewrite_text(line));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;

    use crate::state::OpenFile;

    fn open(path: PathBuf, content: &str, modified: bool) -> OpenFile {
        OpenFile {
            name: path.file_name().unwrap().to_string_lossy().into(),
            path,
            content: content.into(),
            is_modified: modified,
            error: None,
            last_checked_content: content.into(),
            last_cursor_index: None,
            breakpoints: HashSet::new(),
            folds: HashMap::new(),
        }
    }

    fn app(root: &std::path::Path, files: Vec<OpenFile>) -> App {
        let mut app = App::new();
        app.insert_resource(CurrentProject { path: root.to_path_buf(), config: Default::default() });
        let mut state = CodeEditorState::default();
        state.open_files = files;
        app.insert_resource(state);
        app.add_observer(follow_asset_moves);
        app
    }

    /// The bug a disk-only rewrite leaves: the open buffer still has the old
    /// path, and the next save writes it back.
    #[test]
    fn an_open_script_follows_the_move_and_keeps_unsaved_edits() {
        let root = PathBuf::from("C:/proj");
        let mut file = open(
            root.join("scripts/menu.lua"),
            "-- edited, not saved\nplay_sound(\"audio/click.mp3\", 0.5)",
            true,
        );
        file.folds.insert(5, "play_sound(\"audio/click.mp3\")".into());
        let mut app = app(&root, vec![file]);

        app.world_mut().trigger(AssetPathChanged {
            old: "audio/click.mp3".into(),
            new: "audio/ui/click.mp3".into(),
            is_dir: false,
        });

        let f = &app.world().resource::<CodeEditorState>().open_files[0];
        assert_eq!(f.content, "-- edited, not saved\nplay_sound(\"audio/ui/click.mp3\", 0.5)");
        assert!(f.is_modified, "unsaved edits stay unsaved");
        assert_eq!(f.folds[&5], "play_sound(\"audio/ui/click.mp3\")");
    }

    /// A tab whose own file moved saves to the new place, not the old one.
    #[test]
    fn a_tab_whose_file_moved_points_at_the_new_place() {
        let root = PathBuf::from("C:/proj");
        let mut app = app(&root, vec![open(root.join("scripts/menu.lua"), "x = 1", false)]);

        app.world_mut().trigger(AssetPathChanged {
            old: "scripts".into(),
            new: "code".into(),
            is_dir: true,
        });

        let f = &app.world().resource::<CodeEditorState>().open_files[0];
        assert_eq!(f.path, root.join("code/menu.lua"));
        assert_eq!(f.name, "menu.lua");
        assert_eq!(f.content, "x = 1");
    }
}
