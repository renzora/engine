//! Inspector widgets for the editor
//!
//! This module contains all the inspector UI widgets for editing entity components.

#![allow(dead_code)]

pub mod camera;
pub mod environment;
pub mod lights;
pub mod mesh_instance;
pub mod nodes2d;
pub mod physics;
pub mod script;
pub mod transform;
pub mod ui_nodes;

pub use camera::{render_camera_inspector, render_camera_rig_inspector};
pub use environment::render_world_environment_inspector;
pub use lights::{
    render_directional_light_inspector, render_point_light_inspector, render_spot_light_inspector,
};
#[allow(unused_imports)]
pub use mesh_instance::render_mesh_instance_inspector;
pub use nodes2d::{render_camera2d_inspector, render_sprite2d_inspector};
pub use physics::{render_collision_shape_inspector, render_physics_body_inspector};
pub use script::render_script_inspector;
pub use transform::render_transform_inspector;
pub use ui_nodes::{
    render_ui_button_inspector, render_ui_image_inspector, render_ui_label_inspector,
    render_ui_panel_inspector,
};
