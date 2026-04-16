//! Script API autocomplete registry.
//!
//! A hand-maintained list of the Lua/Rhai symbols exposed by
//! `renzora_scripting` and `renzora_game_ui::script_extension`. When the Lua
//! backend grows new functions, add them here too — there is no auto-generator
//! yet.

use crate::highlight::Language;

#[derive(Debug, Clone, Copy)]
pub struct ApiSymbol {
    pub name: &'static str,
    pub signature: &'static str,
    pub category: &'static str,
    pub doc: &'static str,
    /// Which languages expose this symbol.
    pub langs: &'static [Language],
}

const LUA: &[Language] = &[Language::Lua];
const LUA_RHAI: &[Language] = &[Language::Lua, Language::Rhai];

/// All known script API symbols. Sorted by category then name.
pub const SYMBOLS: &[ApiSymbol] = &[
    // ── Transform ──
    ApiSymbol { name: "set_position",        signature: "set_position(x, y, z)",        category: "Transform", doc: "Set the entity's world position.",           langs: LUA_RHAI },
    ApiSymbol { name: "set_rotation",        signature: "set_rotation(x, y, z)",        category: "Transform", doc: "Set the entity's rotation (Euler, radians).", langs: LUA_RHAI },
    ApiSymbol { name: "set_scale",           signature: "set_scale(x, y, z)",           category: "Transform", doc: "Set the entity's scale per axis.",            langs: LUA_RHAI },
    ApiSymbol { name: "set_scale_uniform",   signature: "set_scale_uniform(s)",         category: "Transform", doc: "Set uniform scale on all axes.",              langs: LUA },
    ApiSymbol { name: "translate",           signature: "translate(x, y, z)",           category: "Transform", doc: "Offset the entity by (x, y, z).",             langs: LUA_RHAI },
    ApiSymbol { name: "rotate",              signature: "rotate(x, y, z)",              category: "Transform", doc: "Rotate the entity by Euler (x, y, z).",       langs: LUA_RHAI },
    ApiSymbol { name: "look_at",             signature: "look_at(x, y, z)",             category: "Transform", doc: "Orient the entity to face a point.",          langs: LUA_RHAI },
    ApiSymbol { name: "parent_set_position", signature: "parent_set_position(x, y, z)", category: "Transform", doc: "Set the parent entity's position.",           langs: LUA },
    ApiSymbol { name: "parent_set_rotation", signature: "parent_set_rotation(x, y, z)", category: "Transform", doc: "Set the parent entity's rotation.",           langs: LUA },
    ApiSymbol { name: "parent_translate",    signature: "parent_translate(x, y, z)",    category: "Transform", doc: "Translate the parent entity.",                langs: LUA },
    ApiSymbol { name: "set_child_position",  signature: "set_child_position(name, x, y, z)", category: "Transform", doc: "Set a child's position by name.",        langs: LUA },
    ApiSymbol { name: "set_child_rotation",  signature: "set_child_rotation(name, x, y, z)", category: "Transform", doc: "Set a child's rotation by name.",        langs: LUA },
    ApiSymbol { name: "child_translate",     signature: "child_translate(name, x, y, z)",     category: "Transform", doc: "Translate a named child entity.",        langs: LUA },

    // ── Input ──
    ApiSymbol { name: "is_key_pressed",           signature: "is_key_pressed(key)",            category: "Input", doc: "Is the given key currently pressed?",       langs: LUA },
    ApiSymbol { name: "is_key_just_pressed",      signature: "is_key_just_pressed(key)",       category: "Input", doc: "Did the key transition to pressed this frame?", langs: LUA },
    ApiSymbol { name: "is_key_just_released",     signature: "is_key_just_released(key)",      category: "Input", doc: "Did the key transition to released this frame?", langs: LUA },
    ApiSymbol { name: "input_button_pressed",     signature: "input_button_pressed(action)",   category: "Input", doc: "Is the named input action currently active?",   langs: LUA },
    ApiSymbol { name: "input_button_just_pressed",signature: "input_button_just_pressed(action)", category: "Input", doc: "Did the named action trigger this frame?",   langs: LUA },
    ApiSymbol { name: "input_button_just_released",signature:"input_button_just_released(action)", category: "Input", doc: "Did the named action release this frame?",   langs: LUA },
    ApiSymbol { name: "input_axis_1d",            signature: "input_axis_1d(action)",          category: "Input", doc: "Read a 1-D axis value for the action.",     langs: LUA },
    ApiSymbol { name: "input_axis_2d",            signature: "input_axis_2d(action) -> x, y",  category: "Input", doc: "Read a 2-D axis: returns two values (x, y).", langs: LUA },

    // ── Audio ──
    ApiSymbol { name: "play_sound",         signature: "play_sound(path, volume?, bus?)",  category: "Audio", doc: "Play a one-shot sound from an asset path.", langs: LUA },
    ApiSymbol { name: "play_sound_looping", signature: "play_sound_looping(path, volume)", category: "Audio", doc: "Play a looping sound.",                      langs: LUA },
    ApiSymbol { name: "play_music",         signature: "play_music(path, volume?, fade_in?)", category: "Audio", doc: "Play background music with optional fade-in.", langs: LUA },
    ApiSymbol { name: "stop_music",         signature: "stop_music(fade_out?)",            category: "Audio", doc: "Stop the current music track.",             langs: LUA },
    ApiSymbol { name: "stop_all_sounds",    signature: "stop_all_sounds()",                category: "Audio", doc: "Stop all currently-playing sounds.",        langs: LUA },

    // ── Physics ──
    ApiSymbol { name: "apply_force",        signature: "apply_force(x, y, z)",        category: "Physics", doc: "Apply a force vector to the entity.",       langs: LUA },
    ApiSymbol { name: "apply_impulse",      signature: "apply_impulse(x, y, z)",      category: "Physics", doc: "Apply an instantaneous impulse.",           langs: LUA },
    ApiSymbol { name: "set_velocity",       signature: "set_velocity(x, y, z)",       category: "Physics", doc: "Set the entity's linear velocity.",         langs: LUA },
    ApiSymbol { name: "set_gravity_scale",  signature: "set_gravity_scale(scale)",    category: "Physics", doc: "Multiplier on gravity for this body.",      langs: LUA },

    // ── Timers ──
    ApiSymbol { name: "start_timer", signature: "start_timer(name, duration, repeat?)", category: "Timers", doc: "Schedule a timer that fires `on_timer`.", langs: LUA },
    ApiSymbol { name: "stop_timer",  signature: "stop_timer(name)",                    category: "Timers", doc: "Cancel a running timer by name.",         langs: LUA },

    // ── Debug ──
    ApiSymbol { name: "print_log", signature: "print_log(msg)",                            category: "Debug", doc: "Log a string to the editor console.", langs: LUA },
    ApiSymbol { name: "draw_line", signature: "draw_line(sx,sy,sz, ex,ey,ez, duration?)", category: "Debug", doc: "Draw a debug line for N seconds.",    langs: LUA },

    // ── Rendering ──
    ApiSymbol { name: "set_visibility",     signature: "set_visibility(visible)",          category: "Rendering", doc: "Show or hide the entity.",            langs: LUA },
    ApiSymbol { name: "set_material_color", signature: "set_material_color(r, g, b, a?)",  category: "Rendering", doc: "Override the entity's material tint.", langs: LUA },

    // ── Animation ──
    ApiSymbol { name: "play_animation",       signature: "play_animation(name, looping?, speed?)", category: "Animation", doc: "Play an animation clip by name.",         langs: LUA },
    ApiSymbol { name: "stop_animation",       signature: "stop_animation()",                       category: "Animation", doc: "Stop the current animation.",             langs: LUA },
    ApiSymbol { name: "pause_animation",      signature: "pause_animation()",                      category: "Animation", doc: "Pause the current animation.",            langs: LUA },
    ApiSymbol { name: "resume_animation",     signature: "resume_animation()",                     category: "Animation", doc: "Resume a paused animation.",              langs: LUA },
    ApiSymbol { name: "set_animation_speed",  signature: "set_animation_speed(speed)",             category: "Animation", doc: "Set the playback speed multiplier.",      langs: LUA },
    ApiSymbol { name: "crossfade_animation",  signature: "crossfade_animation(name, duration, looping?)", category: "Animation", doc: "Blend to a new clip over `duration`.", langs: LUA },
    ApiSymbol { name: "set_anim_param",       signature: "set_anim_param(name, value)",            category: "Animation", doc: "Set a numeric animation-graph parameter.", langs: LUA },
    ApiSymbol { name: "set_anim_bool",        signature: "set_anim_bool(name, value)",             category: "Animation", doc: "Set a boolean animation-graph parameter.", langs: LUA },
    ApiSymbol { name: "trigger_anim",         signature: "trigger_anim(name)",                     category: "Animation", doc: "Fire a one-shot trigger parameter.",      langs: LUA },
    ApiSymbol { name: "set_layer_weight",     signature: "set_layer_weight(layer, weight)",        category: "Animation", doc: "Adjust a layer's blend weight.",          langs: LUA },

    // ── Cursor ──
    ApiSymbol { name: "lock_cursor",   signature: "lock_cursor()",   category: "Cursor", doc: "Hide + lock the mouse cursor to the viewport.", langs: LUA },
    ApiSymbol { name: "unlock_cursor", signature: "unlock_cursor()", category: "Cursor", doc: "Release the mouse cursor.",                     langs: LUA },

    // ── Camera ──
    ApiSymbol { name: "screen_shake",  signature: "screen_shake(intensity, duration)", category: "Camera", doc: "Shake the camera for a duration.", langs: LUA },

    // ── ECS / Scene ──
    ApiSymbol { name: "spawn_entity",  signature: "spawn_entity(name)",     category: "ECS",   doc: "Spawn a new empty entity.",                langs: LUA },
    ApiSymbol { name: "despawn_self",  signature: "despawn_self()",         category: "ECS",   doc: "Despawn this entity at end of frame.",    langs: LUA },
    ApiSymbol { name: "load_scene",    signature: "load_scene(path)",       category: "Scene", doc: "Switch to the scene at the given path.",  langs: LUA },

    // ── Environment ──
    ApiSymbol { name: "set_sun_angles", signature: "set_sun_angles(azimuth, elevation)", category: "Environment", doc: "Set the sun's angular position.", langs: LUA },
    ApiSymbol { name: "set_fog",        signature: "set_fog(enabled, start, end)",       category: "Environment", doc: "Configure linear distance fog.",   langs: LUA },

    // ── Reflection ──
    ApiSymbol { name: "set",        signature: "set(path, value)",                  category: "Reflection", doc: "Write a component field on self (e.g. \"Transform.translation.x\").", langs: LUA },
    ApiSymbol { name: "set_on",     signature: "set_on(entity_name, path, value)",  category: "Reflection", doc: "Write a component field on a named entity.",                         langs: LUA },
    ApiSymbol { name: "get",        signature: "get(path)",                         category: "Reflection", doc: "Read a component field from self.",                                  langs: LUA },
    ApiSymbol { name: "get_on",     signature: "get_on(entity_name, path)",         category: "Reflection", doc: "Read a component field from a named entity.",                        langs: LUA },

    // ── Actions ──
    ApiSymbol { name: "action",    signature: "action(name, args?)",               category: "Actions", doc: "Dispatch a named action (UI, etc.) with optional table args.", langs: LUA },
    ApiSymbol { name: "action_on", signature: "action_on(target, name, args?)",    category: "Actions", doc: "Dispatch an action targeting another entity.",                langs: LUA },

    // ── UI (via `action(...)`) ──
    ApiSymbol { name: "ui_show",       signature: "action(\"ui_show\", { name = ... })",                  category: "UI", doc: "Show a UI widget by name.",       langs: LUA },
    ApiSymbol { name: "ui_hide",       signature: "action(\"ui_hide\", { name = ... })",                  category: "UI", doc: "Hide a UI widget by name.",       langs: LUA },
    ApiSymbol { name: "ui_toggle",     signature: "action(\"ui_toggle\", { name = ... })",                category: "UI", doc: "Toggle UI widget visibility.",    langs: LUA },
    ApiSymbol { name: "ui_set_text",   signature: "action(\"ui_set_text\", { name = ..., text = ... })",  category: "UI", doc: "Update a Text widget's content.", langs: LUA },
    ApiSymbol { name: "ui_set_progress",signature:"action(\"ui_set_progress\", { name = ..., value = 0..1 })", category: "UI", doc: "Set a progress bar's value.", langs: LUA },
    ApiSymbol { name: "ui_set_health", signature: "action(\"ui_set_health\", { name, current, max })",   category: "UI", doc: "Set a health bar's state.",        langs: LUA },
    ApiSymbol { name: "ui_set_color",  signature: "action(\"ui_set_color\", { name, r, g, b, a })",      category: "UI", doc: "Tint a UI widget.",                langs: LUA },
    ApiSymbol { name: "ui_set_theme",  signature: "action(\"ui_set_theme\", { theme = \"dark\"|\"light\"|\"high_contrast\" })", category: "UI", doc: "Change active UI theme.", langs: LUA },

    // ── Lifecycle hook templates ──
    ApiSymbol { name: "on_ready",  signature: "function on_ready(ctx, vars)\n  \nend",  category: "Lifecycle", doc: "Called once when the script attaches.", langs: LUA },
    ApiSymbol { name: "on_update", signature: "function on_update(ctx, vars)\n  \nend", category: "Lifecycle", doc: "Called every frame.",                   langs: LUA },
    ApiSymbol { name: "on_timer",  signature: "function on_timer(ctx, vars, name)\n  \nend", category: "Lifecycle", doc: "Called when a timer fires.",        langs: LUA },
];

/// Walk backwards from `cursor` over identifier characters and return
/// `(prefix_byte_start, prefix_str)`. Returns `None` if the cursor isn't on a
/// word.
pub fn extract_prefix(text: &str, cursor: usize) -> Option<(usize, &str)> {
    let cursor = cursor.min(text.len());
    let bytes = text.as_bytes();
    let mut start = cursor;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' {
            start -= 1;
        } else {
            break;
        }
    }
    if start == cursor {
        None
    } else {
        Some((start, &text[start..cursor]))
    }
}

/// Collect matching symbols for the given language + prefix. Prefix is
/// matched case-insensitively.
pub fn matching_symbols(lang: Language, prefix: &str) -> Vec<&'static ApiSymbol> {
    let lower = prefix.to_lowercase();
    let mut out: Vec<&'static ApiSymbol> = SYMBOLS
        .iter()
        .filter(|s| s.langs.contains(&lang))
        .filter(|s| {
            if lower.is_empty() {
                true
            } else {
                s.name.to_lowercase().starts_with(&lower)
            }
        })
        .collect();

    // Best matches first: exact prefix over contains (already starts_with), alpha within.
    out.sort_by(|a, b| a.name.cmp(b.name));
    out.truncate(50);
    out
}
