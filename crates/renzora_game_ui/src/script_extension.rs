//! Game UI script integration — handles UI manipulation actions from scripts.
//!
//! Scripts use the generic `action()` function to control UI:
//!
//! ```lua
//! action("ui_show", { name = "HUD" })
//! action("ui_hide", { name = "PauseMenu" })
//! action("ui_set_text", { name = "ScoreLabel", text = "Score: 100" })
//! action("ui_set_progress", { name = "HealthBar", value = 0.75 })
//! ```
//!
//! Or use the convenience stdlib functions (from `scripts/lib/ui.lua`):
//! ```lua
//! ui_show("HUD")
//! ui_set_text("ScoreLabel", "Score: 100")
//! ```

use bevy::prelude::*;
use renzora::ScriptAction;

use crate::components::*;

/// System that handles UI-related ScriptAction events.
pub fn handle_ui_script_actions(
    trigger: On<ScriptAction>,
    mut canvases: Query<(&Name, &mut Visibility), With<UiCanvas>>,
    mut widgets_vis: Query<(&Name, &mut Visibility), (With<UiWidget>, Without<UiCanvas>)>,
    mut texts: Query<(&Name, &mut bevy::ui::widget::Text), With<UiWidget>>,
    mut progress_bars: Query<(&Name, &mut ProgressBarData)>,
    mut health_bars: Query<(&Name, &mut HealthBarData)>,
    mut sliders: Query<(&Name, &mut SliderData)>,
    mut checkboxes: Query<(&Name, &mut CheckboxData)>,
    mut toggles: Query<(&Name, &mut ToggleData)>,
    mut bg_colors: Query<(&Name, &mut BackgroundColor), With<UiWidget>>,
    mut commands: Commands,
) {
    use renzora::ScriptActionValue;
    let action = trigger.event();

    // Helper to extract a string arg
    let get_str = |key: &str| -> Option<&str> {
        match action.args.get(key) {
            Some(ScriptActionValue::String(s)) => Some(s.as_str()),
            _ => None,
        }
    };
    let get_f32 = |key: &str| -> Option<f32> {
        match action.args.get(key) {
            Some(ScriptActionValue::Float(v)) => Some(*v),
            Some(ScriptActionValue::Int(v)) => Some(*v as f32),
            _ => None,
        }
    };
    let get_bool = |key: &str| -> Option<bool> {
        match action.args.get(key) {
            Some(ScriptActionValue::Bool(v)) => Some(*v),
            _ => None,
        }
    };

    match action.name.as_str() {
        "ui_show" => {
            let Some(name) = get_str("name") else { return };
            for (n, mut vis) in &mut canvases {
                if n.as_str() == name {
                    *vis = Visibility::Inherited;
                }
            }
        }
        "ui_hide" => {
            let Some(name) = get_str("name") else { return };
            for (n, mut vis) in &mut canvases {
                if n.as_str() == name {
                    *vis = Visibility::Hidden;
                }
            }
        }
        "ui_toggle" => {
            let Some(name) = get_str("name") else { return };
            for (n, mut vis) in &mut canvases {
                if n.as_str() == name {
                    *vis = match *vis {
                        Visibility::Hidden => Visibility::Inherited,
                        _ => Visibility::Hidden,
                    };
                }
            }
        }
        "ui_set_text" => {
            let Some(name) = get_str("name") else { return };
            let Some(text) = get_str("text") else { return };
            for (n, mut t) in &mut texts {
                if n.as_str() == name {
                    t.0 = text.to_string();
                }
            }
        }
        "ui_set_progress" => {
            let Some(name) = get_str("name") else { return };
            let Some(value) = get_f32("value") else { return };
            for (n, mut data) in &mut progress_bars {
                if n.as_str() == name {
                    data.value = value;
                }
            }
        }
        "ui_set_health" => {
            let Some(name) = get_str("name") else { return };
            let current = get_f32("current").unwrap_or(0.0);
            let max = get_f32("max").unwrap_or(100.0);
            for (n, mut data) in &mut health_bars {
                if n.as_str() == name {
                    data.current = current;
                    data.max = max;
                }
            }
        }
        "ui_set_slider" => {
            let Some(name) = get_str("name") else { return };
            let Some(value) = get_f32("value") else { return };
            for (n, mut data) in &mut sliders {
                if n.as_str() == name {
                    data.value = value;
                }
            }
        }
        "ui_set_checkbox" => {
            let Some(name) = get_str("name") else { return };
            let Some(checked) = get_bool("checked") else { return };
            for (n, mut data) in &mut checkboxes {
                if n.as_str() == name {
                    data.checked = checked;
                }
            }
        }
        "ui_set_toggle" => {
            let Some(name) = get_str("name") else { return };
            let Some(on) = get_bool("on") else { return };
            for (n, mut data) in &mut toggles {
                if n.as_str() == name {
                    data.on = on;
                }
            }
        }
        "ui_set_visible" => {
            let Some(name) = get_str("name") else { return };
            let Some(visible) = get_bool("visible") else { return };
            for (n, mut vis) in &mut widgets_vis {
                if n.as_str() == name {
                    *vis = if visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
        "ui_set_theme" => {
            let Some(theme_name) = get_str("theme") else { return };
            let theme = match theme_name {
                "light" => UiTheme::light(),
                "high_contrast" => UiTheme::high_contrast(),
                _ => UiTheme::dark(),
            };
            commands.insert_resource(theme);
        }
        "ui_set_color" => {
            let Some(name) = get_str("name") else { return };
            let r = get_f32("r").unwrap_or(1.0);
            let g = get_f32("g").unwrap_or(1.0);
            let b = get_f32("b").unwrap_or(1.0);
            let a = get_f32("a").unwrap_or(1.0);
            for (n, mut bg) in &mut bg_colors {
                if n.as_str() == name {
                    bg.0 = Color::srgba(r, g, b, a);
                }
            }
        }
        _ => {} // Not a UI action — ignore
    }
}
