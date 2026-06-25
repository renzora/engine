//! `renzora_ember` — Renzora's unified **bevy_ui** UI framework.
//!
//! One crate, used by both the editor and exported games, so UI is authored the
//! same way everywhere (Godot-style). Deliberately **no feature flags**: feature
//! deltas would shift `bevy_dylib`'s SVH and re-split the editor/runtime builds
//! that the plugin ABI relies on (see `docs/editor-runtime-plugin-architecture.md`),
//! so everything ships together.
//!
//! Modules:
//! - [`theme`] — the bevy-native palette (raw shared colors).
//! - [`style`] — the runtime theming system (`Theme` tokens + `Styled` + repaint).
//! - [`font`] — fonts + text/icon helpers.
//! - [`dock`] — the dockable panel layout component.
//! - [`widgets`] — reusable UI components (buttons, toggles, …).
//!
//! Add [`EmberPlugin`] to register the theme + dock + widget systems. The
//! [`markup`] runtime (folded in from the former `renzora_hui`) installs itself
//! via `renzora::add!` so it runs in games and the editor alike. Migrating in
//! next: `cinder` (particle UI).

use bevy::prelude::*;
use bevy::ui::{CalculatedClip, ComputedStackIndex, RelativeCursorPosition, UiSystems};
use bevy::window::PrimaryWindow;

use widgets::{ModalSurface, OverlaySurface};

pub mod cursor_icon;
pub mod dock;
pub mod font;
pub mod icons;
pub mod inspector;
pub mod markup;
pub mod panel;
pub mod phosphor_map;
pub mod reactive;
pub mod settings_sections;
pub mod style;
pub mod theme;
pub mod toolbar;
pub mod virtual_scroll;
pub mod widgets;

/// Registers all of ember's runtime systems (theme + dock + widgets + fonts +
/// the reactive bindings/keyed-list drivers).
pub struct EmberPlugin;

impl Plugin for EmberPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            style::ThemePlugin,
            dock::DockPlugin,
            widgets::WidgetsPlugin,
            reactive::ReactivePlugin,
            virtual_scroll::VirtualScrollPlugin,
        ));
        toolbar::register(app);
        // Correct pointer state (clip + occlusion) — see `correct_pointer_state`.
        // Runs right after `UiSystems::Focus` (the sole writer of `Interaction` /
        // `RelativeCursorPosition`) so every consumer reads the corrected values.
        app.add_systems(PreUpdate, correct_pointer_state.after(UiSystems::Focus));
    }
}

/// Fix two gaps in Bevy 0.19's pointer hit-testing — **clipping** and
/// **occlusion** — so a click or wheel can never land on a widget the user can't
/// actually see/reach, and never fires in two places at once.
///
/// 1. **Clipping.** Bevy's picking clip check (`clip_check_recursive`) only clips
///    a node against a *contiguous* chain of clipping ancestors: the first
///    `overflow: visible` ancestor — the default for a scroll area's content
///    container, or the UI-canvas design frame — ends the walk, so the node is
///    treated as unclipped even when a scroll viewport higher up hides it. So an
///    interactive widget nested in a scroll area stays clickable after it scrolls
///    out of view. `CalculatedClip` does NOT have this gap (`update_clipping`
///    inherits it through visible nodes), so we re-test against it.
///
/// 2. **Occlusion.** `FocusPolicy::Block` on an overlay stops `Interaction` from
///    reaching nodes behind it, but it leaves their `RelativeCursorPosition`
///    `cursor_over` set. Every panel-level wheel/drag handler (timeline zoom,
///    code-editor scroll, canvas zoom, …) gates on `cursor_over`, so they all
///    fire *through* an open dropdown/menu/modal onto the panel behind it. We
///    clear `cursor_over` (and `Interaction`) for any node sitting below the
///    topmost overlay/modal under the cursor that the node isn't part of — so
///    those handlers stand down without each needing its own overlay check.
///
/// Both `CalculatedClip` (PostUpdate) and the cursor are physical, window-global
/// pixels — the same basis `ui_focus_system` read a frame earlier. Only nodes
/// that currently carry pointer state are inspected, so the per-frame cost is the
/// handful of nodes actually under the cursor.
fn correct_pointer_state(
    windows: Query<&Window, With<PrimaryWindow>>,
    // A `ParamSet`: the blocker query reads `RelativeCursorPosition` while the
    // node query writes it, and an overlay entity is in both — so they alias and
    // must not be borrowed at once. We read `p0` to pick the blocker, then write
    // through `p1`.
    mut queries: ParamSet<(
        // p0 — pointer-blocking surfaces: floating overlays (dropdowns / menus /
        // popups) and modals. A modal blocks everything behind it; an overlay
        // blocks what's under the cursor and stacked below it.
        Query<
            (
                Entity,
                Option<&RelativeCursorPosition>,
                &ComputedStackIndex,
                &Node,
                Has<ModalSurface>,
            ),
            Or<(With<OverlaySurface>, With<ModalSurface>)>,
        >,
        // p1 — every node carrying pointer state, to be corrected.
        Query<
            (
                Entity,
                Option<&mut Interaction>,
                Option<&mut RelativeCursorPosition>,
                &ComputedStackIndex,
                Option<&CalculatedClip>,
            ),
            Or<(With<Interaction>, With<RelativeCursorPosition>)>,
        >,
    )>,
    parents: Query<&ChildOf>,
) {
    let Some(cursor) = windows.iter().next().and_then(|w| w.physical_cursor_position()) else {
        return;
    };

    // The surface capturing the pointer. A modal wins unconditionally (it covers
    // everything behind it); otherwise it's the highest visible overlay under the
    // cursor. Resolved into an owned value so the blocker borrow ends before we
    // mutate the node query.
    let top_blocker: Option<(Entity, u32)> = {
        let blockers = queries.p0();
        let top_modal = blockers
            .iter()
            .filter(|(_, _, _, node, is_modal)| *is_modal && node.display != Display::None)
            .map(|(e, _, si, _, _)| (e, si.0))
            .max_by_key(|(_, si)| *si);
        top_modal.or_else(|| {
            blockers
                .iter()
                .filter(|(_, rcp, _, node, is_modal)| {
                    !is_modal && node.display != Display::None && rcp.is_some_and(|r| r.cursor_over)
                })
                .map(|(e, _, si, _, _)| (e, si.0))
                .max_by_key(|(_, si)| *si)
        })
    };

    for (e, interaction, rcp, si, clip) in &mut queries.p1() {
        // Skip nodes with nothing to clear — avoids the ancestor walk for the
        // vast majority that aren't under the cursor.
        let active = interaction.as_deref().is_some_and(|i| *i != Interaction::None)
            || rcp.as_deref().is_some_and(|r| r.cursor_over);
        if !active {
            continue;
        }
        let clipped = clip.is_some_and(|c| !c.clip.contains(cursor));
        let occluded = top_blocker.is_some_and(|(b, bsi)| {
            e != b && si.0 < bsi && !is_descendant_of(e, b, &parents)
        });
        if !clipped && !occluded {
            continue;
        }
        if let Some(mut interaction) = interaction {
            if *interaction != Interaction::None {
                interaction.set_if_neq(Interaction::None);
            }
        }
        if let Some(mut rcp) = rcp {
            // `normalized` is left intact — some callers (e.g. the canvas
            // marquee) intentionally use it past the node's bounds.
            if rcp.cursor_over {
                rcp.cursor_over = false;
            }
        }
    }
}

/// Is `e` a descendant of `ancestor` in the UI tree?
fn is_descendant_of(mut e: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    while let Ok(c) = parents.get(e) {
        let p = c.parent();
        if p == ancestor {
            return true;
        }
        e = p;
    }
    false
}
