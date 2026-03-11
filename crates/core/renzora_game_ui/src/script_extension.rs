//! Game UI script extension — exposes UI manipulation functions to Lua/Rhai scripts.
//!
//! ## Script Functions
//!
//! | Function | Description |
//! |----------|-------------|
//! | `ui_show(name)` | Show a named UI canvas |
//! | `ui_hide(name)` | Hide a named UI canvas |
//! | `ui_toggle(name)` | Toggle a named UI canvas visibility |
//! | `ui_set_text(name, text)` | Set text content on a named widget |
//! | `ui_set_progress(name, value)` | Set progress bar value (0.0–1.0) |
//! | `ui_set_health(name, current, max)` | Set health bar values |
//! | `ui_set_slider(name, value)` | Set slider value |
//! | `ui_set_checkbox(name, checked)` | Set checkbox state |
//! | `ui_set_toggle(name, on)` | Set toggle state |
//! | `ui_set_visible(name, visible)` | Set widget visibility |
//! | `ui_set_theme(theme_name)` | Switch theme ("dark", "light", "high_contrast") |
//! | `ui_set_color(name, r, g, b, a)` | Set widget background color |

use bevy::prelude::*;
use renzora_scripting::extension::{ExtensionData, ScriptExtension};
use renzora_scripting::macros::push_ext_command;
use renzora_scripting::systems::execution::ScriptCommandQueue;
use renzora_scripting::{ScriptCommand, ScriptingSet};

use crate::components::*;

// ── Command enum ─────────────────────────────────────────────────────────

renzora_scripting::script_extension_command! {
    #[derive(Debug)]
    pub enum UiScriptCommand {
        Show { name: String },
        Hide { name: String },
        Toggle { name: String },
        SetText { name: String, text: String },
        SetProgress { name: String, value: f32 },
        SetHealth { name: String, current: f32, max: f32 },
        SetSlider { name: String, value: f32 },
        SetCheckbox { name: String, checked: bool },
        SetToggle { name: String, on: bool },
        SetVisible { name: String, visible: bool },
        SetTheme { theme_name: String },
        SetColor { name: String, r: f32, g: f32, b: f32, a: f32 },
    }
}

// ── Script function registration ─────────────────────────────────────────

renzora_scripting::dual_register! {
    lua_fn = register_ui_lua,
    rhai_fn = register_ui_rhai,

    fn ui_show(name: String) {
        push_ext_command(UiScriptCommand::Show { name });
    }

    fn ui_hide(name: String) {
        push_ext_command(UiScriptCommand::Hide { name });
    }

    fn ui_toggle(name: String) {
        push_ext_command(UiScriptCommand::Toggle { name });
    }

    fn ui_set_text(name: String, text: String) {
        push_ext_command(UiScriptCommand::SetText { name, text });
    }

    fn ui_set_progress(name: String, value: f64) {
        push_ext_command(UiScriptCommand::SetProgress { name, value: value as f32 });
    }

    fn ui_set_health(name: String, current: f64, max: f64) {
        push_ext_command(UiScriptCommand::SetHealth {
            name,
            current: current as f32,
            max: max as f32,
        });
    }

    fn ui_set_slider(name: String, value: f64) {
        push_ext_command(UiScriptCommand::SetSlider { name, value: value as f32 });
    }

    fn ui_set_checkbox(name: String, checked: bool) {
        push_ext_command(UiScriptCommand::SetCheckbox { name, checked });
    }

    fn ui_set_toggle(name: String, on: bool) {
        push_ext_command(UiScriptCommand::SetToggle { name, on });
    }

    fn ui_set_visible(name: String, visible: bool) {
        push_ext_command(UiScriptCommand::SetVisible { name, visible });
    }

    fn ui_set_theme(theme_name: String) {
        push_ext_command(UiScriptCommand::SetTheme { theme_name });
    }

    fn ui_set_color(name: String, r: f64, g: f64, b: f64, a: f64) {
        push_ext_command(UiScriptCommand::SetColor {
            name, r: r as f32, g: g as f32, b: b as f32, a: a as f32,
        });
    }
}

// ── Extension implementation ─────────────────────────────────────────────

pub struct GameUiScriptExtension;

impl ScriptExtension for GameUiScriptExtension {
    fn name(&self) -> &str {
        "GameUI"
    }

    fn populate_context(&self, _world: &World, _entity: Entity, _data: &mut ExtensionData) {
        // Future: could populate with UI state data (which canvases are visible, etc.)
    }

    #[cfg(feature = "lua")]
    fn register_lua_functions(&self, lua: &mlua::Lua) {
        register_ui_lua(lua);
    }

    #[cfg(feature = "rhai")]
    fn register_rhai_functions(&self, engine: &mut rhai::Engine) {
        register_ui_rhai(engine);
    }
}

// ── Command processing system ────────────────────────────────────────────

/// Processes `UiScriptCommand`s from the script command queue.
pub fn process_ui_script_commands(
    cmd_queue: Res<ScriptCommandQueue>,
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
    for (_source_entity, cmd) in &cmd_queue.commands {
        let ScriptCommand::Extension(ext_cmd) = cmd else { continue };
        let Some(ui_cmd) = ext_cmd.as_any().downcast_ref::<UiScriptCommand>() else { continue };

        match ui_cmd {
            UiScriptCommand::Show { name } => {
                for (n, mut vis) in &mut canvases {
                    if n.as_str() == name {
                        *vis = Visibility::Inherited;
                    }
                }
            }
            UiScriptCommand::Hide { name } => {
                for (n, mut vis) in &mut canvases {
                    if n.as_str() == name {
                        *vis = Visibility::Hidden;
                    }
                }
            }
            UiScriptCommand::Toggle { name } => {
                for (n, mut vis) in &mut canvases {
                    if n.as_str() == name {
                        *vis = match *vis {
                            Visibility::Hidden => Visibility::Inherited,
                            _ => Visibility::Hidden,
                        };
                    }
                }
            }
            UiScriptCommand::SetText { name, text } => {
                for (n, mut t) in &mut texts {
                    if n.as_str() == name {
                        t.0 = text.clone();
                    }
                }
            }
            UiScriptCommand::SetProgress { name, value } => {
                for (n, mut data) in &mut progress_bars {
                    if n.as_str() == name {
                        data.value = *value;
                    }
                }
            }
            UiScriptCommand::SetHealth { name, current, max } => {
                for (n, mut data) in &mut health_bars {
                    if n.as_str() == name {
                        data.current = *current;
                        data.max = *max;
                    }
                }
            }
            UiScriptCommand::SetSlider { name, value } => {
                for (n, mut data) in &mut sliders {
                    if n.as_str() == name {
                        data.value = *value;
                    }
                }
            }
            UiScriptCommand::SetCheckbox { name, checked } => {
                for (n, mut data) in &mut checkboxes {
                    if n.as_str() == name {
                        data.checked = *checked;
                    }
                }
            }
            UiScriptCommand::SetToggle { name, on } => {
                for (n, mut data) in &mut toggles {
                    if n.as_str() == name {
                        data.on = *on;
                    }
                }
            }
            UiScriptCommand::SetVisible { name, visible } => {
                for (n, mut vis) in &mut widgets_vis {
                    if n.as_str() == name {
                        *vis = if *visible {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        };
                    }
                }
            }
            UiScriptCommand::SetTheme { theme_name } => {
                let theme = match theme_name.as_str() {
                    "light" => UiTheme::light(),
                    "high_contrast" => UiTheme::high_contrast(),
                    _ => UiTheme::dark(),
                };
                commands.insert_resource(theme);
            }
            UiScriptCommand::SetColor { name, r, g, b, a } => {
                for (n, mut bg) in &mut bg_colors {
                    if n.as_str() == name {
                        bg.0 = Color::srgba(*r, *g, *b, *a);
                    }
                }
            }
        }
    }
}
