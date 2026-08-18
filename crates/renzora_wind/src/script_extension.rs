//! Wind scripting bindings — declared here, built by whichever language
//! backend is loaded.
//!
//! Reading the wind goes through the reflection path
//! (`get("WindState.speed")`), so only the two mutations need declaring.

use renzora_scripting::extension::{Bind, Binding, ParamKind, ScriptExtension};

pub struct WindScriptExtension;

impl ScriptExtension for WindScriptExtension {
    fn name(&self) -> &str {
        "wind"
    }

    fn bindings(&self) -> Vec<Binding> {
        vec![
            Bind::action("set_wind", "set_wind")
                .arg("speed", ParamKind::Float)
                .arg("direction", ParamKind::Float)
                .doc("Set the world wind: speed in m/s, direction in degrees the wind travels toward.")
                .build(),
            Bind::action("set_wind_gusts", "set_wind_gusts")
                .arg("strength", ParamKind::Float)
                .arg("frequency", ParamKind::Float)
                .arg("turbulence", ParamKind::Float)
                .doc("Set gust depth (0-1), gusts per second, and cross-wind turbulence (0-1).")
                .build(),
        ]
    }
}
