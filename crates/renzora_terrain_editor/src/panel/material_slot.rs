//! The material slot at the top of the Terrain component: which landscape
//! material the terrain wears, with its thumbnail, path and layer summary, an
//! Edit button into the material graph, and a drop target for a `.material`.
//!
//! It sits above the mode tabs because every mode depends on it: Paint lists
//! its layers, and foliage and the look of every sculpt stroke follow from it.
//! A terrain without one draws the blockout grid, and the slot offers Alpine
//! Landscape, the engine's default, in one click.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::CurrentProject;
use renzora_editor_framework::material_thumbnail_registry::MaterialThumbnailRegistry;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_display, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;

use renzora_shader::material::landscape::LandscapeBlend;
use renzora_terrain::landscape::{LandscapeStatus, TerrainLandscape, TerrainMaterial};

use super::widgets::{button, live_text};
use super::{file_stem, AssetSlot, ManageAction};

/// The slot's thumbnail, with the material path it last showed.
#[derive(Component)]
pub(crate) struct MaterialThumb {
    terrain: Entity,
    shown: Option<String>,
}

fn material_path(w: &Rx, terrain: Entity) -> Option<String> {
    w.get::<TerrainMaterial>(terrain).map(|m| m.path.clone()).filter(|p| !p.is_empty())
}

/// "4 layers, height blend", or what is in the way of that.
pub(crate) fn summary(landscape: Option<&TerrainLandscape>) -> String {
    let Some(l) = landscape else { return "Loading...".into() };
    match &l.status {
        LandscapeStatus::None => "No material: the blockout grid".into(),
        LandscapeStatus::Loading => "Loading...".into(),
        LandscapeStatus::Plain => "A plain material, nothing to paint".into(),
        LandscapeStatus::Failed(why) => why.clone(),
        LandscapeStatus::Ready => {
            let n = l.layers.len();
            let blend = match l.layers.first().map(|x| x.blend) {
                Some(first) if l.layers.iter().all(|x| x.blend == first) => match first {
                    LandscapeBlend::Weight => "weight blend",
                    LandscapeBlend::Height => "height blend",
                    LandscapeBlend::Alpha => "alpha blend",
                },
                Some(_) => "mixed blends",
                None => "no layers",
            };
            format!("{n} layer{}, {blend}", if n == 1 { "" } else { "s" })
        }
    }
}

pub(super) fn material_slot(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let slot = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
            RelativeCursorPosition::default(),
            AssetSlot::Material,
            Name::new("terrain-material-slot"),
        ))
        .id();

    let thumb = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            MaterialThumb { terrain, shown: None },
            Name::new("terrain-material-thumb"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "paint-roller", text_muted(), 20.0);
    commands.entity(ic).insert(PASS_THROUGH);
    commands.entity(thumb).add_child(ic);

    let text = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            // The path does not wrap, so a long one ran under the buttons.
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    let name = live_text(commands, fonts, 13.0, text_primary(), move |w| {
        material_path(w, terrain).map(|p| pretty(&file_stem(&p))).unwrap_or_else(|| "No material".into())
    });
    let path = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 10.5), TextColor(rgb(text_muted())), bevy::text::TextLayout::no_wrap(), PASS_THROUGH))
        .id();
    bind_text(commands, path, move |w| material_path(w, terrain).unwrap_or_else(|| "drop a landscape .material here".into()));
    // A terrain with no material has no `TerrainLandscape` yet either, which
    // `summary` reads as one still loading.
    let info = live_text(commands, fonts, 10.5, placeholder(), move |w| match material_path(w, terrain) {
        None => "the blockout grid".into(),
        Some(_) => summary(w.get::<TerrainLandscape>(terrain)),
    });
    commands.entity(text).add_children(&[name, path, info]);

    let buttons = commands
        .spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), flex_shrink: 0.0, ..default() })
        .id();
    let edit = button(commands, fonts, Some("pencil-simple"), "Edit", false);
    commands.entity(edit).insert(ManageAction::EditMaterial);
    bind_display(commands, edit, move |w| material_path(w, terrain).is_some());
    let alpine = button(commands, fonts, Some("mountains"), "Use Alpine Landscape", true);
    commands.entity(alpine).insert(ManageAction::UseAlpine);
    bind_display(commands, alpine, move |w| material_path(w, terrain).is_none());
    let choose = button(commands, fonts, None, "Choose...", false);
    commands.entity(choose).insert(ManageAction::ChooseMaterial);
    commands.entity(buttons).add_children(&[edit, alpine, choose]);

    commands.entity(slot).add_children(&[thumb, text, buttons]);
    slot
}

/// `alpine_landscape` -> `Alpine Landscape`.
pub(crate) fn pretty(stem: &str) -> String {
    stem.split(['_', '-', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Show the material's rendered thumbnail once the thumbnail renderer has one,
/// and ask it for one when the material changes.
pub(crate) fn material_thumbnails(
    mut thumbs: Query<(Entity, &mut MaterialThumb, Option<&ImageNode>)>,
    materials: Query<&TerrainMaterial>,
    project: Option<Res<CurrentProject>>,
    registry: Option<ResMut<MaterialThumbnailRegistry>>,
    mut commands: Commands,
) {
    let (Some(project), Some(mut registry)) = (project, registry) else { return };
    for (e, mut thumb, image) in &mut thumbs {
        let path = materials.get(thumb.terrain).ok().map(|m| m.path.clone()).filter(|p| !p.is_empty());
        let abs = path.as_ref().map(|p| project.resolve_path(p));
        let handle = abs.as_ref().and_then(|a| registry.handle(a));
        if thumb.shown != path {
            thumb.shown = path.clone();
            if let Some(a) = abs.clone() {
                registry.request(a);
            }
        }
        match (handle, image) {
            (Some(h), Some(img)) if img.image == h => {}
            (Some(h), _) => {
                commands.entity(e).insert(ImageNode::new(h));
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<ImageNode>();
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_terrain::landscape::LandscapeLayer;

    fn layer(name: &str, blend: LandscapeBlend) -> LandscapeLayer {
        LandscapeLayer { name: name.into(), blend, channel: 0, coverage: 0.0 }
    }

    #[test]
    fn the_summary_counts_layers_and_names_their_blend() {
        let mut l = TerrainLandscape { status: LandscapeStatus::Ready, ..default() };
        l.layers = vec![layer("Grass", LandscapeBlend::Height), layer("Rock", LandscapeBlend::Height)];
        assert_eq!(summary(Some(&l)), "2 layers, height blend");
        l.layers.push(layer("Snow", LandscapeBlend::Weight));
        assert_eq!(summary(Some(&l)), "3 layers, mixed blends");
        l.status = LandscapeStatus::None;
        assert!(summary(Some(&l)).contains("blockout"));
    }

    #[test]
    fn file_names_read_as_titles() {
        assert_eq!(pretty("alpine_landscape"), "Alpine Landscape");
        assert_eq!(pretty("rolling-hills"), "Rolling Hills");
    }
}
