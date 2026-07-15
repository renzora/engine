//! The reaction picker — a popup grid of phosphor icons with a search box.
//! Opened from the "+" button on feed posts and forum posts; picking an icon
//! toggles that reaction on the target.

use bevy::prelude::*;
use renzora::core::SocialBridge;
use renzora_auth::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::phosphor_map::ICONS;
use renzora_ember::theme::{placeholder, popup_bg, rgb, text_primary};
use renzora_ember::widgets::{text_input, tint, EmberTextInput, HoverTint};

use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, session_clone};

const GRID_ICONS: usize = 160;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReactionTarget {
    FeedPost(String),
}

/// Marker: opens the picker for this target.
#[derive(Component)]
pub(crate) struct AddReactionBtn(pub ReactionTarget);

/// Marker: toggles an existing reaction directly.
#[derive(Component)]
pub(crate) struct ReactBtn(pub ReactionTarget, pub String);

#[derive(Resource, Default)]
pub(crate) struct ReactionPicker {
    root: Option<Entity>,
    target: Option<ReactionTarget>,
}

#[derive(Component)]
pub(crate) struct PickerBackdrop;
#[derive(Component)]
pub(crate) struct PickerSearch;
#[derive(Component)]
pub(crate) struct PickerIcon(String);
#[derive(Component)]
pub(crate) struct PickerGrid;

pub(crate) fn open_clicks(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut picker: ResMut<ReactionPicker>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    opens: Query<(&Interaction, &AddReactionBtn), Changed<Interaction>>,
    nodes: Query<(), With<Node>>,
) {
    let Some(fonts) = fonts else { return };
    for (i, b) in &opens {
        if *i != Interaction::Pressed {
            continue;
        }
        // Toggle off if already open. The stored root can go stale — a
        // workspace/layout rebuild tears down the parentless backdrop without
        // this resource hearing about it — so only a still-live root closes
        // (a plain `despawn` on the stale id panics once the entity index is
        // reused); a stale one just clears the state and falls through to
        // open a fresh picker, so the click isn't swallowed.
        if let Some(root) = picker.root.take() {
            picker.target = None;
            if nodes.get(root).is_ok() {
                commands.entity(root).despawn();
                return;
            }
        }
        let cursor = windows
            .iter()
            .next()
            .and_then(|w| w.cursor_position())
            .unwrap_or(Vec2::new(300.0, 200.0));

        let backdrop = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                renzora_ember::cursor_icon::NoAutoCursor,
                PickerBackdrop,
                GlobalZIndex(960),
                Name::new("reaction_picker"),
            ))
            .id();
        let panel = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px((cursor.x - 130.0).max(8.0)),
                    top: Val::Px((cursor.y - 260.0).max(8.0)),
                    width: Val::Px(260.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(rgb(popup_bg())),
                BorderColor::all(tint(util::HUE_FEED, 60)),
                bevy::ui::FocusPolicy::Block,
            ))
            .id();
        let search = text_input(&mut commands, &fonts.ui, "Search icons...", "");
        commands.entity(search).insert(PickerSearch);
        let grid = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: Val::Px(2.0),
                    column_gap: Val::Px(2.0),
                    ..default()
                },
                PickerGrid,
            ))
            .id();
        // Scrollable grid area (the icon set is big).
        let grid_scroll = renzora_ember::widgets::scroll_view(&mut commands, grid);
        commands.entity(grid_scroll).insert(Node {
            width: Val::Percent(100.0),
            height: Val::Px(216.0),
            min_height: Val::Px(0.0),
            overflow: Overflow::scroll_y(),
            ..default()
        });
        commands.entity(panel).add_children(&[search, grid_scroll]);
        commands.entity(backdrop).add_child(panel);

        build_grid(&mut commands, &fonts, grid, "");
        picker.root = Some(backdrop);
        picker.target = Some(b.0.clone());
    }
}

/// Rebuild the icon grid when the search text changes.
pub(crate) fn search_filter(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    picker: Res<ReactionPicker>,
    inputs: Query<&EmberTextInput, (With<PickerSearch>, Changed<EmberTextInput>)>,
    grids: Query<Entity, With<PickerGrid>>,
    children: Query<&Children>,
) {
    if picker.root.is_none() {
        return;
    }
    let (Some(fonts), Ok(input), Ok(grid)) = (fonts, inputs.single(), grids.single()) else {
        return;
    };
    // Clear + rebuild. `try_despawn` in case a same-frame teardown already
    // queued these away.
    if let Ok(kids) = children.get(grid) {
        for k in kids.iter() {
            commands.entity(k).try_despawn();
        }
    }
    build_grid(&mut commands, &fonts, grid, input.value.trim());
}

fn build_grid(commands: &mut Commands, fonts: &EmberFonts, grid: Entity, query: &str) {
    let q = query.to_lowercase();
    let mut shown = 0;
    for (name, _) in ICONS.iter() {
        if !q.is_empty() && !name.contains(&q) {
            continue;
        }
        if shown >= GRID_ICONS {
            break;
        }
        shown += 1;
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(28.0),
                    height: Val::Px(28.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                HoverTint { base: Color::NONE, hover: tint(util::HUE_FEED, 50), pressed: tint(util::HUE_FEED, 90) },
                PickerIcon((*name).to_string()),
            ))
            .id();
        let ic = icon_text(commands, &fonts.phosphor, name, (220, 220, 228), 15.0);
        commands.entity(btn).add_child(ic);
        commands.entity(grid).add_child(btn);
    }
    if shown == 0 {
        let none = commands
            .spawn((Text::new("No icons match"), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder()))))
            .id();
        commands.entity(grid).add_child(none);
        let _ = rgb(text_primary());
    }
}

/// Icon picked / reaction chip toggled / backdrop dismissed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn picks(
    mut commands: Commands,
    mut picker: ResMut<ReactionPicker>,
    session: Res<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
    mut feed: ResMut<crate::panels::feed::FeedPanel>,
    bridge: Res<SocialBridge>,
    icons: Query<(&Interaction, &PickerIcon), Changed<Interaction>>,
    chips: Query<(&Interaction, &ReactBtn), Changed<Interaction>>,
    backdrops: Query<&Interaction, (With<PickerBackdrop>, Changed<Interaction>)>,
) {
    let _ = &bridge;
    // Direct chip toggles.
    for (i, b) in &chips {
        if *i == Interaction::Pressed {
            apply_reaction(&session, &mut toasts, &mut feed, &b.0, &b.1);
        }
    }
    // Picker selection.
    let mut close = false;
    for (i, ic) in &icons {
        if *i == Interaction::Pressed {
            if let Some(target) = picker.target.clone() {
                apply_reaction(&session, &mut toasts, &mut feed, &target, &ic.0);
            }
            close = true;
        }
    }
    for i in &backdrops {
        if *i == Interaction::Pressed {
            close = true;
        }
    }
    if close {
        // `try_despawn`: the backdrop may already be gone if something tore
        // the UI down between the press and this frame's command flush.
        if let Some(root) = picker.root.take() {
            commands.entity(root).try_despawn();
        }
        picker.target = None;
    }
}

/// Optimistically toggle locally, then tell the server.
fn apply_reaction(
    session: &AuthSession,
    toasts: &mut ToastQueue,
    feed: &mut crate::panels::feed::FeedPanel,
    target: &ReactionTarget,
    icon: &str,
) {
    if !session.is_signed_in() {
        toasts.push(Tone::Warn, "Sign in to react", None);
        return;
    }
    match target {
        ReactionTarget::FeedPost(id) => {
            if let Some(post) = feed.posts.iter_mut().find(|p| p.id == *id) {
                toggle_local(&mut post.reactions, icon);
            }
            feed.bump();
            let s = session_clone(session);
            let (id, icon) = (id.clone(), icon.to_string());
            spawn_thread(move || {
                let _ = renzora_auth::feed::react_to_post(&s, &id, &icon);
            });
        }
    }
}

fn toggle_local(reactions: &mut Vec<renzora_auth::feed::Reaction>, icon: &str) {
    if let Some(r) = reactions.iter_mut().find(|r| r.icon == icon) {
        if r.reacted {
            r.reacted = false;
            r.count -= 1;
        } else {
            r.reacted = true;
            r.count += 1;
        }
        reactions.retain(|r| r.count > 0);
    } else {
        reactions.push(renzora_auth::feed::Reaction { icon: icon.to_string(), count: 1, reacted: true });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

/// A row of reaction chips + the "add reaction" button, shared by feed and
/// forum post cards. Chips use the shared neutral/accent [`util::action_chip`]
/// look — reacted chips light up with the theme accent instead of a per-panel
/// hue. Each chip's tooltip says how many reacted and whether you're among
/// them (the API only reports counts + "did I react", not reactor names).
pub(crate) fn reaction_bar(
    commands: &mut Commands,
    fonts: &EmberFonts,
    target: ReactionTarget,
    reactions: &[renzora_auth::feed::Reaction],
) -> Entity {
    let bar = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    for r in reactions {
        let count = r.count.to_string();
        let tip = match (r.count, r.reacted) {
            (1, true) => "You reacted — click to remove".to_string(),
            (n, true) => format!("You and {} other{} reacted", n - 1, if n == 2 { "" } else { "s" }),
            (n, false) => format!("{n} reacted — click to join"),
        };
        let chip = util::action_chip(commands, fonts, &r.icon, Some(&count), r.reacted, Some(tip));
        commands.entity(chip).insert(ReactBtn(target.clone(), r.icon.clone()));
        commands.entity(bar).add_child(chip);
    }
    // "+" opens the picker.
    let add = util::action_chip(
        commands,
        fonts,
        "smiley",
        None,
        false,
        Some("Add a reaction".to_string()),
    );
    commands.entity(add).insert(AddReactionBtn(target));
    commands.entity(bar).add_child(add);
    bar
}
