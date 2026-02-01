use bevy::prelude::*;

use crate::{ComputedOutline, InheritOutline, OutlineMode, OutlineStencil, OutlineVolume};

/// A bundle for rendering stenciled outlines around meshes.
#[derive(Bundle, Clone, Default)]
pub struct OutlineBundle {
    pub outline: OutlineVolume,
    pub stencil: OutlineStencil,
    pub mode: OutlineMode,
    pub computed: ComputedOutline,
}

/// A bundle for stenciling meshes in the outlining pass.
#[derive(Bundle, Clone, Default)]
pub struct OutlineStencilBundle {
    pub stencil: OutlineStencil,
    pub mode: OutlineMode,
    pub computed: ComputedOutline,
}

/// A bundle for inheriting outlines from the parent entity.
#[derive(Bundle, Clone, Default)]
pub struct InheritOutlineBundle {
    pub inherit: InheritOutline,
    pub computed: ComputedOutline,
}
