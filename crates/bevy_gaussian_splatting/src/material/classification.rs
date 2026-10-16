use bevy::{
    asset::uuid_handle,
    prelude::*,
};

const CLASSIFICATION_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("8b453dba-5095-47f2-9c60-ae369fe51579");

pub struct ClassificationMaterialPlugin;

impl Plugin for ClassificationMaterialPlugin {
    fn build(&self, app: &mut App) {
        crate::load_wesl!(
            app,
            CLASSIFICATION_SHADER_HANDLE,
            "classification.wesl",
            "material/classification"
        );
    }
}
