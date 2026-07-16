use bevy::{
    asset::{load_internal_asset, uuid_handle},
    prelude::*,
};

const CLASSIFICATION_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("8b453dba-5095-47f2-9c60-ae369fe51579");

pub struct ClassificationMaterialPlugin;

impl Plugin for ClassificationMaterialPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CLASSIFICATION_SHADER_HANDLE,
            "classification.wgsl",
            Shader::from_wgsl
        );
    }
}
