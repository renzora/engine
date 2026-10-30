//! The loading card shown in the middle of the viewport while a dropped model
//! is still on its way.
//!
//! **Why it exists.** A model can be dropped before its GLB has finished
//! decoding (see `model_drop`), and for a large file the wait is several
//! seconds. The drop point gets a placeholder box, but a box alone does not say
//! *what* is coming or that anything is happening at all, so this card names
//! the model and runs a striped bar until it arrives.
//!
//! **Why the bar has no fraction.** There is none to show: Bevy's glTF loader
//! decodes the whole file, textures included, inside one load, and reports
//! nothing until it is done. So the bar is full and its stripes move, which
//! says "working" without pretending to know how far along it is.
//!
//! **It doesn't take the pointer**, for the same reason the stats readout
//! doesn't: you can carry on working while the model loads, including dropping
//! another one, and a card that swallowed clicks would get in the way of that.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use renzora::core::PlayModeState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_text};
use renzora_ember::theme::{accent, panel_bg, rgb, text_primary};

use crate::model_drop::PendingGltfLoads;

/// Bar size in logical pixels.
const BAR_WIDTH: u32 = 360;
const BAR_HEIGHT: u32 = 16;
/// Horizontal distance after which the stripe pattern repeats. The stripe
/// layer is one period wider than the bar and slides left by up to a period,
/// so wrapping it back to the start is invisible.
const STRIPE_PERIOD: u32 = 16;
/// Pixels per second the stripes travel.
const STRIPE_SPEED: f32 = 24.0;

/// The sliding stripe layer inside the bar, moved by [`animate_stripes`].
#[derive(Component)]
struct LoadStripes;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, animate_stripes);
}

/// Build the card for a viewport's content node. Slot 0 only: the loads are
/// global, so a copy in every view would repeat the same line.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Covers the viewport so the card can centre itself without knowing the
    // viewport's size.
    let cover = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-load-overlay"),
        ))
        .id();
    bind_display(commands, cover, |w| {
        w.get_resource::<PendingGltfLoads>()
            .is_some_and(|p| !p.loads.is_empty())
            && !w
                .get_resource::<PlayModeState>()
                .is_some_and(|p| p.is_in_play_mode())
    });

    let card = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(20.0), Val::Px(16.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            // Translucent like the stats readout: it sits over the render.
            BackgroundColor(rgb(panel_bg()).with_alpha(0.9)),
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-load-card"),
        ))
        .id();

    let label = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 16.0),
            TextColor(rgb(text_primary())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    bind_text(commands, label, |w| {
        let Some(pending) = w.get_resource::<PendingGltfLoads>() else {
            return String::new();
        };
        let Some(first) = pending.loads.first() else {
            return String::new();
        };
        match pending.loads.len() {
            1 => renzora::lang::t("viewport.loading.one").replace("{name}", &first.name),
            n => renzora::lang::t("viewport.loading.many")
                .replace("{name}", &first.name)
                .replace("{n}", &(n - 1).to_string()),
        }
    });

    let bar = striped_bar(commands);
    commands.entity(card).add_children(&[label, bar]);
    commands.entity(cover).add_child(card);
    cover
}

/// A full accent-coloured track with a layer of lighter diagonal stripes over
/// it. The track clips, so the stripe layer can be wider than the bar and
/// slide under its edges.
pub(crate) fn striped_bar(commands: &mut Commands) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(BAR_WIDTH as f32),
                height: Val::Px(BAR_HEIGHT as f32),
                border_radius: BorderRadius::all(Val::Px(BAR_HEIGHT as f32 * 0.5)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-load-bar"),
        ))
        .id();
    let stripes = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px((BAR_WIDTH + STRIPE_PERIOD) as f32),
                height: Val::Px(BAR_HEIGHT as f32),
                ..default()
            },
            LoadStripes,
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-load-stripes"),
        ))
        .id();
    // The image is made here rather than at plugin setup because it is only
    // needed once a viewport panel exists, and `build` only has `Commands`.
    commands.queue(move |world: &mut World| {
        let handle = world.resource_mut::<Assets<Image>>().add(stripe_image());
        if let Ok(mut entity) = world.get_entity_mut(stripes) {
            entity.insert(ImageNode::new(handle));
        }
    });
    commands.entity(track).add_child(stripes);
    track
}

/// White diagonal bands at partial alpha on transparent, one pixel per
/// logical pixel of the stripe layer, so nothing is stretched. Bands run
/// along `x + y`, which makes the pattern repeat every [`STRIPE_PERIOD`]
/// pixels across.
///
/// The alpha is kept low so a band is only a shade lighter than the track: at
/// 70 the bands read as a separate pale blue that glared on a dark panel.
fn stripe_image() -> Image {
    const BAND_ALPHA: u8 = 32;
    let width = BAR_WIDTH + STRIPE_PERIOD;
    let mut data = Vec::with_capacity((width * BAR_HEIGHT * 4) as usize);
    for y in 0..BAR_HEIGHT {
        for x in 0..width {
            let lit = (x + y) % STRIPE_PERIOD < STRIPE_PERIOD / 2;
            data.extend_from_slice(&[255, 255, 255, if lit { BAND_ALPHA } else { 0 }]);
        }
    }
    Image::new(
        Extent3d {
            width,
            height: BAR_HEIGHT,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Slide the stripe layer left, wrapping every period.
///
/// Only while it is on screen. Moving a hidden bar still changed a `Node` every
/// frame, and every changed `Node` makes Bevy lay out the path to the window
/// root again, so the idle editor paid for an animation nobody could see.
///
/// "On screen" is read from the hierarchy (no `Display::None` above it), not
/// from the bar's computed size: Bevy does not lay out a hidden subtree again,
/// so a bar hidden after its first layout keeps reporting its old size.
fn animate_stripes(
    time: Res<Time>,
    mut stripes: Query<(Entity, &mut Node), With<LoadStripes>>,
    ancestors: Query<&Node, Without<LoadStripes>>,
    parents: Query<&ChildOf>,
) {
    if stripes.is_empty() {
        return;
    }
    let shift = (time.elapsed_secs() * STRIPE_SPEED) % STRIPE_PERIOD as f32;
    for (entity, mut node) in &mut stripes {
        let hidden = parents
            .iter_ancestors(entity)
            .any(|a| ancestors.get(a).is_ok_and(|n| n.display == Display::None));
        if hidden {
            continue;
        }
        node.left = Val::Px(-shift);
    }
}
