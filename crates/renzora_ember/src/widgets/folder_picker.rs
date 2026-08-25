//! Folder picker — a project's own directory tree as a bordered, scrollable
//! list of rows with exactly one selected. For every "where should this land?"
//! prompt in the editor: the marketplace's install-into confirmation, the
//! hierarchy's Create-asset overlay.
//!
//! The pick lives in one [`FolderPick`] resource rather than in each caller's
//! own state. That's deliberate: a picker only ever appears inside a modal
//! overlay, so at most one is on screen, and a shared resource is what lets the
//! selected-row highlight be a plain reactive binding instead of click plumbing
//! re-written per caller.
//!
//! The walk is depth-bounded and count-capped, and skips hidden / build /
//! dependency directories — a huge project shouldn't stall the overlay opening,
//! and `target/` is never a place an asset belongs.
//!
//! [`folder_new_button`] adds a **New Folder** control, because "the folder I
//! want doesn't exist yet" is the ordinary case, not the exception — without it
//! the only way out of the overlay is to cancel, make the folder elsewhere, and
//! start over. It's handed back parentless rather than placed under the tree: it
//! belongs at the left end of the overlay's *own* Cancel/Confirm row, and a bar
//! of its own would read as a second, competing row of controls.
//!
//! It behaves the way a file manager does rather than the way a form does:
//! pressing it *creates* the folder immediately, under whichever row is picked,
//! and opens the name for editing **in place on its own row**. Enter keeps what
//! you typed, Escape keeps the generated name. There is no confirm/cancel pair,
//! because there is nothing to confirm — the folder already exists either way,
//! so the only question left is what it's called.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::tracked::bind_bg;
use crate::theme::*;

use super::button::icon_label_button;
use super::scroll_view;
use super::text_input::{text_input, EmberTextInput};

/// How many rows the walk will produce before it gives up descending.
const MAX_ROWS: usize = 300;

/// The folder the open picker currently targets. [`folder_picker`] seeds it;
/// the caller reads it when its overlay is confirmed.
#[derive(Resource, Default)]
pub struct FolderPick(pub Option<PathBuf>);

impl FolderPick {
    pub fn path(&self) -> Option<&Path> {
        self.0.as_deref()
    }
}

/// One row of the tree, carrying the directory it selects.
#[derive(Component)]
pub(crate) struct FolderPickRow(PathBuf);

/// On the picker box: everything the walk needs to be redone. Creating a folder
/// rebuilds the rows from disk rather than splicing one in — a new folder can
/// land anywhere in the depth-first order, and a re-walk is a few hundred
/// `read_dir` entries against a gesture the user just made by hand.
///
/// It lives on the box rather than on the row column because the box is the
/// entity callers hold, and [`folder_new_button`] has to be addressable from
/// wherever the caller decides to put it.
#[derive(Component)]
pub(crate) struct FolderPickerTree {
    /// The column of row entities, down inside the scroll view.
    rows: Entity,
    root: PathBuf,
    /// Raised (never lowered) when a folder is created below the current bound,
    /// so the thing that was just created is never invisible.
    max_depth: usize,
}

/// The New Folder button, carrying the picker box it creates into.
#[derive(Component, Clone, Copy)]
pub(crate) struct FolderNewBtn(Entity);

/// The inline name field on a just-created row. Only ever one exists.
#[derive(Component)]
pub(crate) struct FolderRenameField {
    /// The picker box, not the row column — same reason as [`FolderPickerTree`].
    picker: Entity,
    /// Where the folder is on disk *now* — the generated name until Enter moves
    /// it. Held here rather than looked up, because the row is rebuilt out from
    /// under the field on every tree refresh.
    path: PathBuf,
    /// Set once the field has actually been seen holding focus. Until then a
    /// blur is not a blur: the very press that spawned this field is still
    /// `just_pressed` during the Update it appears in, so `text_input_focus`
    /// can blur it on arrival for landing on a button rather than an input.
    /// Committing on that would close the rename before it was ever visible.
    armed: bool,
}

/// A bordered, scrollable folder tree rooted at `root` with `selected`
/// pre-picked. Returns the box entity: drop it straight into an overlay body.
/// It flex-grows, so put it between the fixed content above and the buttons
/// below and it fills whatever height is left instead of leaving dead space.
///
/// `max_depth` bounds the walk below `root` (`0` = the root's own children
/// only). `root` itself is always the first row, so "put it at the top level"
/// is reachable without scrolling.
///
/// Pair it with [`folder_new_button`] to let the user make a folder that doesn't
/// exist yet.
pub fn folder_picker(
    commands: &mut Commands,
    fonts: &EmberFonts,
    root: &Path,
    selected: &Path,
    max_depth: usize,
) -> Entity {
    commands.insert_resource(FolderPick(Some(selected.to_path_buf())));

    // Reserved up front: every row points back at the box (that's how an inline
    // rename finds the walk to redo), so its id has to exist before the rows do.
    let boxed = commands.spawn_empty().id();

    let tree = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            },
            Name::new("folder-picker-tree"),
        ))
        .id();

    let rows = spawn_rows(commands, fonts, boxed, root, max_depth, None);
    commands.entity(tree).add_children(&rows);

    let scroll = scroll_view(commands, tree);
    commands.entity(boxed).insert((
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(60.0),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            overflow: Overflow::clip(),
            padding: UiRect::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(rgb(section_bg())),
        BorderColor::all(rgb(border())),
        FolderPickerTree {
            rows: tree,
            root: root.to_path_buf(),
            max_depth,
        },
        Name::new("folder-picker"),
    ));
    commands.entity(boxed).add_child(scroll);
    boxed
}

/// The **New Folder** button for `picker`, spawned parentless.
///
/// Handed back rather than placed, because the only place it belongs is the
/// overlay's *own* button row — a bar of its own under the tree reads as a
/// second, competing row of controls.
///
/// It is **absolutely positioned at the row's left edge**, so it is out of flow
/// entirely and the row lays its Cancel/Confirm pair out exactly as it did
/// before the button existed. The obvious alternative — `margin-right: auto` on
/// an in-flow first child — does not survive contact with a right-aligned row:
/// the auto margin claims the free space the other buttons were sized against,
/// and anything left with the default `flex_shrink: 1` (a plain [`button`],
/// unlike [`icon_label_button`]) is squeezed to nothing and vanishes.
pub fn folder_new_button(commands: &mut Commands, fonts: &EmberFonts, picker: Entity) -> Entity {
    let button = icon_label_button(
        commands,
        fonts,
        "folder-plus",
        &renzora::lang::t_or("folder_picker.new", "New Folder"),
    );
    commands.entity(button).insert(FolderNewBtn(picker));
    commands.entity(button).entry::<Node>().and_modify(|mut n| {
        n.position_type = PositionType::Absolute;
        n.left = Val::Px(0.0);
        // Pinned top and bottom rather than given a height: the row is only as
        // tall as the in-flow buttons, and matching it keeps the two ends of the
        // row on the same baseline whatever the theme's button padding is.
        n.top = Val::Px(0.0);
        n.bottom = Val::Px(0.0);
    });
    button
}

/// The tree's rows: `root` first, then its depth-first descendants. The row for
/// `editing` (if any) renders an inline name field in place of its label.
fn spawn_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    picker: Entity,
    root: &Path,
    max_depth: usize,
    editing: Option<&Path>,
) -> Vec<Entity> {
    let root_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    let mut rows = vec![folder_row(commands, fonts, picker, root.to_path_buf(), 0, &root_name, false)];
    for (path, depth, name) in folder_dirs(root, max_depth) {
        let edit = editing == Some(path.as_path());
        rows.push(folder_row(commands, fonts, picker, path, depth + 1, &name, edit));
    }
    rows
}

/// Rebuild the picker's rows from disk, optionally opening `editing`'s name for
/// renaming. Every path that changes what's on disk goes through here rather
/// than splicing a row in: a folder can land anywhere in the depth-first order,
/// and a re-walk is a few hundred `read_dir` entries against a gesture the user
/// just made by hand.
fn refresh_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    children: &Query<&Children>,
    picker: Entity,
    spec: &FolderPickerTree,
    editing: Option<&Path>,
) {
    if let Ok(kids) = children.get(spec.rows) {
        for kid in kids.iter() {
            commands.entity(kid).try_despawn();
        }
    }
    let rows = spawn_rows(commands, fonts, picker, &spec.root, spec.max_depth, editing);
    commands.entity(spec.rows).replace_children(&rows);
}

/// `base`, or the first free `base N` — what a fresh folder is called before
/// anyone types anything.
fn unique_folder(parent: &Path, base: &str) -> PathBuf {
    let first = parent.join(base);
    if !first.exists() {
        return first;
    }
    // Bounded: a parent with a thousand `New Folder N` in it is not a case worth
    // scanning forever for, and the create below reports the collision anyway.
    (1..1000)
        .map(|n| parent.join(format!("{base} {n}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| parent.join(format!("{base} {}", 1000)))
}

/// The directories under `root`, depth-first and alphabetical, as
/// `(path, depth, name)` — `depth` counted from `root`'s children at 0. Public
/// so a caller that wants its own row rendering can still share the walk (and
/// its skip rules) rather than re-deriving them.
pub fn folder_dirs(root: &Path, max_depth: usize) -> Vec<(PathBuf, usize, String)> {
    fn rec(dir: &Path, depth: usize, max: usize, out: &mut Vec<(PathBuf, usize, String)>) {
        if depth > max || out.len() > MAX_ROWS {
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        entries.sort();
        for path in entries {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Hidden dotfolders, cargo's build dir and npm's dependency dir are
            // never asset destinations, and `target/` alone would blow the row
            // cap on its own.
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            out.push((path.clone(), depth, name));
            rec(&path, depth + 1, max, out);
        }
    }
    let mut out = Vec::new();
    rec(root, 0, max_depth, &mut out);
    out
}

fn folder_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    picker: Entity,
    path: PathBuf,
    depth: usize,
    name: &str,
    editing: bool,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::left(Val::Px(8.0 + depth as f32 * 14.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            FolderPickRow(path.clone()),
            Name::new("folder-picker-row"),
        ))
        .id();
    let p = path.clone();
    bind_bg(commands, row, move |w| {
        if w.get_resource::<FolderPick>().is_some_and(|f| f.0.as_deref() == Some(p.as_path())) {
            rgb(accent()).with_alpha(0.20)
        } else if matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "folder", text_muted(), 12.0);
    let label = if editing {
        inline_name_field(commands, fonts, picker, &path, name)
    } else {
        commands
            .spawn((
                Text::new(name.to_string()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_primary())),
            ))
            .id()
    };
    commands.entity(row).add_children(&[icon, label]);
    row
}

/// The name field a freshly created row wears in place of its label: pre-filled
/// with the generated name and select-all'd, so the first keystroke replaces it.
///
/// Sized to sit inside the 22px row without growing it — the row's height is
/// what keeps the tree from jumping as the field appears and disappears.
fn inline_name_field(
    commands: &mut Commands,
    fonts: &EmberFonts,
    picker: Entity,
    path: &Path,
    name: &str,
) -> Entity {
    let input = text_input(commands, &fonts.ui, name, name);
    commands.entity(input).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(0.0);
        n.height = Val::Px(18.0);
        n.margin = UiRect::right(Val::Px(6.0));
        n.padding = UiRect::axes(Val::Px(4.0), Val::Px(0.0));
    });
    commands
        .entity(input)
        .entry::<EmberTextInput>()
        .and_modify(|mut i| {
            i.focused = true;
            i.select_all = true;
            i.caret_index = i.value.chars().count();
        });
    commands.entity(input).insert(FolderRenameField {
        picker,
        path: path.to_path_buf(),
        armed: false,
    });
    input
}

/// Click a row → it becomes the pick.
pub(crate) fn folder_pick_click(
    q: Query<(&Interaction, &FolderPickRow), Changed<Interaction>>,
    mut pick: ResMut<FolderPick>,
) {
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed && pick.0.as_deref() != Some(row.0.as_path()) {
            pick.0 = Some(row.0.clone());
        }
    }
}

/// **New Folder** → create it now, under whatever row is picked, and open its
/// name for editing in place.
///
/// The folder exists before the user has typed anything, deliberately: it means
/// there is no half-finished state to abandon, no confirm button, and no way to
/// end up back where you started. Escape is a no-op that leaves a real folder
/// called "New Folder"; Enter renames that folder.
pub(crate) fn folder_new_click(
    clicks: Query<(&Interaction, &FolderNewBtn), Changed<Interaction>>,
    mut trees: Query<&mut FolderPickerTree>,
    children: Query<&Children>,
    fonts: Option<Res<EmberFonts>>,
    mut pick: ResMut<FolderPick>,
    mut commands: Commands,
) {
    for (interaction, btn) in &clicks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(mut tree) = trees.get_mut(btn.0) else {
            continue;
        };
        let Some(fonts) = fonts.as_deref() else {
            continue;
        };
        // A pick from a *previous* picker (the resource outlives the overlay)
        // could point outside this tree entirely.
        let parent = pick
            .0
            .clone()
            .filter(|p| p.starts_with(&tree.root))
            .unwrap_or_else(|| tree.root.clone());
        let path = unique_folder(
            &parent,
            &renzora::lang::t_or("folder_picker.new_folder", "New Folder"),
        );
        // `create_dir_all`, not `create_dir`: the only chain it can recreate is
        // a parent that was deleted out from under a stale pick.
        if let Err(e) = std::fs::create_dir_all(&path) {
            renzora::core::console_log::console_error(
                "Assets",
                format!("Could not create {} — {e}", path.display()),
            );
            continue;
        }
        // Created below the walk's bound (the picker was opened with a shallow
        // one) it would exist but never appear — deepen instead.
        let below_root = path
            .strip_prefix(&tree.root)
            .map(|r| r.components().count())
            .unwrap_or(1);
        tree.max_depth = tree.max_depth.max(below_root.saturating_sub(1));

        refresh_rows(&mut commands, fonts, &children, btn.0, &tree, Some(&path));
        pick.0 = Some(path);
    }
}

/// Enter keeps the typed name, Escape keeps the generated one, and clicking
/// away is Enter — the file-manager contract for an inline rename.
///
/// **PreUpdate, ahead of the form and overlay key handlers**, because both keys
/// are already spoken for inside a modal: Escape would close the whole overlay
/// and Enter would press its Create button. Consuming the press with
/// `clear_just_pressed` is what keeps a rename from leaking out into the host.
pub(crate) fn folder_rename_keys(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut fields: Query<(&mut FolderRenameField, &mut EmberTextInput)>,
    trees: Query<&FolderPickerTree>,
    children: Query<&Children>,
    fonts: Option<Res<EmberFonts>>,
    mut pick: ResMut<FolderPick>,
    mut commands: Commands,
) {
    let Ok((mut field, mut input)) = fields.single_mut() else {
        return;
    };
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let escape = keys.just_pressed(KeyCode::Escape);
    if input.focused {
        if !field.armed {
            field.armed = true;
        }
    } else if !field.armed {
        // Blurred before it was ever focused — the spawning press, not the user.
        // Take the focus back; this runs ahead of Update's focus system, so
        // nothing re-blurs it this frame.
        input.focused = true;
        consume(&mut keys, enter, escape);
        return;
    }
    // Losing focus commits, the way a file manager's rename does — the typed
    // name is what the user meant whether or not they reached for Enter.
    if !enter && !escape && input.focused {
        return;
    }
    let Some(fonts) = fonts.as_deref() else {
        return;
    };
    let Ok(tree) = trees.get(field.picker) else {
        return;
    };

    let typed = input.value.trim().to_string();
    let current = field
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    if !escape && typed != current {
        if valid_folder_name(&typed) {
            let dest = field.path.with_file_name(&typed);
            match std::fs::rename(&field.path, &dest) {
                Ok(()) => {
                    // Only follow the rename if the pick is still the folder
                    // being renamed: clicking another row is one of the ways
                    // focus is lost, and that click's pick must win.
                    if pick.0.as_deref() == Some(field.path.as_path()) {
                        pick.0 = Some(dest);
                    }
                }
                Err(e) => {
                    renzora::core::console_log::console_error(
                        "Assets",
                        format!("Could not rename to {typed:?} — {e}"),
                    );
                }
            }
        } else {
            // An unusable name closes the rename anyway, keeping the generated
            // one. Holding the field open instead can't be made safe: a blur is
            // one of the triggers, so a re-opened field would blur again the
            // next frame and loop.
            renzora::core::console_log::console_error(
                "Assets",
                format!("Not a usable folder name: {typed:?} — kept {current:?}"),
            );
        }
    }

    // Rebuild with no field open — the rename reorders the tree, and a blur
    // commit has to close the row it just left behind either way.
    refresh_rows(&mut commands, fonts, &children, field.picker, tree, None);
    consume(&mut keys, enter, escape);
}

/// Swallow the keys the rename actually acted on, so the form above and the
/// overlay's Escape-to-close never see them.
fn consume(keys: &mut ButtonInput<KeyCode>, enter: bool, escape: bool) {
    if enter {
        keys.clear_just_pressed(KeyCode::Enter);
        keys.clear_just_pressed(KeyCode::NumpadEnter);
    }
    if escape {
        keys.clear_just_pressed(KeyCode::Escape);
    }
}

/// Whether `name` is a folder we're willing to create.
///
/// Beyond the obvious traversal/separator rules this rejects dotfolders, which
/// the tree walk skips: creating one would look exactly like the create having
/// silently failed. The reserved-character set is Windows', which is the
/// stricter of the two and so safe to apply everywhere.
fn valid_folder_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !name.ends_with(' ')
        && !name.contains(|c: char| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
}
