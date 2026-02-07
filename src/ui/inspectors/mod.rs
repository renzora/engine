//! Inspector widgets for the editor
//!
//! This module contains shared inspector utility widgets.

pub mod transform;
pub mod utils;

pub use utils::sanitize_f32;
pub use transform::render_transform_inspector;
