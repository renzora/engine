//! The three regions inside the frame: the left list pane (its file queue,
//! model views and destination tree), the centre viewport with the preview's
//! own lighting controls over it, and the right rail — the selection's
//! properties, the import settings, and the footer that ends the window.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    drag_value, dropdown, folder_new_button, folder_picker_folded, progress_indeterminate,
    radio_group, scroll_view,
};

use renzora_import::settings::{SceneStructure, UpAxis};

use crate::overlay::{ImportLayout, ImportOverlayState, ImportProgress};

use super::lifecycle::Init;
use super::lists::{
    files_snapshot, findings_snapshot, log_snapshot, materials_snapshot, meshes_snapshot,
    selection_properties, staged_snapshot,
};
use super::rows::{active_tab, has_staged, showing_material, staged};
use super::tree::scene_snapshot;
use super::widgets::{field_row, g_settings, s_settings, toggle_row};
use super::{
    FileBrowseBtn, FilesContainer, FolderBrowseBtn, ImportColumns, ImportTab, LogContainer, Side,
};

// ── Left pane ────────────────────────────────────────────────────────────────

pub(super) fn build_left_pane(commands: &mut Commands, fonts: &EmberFonts, init: &Init) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(310.0),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                // No padding on the right: the scroll views inside already
                // inset their bar by 2px, so a pane margin on top of it left a
                // strip of dead panel between the bar and the splitter.
                padding: UiRect {
                    left: Val::Px(10.0),
                    right: Val::Px(0.0),
                    top: Val::Px(10.0),
                    bottom: Val::Px(10.0),
                },
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    bind_column_width(commands, col, Side::Left);

    // Files — the queue and everything converted, in one list.
    let files = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, files, |w| active_tab(w) == ImportTab::Files);
    // "Files" and "Folder" read as a pair of nouns beside a tab of the same
    // name, which said nothing about what pressing one does. They are verbs
    // now, and the icons carry the distinction the labels used to have to.
    let browse_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let b1 = super::widgets::pill_button(commands, fonts, "file-plus", "Add files");
    commands.entity(b1).insert(FileBrowseBtn);
    let b2 = super::widgets::pill_button(commands, fonts, "folder-plus", "Add folder");
    commands.entity(b2).insert(FolderBrowseBtn);
    commands.entity(browse_row).add_children(&[b1, b2]);
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            FilesContainer,
        ))
        .id();
    keyed_list(commands, list, files_snapshot);
    let staged_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, staged_list, staged_snapshot);
    let stack = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    commands.entity(stack).add_children(&[staged_list, list]);
    let files_scroll = scroll_view(commands, stack);
    commands
        .entity(files)
        .add_children(&[browse_row, files_scroll]);

    // Scene — flattened tree with expand state.
    let scene = list_pane(commands, ImportTab::Scene, scene_snapshot);
    let meshes = list_pane(commands, ImportTab::Meshes, meshes_snapshot);
    let materials = list_pane(commands, ImportTab::Materials, materials_snapshot);

    // Destination — where a committed import lands.
    //
    // The layout choice leads, above the tree rather than under it. It decides
    // what the tree's answer *means* — whether the picked folder receives a
    // `<stem>/` per file or every file directly — so reading it second is
    // reading the two halves in the wrong order, and a long tree pushed it out
    // of sight entirely.
    let dest = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, dest, |w| active_tab(w) == ImportTab::Destination);
    let org = radio_group(
        commands,
        &fonts.ui,
        &["Folder per file", "All in one folder"],
        init.layout,
    );
    bind_2way(
        commands,
        org,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.layout) {
            Some(ImportLayout::Combined) => 1usize,
            _ => 0,
        },
        |w, v: &usize| {
            if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
                s.layout = if *v == 1 {
                    ImportLayout::Combined
                } else {
                    ImportLayout::PerFileFolder
                };
            }
        },
    );
    let mut dest_kids = vec![org];
    // Ember's own folder picker, not a tree of our own: it already walks,
    // indents, scrolls, remembers what is collapsed, and — with
    // `folder_new_button` — creates a folder and opens its name for editing in
    // place. The hand-rolled version here could only pick from folders that
    // already existed, so "import into a new folder" meant leaving the window,
    // making the folder in the asset browser, and coming back.
    if let Some(root) = init.project_root.clone() {
        let selected = if init.target_dir.is_empty() {
            root.clone()
        } else {
            init.target_dir
                .split('/')
                .fold(root.clone(), |acc, seg| acc.join(seg))
        };
        // Folded, because a project's `assets/` tree fully expanded is a
        // hundred rows to scroll past to reach the one folder you wanted.
        let picker = folder_picker_folded(commands, fonts, &root, &selected, 3);
        // Flat, not a card. The widget draws itself as a bordered box with its
        // own fill, which is right for the overlays it was built for — it is
        // the only thing in them. Here it fills a pane that already has a
        // background and a border of its own, and a second box inside that is
        // just an inset rectangle around a list.
        commands
            .entity(picker)
            .insert((BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)));
        // The New Folder button pins itself to its parent's left edge and full
        // height, so it needs a relative row of its own rather than being
        // dropped straight into the column.
        let new_row = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(28.0),
                position_type: PositionType::Relative,
                flex_shrink: 0.0,
                ..default()
            })
            .id();
        let new_btn = folder_new_button(commands, fonts, picker);
        commands.entity(new_row).add_child(new_btn);
        // Under the tree, not over it: it acts on whichever folder the tree has
        // selected, so it reads as something you do *after* choosing where.
        dest_kids.push(picker);
        dest_kids.push(new_row);
    }
    commands.entity(dest).add_children(&dest_kids);

    commands
        .entity(col)
        .add_children(&[files, scene, meshes, materials, dest]);
    col
}

/// A tab-gated scrolling keyed list, used for the tree and the two flat lists.
fn list_pane(commands: &mut Commands, tab: ImportTab, snapshot: fn(&Rx) -> KeyedSnapshot) -> Entity {
    let holder = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    bind_display(commands, holder, move |w| active_tab(w) == tab);
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, snapshot);
    let scroll = scroll_view(commands, list);
    commands.entity(holder).add_child(scroll);
    holder
}

/// Height of the two bars the centre shows: the conversion fill under the
/// placeholder and the loading sweep on its card. Thicker than a hairline
/// because each is the only thing on screen saying the window is still working,
/// and at 5px that read as a divider rather than as progress.
const PROGRESS_BAR_H: f32 = 8.0;

// ── Centre ───────────────────────────────────────────────────────────────────

pub(super) fn build_centre(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let centre = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                // No padding: the render fills the region edge to edge, so
                // there is no letterbox between it and the columns.
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
        ))
        .id();

    // The staged model, filling the region.
    let view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
            Interaction::default(),
            // Blocks the press from reaching the editor viewport behind; the
            // orbit handler reads this node's own `Interaction`.
            FocusPolicy::Block,
            crate::preview3d::ImportPreviewViewport,
        ))
        .id();
    bind_display(commands, view, |w| has_staged(w) && !showing_material(w));
    bind_with(
        commands,
        view,
        |w| {
            w.get_resource::<crate::preview3d::ImportPreviewImage>()
                .map(|i| i.handle.id())
        },
        |world, entity, _| {
            let Some(handle) = crate::preview3d::preview_image(world) else {
                return;
            };
            if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
                node.image = handle;
            }
        },
    );

    // The selected material, shown in the main viewport rather than a
    // thumbnail in the rail — a 190px square is not enough to judge a surface.
    let mat_view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
            Interaction::default(),
            FocusPolicy::Block,
            crate::matpreview::MaterialPreviewViewport,
        ))
        .id();
    bind_display(commands, mat_view, showing_material);
    bind_with(
        commands,
        mat_view,
        |w| {
            w.get_resource::<crate::matpreview::MaterialPreviewImage>()
                .map(|i| i.handle.id())
        },
        |world, entity, _| {
            let Some(handle) = crate::matpreview::preview_image(world) else {
                return;
            };
            if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
                node.image = handle;
            }
        },
    );

    // Before anything is staged the centre explains what the window is for.
    let placeholder = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    bind_display(commands, placeholder, |w| !has_staged(w));
    let ph_icon = icon_text(commands, &fonts.phosphor, "cube-transparent", text_muted(), 40.0);
    let ph_text = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, ph_text, |w| {
        let Some(s) = w.get_resource::<ImportOverlayState>() else {
            return String::new();
        };
        // Conversion starts on its own once files are chosen, so this reports
        // what is happening rather than asking for another click.
        if s.active_task.is_some() {
            return match &s.progress {
                ImportProgress::Working { label, .. } if !label.is_empty() => label.clone(),
                _ => "Converting…".to_string(),
            };
        }
        match s.pending_files.len() {
            0 => "Choose a model to import".to_string(),
            1 => "1 file queued".to_string(),
            n => format!("{n} files queued"),
        }
    });
    let ph_bar = conversion_bar(commands);
    commands
        .entity(placeholder)
        .add_children(&[ph_icon, ph_text, ph_bar]);

    // The preview's own loading bar, over the (empty) viewport rather than
    // beside it. A staged scene is often hundreds of megabytes and Bevy's
    // loader gives no fraction to report, so this is the indeterminate bar:
    // between staging finishing and the model appearing, the centre was
    // otherwise a flat empty rectangle for several seconds and read as a
    // preview that had failed.
    // On a card, like the progress pill and the lighting panel. It floats over
    // whatever the camera is already rendering — the previous model's last
    // frame, or the clear colour — and bare text on that is at the mercy of
    // what is behind it.
    let loading = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(11.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg()).with_alpha(0.92)),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
        ))
        .id();
    bind_display(commands, loading, |w| {
        has_staged(w)
            && !showing_material(w)
            && w.get_resource::<crate::preview3d::ImportPreview>()
                .is_some_and(|p| p.status == crate::preview3d::PreviewStatus::Loading)
    });
    let ld_text = commands
        .spawn((
            Text::new("Loading preview…".to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let ld_bar = progress_indeterminate(commands, 240.0, PROGRESS_BAR_H);
    commands.entity(loading).add_children(&[ld_text, ld_bar]);

    let env_bar = build_env_bar(commands, fonts);
    let nav = super::gizmo::build(commands, fonts);

    commands
        .entity(centre)
        .add_children(&[view, mat_view, placeholder, loading, env_bar, nav]);
    centre
}

/// The preview's lighting controls, floating over the bottom-right of the
/// viewport.
///
/// They live here rather than in the settings rail because they change nothing
/// about the import — the rail is what the file will *become*, and mixing a
/// view setting into it would make every row there suspect. Over the thing they
/// affect is where a viewport's own controls belong.
///
/// Bottom-right rather than top: the top-right corner is where the editor
/// viewport puts its axis gizmo and navigation buttons, and those have the
/// stronger claim on it — they are the controls you reach for without looking.
fn build_env_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(18.0),
                right: Val::Px(18.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg()).with_alpha(0.88)),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
        ))
        .id();
    // Both previews, not just the model: the material sphere is lit by the same
    // environment and the same rig now, so the switches mean the same thing on
    // either — and a material is judged by what it reflects, which makes the
    // Environment switch matter most there.
    bind_display(commands, bar, has_staged);

    let head = commands
        .spawn((
            Text::new("PREVIEW".to_string()),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    let mut kids = vec![head];
    for (label, get, set) in [
        (
            "Environment",
            (|e: &crate::preview3d::ImportPreviewEnv| e.environment) as fn(&_) -> bool,
            (|e: &mut crate::preview3d::ImportPreviewEnv, v: bool| e.environment = v)
                as fn(&mut _, bool),
        ),
        (
            "Lights",
            |e| e.lights,
            |e, v| e.lights = v,
        ),
        (
            "Grid",
            |e| e.grid,
            |e, v| e.grid = v,
        ),
    ] {
        kids.push(env_toggle_row(commands, fonts, label, get, set));
    }
    commands.entity(bar).add_children(&kids);
    bar
}

/// One row of the preview toolbar: a label and a switch over
/// [`ImportPreviewEnv`](crate::preview3d::ImportPreviewEnv).
fn env_toggle_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    get: fn(&crate::preview3d::ImportPreviewEnv) -> bool,
    set: fn(&mut crate::preview3d::ImportPreviewEnv, bool),
) -> Entity {
    use renzora_ember::widgets::toggle_switch;
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(14.0),
                min_height: Val::Px(22.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    let sw = toggle_switch(commands, get(&default()));
    bind_2way(
        commands,
        sw,
        move |w| {
            w.get_resource::<crate::preview3d::ImportPreviewEnv>()
                .map(get)
                .unwrap_or(false)
        },
        move |w, v: &bool| {
            if let Some(mut e) = w.get_resource_mut::<crate::preview3d::ImportPreviewEnv>() {
                set(&mut e, *v);
            }
        },
    );
    commands.entity(row).add_children(&[t, sw]);
    row
}

/// A determinate bar tracking the conversion worker's `[done/total]`, shown
/// under the placeholder while files are converting and hidden otherwise.
fn conversion_bar(commands: &mut Commands) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(240.0),
                height: Val::Px(PROGRESS_BAR_H),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(PROGRESS_BAR_H / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
        ))
        .id();
    bind_display(commands, track, |w| {
        w.get_resource::<ImportOverlayState>()
            .is_some_and(|s| matches!(s.progress, ImportProgress::Working { .. }))
    });
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(PROGRESS_BAR_H / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
        ))
        .id();
    bind_with(
        commands,
        fill,
        |w| {
            // Rounded to whole percent: bindings compare by value and f32 is
            // not `Eq`, so a raw fraction would rewrite the node every frame.
            match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
                Some(ImportProgress::Working { current, total, .. }) if total > 0 => {
                    ((current as f32 / total as f32) * 100.0).round() as i32
                }
                _ => 0,
            }
        },
        |world, e, pct| {
            if let Some(mut n) = world.get_mut::<Node>(e) {
                n.width = Val::Percent((*pct as f32).clamp(0.0, 100.0));
            }
        },
    );
    commands.entity(track).add_child(fill);
    track
}

// ── Right rail ───────────────────────────────────────────────────────────────

pub(super) fn build_right_rail(commands: &mut Commands, fonts: &EmberFonts, init: &Init) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(320.0),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    bind_column_width(commands, col, Side::Right);

    let inner = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .id();

    // ── Selected-item properties (staged only) ──────────────────────────
    let props = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, props, has_staged);
    let props_head = group_label(commands, fonts, "Properties");
    let props_body = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.mono, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, props_body, selection_properties);
    commands
        .entity(props)
        .add_children(&[props_head, props_body]);

    // ── Findings (staged only) ──────────────────────────────────────────
    let findings = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, findings, has_staged);
    let f_head = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
        ))
        .id();
    bind_text(commands, f_head, |w| {
        staged(w)
            .map(|s| match s.problems() {
                0 => "FINDINGS — nothing looks wrong".to_string(),
                n => format!("FINDINGS — {n} to look at"),
            })
            .unwrap_or_default()
    });
    let f_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, f_list, findings_snapshot);
    commands.entity(findings).add_children(&[f_head, f_list]);

    // ── Import settings (before staging) ────────────────────────────────
    let settings = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let s_head = group_label(commands, fonts, "Import");

    let scale = drag_value(commands, &fonts.ui, "", text_primary(), init.scale, 0.01);
    bind_2way(
        commands,
        scale,
        |w| g_settings(w, |s| s.scale),
        |w, v: &f32| {
            s_settings(w, |s| s.scale = (*v).clamp(0.001, 1000.0));
            // Only reached when the widget's value differs from state, i.e. the
            // user scrubbed or typed — so it marks a deliberate choice and stops
            // the next queue auto-detecting over the top of it.
            if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
                s.scale_is_user_set = true;
            }
        },
    );
    let scale_row = field_row(commands, fonts, "Scale", scale);

    let axis = dropdown(
        commands,
        fonts,
        &["Auto", "Y-Up (GLTF/Bevy)", "Z-Up (Blender/CAD)"],
        init.up_axis,
    );
    bind_2way(
        commands,
        axis,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.settings.up_axis) {
            Some(UpAxis::YUp) => 1usize,
            Some(UpAxis::ZUp) => 2,
            _ => 0,
        },
        |w, v: &usize| {
            s_settings(w, |s| {
                s.up_axis = match v {
                    1 => UpAxis::YUp,
                    2 => UpAxis::ZUp,
                    _ => UpAxis::Auto,
                }
            })
        },
    );
    let axis_row = field_row(commands, fonts, "Up axis", axis);

    // How the scene graph comes out. `Combined` is what the transcoders do
    // today; `One node per mesh` is the way to undo it and get pickable,
    // independently-culled objects back.
    let structure = dropdown(
        commands,
        fonts,
        &["As authored", "One node per mesh", "Combine meshes"],
        init.structure,
    );
    bind_2way(
        commands,
        structure,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.settings.structure) {
            Some(SceneStructure::FlatPerMesh) => 1usize,
            Some(SceneStructure::Combined) => 2,
            _ => 0,
        },
        |w, v: &usize| {
            s_settings(w, |s| {
                s.structure = match v {
                    1 => SceneStructure::FlatPerMesh,
                    2 => SceneStructure::Combined,
                    _ => SceneStructure::Preserve,
                }
            })
        },
    );
    let structure_row = field_row(commands, fonts, "Hierarchy", structure);

    // Sibling texture sets, for a format that stores no materials of its own.
    // Only built when the queue actually offers some, so the row is absent
    // rather than empty for every other format.
    let texture_set_row = (!init.texture_sets.is_empty()).then(|| {
        let mut labels = vec!["None".to_string()];
        labels.extend(
            init.texture_sets
                .iter()
                .map(|(stem, roles)| format!("{stem}  ({roles})")),
        );
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let picker = dropdown(commands, fonts, &refs, init.texture_set);
        // The set list is captured rather than re-read: it is fixed for the
        // window's lifetime, and the binding stores the *name* so the choice
        // survives a reimport even if the folder gains a file.
        let stems: Vec<String> = init.texture_sets.iter().map(|(s, _)| s.clone()).collect();
        let get_stems = stems.clone();
        bind_2way(
            commands,
            picker,
            move |w| {
                w.get_resource::<ImportOverlayState>()
                    .and_then(|s| s.settings.texture_set.clone())
                    .and_then(|want| get_stems.iter().position(|s| *s == want))
                    .map_or(0usize, |i| i + 1)
            },
            move |w, v: &usize| {
                let chosen = v.checked_sub(1).and_then(|i| stems.get(i).cloned());
                s_settings(w, |s| s.texture_set = chosen);
            },
        );
        field_row(commands, fonts, "Textures", picker)
    });

    let flip = toggle_row(commands, fonts, "Flip UVs", |s| s.flip_uvs, |s, v| s.flip_uvs = v);
    let normals = toggle_row(
        commands,
        fonts,
        "Generate normals",
        |s| s.generate_normals,
        |s, v| s.generate_normals = v,
    );

    let e_head = group_label(commands, fonts, "Extract");
    let e1 = toggle_row(commands, fonts, "Skeleton + skin", |s| s.extract_skeleton, |s, v| s.extract_skeleton = v);
    let e2 = toggle_row(commands, fonts, "Animations", |s| s.extract_animations, |s, v| s.extract_animations = v);
    let e3 = toggle_row(commands, fonts, "Textures", |s| s.extract_textures, |s, v| s.extract_textures = v);
    let e4 = toggle_row(commands, fonts, "Materials", |s| s.extract_materials, |s, v| s.extract_materials = v);

    let o_head = group_label(commands, fonts, "Optimize");
    let o1 = toggle_row(commands, fonts, "Vertex cache", |s| s.optimize_vertex_cache, |s, v| s.optimize_vertex_cache = v);
    let o2 = toggle_row(commands, fonts, "Overdraw", |s| s.optimize_overdraw, |s, v| s.optimize_overdraw = v);
    let o3 = toggle_row(commands, fonts, "Vertex fetch", |s| s.optimize_vertex_fetch, |s, v| s.optimize_vertex_fetch = v);

    let mut kids = vec![s_head, scale_row, axis_row, structure_row];
    kids.extend(texture_set_row);
    kids.extend([flip, normals, e_head, e1, e2, e3, e4, o_head, o1, o2, o3]);
    // `add_children` takes a slice, and the settings column is past the
    // tuple-bundle limit, so build the vector and hand it over in one call.
    commands.entity(settings).add_children(&kids);

    // Per-file results from the last run. Hidden until something has been
    // logged, so the rail is not carrying an empty heading most of the time.
    let results = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    bind_display(commands, results, |w| {
        !has_staged(w)
            && w.get_resource::<ImportOverlayState>()
                .is_some_and(|s| !s.log_entries.is_empty())
    });
    let r_head = group_label(commands, fonts, "Results");
    let r_list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            LogContainer,
        ))
        .id();
    keyed_list(commands, r_list, log_snapshot);
    commands.entity(results).add_children(&[r_head, r_list]);

    commands
        .entity(inner)
        .add_children(&[props, findings, settings, results]);
    // `scroll_view` already grows into a column parent (`flex_grow: 1`,
    // `flex_basis: 0`), so the footer below it takes only the height it needs.
    let scroll = scroll_view(commands, inner);
    let footer = build_rail_footer(commands, fonts);
    commands.entity(col).add_children(&[scroll, footer]);
    col
}

/// The rail's footer, which is the Reconvert notice and nothing else — Import
/// is in the header now, beside the tabs and the window's Close.
///
/// The whole strip is display-bound, so it is absent rather than an empty
/// bordered band for the great majority of the time that no reconvert is owed.
///
/// Reconvert is here rather than at the bottom of the settings scroll, which is
/// the one place a notice cannot do its job: the moment it appeared it was below
/// the fold, so the model sat there stale with nothing on screen saying so.
fn build_rail_footer(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let footer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    bind_display(commands, footer, super::rows::settings_are_stale);
    let note = commands
        .spawn((
            Text::new("Settings changed since this was converted.".to_string()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let btn_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .id();
    let btn = super::frame::action_button(
        commands,
        fonts,
        "arrows-clockwise",
        "Reconvert",
        (255, 255, 255),
        1.0,
    );
    commands.entity(btn).insert(super::ReconvertBtn);
    bind_bg(commands, btn, |_| rgb(accent()));
    commands.entity(btn_row).add_child(btn);
    commands.entity(footer).add_children(&[note, btn_row]);
    footer
}

/// Keep a column's width in step with [`ImportColumns`].
fn bind_column_width(commands: &mut Commands, target: Entity, side: Side) {
    bind_with(
        commands,
        target,
        move |w| {
            let c = w.get_resource::<ImportColumns>();
            let v = match side {
                Side::Left => c.map(|c| c.left).unwrap_or(310.0),
                Side::Right => c.map(|c| c.right).unwrap_or(320.0),
            };
            // Bindings compare by value, and f32 is not Eq — round to whole
            // pixels so this only fires when the width actually changes.
            v.round() as i32
        },
        |world, e, px| {
            if let Some(mut node) = world.get_mut::<Node>(e) {
                node.width = Val::Px(*px as f32);
            }
        },
    );
}

/// A small uppercase group heading for the right rail.
fn group_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_uppercase()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id()
}

