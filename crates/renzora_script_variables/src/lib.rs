mod native;

use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ScriptVariablesPlugin;

impl Plugin for ScriptVariablesPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ScriptVariablesPlugin");
        native::register_native_script_variables(app);
    }
}

renzora::add!(ScriptVariablesPlugin, Editor);
