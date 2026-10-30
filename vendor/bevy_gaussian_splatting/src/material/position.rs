use bevy::{
    asset::uuid_handle,
    prelude::*,
};

const POSITION_SHADER_HANDLE: Handle<Shader> = uuid_handle!("91ad4ad8-5e95-4f30-a262-7d3de4abd5a8");

pub struct PositionMaterialPlugin;

impl Plugin for PositionMaterialPlugin {
    fn build(&self, app: &mut App) {
        crate::load_wesl!(
            app,
            POSITION_SHADER_HANDLE,
            "position.wesl",
            "material/position"
        );
    }
}
