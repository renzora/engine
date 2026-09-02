//! The Shortcuts page — the editor's own keyboard bindings.
//!
//! A rebind button's label and colour are bound reactively rather than rebuilt,
//! so pressing one shows "Press key…" without re-spawning the overlay; the
//! capture then happens in [`rebind_capture`], which runs whether or not the
//! settings overlay is open (a plugin can request a rebind too).

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::section;
use renzora_keybindings::{EditorAction, KeyBinding, KeyBindings};

use crate::lang::tr;
use crate::rows::settings_row;
use crate::state::A_YELLOW;

#[derive(Component)]
pub(crate) struct RebindBtn(EditorAction);

#[derive(Component)]
pub(crate) struct ResetBindingsBtn;

pub(crate) fn tab_shortcuts(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    // Group built-in actions by category, preserving first-seen order.
    let mut groups: Vec<(&'static str, Vec<EditorAction>)> = Vec::new();
    for a in EditorAction::all() {
        let cat = a.category();
        if let Some(g) = groups.iter_mut().find(|(c, _)| *c == cat) {
            g.1.push(a);
        } else {
            groups.push((cat, vec![a]));
        }
    }

    for (cat, actions) in groups {
        let (sec, body) = section(commands, fonts, "keyboard", cat, A_YELLOW);
        commands.entity(col).add_child(sec);
        for (i, action) in actions.into_iter().enumerate() {
            let btn = rebind_button(commands, fonts, action);
            settings_row(commands, fonts, body, i, action.display_name(), btn);
        }
    }

    // Reset-all row.
    let reset_lbl = commands
        .spawn((
            Text::new(tr("settings.btn.reset_all")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let reset = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb((60, 40, 40))),
            Interaction::default(),
            ResetBindingsBtn,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("reset-bindings"),
        ))
        .id();
    commands.entity(reset).add_child(reset_lbl);
    commands.entity(col).add_child(reset);
}

/// A rebind button whose label/colour live-track the binding + rebinding state
/// (so it shows "Press key..." while listening, without rebuilding the overlay).
fn rebind_button(commands: &mut Commands, fonts: &EmberFonts, action: EditorAction) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, lbl, move |w| {
        let kb = w.resource::<KeyBindings>();
        if kb.rebinding == Some(action) {
            tr("settings.input.press_key")
        } else {
            kb.get(action)
                .map(|b| b.display())
                .unwrap_or_else(|| tr("settings.input.unbound"))
        }
    });
    bind_text_color(commands, lbl, move |w| {
        let kb = w.resource::<KeyBindings>();
        if kb.rebinding == Some(action) {
            rgb(renzora_ember::theme::warn_amber())
        } else if kb.get(action).is_some() {
            rgb(accent())
        } else {
            rgb(text_muted())
        }
    });
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            RebindBtn(action),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("rebind-btn"),
        ))
        .id();
    commands.entity(btn).add_child(lbl);
    btn
}

pub(crate) fn rebind_btn_click(
    btns: Query<(&Interaction, &RebindBtn), Changed<Interaction>>,
    mut kb: ResMut<KeyBindings>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            kb.rebinding = Some(btn.0);
            kb.plugin_rebinding = None;
        }
    }
}

pub(crate) fn reset_bindings_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<ResetBindingsBtn>)>,
    mut kb: ResMut<KeyBindings>,
) {
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            *kb = KeyBindings::default();
        }
    }
}

/// A key that only ever qualifies a binding, never *is* one. Shared with the
/// Input tab's own capture loop, which has to skip them for the same reason.
pub(crate) fn is_modifier_key(k: KeyCode) -> bool {
    matches!(
        k,
        KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
    )
}

/// While a (plugin) rebind is pending, capture the next non-modifier key + its
/// held modifiers and commit it. Escape cancels.
pub(crate) fn rebind_capture(keys: Res<ButtonInput<KeyCode>>, mut kb: ResMut<KeyBindings>) {
    let action = kb.rebinding;
    let plugin = kb.plugin_rebinding;
    if action.is_none() && plugin.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        kb.rebinding = None;
        kb.plugin_rebinding = None;
        return;
    }
    let key = keys
        .get_just_pressed()
        .copied()
        .find(|k| !is_modifier_key(*k));
    let Some(key) = key else {
        return;
    };
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let mut b = KeyBinding::new(key);
    if ctrl {
        b = b.ctrl();
    }
    if shift {
        b = b.shift();
    }
    if alt {
        b = b.alt();
    }
    if let Some(a) = action {
        kb.set(a, b);
        kb.rebinding = None;
    } else if let Some(id) = plugin {
        kb.set_plugin(id, b);
        kb.plugin_rebinding = None;
    }
}
