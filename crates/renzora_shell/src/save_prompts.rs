//! The two "you have unsaved changes" confirmations: closing the **window**, and
//! closing a single **document tab**.
//!
//! Both follow the same three-way shape — Save & Close / Don't Save / Cancel —
//! and both defer the destructive half until the scene-save has actually landed,
//! so a Save-As the user cancelled aborts the close instead of losing the edits.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{rgb, text_muted};

use crate::doc_tabs::close_doc_tab_by_id;

// ── Save-before-exit flow ────────────────────────────────────────────────────

/// Set when the user asks to close the window (the × button). Consumed by
/// [`process_exit_request`], which either exits straight away or — if any
/// document has unsaved changes — opens the [`ExitPromptRoot`] overlay.
#[derive(Resource)]
pub(crate) struct ExitRequest;

/// Set while we've asked the scene-save system to run and are waiting for it to
/// finish before exiting (see [`pending_exit_after_save`]).
#[derive(Resource)]
pub(crate) struct PendingExitAfterSave;

/// The backdrop root of the "unsaved changes" overlay.
#[derive(Component)]
pub(crate) struct ExitPromptRoot;

/// The overlay's three actions.
#[derive(Component)]
pub(crate) struct ExitPromptSave;
#[derive(Component)]
pub(crate) struct ExitPromptDiscard;
#[derive(Component)]
pub(crate) struct ExitPromptCancel;

/// Are there any documents with unsaved edits?
fn any_unsaved(tabs: &renzora_ui::DocumentTabState) -> bool {
    tabs.tabs.iter().any(|t| t.is_modified)
}

/// Handle a pending [`ExitRequest`]: exit immediately when nothing is dirty,
/// otherwise open the save-confirmation overlay.
pub(crate) fn process_exit_request(
    req: Option<Res<ExitRequest>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    fonts: Option<Res<EmberFonts>>,
    mut exit: MessageWriter<AppExit>,
    open: Query<(), With<ExitPromptRoot>>,
    mut commands: Commands,
) {
    if req.is_none() {
        return;
    }
    commands.remove_resource::<ExitRequest>();
    // A prompt is already up — ignore repeat clicks.
    if !open.is_empty() {
        return;
    }

    let dirty = tabs.as_ref().is_some_and(|t| any_unsaved(t));
    // Nothing unsaved (or we can't render the prompt) → exit straight away.
    // Written here in `Update` because this flow already owns the decision — it
    // had to work out whether to prompt first — so handing back out to the
    // `WindowAction::Close` queue would be a detour.
    //
    // It used to be a *fix*: the queue is drained in `Last` beside
    // `kill_on_app_exit`, the two raced, and losing the race meant the fast exit
    // was missed and the `World` unwound slowly instead. That race is gone —
    // `kill_on_app_exit` is now ordered after
    // `renzora_ui::window_chrome::WindowActionSet` — so either route exits
    // promptly.
    if !dirty || fonts.is_none() {
        exit.write(AppExit::Success);
        return;
    }
    let fonts = fonts.unwrap();
    let count = tabs
        .map(|t| t.tabs.iter().filter(|x| x.is_modified).count())
        .unwrap_or(0);
    spawn_exit_prompt(&mut commands, &fonts, count);
}

/// Build the centered "unsaved changes" confirmation overlay.
fn spawn_exit_prompt(commands: &mut Commands, fonts: &EmberFonts, count: usize) {
    let (root, content) =
        renzora_ember::widgets::overlay_sized(commands, fonts, "Unsaved Changes", 440.0, 188.0, true);
    commands.entity(root).insert(ExitPromptRoot);

    let body = if count == 1 {
        "You have unsaved changes. Save before closing?".to_string()
    } else {
        format!("You have unsaved changes in {count} documents. Save before closing?")
    };

    // Pad the content and lay out the message above a right-aligned button row.
    commands.entity(content).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::all(Val::Px(16.0)),
        ..default()
    });

    let message = commands
        .spawn((
            Text::new(body),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let cancel = renzora_ember::widgets::button(commands, &fonts.ui, "Cancel");
    commands.entity(cancel).insert(ExitPromptCancel);
    let discard = renzora_ember::widgets::button(commands, &fonts.ui, "Don't Save");
    commands.entity(discard).insert(ExitPromptDiscard);
    let save = renzora_ember::widgets::button(commands, &fonts.ui, "Save & Close");
    // Tag it as the accent (primary) action so `apply_theme` paints it the
    // highlight color instead of the plain button color.
    commands.entity(save).insert((
        ExitPromptSave,
        renzora_ember::style::Styled::new(renzora_ember::style::Role::ButtonAccent),
    ));

    commands.entity(row).add_children(&[cancel, discard, save]);
    commands.entity(content).add_children(&[message, row]);
}

/// Drive the overlay's buttons. (Escape / backdrop click / the title × are
/// handled by ember's generic `overlay_dismiss`, which despawns the root — i.e.
/// the same as Cancel.)
pub(crate) fn exit_prompt_buttons(
    save: Query<&Interaction, (Changed<Interaction>, With<ExitPromptSave>)>,
    discard: Query<&Interaction, (Changed<Interaction>, With<ExitPromptDiscard>)>,
    cancel: Query<&Interaction, (Changed<Interaction>, With<ExitPromptCancel>)>,
    roots: Query<Entity, With<ExitPromptRoot>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    let save = save.iter().any(|i| *i == Interaction::Pressed);
    let discard = discard.iter().any(|i| *i == Interaction::Pressed);
    let cancel = cancel.iter().any(|i| *i == Interaction::Pressed);

    if !(save || discard || cancel) {
        return;
    }

    // Either way the prompt goes away.
    for r in &roots {
        commands.entity(r).despawn();
    }

    if save {
        // Run the same Save the title bar uses, then exit once it lands.
        commands.insert_resource(renzora::core::SaveSceneRequested);
        commands.insert_resource(PendingExitAfterSave);
    } else if discard {
        exit.write(AppExit::Success);
    }
    // cancel → nothing else; the close is abandoned.
}

/// After "Save & Close", wait for the scene-save to complete, then exit. If the
/// save was redirected to a Save-As dialog the user cancelled (changes remain
/// unsaved), abort the exit instead of losing work.
pub(crate) fn pending_exit_after_save(
    pending: Option<Res<PendingExitAfterSave>>,
    save_req: Option<Res<renzora::core::SaveSceneRequested>>,
    save_as_req: Option<Res<renzora::core::SaveAsSceneRequested>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    if pending.is_none() {
        return;
    }
    // Still saving (or prompting for a path) — keep waiting.
    if save_req.is_some() || save_as_req.is_some() {
        return;
    }
    commands.remove_resource::<PendingExitAfterSave>();

    let still_dirty = tabs.is_some_and(|t| any_unsaved(&t));
    if !still_dirty {
        exit.write(AppExit::Success);
    }
    // else: save failed or Save-As was cancelled → stay open, don't lose work.
}

// ── Close-tab save prompt ─────────────────────────────────────────────────────

/// Set by `doc_tab_close` when the × is clicked on a tab with unsaved changes.
/// Consumed by [`process_tab_close_request`], which foregrounds the tab and
/// opens the save-confirmation prompt.
#[derive(Resource)]
pub(crate) struct TabCloseRequest {
    pub(crate) id: u64,
}

/// Set after "Save & Close" while we wait for the scene-save to land before
/// closing the tab (see [`pending_close_after_save`]). Carries the tab id.
#[derive(Resource)]
pub(crate) struct PendingCloseAfterSave {
    id: u64,
}

/// Backdrop root of the "unsaved changes" prompt for a single tab. Stores the
/// id of the tab whose close is pending so the buttons know what to act on.
#[derive(Component)]
pub(crate) struct CloseTabPromptRoot(u64);

/// The prompt's three actions.
#[derive(Component)]
pub(crate) struct CloseTabPromptSave;
#[derive(Component)]
pub(crate) struct CloseTabPromptDiscard;
#[derive(Component)]
pub(crate) struct CloseTabPromptCancel;

/// Handle a pending [`TabCloseRequest`]: foreground the target tab (so what the
/// user decides about is what they see, and so a subsequent Save targets this
/// tab's live scene) and open the save-confirmation prompt. If the tab turned
/// out clean in the meantime, just close it.
pub(crate) fn process_tab_close_request(
    req: Option<Res<TabCloseRequest>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    fonts: Option<Res<EmberFonts>>,
    open: Query<(), With<CloseTabPromptRoot>>,
    mut commands: Commands,
) {
    let Some(req) = req else { return };
    // A prompt is already up — leave the request until it's resolved.
    if !open.is_empty() {
        return;
    }
    let id = req.id;
    commands.remove_resource::<TabCloseRequest>();

    let (Some(mut state), Some(fonts)) = (state, fonts) else { return };
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else { return };
    // Not dirty anymore (saved elsewhere since the click) → close outright.
    if !state.tabs[idx].is_modified {
        close_doc_tab_by_id(&mut state, id, &mut commands);
        return;
    }
    let name = state.tabs[idx].name.clone();
    // Bring the tab forward if it's in the background.
    if state.active_tab != idx {
        if let Some((old_id, new_id)) = state.activate_tab(idx) {
            commands.insert_resource(renzora::TabSwitchRequest {
                old_tab_id: old_id,
                new_tab_id: new_id,
            });
        }
    }
    spawn_close_tab_prompt(&mut commands, &fonts, id, &name);
}

/// Build the centered "unsaved changes" prompt for closing a single tab.
fn spawn_close_tab_prompt(commands: &mut Commands, fonts: &EmberFonts, id: u64, name: &str) {
    let (root, content) =
        renzora_ember::widgets::overlay_sized(commands, fonts, "Unsaved Changes", 440.0, 188.0, true);
    commands.entity(root).insert(CloseTabPromptRoot(id));

    let body = format!("\"{name}\" has unsaved changes. Save before closing?");

    // Pad the content and lay out the message above a right-aligned button row.
    commands.entity(content).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::all(Val::Px(16.0)),
        ..default()
    });

    let message = commands
        .spawn((
            Text::new(body),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let cancel = renzora_ember::widgets::button(commands, &fonts.ui, "Cancel");
    commands.entity(cancel).insert(CloseTabPromptCancel);
    let discard = renzora_ember::widgets::button(commands, &fonts.ui, "Don't Save");
    commands.entity(discard).insert(CloseTabPromptDiscard);
    let save = renzora_ember::widgets::button(commands, &fonts.ui, "Save & Close");
    commands.entity(save).insert((
        CloseTabPromptSave,
        renzora_ember::style::Styled::new(renzora_ember::style::Role::ButtonAccent),
    ));

    commands.entity(row).add_children(&[cancel, discard, save]);
    commands.entity(content).add_children(&[message, row]);
}

/// Drive the close prompt's buttons. (Escape / backdrop click / the title × are
/// handled by ember's generic `overlay_dismiss`, which despawns the root — same
/// as Cancel: the tab stays open.)
pub(crate) fn close_tab_prompt_buttons(
    save: Query<&Interaction, (Changed<Interaction>, With<CloseTabPromptSave>)>,
    discard: Query<&Interaction, (Changed<Interaction>, With<CloseTabPromptDiscard>)>,
    cancel: Query<&Interaction, (Changed<Interaction>, With<CloseTabPromptCancel>)>,
    roots: Query<(Entity, &CloseTabPromptRoot)>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mut commands: Commands,
) {
    let save = save.iter().any(|i| *i == Interaction::Pressed);
    let discard = discard.iter().any(|i| *i == Interaction::Pressed);
    let cancel = cancel.iter().any(|i| *i == Interaction::Pressed);

    if !(save || discard || cancel) {
        return;
    }

    // The target tab id lives on the root; capture it before despawning.
    let target = roots.iter().next().map(|(_, r)| r.0);
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let Some(id) = target else { return };

    if save {
        // Save the now-foregrounded tab, then close it once the save lands.
        commands.insert_resource(renzora::core::SaveSceneRequested);
        commands.insert_resource(PendingCloseAfterSave { id });
    } else if discard {
        if let Some(mut state) = state {
            close_doc_tab_by_id(&mut state, id, &mut commands);
        }
    }
    // cancel → nothing; the close is abandoned.
}

/// After "Save & Close", wait for the scene-save to complete, then close the
/// tab. If the save was redirected to a Save-As dialog the user cancelled (the
/// tab is still dirty), abort the close instead of losing work.
pub(crate) fn pending_close_after_save(
    pending: Option<Res<PendingCloseAfterSave>>,
    save_req: Option<Res<renzora::core::SaveSceneRequested>>,
    save_as_req: Option<Res<renzora::core::SaveAsSceneRequested>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };
    // Still saving (or prompting for a path) — keep waiting.
    if save_req.is_some() || save_as_req.is_some() {
        return;
    }
    let id = pending.id;
    commands.remove_resource::<PendingCloseAfterSave>();

    let Some(mut state) = state else { return };
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else { return };
    // Clean now → the save succeeded; close it. Still dirty → Save-As was
    // cancelled, so keep the tab open and don't lose the edits.
    if !state.tabs[idx].is_modified {
        close_doc_tab_by_id(&mut state, id, &mut commands);
    }
}
