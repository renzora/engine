//! Mesh Edit — Blender-style modeling + sculpting for Bevy meshes.
//!
//! Activates when the viewport is in [`ViewportMode::Edit`] or
//! [`ViewportMode::Sculpt`]. While active, the editor's currently selected
//! entity is promoted into an [`EditMesh`] component and mutated in place.
//! Edits stream back into the source `Mesh` asset every dirty frame and the
//! component drops on exit.
//!
//! Edit mode:
//!   - `Tab` toggles Scene ↔ Edit (registered shortcut, rebindable)
//!   - `1` / `2` / `3` switch vertex / edge / face select (selection flushes
//!     across modes), `A` select-all, Shift+click toggles, Alt+click selects
//!     an edge loop
//!   - `G` grab (X/Y/Z axis lock), `E` extrude, `I` inset, `Ctrl+R` loop cut
//!     (scroll = cuts), `X`/`Del` delete, `Ctrl+X` dissolve, `M` merge at
//!     center
//!   - Panel ops: subdivide, merge by distance, bisect, mirror, array
//!   - X-symmetry mirrors grab edits onto the opposite-side verts
//!
//! Sculpt mode:
//!   - Draw / Smooth / Grab / Inflate / Flatten / Pinch brushes
//!   - `Ctrl` inverts, `Shift` is temporary Smooth, `[` / `]` resize
//!   - X-symmetry applies the brush mirrored across the local X plane

use bevy::prelude::*;
use renzora::core::viewport_types::{ViewportMode, ViewportSettings};
use renzora::keybindings::KeyBinding;
use renzora::{AppEditorExt, ShortcutEntry};
use renzora_editor_framework::sdk::conditions::in_mode;

pub mod edit_mesh;
pub mod native;
pub mod operators;
pub mod sculpt;
pub mod selection;
pub mod systems;
pub mod toolbar;
pub mod tools;
pub mod undo;

pub use edit_mesh::{EdgeId, EditMesh, FaceId, VertexId};
pub use sculpt::{BrushKind, SculptBrush};
pub use selection::{MeshSelection, SelectMode};
pub use tools::{ModelingOp, ModelingSettings, PendingOps};

#[derive(Default)]
pub struct MeshEditPlugin;

impl Plugin for MeshEditPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MeshEditPlugin");
        app.add_plugins(native::NativeModeling);
        toolbar::register(app);
        app.init_resource::<MeshSelection>()
            .init_resource::<systems::GrabState>()
            .init_resource::<systems::EditModeActive>()
            .init_resource::<tools::ModelingSettings>()
            .init_resource::<tools::PendingOps>()
            .init_resource::<tools::LoopCutState>()
            .init_resource::<sculpt::SculptBrush>()
            .init_resource::<sculpt::SculptHover>()
            .init_resource::<sculpt::SculptStroke>();

        // Tab toggles Scene ↔ Edit (and exits Sculpt). Registered as a
        // plugin shortcut: rebindable in Settings → Shortcuts, and the
        // dispatcher already skips it while a text field has focus.
        app.register_shortcut(ShortcutEntry::new(
            "mesh_edit.toggle_edit_mode",
            "Toggle Edit Mode",
            "Modeling",
            KeyBinding::new(KeyCode::Tab),
            tools::toggle_edit_mode,
        ));

        // Lifecycle + bake are shared by Edit and Sculpt: both need the
        // promoted EditMesh and the dirty→asset stream.
        app.add_systems(
            Update,
            (
                systems::enter_edit_mode,
                systems::bake_if_dirty,
                tools::update_mode_status,
            )
                .chain()
                .run_if(in_editing_mode),
        )
        .add_systems(
            Update,
            (systems::exit_edit_mode, tools::update_mode_status).run_if(not_in_editing_mode),
        );

        // Edit-mode interaction.
        app.add_systems(
            Update,
            (
                systems::switch_select_mode,
                systems::select_all_toggle,
                tools::op_shortcuts,
                tools::apply_pending_ops,
                tools::loop_cut_modal,
                systems::extrude_system,
                systems::grab_start,
                systems::grab_update,
                systems::pick_element,
                tools::loop_select,
                systems::draw_overlay,
            )
                .chain()
                .run_if(in_mode(ViewportMode::Edit)),
        );

        // Scene-mode: Ctrl+J joins the selected mesh entities into one.
        app.add_systems(
            Update,
            tools::join_selected
                .run_if(in_mode(ViewportMode::Scene))
                .run_if(renzora::core::not_in_play_mode),
        );

        // Sculpt-mode interaction.
        app.add_systems(
            Update,
            (
                sculpt::brush_size_keys,
                sculpt::sculpt_stroke,
                sculpt::draw_brush_cursor,
                tools::apply_pending_ops,
            )
                .chain()
                .run_if(in_mode(ViewportMode::Sculpt)),
        );
    }
}

fn in_editing_mode(s: Option<Res<ViewportSettings>>) -> bool {
    s.map(|s| {
        matches!(
            s.viewport_mode,
            ViewportMode::Edit | ViewportMode::Sculpt
        )
    })
    .unwrap_or(false)
}

fn not_in_editing_mode(s: Option<Res<ViewportSettings>>) -> bool {
    !in_editing_mode(s)
}

renzora::add!(MeshEditPlugin, Editor);
