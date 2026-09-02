//! Node emitters, one module per node category.
//!
//! Each module is an `impl Ctx` block holding the match arms for its
//! `"<category>/*"` prefix; [`super::ctx::Ctx::gen_node_body`] splits the node
//! type on `/` and dispatches to exactly one of them. This was a single
//! ~1,970-line match before — the categories were already the only structure
//! it had, marked by comment banners.
//!
//! Named `emit` rather than `nodes` because `material::nodes` already exists
//! and holds the node *definitions* (pin templates); this holds the code that
//! turns an instance of one into WGSL.

mod animation;
mod color;
mod control;
mod functions;
mod inputs;
mod math;
mod procedural;
mod scene;
mod textures;
