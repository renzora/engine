//! A gamepad and keyboard driven main menu. It spawns the menu on ready and
//! moves a focus cursor with the left stick, the D-pad or the arrow keys;
//! South (A / Cross), Enter or Space confirms. Mouse hover moves the same
//! cursor, so all three input methods agree on which button is focused.
//! Escape, Start or East (B / Circle) always brings the menu back.
//!
//! Setup: attach to ONE entity. Pairs with `ui/example_main_menu.html`
//! (buttons "btn_play", "btn_settings", "btn_credits", "btn_exit" under
//! "main_menu_root") and `ui/example_settings.html` ("settings_root"). The
//! audio paths below are placeholders; supply your own.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const TEMPLATE_MENU: &str = "ui/example_main_menu.html";
const TEMPLATE_SETTINGS: &str = "ui/example_settings.html";

const SFX_MOVE: &str = "audio/menu_button.mp3";
const SFX_CONFIRM: &str = "audio/confirm.mp3";
const MUSIC_MENU: &str = "audio/main_music.mp3";
const VOL_MOVE: f32 = 0.5;
const VOL_CONFIRM: f32 = 0.8;
const VOL_MUSIC: f32 = 0.5;

/// Flip if pushing up on the left stick moves the cursor down. Bevy reports
/// stick up as +Y, which moves the cursor up here.
const INVERT_Y: bool = false;
/// The stick must return below this before it can step again.
const AXIS_DEADZONE: f32 = 0.25;
/// The stick must pass this (once re-armed) to step the cursor.
const AXIS_STEP: f32 = 0.5;

const ITEMS: [&str; 4] = ["btn_play", "btn_settings", "btn_credits", "btn_exit"];
/// #4C8BF5
const COLOR_FOCUS: Color = Color::srgb(0.298, 0.545, 0.961);
/// #1B1F27
const COLOR_IDLE: Color = Color::srgb(0.106, 0.122, 0.153);

#[derive(Component)]
struct Menu {
    cursor: usize,
    on_settings: bool,
    hidden: bool,
    axis_armed: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let Some(input) = ctx.get_resource::<ScriptInput>() else { return };
    let pad = input.first_gamepad();
    let pad_edge = |button| pad.is_some_and(|p| input.is_gamepad_button_just_pressed(p, button));
    let back = input.is_key_just_pressed(KeyCode::Escape) || pad_edge(GamepadButton::Start) || pad_edge(GamepadButton::East);
    let dpad_down = pad_edge(GamepadButton::DPadDown);
    let dpad_up = pad_edge(GamepadButton::DPadUp);
    let confirm_pressed = pad_edge(GamepadButton::South)
        || input.is_key_just_pressed(KeyCode::Enter)
        || input.is_key_just_pressed(KeyCode::Space);
    let arrow_down = input.is_key_just_pressed(KeyCode::ArrowDown);
    let arrow_up = input.is_key_just_pressed(KeyCode::ArrowUp);
    let stick_y = pad.map_or(0.0, |p| input.get_gamepad_left_stick(p).y);
    let stick_y = if INVERT_Y { -stick_y } else { stick_y };

    let Some(menu) = ctx.get::<Menu>() else { return };
    let (on_settings, hidden) = (menu.on_settings, menu.hidden);

    if back {
        if on_settings {
            ui(ctx, "hui_despawn", "name", "settings_root");
        }
        if hidden {
            // Coming back from the hidden "in game" state brings the music too.
            play_music(ctx, 0.4);
        }
        if let Some(mut menu) = ctx.get_mut::<Menu>() {
            menu.on_settings = false;
            menu.hidden = false;
        }
        ui(ctx, "hui_show", "name", "main_menu_root");
        repaint(ctx);
        play_sound(ctx, SFX_MOVE, VOL_MOVE);
        return;
    }
    if on_settings || hidden {
        return;
    }

    let mut step = 0i32;
    if let Some(mut menu) = ctx.get_mut::<Menu>() {
        if stick_y.abs() < AXIS_DEADZONE {
            menu.axis_armed = true;
        } else if menu.axis_armed {
            if stick_y > AXIS_STEP {
                step -= 1;
            } else if stick_y < -AXIS_STEP {
                step += 1;
            }
            menu.axis_armed = false;
        }
    }
    step += dpad_down as i32 + arrow_down as i32 - dpad_up as i32 - arrow_up as i32;
    for _ in 0..step.unsigned_abs() {
        move_cursor(ctx, step.signum());
    }

    if confirm_pressed {
        confirm(ctx);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            ctx.insert(Menu { cursor: 0, on_settings: false, hidden: false, axis_armed: true });
            ui(ctx, "hui_spawn", "template", TEMPLATE_MENU);
            play_music(ctx, 0.6);
        }
        ScriptHook::Ui { name, .. } => {
            if let Some(item) = name.strip_prefix("press_") {
                if let Some(index) = ITEMS.iter().position(|i| i.strip_prefix("btn_") == Some(item)) {
                    set_cursor(ctx, index);
                    repaint(ctx);
                    confirm(ctx);
                }
            } else if let Some(item) = name.strip_prefix("hover_") {
                // Only a real change plays the move sound; otherwise wiggling
                // the mouse over one button retriggers it and the audio
                // stutters.
                let current = ctx.get::<Menu>().map(|m| m.cursor);
                if let Some(index) = ITEMS.iter().position(|i| *i == item) {
                    if current != Some(index) {
                        set_cursor(ctx, index);
                        repaint(ctx);
                        play_sound(ctx, SFX_MOVE, VOL_MOVE);
                    }
                }
            } else if *name == "settings_back" {
                ui(ctx, "hui_despawn", "name", "settings_root");
                ui(ctx, "hui_show", "name", "main_menu_root");
                if let Some(mut menu) = ctx.get_mut::<Menu>() {
                    menu.on_settings = false;
                }
            }
        }
        _ => {}
    }
}

fn set_cursor(ctx: &mut ScriptCtx, index: usize) {
    if let Some(mut menu) = ctx.get_mut::<Menu>() {
        menu.cursor = index;
    }
}

/// Step the cursor by +1 (down) or -1 (up), wrapping.
fn move_cursor(ctx: &mut ScriptCtx, delta: i32) {
    let Some(mut menu) = ctx.get_mut::<Menu>() else { return };
    menu.cursor = (menu.cursor as i32 + delta).rem_euclid(ITEMS.len() as i32) as usize;
    drop(menu);
    repaint(ctx);
    play_sound(ctx, SFX_MOVE, VOL_MOVE);
}

fn confirm(ctx: &mut ScriptCtx) {
    let Some(cursor) = ctx.get::<Menu>().map(|m| m.cursor) else { return };
    let choice = ITEMS[cursor];
    renzora::clog_info!("Script", "[main_menu] confirm -> {choice}");
    play_sound(ctx, SFX_CONFIRM, VOL_CONFIRM);
    let me = ctx.entity();
    match choice {
        "btn_play" => {
            ui(ctx, "hui_hide", "name", "main_menu_root");
            if let Some(mut menu) = ctx.get_mut::<Menu>() {
                menu.hidden = true;
            }
            ScriptAction::new("stop_music", me).arg("fade_out", 0.5).send(ctx.world());
        }
        "btn_settings" => {
            ui(ctx, "hui_hide", "name", "main_menu_root");
            ui(ctx, "hui_spawn", "template", TEMPLATE_SETTINGS);
            if let Some(mut menu) = ctx.get_mut::<Menu>() {
                menu.on_settings = true;
            }
        }
        "btn_credits" => renzora::clog_info!("Script", "[main_menu] (credits would open here)"),
        "btn_exit" => ScriptAction::new("quit", me).send(ctx.world()),
        _ => {}
    }
}

/// Paint the focus border on the cursor's button and the idle one on the rest.
fn repaint(ctx: &mut ScriptCtx) {
    let Some(cursor) = ctx.get::<Menu>().map(|m| m.cursor) else { return };
    let world = ctx.world();
    let mut names = world.query::<(Entity, &Name)>();
    let buttons: Vec<(Entity, usize)> = names
        .iter(world)
        .filter_map(|(e, n)| ITEMS.iter().position(|i| *i == n.as_str()).map(|i| (e, i)))
        .collect();
    for (button, index) in buttons {
        let color = if index == cursor { COLOR_FOCUS } else { COLOR_IDLE };
        // Bevy splits border colours per side, so all four are set.
        world
            .entity_mut(button)
            .insert(BorderColor { top: color, right: color, bottom: color, left: color });
    }
}

fn ui(ctx: &mut ScriptCtx, action: &str, key: &str, value: &str) {
    let me = ctx.entity();
    ScriptAction::new(action, me).arg(key, value).send(ctx.world());
}

fn play_sound(ctx: &mut ScriptCtx, path: &str, volume: f32) {
    let me = ctx.entity();
    ScriptAction::new("play_sound", me)
        .arg("path", path)
        .arg("volume", volume)
        .arg("looping", false)
        .arg("bus", "Sfx")
        .send(ctx.world());
}

fn play_music(ctx: &mut ScriptCtx, fade_in: f32) {
    let me = ctx.entity();
    ScriptAction::new("play_music", me)
        .arg("path", MUSIC_MENU)
        .arg("volume", VOL_MUSIC)
        .arg("fade_in", fade_in)
        .arg("bus", "Music")
        .send(ctx.world());
}

renzora::script!(update, hooks = hooks);
