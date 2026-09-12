//! The three "you have unsaved changes" confirmations: closing the **window**,
//! closing a single **document tab**, and leaving the **project** (File > New
//! Project / Open Project / Recent Projects).
//!
//! All three follow the same three-way shape — Save & <verb> / Don't Save /
//! Cancel — and all three defer the destructive half until the scene-save has
//! actually landed, so a Save-As the user cancelled aborts the close instead of
//! losing the edits. The card itself is built once, by [`spawn_prompt`].

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;

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

/// An OS close request for the **primary** window is the same request the title
/// bar's × makes.
///
/// Alt+F4, the taskbar's Close, the window manager's own × — all of them arrive
/// as `WindowCloseRequested`, and until this system existed Bevy's
/// `close_when_requested` answered them by despawning the window. That skipped
/// both halves of the editor's quit: the unsaved-changes prompt (edits went
/// silently) and the fast exit (the World unwound the slow way, seconds of
/// `FreeLibrary` and driver teardown with the window already gone). The editor
/// turns that handler off — see `renzora_runtime::add_default_rendering` — and
/// routes the request here instead.
///
/// Float dock windows are deliberately not touched: `renzora_ember`'s
/// `process_dock_window_closes` already answers their close requests, and it has
/// to, because their camera and UI root must go in the same command batch.
pub(crate) fn exit_on_os_close(
    mut closes: MessageReader<bevy::window::WindowCloseRequested>,
    primary: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut commands: Commands,
) {
    let Ok(primary) = primary.single() else {
        closes.clear();
        return;
    };
    if closes.read().any(|e| e.window == primary) {
        commands.insert_resource(ExitRequest);
    }
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

/// The card every one of these prompts is: a message over a right-aligned
/// Cancel / Don't Save / *confirm* row.
///
/// Returns `(root, cancel, discard, confirm)` for the caller to tag with its own
/// markers — what differs between the three flows is the wording and what the
/// buttons act on, never the card. The accent goes on the confirm button so
/// `apply_theme` paints it the highlight color rather than the plain one.
/// The unsaved-changes overlay, as `(root, cancel, discard, confirm)`.
///
/// A thin wrapper over [`renzora_ember::widgets::confirm_dialog`], which owns
/// the shape. This used to lay the overlay out itself, and the scene-conflict
/// prompt then wrote the same forty lines again — sizing, padding and the
/// button row being exactly the things that should not drift between two
/// dialogs in the same editor. Kept as a named helper because both call sites
/// below want the same three buttons in the same order, which is a fact about
/// saving rather than about dialogs.
fn spawn_prompt(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: String,
    confirm_label: &str,
) -> (Entity, Entity, Entity, Entity) {
    let (root, buttons) = renzora_ember::widgets::confirm_dialog(
        commands,
        fonts,
        "Unsaved Changes",
        body,
        440.0,
        188.0,
        &["Cancel", "Don't Save", confirm_label],
    );
    let [cancel, discard, confirm] = buttons[..] else {
        unreachable!("confirm_dialog returns one entity per label");
    };
    (root, cancel, discard, confirm)
}

/// Build the centered "unsaved changes" confirmation overlay.
fn spawn_exit_prompt(commands: &mut Commands, fonts: &EmberFonts, count: usize) {
    let body = if count == 1 {
        "You have unsaved changes. Save before closing?".to_string()
    } else {
        format!("You have unsaved changes in {count} documents. Save before closing?")
    };
    let (root, cancel, discard, save) = spawn_prompt(commands, fonts, body, "Save & Close");
    commands.entity(root).insert(ExitPromptRoot);
    commands.entity(cancel).insert(ExitPromptCancel);
    commands.entity(discard).insert(ExitPromptDiscard);
    commands.entity(save).insert(ExitPromptSave);
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
    let body = format!("\"{name}\" has unsaved changes. Save before closing?");
    let (root, cancel, discard, save) = spawn_prompt(commands, fonts, body, "Save & Close");
    commands.entity(root).insert(CloseTabPromptRoot(id));
    commands.entity(cancel).insert(CloseTabPromptCancel);
    commands.entity(discard).insert(CloseTabPromptDiscard);
    commands.entity(save).insert(CloseTabPromptSave);
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

// ── Switch-project save prompt ───────────────────────────────────────────────

/// Where File wants to go when it leaves the current project.
///
/// The File menu asks for *this* rather than acting, because leaving a project
/// closes every document in it — the same loss the window's × prompts about, and
/// until this existed it happened on one click with no warning at all.
#[derive(Clone)]
pub(crate) enum ProjectSwitch {
    /// File > New Project: back to the dashboard to make one.
    New,
    /// File > Open Project: the OS picker.
    Pick,
    /// File > Recent Projects > one of them.
    Recent(std::path::PathBuf),
}

impl ProjectSwitch {
    /// The verb for the confirm button, so it names what is about to happen
    /// rather than a generic "OK".
    fn confirm_label(&self) -> &'static str {
        match self {
            ProjectSwitch::New => "Save & New Project",
            ProjectSwitch::Pick | ProjectSwitch::Recent(_) => "Save & Open",
        }
    }

    /// Actually leave. Queued rather than run inline because two of the three
    /// are `&mut World` handlers owned by `renzora_editor_framework` — the same
    /// ones the menu used to call directly.
    fn perform(self, commands: &mut Commands) {
        match self {
            ProjectSwitch::New => {
                commands.queue(|w: &mut World| renzora_editor_framework::handle_new_project(w));
            }
            ProjectSwitch::Pick => {
                commands.queue(|w: &mut World| renzora_editor_framework::handle_open_project(w));
            }
            ProjectSwitch::Recent(path) => {
                commands.insert_resource(renzora::RequestOpenProjectPath(path));
            }
        }
    }
}

/// Set by a File menu row that leaves the project. Consumed by
/// [`process_project_switch_request`], which either switches straight away or
/// opens the prompt first.
#[derive(Resource)]
pub(crate) struct ProjectSwitchRequest(pub(crate) ProjectSwitch);

/// Set after "Save & …" while we wait for the scene-save to land before leaving
/// (see [`pending_switch_after_save`]). Carries where we were going.
#[derive(Resource)]
pub(crate) struct PendingSwitchAfterSave(ProjectSwitch);

/// Backdrop root of the switch-project prompt, holding the destination so the
/// buttons know what to act on.
#[derive(Component)]
pub(crate) struct SwitchPromptRoot(ProjectSwitch);

/// The prompt's three actions.
#[derive(Component)]
pub(crate) struct SwitchPromptSave;
#[derive(Component)]
pub(crate) struct SwitchPromptDiscard;
#[derive(Component)]
pub(crate) struct SwitchPromptCancel;

/// Handle a pending [`ProjectSwitchRequest`]: switch immediately when nothing is
/// dirty, otherwise open the save-confirmation prompt.
pub(crate) fn process_project_switch_request(
    req: Option<Res<ProjectSwitchRequest>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    fonts: Option<Res<EmberFonts>>,
    open: Query<(), With<SwitchPromptRoot>>,
    mut commands: Commands,
) {
    let Some(req) = req else { return };
    // A prompt is already up — leave the request until it's resolved.
    if !open.is_empty() {
        return;
    }
    let switch = req.0.clone();
    commands.remove_resource::<ProjectSwitchRequest>();

    let dirty = tabs.as_ref().is_some_and(|t| any_unsaved(t));
    // Nothing unsaved (or we can't render the prompt) → go straight there. The
    // second half matters: a missing font must not be a reason a menu row does
    // nothing at all.
    if !dirty || fonts.is_none() {
        switch.perform(&mut commands);
        return;
    }
    let fonts = fonts.unwrap();
    let count = tabs
        .map(|t| t.tabs.iter().filter(|x| x.is_modified).count())
        .unwrap_or(0);
    spawn_switch_prompt(&mut commands, &fonts, switch, count);
}

/// Build the centered "unsaved changes" prompt for leaving the project.
fn spawn_switch_prompt(
    commands: &mut Commands,
    fonts: &EmberFonts,
    switch: ProjectSwitch,
    count: usize,
) {
    let body = if count == 1 {
        "You have unsaved changes. Save before switching projects?".to_string()
    } else {
        format!("You have unsaved changes in {count} documents. Save before switching projects?")
    };
    let label = switch.confirm_label();
    let (root, cancel, discard, save) = spawn_prompt(commands, fonts, body, label);
    commands.entity(root).insert(SwitchPromptRoot(switch));
    commands.entity(cancel).insert(SwitchPromptCancel);
    commands.entity(discard).insert(SwitchPromptDiscard);
    commands.entity(save).insert(SwitchPromptSave);
}

/// Drive the switch prompt's buttons. (Escape / backdrop click / the title × are
/// handled by ember's generic `overlay_dismiss`, which despawns the root — same
/// as Cancel: the project stays open.)
pub(crate) fn switch_prompt_buttons(
    save: Query<&Interaction, (Changed<Interaction>, With<SwitchPromptSave>)>,
    discard: Query<&Interaction, (Changed<Interaction>, With<SwitchPromptDiscard>)>,
    cancel: Query<&Interaction, (Changed<Interaction>, With<SwitchPromptCancel>)>,
    roots: Query<(Entity, &SwitchPromptRoot)>,
    mut commands: Commands,
) {
    let save = save.iter().any(|i| *i == Interaction::Pressed);
    let discard = discard.iter().any(|i| *i == Interaction::Pressed);
    let cancel = cancel.iter().any(|i| *i == Interaction::Pressed);

    if !(save || discard || cancel) {
        return;
    }

    // The destination lives on the root; capture it before despawning.
    let target = roots.iter().next().map(|(_, r)| r.0.clone());
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let Some(switch) = target else { return };

    if save {
        commands.insert_resource(renzora::core::SaveSceneRequested);
        commands.insert_resource(PendingSwitchAfterSave(switch));
    } else if discard {
        switch.perform(&mut commands);
    }
    // cancel → nothing; the switch is abandoned.
}

/// After "Save & …", wait for the scene-save to complete, then leave. If the
/// save was redirected to a Save-As dialog the user cancelled (something is
/// still dirty), abort the switch instead of losing work — the same conservative
/// ending [`pending_exit_after_save`] has, and for the same reason: `Save` saves
/// the *active* scene, so anything still unsaved means the answer to "may I
/// throw this away" is still no.
pub(crate) fn pending_switch_after_save(
    pending: Option<Res<PendingSwitchAfterSave>>,
    save_req: Option<Res<renzora::core::SaveSceneRequested>>,
    save_as_req: Option<Res<renzora::core::SaveAsSceneRequested>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };
    // Still saving (or prompting for a path) — keep waiting.
    if save_req.is_some() || save_as_req.is_some() {
        return;
    }
    let switch = pending.0.clone();
    commands.remove_resource::<PendingSwitchAfterSave>();

    if !tabs.is_some_and(|t| any_unsaved(&t)) {
        switch.perform(&mut commands);
    }
}
