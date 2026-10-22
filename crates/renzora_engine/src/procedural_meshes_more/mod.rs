//! The second wave of built-in shapes: polyhedra, blocks, architecture,
//! turning stairs, pipe fittings and props.
//!
//! A directory beside [`procedural_meshes`](crate::procedural_meshes) rather
//! than more of it: that file was already past the size this codebase splits
//! at, and these generators share a builder the older ones do not use. Each
//! group lives in its own module, and everything is re-exported flat so the
//! shape table reads `pm2::create_gear_mesh` without caring which file the
//! gear is in.
//!
//! Conventions every generator here keeps, and the tests check:
//!
//! - **Pivot at the bounding-box centre**, like the older shapes, except the
//!   pipe tee and cross, which pivot on their hub so their openings sit on
//!   axes through the origin.
//! - **Closed, outward-wound, unit normals.** Hard edges have split vertices
//!   and face normals; curved surfaces have shared analytic normals.
//! - **UVs in metres**, so a tiling material keeps one scale across shapes.
//!   The blockout grid re-projects UVs from geometry on its own, so this is
//!   what a shape wears once a real material replaces the grid.

mod architecture;
mod blocks;
mod builder;
mod pipes;
mod primitives;
mod props;
mod stairs;

#[cfg(test)]
mod tests;

pub use architecture::*;
pub use blocks::*;
pub use pipes::*;
pub use primitives::{
    create_disc_mesh, create_dodecahedron_mesh, create_domed_cylinder_mesh, create_ellipsoid_mesh,
    create_hex_prism_mesh, create_icosahedron_mesh, create_octahedron_mesh, create_rounded_cube_mesh,
    create_star_prism_mesh, create_tetrahedron_mesh,
};
pub use props::*;
pub use stairs::*;
