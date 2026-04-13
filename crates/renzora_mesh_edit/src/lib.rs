//! Mesh Edit — Blender-style vertex / edge / face editing for Bevy meshes.
//!
//! Activates when the viewport is in [`ViewportMode::Edit`]. While active,
//! the editor's currently selected entity is promoted into an [`EditMesh`]
//! component and mutated in place. On exit the edits bake back to the
//! source `Mesh` asset.
//!
//! Phase 2 capabilities:
//!   - Vertex / edge / face picking (click, shift-click to toggle)
//!   - 1 / 2 / 3 switches select mode (like Blender)
//!   - A toggles select-all
//!   - G starts grab translation on the view plane; LMB commits, Esc/RMB cancels
//!   - Automatic bake to the Mesh asset on dirty
//!
//! Planned:
//!   - Phase 3: extrude, inset, delete, merge, subdivide
//!   - Phase 4: loop cut, bevel, bridge, knife
//!   - Phase 5: normals, undo integration, UV preservation

use bevy::prelude::*;
use renzora::core::viewport_types::ViewportMode;
use renzora::editor::AppEditorExt;
use renzora::sdk::conditions::in_mode;

pub mod edit_mesh;
pub mod header;
pub mod operators;
pub mod selection;
pub mod systems;
pub mod undo;

pub use edit_mesh::{EditMesh, EdgeId, FaceId, VertexId};
pub use selection::{MeshSelection, SelectMode};

#[derive(Default)]
pub struct MeshEditPlugin;

impl Plugin for MeshEditPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] MeshEditPlugin");
        app.init_resource::<MeshSelection>()
            .init_resource::<systems::GrabState>()
            .init_resource::<systems::EditModeActive>()
            .register_mode_options(ViewportMode::Edit, header::draw_edit_header)
            .add_systems(
                Update,
                (
                    systems::enter_edit_mode,
                    systems::switch_select_mode,
                    systems::select_all_toggle,
                    systems::extrude_system,
                    systems::grab_start,
                    systems::grab_update,
                    systems::pick_element,
                    systems::bake_if_dirty,
                    systems::draw_overlay,
                )
                    .chain()
                    .run_if(in_mode(ViewportMode::Edit)),
            )
            .add_systems(
                Update,
                systems::exit_edit_mode.run_if(not_in_edit_mode),
            );
    }
}

renzora::add!(MeshEditPlugin, Editor);

fn not_in_edit_mode(
    s: Option<Res<renzora::core::viewport_types::ViewportSettings>>,
) -> bool {
    s.map(|s| s.viewport_mode != ViewportMode::Edit).unwrap_or(true)
}
