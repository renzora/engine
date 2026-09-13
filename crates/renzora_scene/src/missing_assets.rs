//! Telling the user when the open scene points at a file that is no longer there.
//!
//! Delete a model, material or script that entities in the scene reference and
//! nothing used to say so. The mesh simply stopped drawing, the material fell
//! back to the blockout grid, the script quietly did not run, and the reason was
//! a file in a folder nobody had open.
//!
//! # Why this is not a reload
//!
//! The other consumers of [`ProjectFileChanged`] react by re-reading something.
//! There is nothing to re-read here: the file is gone, and the engine cannot
//! invent it. The only useful response is to say so, in the place someone looks
//! to ask whether the project is healthy, and to keep saying it until the file
//! comes back or the reference does.
//!
//! # Why a removal is not believed on sight
//!
//! Almost nothing writes a file in place. A save is a write to a temporary file
//! and a rename over the target, so the real file genuinely does not exist for a
//! moment in the middle of every ordinary save. The scratch file is filtered out
//! upstream, but the unlink of the real one still arrives, and warning on it
//! would fire on saves rather than deletions.
//!
//! So a removal is a question: [`ProjectFileChanged::still_missing`] asks the
//! filesystem, and only a file that is really gone is reported. The rename that
//! completes the save arrives as an arrival, which clears the warning again.

use bevy::prelude::*;
use renzora::content_problems::{ContentProblem, ContentProblems, ProblemSeverity};
use renzora::core::console_log::console_warn;
use renzora::core::components::MeshInstanceData;
use renzora::core::project_files::ProjectFileChanged;
use renzora::core::MaterialRef;

/// Report entities whose asset just disappeared, and clear the report when it
/// comes back.
pub(crate) fn report_missing_assets(
    mut changes: MessageReader<ProjectFileChanged>,
    mut problems: ResMut<ContentProblems>,
    models: Query<(&Name, &MeshInstanceData)>,
    materials: Query<(&Name, &MaterialRef)>,
) {
    if changes.is_empty() {
        return;
    }
    for change in changes.read() {
        // Directories are skipped: removing one produces a removal for the
        // folder, and separate removals for the files that were in it. Acting on
        // both would report the same loss twice, with the folder's report naming
        // nothing an entity actually references.
        if change.is_dir {
            continue;
        }

        let gone = !change.is_live() && change.still_missing();
        if !gone {
            // Arrived or came back. Clear any warning we put on it, including
            // the one we may have raised moments ago for the unlink half of a
            // save, so a save never leaves a stale complaint behind.
            problems.set_severity(
                change.relative.clone(),
                ProblemSeverity::Warning,
                Vec::new(),
            );
            continue;
        }

        // `ends_with` on the relative path rather than equality. A reference is
        // stored asset-relative (`models/chair.glb`), and so is the event's
        // `relative`, but a project whose assets live under a subfolder can have
        // the two differ by that prefix. Comparing by suffix matches the
        // resolver's own leniency instead of inventing a stricter rule that
        // would silently report nothing.
        let names: Vec<String> = models
            .iter()
            .filter(|(_, m)| {
                m.model_path
                    .as_deref()
                    .is_some_and(|p| paths_match(p, &change.relative))
            })
            .map(|(n, _)| n.to_string())
            .chain(
                materials
                    .iter()
                    .filter(|(_, m)| paths_match(&m.0, &change.relative))
                    .map(|(n, _)| n.to_string()),
            )
            .collect();

        if names.is_empty() {
            continue;
        }

        let listed = summarise(&names);
        console_warn(
            "Scene",
            format!(
                "{} was deleted, but {listed} still {} it",
                change.relative,
                if names.len() == 1 { "uses" } else { "use" }
            ),
        );
        warn!(
            "[scene] {} was deleted and is still referenced by {} entity(ies)",
            change.relative,
            names.len()
        );
        problems.set_severity(
            change.relative.clone(),
            ProblemSeverity::Warning,
            vec![ContentProblem {
                severity: ProblemSeverity::Warning,
                line: None,
                message: format!("Deleted, but still used by {listed}."),
                node_id: None,
            }],
        );
    }
}

/// Does an entity's stored asset path refer to this changed file?
fn paths_match(reference: &str, relative: &str) -> bool {
    let reference = reference.replace('\\', "/");
    reference == relative || relative.ends_with(&reference) || reference.ends_with(relative)
}

/// "cube", "cube and lamp", or "cube, lamp and 3 others".
///
/// Truncated because the message is one line in a panel: a scene where forty
/// entities share a deleted model would otherwise produce a wall of names,
/// which is less readable than the count, not more.
fn summarise(names: &[String]) -> String {
    match names.len() {
        1 => names[0].clone(),
        2 => format!("{} and {}", names[0], names[1]),
        _ => format!("{}, {} and {} others", names[0], names[1], names.len() - 2),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exact_path_matches() {
        assert!(paths_match("models/chair.glb", "models/chair.glb"));
    }

    #[test]
    fn a_windows_spelled_reference_matches() {
        assert!(paths_match("models\\chair.glb", "models/chair.glb"));
    }

    #[test]
    fn a_different_file_does_not_match() {
        assert!(!paths_match("models/chair.glb", "models/table.glb"));
        // Guards the `ends_with` leniency: `chair.glb` must not match
        // `armchair.glb` just because one is a suffix of the other.
        assert!(!paths_match("models/chair.glb", "models/armchair.glb"));
    }

    #[test]
    fn names_are_summarised_not_listed() {
        assert_eq!(summarise(&["a".into()]), "a");
        assert_eq!(summarise(&["a".into(), "b".into()]), "a and b");
        assert_eq!(
            summarise(&["a".into(), "b".into(), "c".into(), "d".into()]),
            "a, b and 2 others"
        );
    }
}
