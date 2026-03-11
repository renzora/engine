//! Widget spawn functions — each creates the correct entity hierarchy for a widget type.

use std::path::Path;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::components::*;

/// Reference resolution for computing percent values.
struct Ref {
    w: f32,
    h: f32,
}

fn pct_w(px: f32, r: &Ref) -> Val {
    Val::Percent(px / r.w * 100.0)
}
fn pct_h(px: f32, r: &Ref) -> Val {
    Val::Percent(px / r.h * 100.0)
}

/// Spawn any widget by type, parenting to a canvas.
pub fn spawn_widget(world: &mut World, widget_type: &UiWidgetType, parent: Option<Entity>) {
    // Find or create canvas
    let canvas_entity = {
        let mut q = world.query_filtered::<Entity, With<UiCanvas>>();
        match parent.or_else(|| q.iter(world).next()) {
            Some(e) => e,
            None => world
                .spawn((
                    Name::new("UI Canvas"),
                    UiCanvas::default(),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                ))
                .id(),
        }
    };

    let r = parent
        .and_then(|p| world.get::<UiCanvas>(p))
        .map(|c| Ref {
            w: c.reference_width,
            h: c.reference_height,
        })
        .unwrap_or(Ref {
            w: 1280.0,
            h: 720.0,
        });

    let entity = match widget_type {
        UiWidgetType::Container => spawn_container(world, &r),
        UiWidgetType::Panel => spawn_panel(world, &r),
        UiWidgetType::Text => spawn_text(world),
        UiWidgetType::Image => spawn_image(world, &r),
        UiWidgetType::Button => spawn_button(world, &r),
        UiWidgetType::ProgressBar => spawn_progress_bar(world, &r),
        UiWidgetType::HealthBar => spawn_health_bar(world, &r),
        UiWidgetType::Slider => spawn_slider(world, &r),
        UiWidgetType::Checkbox => spawn_checkbox(world, &r),
        UiWidgetType::Toggle => spawn_toggle(world, &r),
        UiWidgetType::ScrollView => spawn_scroll_view(world, &r),
        UiWidgetType::Spinner => spawn_spinner(world, &r),
        _ => spawn_container(world, &r), // Fallback for unimplemented types
    };

    // Mark as themed + parent to canvas
    world.entity_mut(entity).insert(UiThemed).set_parent_in_place(canvas_entity);

    #[cfg(feature = "editor")]
    if let Some(sel) = world.get_resource::<renzora_editor::EditorSelection>() {
        sel.set(Some(entity));
    }
}

fn spawn_container(world: &mut World, r: &Ref) -> Entity {
    world
        .spawn((
            Name::new("Container"),
            UiWidget {
                widget_type: UiWidgetType::Container,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(100.0, r),
                ..default()
            },
        ))
        .id()
}

fn spawn_panel(world: &mut World, r: &Ref) -> Entity {
    world
        .spawn((
            Name::new("Panel"),
            UiWidget {
                widget_type: UiWidgetType::Panel,
                locked: false,
            },
            Node {
                width: pct_w(300.0, r),
                height: pct_h(200.0, r),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.15, 0.18, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.35, 1.0)),
        ))
        .id()
}

fn spawn_text(world: &mut World) -> Entity {
    world
        .spawn((
            Name::new("Text"),
            UiWidget {
                widget_type: UiWidgetType::Text,
                locked: false,
            },
            Node::default(),
            bevy::ui::widget::Text::new("Hello World"),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 16.0,
                ..default()
            },
        ))
        .id()
}

fn spawn_image(world: &mut World, r: &Ref) -> Entity {
    world
        .spawn((
            Name::new("Image"),
            UiWidget {
                widget_type: UiWidgetType::Image,
                locked: false,
            },
            Node {
                width: pct_w(128.0, r),
                height: pct_h(128.0, r),
                ..default()
            },
            BackgroundColor(Color::srgba(0.3, 0.3, 0.3, 1.0)),
        ))
        .id()
}

fn spawn_button(world: &mut World, r: &Ref) -> Entity {
    world
        .spawn((
            Name::new("Button"),
            UiWidget {
                widget_type: UiWidgetType::Button,
                locked: false,
            },
            Node {
                width: pct_w(150.0, r),
                height: pct_h(40.0, r),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            Button,
            Interaction::default(),
            BackgroundColor(Color::srgba(0.25, 0.25, 0.8, 1.0)),
            BorderColor::all(Color::srgba(0.4, 0.4, 0.9, 1.0)),
            UiInteractionStyle::default(),
            UiTransition::default(),
        ))
        .id()
}

fn spawn_progress_bar(world: &mut World, r: &Ref) -> Entity {
    let data = ProgressBarData::default();
    let bg_color = data.bg_color;
    let fill_color = data.fill_color;

    let parent = world
        .spawn((
            Name::new("Progress Bar"),
            UiWidget {
                widget_type: UiWidgetType::ProgressBar,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(24.0, r),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            data,
            BackgroundColor(bg_color),
        ))
        .id();

    // Fill child
    let fill = world
        .spawn((
            UiWidgetPart::new("fill"),
            Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(fill_color),
        ))
        .id();

    world.entity_mut(fill).set_parent_in_place(parent);
    parent
}

fn spawn_health_bar(world: &mut World, r: &Ref) -> Entity {
    let data = HealthBarData::default();
    let bg_color = data.bg_color;
    let fill_color = data.fill_color;

    let parent = world
        .spawn((
            Name::new("Health Bar"),
            UiWidget {
                widget_type: UiWidgetType::HealthBar,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(20.0, r),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            data,
            BackgroundColor(bg_color),
        ))
        .id();

    let fill = world
        .spawn((
            UiWidgetPart::new("fill"),
            Node {
                width: Val::Percent(75.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(fill_color),
        ))
        .id();

    world.entity_mut(fill).set_parent_in_place(parent);
    parent
}

fn spawn_slider(world: &mut World, r: &Ref) -> Entity {
    let data = SliderData::default();

    let parent = world
        .spawn((
            Name::new("Slider"),
            UiWidget {
                widget_type: UiWidgetType::Slider,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(24.0, r),
                align_items: AlignItems::Center,
                ..default()
            },
            data.clone(),
            Interaction::default(),
            RelativeCursorPosition::default(),
        ))
        .id();

    // Track background
    let track = world
        .spawn((
            UiWidgetPart::new("track"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                position_type: PositionType::Absolute,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(data.track_color),
        ))
        .id();

    // Fill
    let fill = world
        .spawn((
            UiWidgetPart::new("fill"),
            Node {
                width: Val::Percent(50.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(data.fill_color),
        ))
        .id();

    // Thumb
    let thumb = world
        .spawn((
            UiWidgetPart::new("thumb"),
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(data.thumb_color),
        ))
        .id();

    world.entity_mut(fill).set_parent_in_place(track);
    world.entity_mut(track).set_parent_in_place(parent);
    world.entity_mut(thumb).set_parent_in_place(parent);
    parent
}

fn spawn_checkbox(world: &mut World, r: &Ref) -> Entity {
    let data = CheckboxData::default();

    let parent = world
        .spawn((
            Name::new("Checkbox"),
            UiWidget {
                widget_type: UiWidgetType::Checkbox,
                locked: false,
            },
            Node {
                width: pct_w(150.0, r),
                height: pct_h(28.0, r),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            data.clone(),
            Button,
            Interaction::default(),
        ))
        .id();

    // Box
    let checkbox_box = world
        .spawn((
            UiWidgetPart::new("box"),
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(data.box_color),
            BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 1.0)),
        ))
        .id();

    // Checkmark (hidden when unchecked)
    let checkmark = world
        .spawn((
            UiWidgetPart::new("checkmark"),
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(data.check_color),
            if data.checked {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ))
        .id();

    // Label
    let label = world
        .spawn((
            UiWidgetPart::new("label"),
            Node::default(),
            bevy::ui::widget::Text::new(data.label.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(checkmark).set_parent_in_place(checkbox_box);
    world.entity_mut(checkbox_box).set_parent_in_place(parent);
    world.entity_mut(label).set_parent_in_place(parent);
    parent
}

fn spawn_toggle(world: &mut World, r: &Ref) -> Entity {
    let data = ToggleData::default();

    let parent = world
        .spawn((
            Name::new("Toggle"),
            UiWidget {
                widget_type: UiWidgetType::Toggle,
                locked: false,
            },
            Node {
                width: pct_w(120.0, r),
                height: pct_h(28.0, r),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            data.clone(),
            Button,
            Interaction::default(),
        ))
        .id();

    // Track
    let track = world
        .spawn((
            UiWidgetPart::new("track"),
            Node {
                width: Val::Px(44.0),
                height: Val::Px(24.0),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(data.off_color),
        ))
        .id();

    // Knob
    let knob = world
        .spawn((
            UiWidgetPart::new("knob"),
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                left: Val::Percent(0.0),
                ..default()
            },
            BackgroundColor(data.knob_color),
        ))
        .id();

    // Label
    let label = world
        .spawn((
            UiWidgetPart::new("label"),
            Node::default(),
            bevy::ui::widget::Text::new(data.label.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(knob).set_parent_in_place(track);
    world.entity_mut(track).set_parent_in_place(parent);
    world.entity_mut(label).set_parent_in_place(parent);
    parent
}

fn spawn_scroll_view(world: &mut World, r: &Ref) -> Entity {
    world
        .spawn((
            Name::new("Scroll View"),
            UiWidget {
                widget_type: UiWidgetType::ScrollView,
                locked: false,
            },
            Node {
                width: pct_w(300.0, r),
                height: pct_h(200.0, r),
                overflow: Overflow::clip(),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            ScrollViewData::default(),
            BackgroundColor(Color::srgba(0.12, 0.12, 0.15, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.35, 1.0)),
        ))
        .id()
}

fn spawn_spinner(world: &mut World, r: &Ref) -> Entity {
    let data = SpinnerData::default();
    world
        .spawn((
            Name::new("Spinner"),
            UiWidget {
                widget_type: UiWidgetType::Spinner,
                locked: false,
            },
            Node {
                width: pct_w(32.0, r),
                height: pct_h(32.0, r),
                border: UiRect::new(Val::Px(3.0), Val::Px(3.0), Val::Px(3.0), Val::Px(0.0)),
                border_radius: BorderRadius::all(Val::Percent(50.0)),
                ..default()
            },
            data,
            BorderColor::all(Color::WHITE),
            BackgroundColor(Color::NONE),
        ))
        .id()
}

/// Spawn an Image widget at a specific canvas position, loading the image from an asset path.
///
/// Called when an image file is drag-dropped from the asset browser onto the UI canvas.
pub fn spawn_image_at(
    world: &mut World,
    asset_path: &Path,
    x: f32,
    y: f32,
    snap: bool,
    grid: f32,
    parent: Option<Entity>,
) {
    // Find or create canvas
    let canvas_entity = {
        let mut q = world.query_filtered::<Entity, With<UiCanvas>>();
        match parent.or_else(|| q.iter(world).next()) {
            Some(e) => e,
            None => world
                .spawn((
                    Name::new("UI Canvas"),
                    UiCanvas::default(),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        position_type: PositionType::Absolute,
                        ..default()
                    },
                ))
                .id(),
        }
    };

    let r = parent
        .and_then(|p| world.get::<UiCanvas>(p))
        .map(|c| Ref {
            w: c.reference_width,
            h: c.reference_height,
        })
        .unwrap_or(Ref {
            w: 1280.0,
            h: 720.0,
        });

    // Convert to asset-relative path (e.g. "textures/player.png") for portability.
    // Works in editor via EmbeddedAssetReader and in standalone runtime.
    let load_path = if let Some(project) = world.get_resource::<renzora_core::CurrentProject>() {
        project.make_asset_relative(asset_path)
    } else {
        asset_path.to_string_lossy().replace('\\', "/")
    };

    let image_handle: Handle<Image> = world.resource::<AssetServer>().load(load_path);

    // Snap position if enabled
    let mut px = x;
    let mut py = y;
    if snap {
        px = (px / grid).round() * grid;
        py = (py / grid).round() * grid;
    }

    let name = asset_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Image")
        .to_string();

    let entity = world
        .spawn((
            Name::new(name),
            UiWidget {
                widget_type: UiWidgetType::Image,
                locked: false,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(px / r.w * 100.0),
                top: Val::Percent(py / r.h * 100.0),
                width: pct_w(128.0, &r),
                height: pct_h(128.0, &r),
                ..default()
            },
            ImageNode::new(image_handle),
            UiThemed,
        ))
        .id();

    world.entity_mut(entity).set_parent_in_place(canvas_entity);

    #[cfg(feature = "editor")]
    if let Some(sel) = world.get_resource::<renzora_editor::EditorSelection>() {
        sel.set(Some(entity));
    }
}
