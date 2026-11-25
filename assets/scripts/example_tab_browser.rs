//! A gamepad-driven tab browser, paired with `ui/example_tab_browser.html`.
//! It spawns the markup on ready, then every frame:
//! - L1 / R1 cycle the active tab, once per press.
//! - The left stick's Y steps the item cursor, with a deadzone and a re-arm so
//!   a held direction steps only once.
//! - South (A on Xbox, Cross on PlayStation) confirms, logging the selection.
//!
//! The markup's `on_press` names land in the same handler, so clicking a tab
//! works too. The script is the source of truth and the markup's colours are
//! a derived view, repainted whenever the tab or cursor changes.
//!
//! Setup: attach to ONE entity. The template names its tabs `tab_loadout`,
//! `tab_perks`, `tab_maps`, `tab_career` and its rows `item_0` to `item_4`.

use bevy::prelude::*;
use renzora::script_bridge::ScriptAction;
use renzora::{ScriptCtx, ScriptHook, ScriptInput};

const TEMPLATE: &str = "ui/example_tab_browser.html";
const TABS: [&str; 4] = ["tab_loadout", "tab_perks", "tab_maps", "tab_career"];
const ITEM_COUNT: usize = 5;
/// Stick magnitude below which it counts as neutral and re-arms.
const REARM: f32 = 0.3;
/// Stick magnitude that steps the cursor.
const STEP: f32 = 0.5;

/// #4C8BF5
const TAB_ACTIVE: Color = Color::srgba(0.298, 0.545, 0.961, 1.0);
/// #11151C
const TAB_IDLE: Color = Color::srgba(0.067, 0.082, 0.110, 1.0);
/// #1B1F27, used for both fill and border so an idle row shows no outline.
const ITEM_IDLE: Color = Color::srgba(0.106, 0.122, 0.153, 1.0);
/// #222A36
const ITEM_FOCUS: Color = Color::srgba(0.133, 0.165, 0.212, 1.0);
/// #4C8BF5
const ITEM_FOCUS_BORDER: Color = Color::srgba(0.298, 0.545, 0.961, 1.0);

#[derive(Component)]
struct TabBrowser {
    /// Index into `TABS`.
    active_tab: usize,
    /// Index of the focused `item_N` row.
    cursor: usize,
    /// Buttons held last frame, so a hold fires once.
    prev_l1: bool,
    prev_r1: bool,
    prev_south: bool,
    axis_armed: bool,
}

fn update(ctx: &mut ScriptCtx) {
    let (l1, r1, south, stick_y) = match ctx.get_resource::<ScriptInput>() {
        Some(input) => match input.first_gamepad() {
            Some(pad) => (
                input.is_gamepad_button_pressed(pad, GamepadButton::LeftTrigger),
                input.is_gamepad_button_pressed(pad, GamepadButton::RightTrigger),
                input.is_gamepad_button_pressed(pad, GamepadButton::South),
                input.get_gamepad_left_stick(pad).y,
            ),
            None => (false, false, false, 0.0),
        },
        None => (false, false, false, 0.0),
    };

    let Some(mut state) = ctx.get_mut::<TabBrowser>() else { return };
    let mut dirty = false;
    if l1 && !state.prev_l1 {
        state.active_tab = (state.active_tab + TABS.len() - 1) % TABS.len();
        dirty = true;
    }
    if r1 && !state.prev_r1 {
        state.active_tab = (state.active_tab + 1) % TABS.len();
        dirty = true;
    }
    state.prev_l1 = l1;
    state.prev_r1 = r1;

    if stick_y.abs() < REARM {
        state.axis_armed = true;
    } else if state.axis_armed {
        if stick_y > STEP {
            state.cursor = (state.cursor + 1) % ITEM_COUNT;
            dirty = true;
        } else if stick_y < -STEP {
            state.cursor = (state.cursor + ITEM_COUNT - 1) % ITEM_COUNT;
            dirty = true;
        }
        state.axis_armed = false;
    }

    let confirm = south && !state.prev_south;
    state.prev_south = south;
    let (active_tab, cursor) = (state.active_tab, state.cursor);
    drop(state);

    if confirm {
        renzora::clog_info!(
            "Script",
            "[tab_browser] confirm: tab={} cursor={}",
            TABS[active_tab],
            cursor
        );
    }
    if dirty {
        repaint(ctx.world(), active_tab, cursor);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    match hook {
        ScriptHook::Ready => {
            ctx.insert(TabBrowser {
                active_tab: 0,
                cursor: 0,
                prev_l1: false,
                prev_r1: false,
                prev_south: false,
                axis_armed: true,
            });
            let me = ctx.entity();
            ScriptAction::new("hui_spawn", me).arg("template", TEMPLATE).send(ctx.world());
        }
        ScriptHook::Ui { name, .. } => {
            let Some(tab) = name
                .strip_prefix("select_")
                .and_then(|tab| TABS.iter().position(|t| *t == tab))
            else {
                return;
            };
            let Some(mut state) = ctx.get_mut::<TabBrowser>() else { return };
            state.active_tab = tab;
            let cursor = state.cursor;
            drop(state);
            repaint(ctx.world(), tab, cursor);
        }
        _ => {}
    }
}

/// Paint every tab and row from the current state.
fn repaint(world: &mut World, active_tab: usize, cursor: usize) {
    for (i, tab) in TABS.iter().enumerate() {
        let color = if i == active_tab { TAB_ACTIVE } else { TAB_IDLE };
        paint(world, tab, color, None);
    }
    for i in 0..ITEM_COUNT {
        let name = format!("item_{i}");
        if i == cursor {
            paint(world, &name, ITEM_FOCUS, Some(ITEM_FOCUS_BORDER));
        } else {
            paint(world, &name, ITEM_IDLE, Some(ITEM_IDLE));
        }
    }
}

/// Set a named node's background, and its border on all four sides if given.
fn paint(world: &mut World, name: &str, background: Color, border: Option<Color>) {
    let Some(entity) = find_named(world, name) else { return };
    if let Some(mut bg) = world.get_mut::<BackgroundColor>(entity) {
        bg.0 = background;
    }
    if let Some(border) = border {
        if let Some(mut bc) = world.get_mut::<BorderColor>(entity) {
            *bc = BorderColor::all(border);
        }
    }
}

/// The entity with this name (its snake_case id), if there is one.
fn find_named(world: &mut World, name: &str) -> Option<Entity> {
    let mut names = world.query::<(Entity, &Name)>();
    names.iter(world).find(|(_, n)| n.as_str() == name).map(|(e, _)| e)
}

renzora::script!(update, hooks = hooks);
