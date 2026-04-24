#![allow(unused_variables, unused_assignments, dead_code)]

//! Widget spawn functions — each creates the correct entity hierarchy for a widget type.

use std::path::Path;

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::components::*;
use crate::shapes::{self, UiShapeWidget};

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

/// Spawn any widget by type, parenting to a canvas. Returns the spawned entity.
pub fn spawn_widget(world: &mut World, widget_type: &UiWidgetType, parent: Option<Entity>) -> Entity {
    // Find or create canvas.
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
        UiWidgetType::Text => spawn_text(world, &r),
        UiWidgetType::Image => spawn_image(world, &r),
        UiWidgetType::Button => spawn_button(world, &r),
        UiWidgetType::ProgressBar => spawn_progress_bar(world, &r),
        UiWidgetType::HealthBar => spawn_health_bar(world, &r),
        UiWidgetType::Slider => spawn_slider(world, &r),
        UiWidgetType::Checkbox => spawn_checkbox(world, &r),
        UiWidgetType::Toggle => spawn_toggle(world, &r),
        UiWidgetType::RadioButton => spawn_radio_button(world, &r),
        UiWidgetType::Dropdown => spawn_dropdown(world, &r),
        UiWidgetType::TextInput => spawn_text_input(world, &r),
        UiWidgetType::ScrollView => spawn_scroll_view(world, &r),
        UiWidgetType::TabBar => spawn_tab_bar(world, &r),
        UiWidgetType::Spinner => spawn_spinner(world, &r),
        UiWidgetType::Tooltip => spawn_tooltip(world, &r),
        UiWidgetType::Modal => spawn_modal(world, &r),
        UiWidgetType::DraggableWindow => spawn_draggable_window(world, &r),
        UiWidgetType::Crosshair => spawn_crosshair(world, &r),
        UiWidgetType::AmmoCounter => spawn_ammo_counter(world, &r),
        UiWidgetType::Compass => spawn_compass(world, &r),
        UiWidgetType::StatusEffectBar => spawn_status_effect_bar(world, &r),
        UiWidgetType::NotificationFeed => spawn_notification_feed(world, &r),
        UiWidgetType::RadialMenu => spawn_radial_menu(world, &r),
        UiWidgetType::Minimap => spawn_minimap(world, &r),
        UiWidgetType::InventoryGrid => spawn_inventory_grid(world, &r),
        UiWidgetType::DialogBox => spawn_dialog_box(world, &r),
        UiWidgetType::ObjectiveTracker => spawn_objective_tracker(world, &r),
        UiWidgetType::LoadingScreen => spawn_loading_screen(world, &r),
        UiWidgetType::KeybindRow => spawn_keybind_row(world, &r),
        UiWidgetType::SettingsRow => spawn_settings_row(world, &r),
        UiWidgetType::Separator => spawn_separator(world, &r),
        UiWidgetType::NumberInput => spawn_number_input(world, &r),
        UiWidgetType::VerticalSlider => spawn_vertical_slider(world, &r),
        UiWidgetType::Scrollbar => spawn_scrollbar(world, &r),
        UiWidgetType::List => spawn_list(world, &r),
        UiWidgetType::Circle => spawn_circle(world, &r),
        UiWidgetType::Arc => spawn_arc(world, &r),
        UiWidgetType::RadialProgress => spawn_radial_progress(world, &r),
        UiWidgetType::Line => spawn_line(world, &r),
        UiWidgetType::Triangle => spawn_triangle(world, &r),
        UiWidgetType::Polygon => spawn_polygon(world, &r),
        UiWidgetType::Rectangle => spawn_rectangle(world, &r),
        UiWidgetType::Wedge => spawn_wedge(world, &r),
    };

    // Mark as themed + parent to canvas. Use `set_parent_in_place` to match
    // the pattern used by composite widget spawners elsewhere in this file —
    // bare `insert(ChildOf)` doesn't fire the Children-update hook at runtime.
    world.entity_mut(entity).insert(UiThemed);
    world.entity_mut(entity).set_parent_in_place(canvas_entity);

    #[cfg(feature = "editor")]
    {
        if let Some(requests) =
            world.get_resource::<renzora_editor_framework::HierarchyExpandRequests>()
        {
            requests.push(canvas_entity);
        }
        if let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() {
            sel.set(Some(entity));
        }
    }

    entity
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
            UiStroke::new(Color::srgba(0.3, 0.3, 0.35, 0.5), 1.0),
            UiBorderRadius::all(2.0),
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
                ..default()
            },
            UiFill::solid(Color::srgba(0.15, 0.15, 0.18, 0.9)),
            UiStroke::new(Color::srgba(0.3, 0.3, 0.35, 1.0), 1.0),
            UiBorderRadius::all(6.0),
            UiPadding::all(8.0),
            BackgroundColor(Color::srgba(0.15, 0.15, 0.18, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.35, 1.0)),
        ))
        .id()
}

fn spawn_text(world: &mut World, r: &Ref) -> Entity {
    world
        .spawn((
            Name::new("Text"),
            UiWidget {
                widget_type: UiWidgetType::Text,
                locked: false,
            },
            Node {
                width: pct_w(150.0, r),
                height: pct_h(30.0, r),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            bevy::ui::widget::Text::new("Hello World"),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            UiTextStyle {
                color: Color::WHITE,
                size: 16.0,
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
            UiFill::solid(Color::srgba(0.3, 0.3, 0.3, 1.0)),
            UiBorderRadius::all(2.0),
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
                ..default()
            },
            Button,
            Interaction::default(),
            UiFill::solid(Color::srgba(0.25, 0.25, 0.8, 1.0)),
            UiStroke::new(Color::srgba(0.4, 0.4, 0.9, 1.0), 1.0),
            UiBorderRadius::all(4.0),
            UiCursor::Pointer,
            UiTextStyle {
                color: Color::WHITE,
                size: 14.0,
                bold: true,
                ..default()
            },
            UiPadding::symmetric(6.0, 16.0),
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
                ..default()
            },
            data,
            UiFill::solid(bg_color),
            UiBorderRadius::all(4.0),
            UiClipContent(true),
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
                ..default()
            },
            data,
            UiFill::solid(bg_color),
            UiBorderRadius::all(3.0),
            UiClipContent(true),
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
            UiCursor::Pointer,
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
            UiCursor::Pointer,
            UiTextStyle {
                color: Color::WHITE,
                size: 14.0,
                ..default()
            },
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
            UiCursor::Pointer,
            UiTextStyle {
                color: Color::WHITE,
                size: 14.0,
                ..default()
            },
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
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ScrollViewData::default(),
            UiFill::solid(Color::srgba(0.12, 0.12, 0.15, 0.9)),
            UiStroke::new(Color::srgba(0.3, 0.3, 0.35, 1.0), 1.0),
            UiBorderRadius::all(4.0),
            UiClipContent(true),
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
                ..default()
            },
            data,
            UiStroke {
                color: Color::WHITE,
                width: 3.0,
                sides: UiSides { top: true, right: true, bottom: true, left: false },
            },
            UiBorderRadius::all(999.0),
            BorderColor::all(Color::WHITE),
            BackgroundColor(Color::NONE),
        ))
        .id()
}

fn spawn_radio_button(world: &mut World, r: &Ref) -> Entity {
    let data = RadioButtonData::default();

    let parent = world
        .spawn((
            Name::new("Radio Button"),
            UiWidget {
                widget_type: UiWidgetType::RadioButton,
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
            UiCursor::Pointer,
            UiTextStyle {
                color: Color::WHITE,
                size: 14.0,
                ..default()
            },
        ))
        .id();

    // Radio circle
    let circle = world
        .spawn((
            UiWidgetPart::new("circle"),
            Node {
                width: Val::Px(20.0),
                height: Val::Px(20.0),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(2.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::srgba(0.5, 0.5, 0.5, 1.0)),
        ))
        .id();

    // Inner dot (visible when selected)
    let dot = world
        .spawn((
            UiWidgetPart::new("dot"),
            Node {
                width: Val::Px(10.0),
                height: Val::Px(10.0),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(data.active_color),
            if data.selected {
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

    world.entity_mut(dot).set_parent_in_place(circle);
    world.entity_mut(circle).set_parent_in_place(parent);
    world.entity_mut(label).set_parent_in_place(parent);
    parent
}

fn spawn_dropdown(world: &mut World, r: &Ref) -> Entity {
    let data = DropdownData::default();
    let display_text = if data.selected >= 0 && (data.selected as usize) < data.options.len() {
        data.options[data.selected as usize].clone()
    } else {
        data.placeholder.clone()
    };

    let parent = world
        .spawn((
            Name::new("Dropdown"),
            UiWidget {
                widget_type: UiWidgetType::Dropdown,
                locked: false,
            },
            Node {
                width: pct_w(180.0, r),
                height: pct_h(32.0, r),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            data,
            Button,
            Interaction::default(),
            UiFill::solid(Color::srgba(0.18, 0.18, 0.22, 1.0)),
            UiStroke::new(Color::srgba(0.35, 0.35, 0.4, 1.0), 1.0),
            UiBorderRadius::all(4.0),
            UiCursor::Pointer,
            UiPadding::symmetric(4.0, 10.0),
            BackgroundColor(Color::srgba(0.18, 0.18, 0.22, 1.0)),
            BorderColor::all(Color::srgba(0.35, 0.35, 0.4, 1.0)),
        ))
        .id();

    // Display text
    let text = world
        .spawn((
            UiWidgetPart::new("label"),
            Node::default(),
            bevy::ui::widget::Text::new(display_text),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    // Arrow indicator
    let arrow = world
        .spawn((
            UiWidgetPart::new("arrow"),
            Node::default(),
            bevy::ui::widget::Text::new("▼"),
            TextColor(Color::srgba(0.6, 0.6, 0.6, 1.0)),
            TextFont {
                font_size: 10.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(text).set_parent_in_place(parent);
    world.entity_mut(arrow).set_parent_in_place(parent);
    parent
}

fn spawn_text_input(world: &mut World, r: &Ref) -> Entity {
    let data = TextInputData::default();

    let parent = world
        .spawn((
            Name::new("Text Input"),
            UiWidget {
                widget_type: UiWidgetType::TextInput,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(32.0, r),
                align_items: AlignItems::Center,
                ..default()
            },
            data.clone(),
            Button,
            Interaction::default(),
            UiFill::solid(Color::srgba(0.12, 0.12, 0.15, 1.0)),
            UiStroke::new(Color::srgba(0.35, 0.35, 0.4, 1.0), 1.0),
            UiBorderRadius::all(4.0),
            UiCursor::Text,
            UiPadding::symmetric(4.0, 8.0),
            BackgroundColor(Color::srgba(0.12, 0.12, 0.15, 1.0)),
            BorderColor::all(Color::srgba(0.35, 0.35, 0.4, 1.0)),
        ))
        .id();

    // Text display
    let display = world
        .spawn((
            UiWidgetPart::new("text"),
            Node::default(),
            bevy::ui::widget::Text::new(if data.text.is_empty() {
                data.placeholder.clone()
            } else {
                data.text.clone()
            }),
            TextColor(if data.text.is_empty() {
                Color::srgba(0.5, 0.5, 0.5, 1.0)
            } else {
                Color::WHITE
            }),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(display).set_parent_in_place(parent);
    parent
}

fn spawn_tab_bar(world: &mut World, r: &Ref) -> Entity {
    let data = TabBarData::default();

    let parent = world
        .spawn((
            Name::new("Tab Bar"),
            UiWidget {
                widget_type: UiWidgetType::TabBar,
                locked: false,
            },
            Node {
                width: pct_w(400.0, r),
                height: pct_h(36.0, r),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            data.clone(),
            UiFill::solid(Color::srgba(0.15, 0.15, 0.18, 1.0)),
            UiBorderRadius::all(4.0),
            UiClipContent(true),
            BackgroundColor(Color::srgba(0.15, 0.15, 0.18, 1.0)),
        ))
        .id();

    // Spawn tab children
    for (i, tab_name) in data.tabs.iter().enumerate() {
        let is_active = i == data.active;
        let tab = world
            .spawn((
                UiWidgetPart::new(&format!("tab_{}", i)),
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                Button,
                Interaction::default(),
                BackgroundColor(if is_active { data.active_color } else { data.tab_color }),
                bevy::ui::widget::Text::new(tab_name.clone()),
                TextColor(Color::WHITE),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
            ))
            .id();

        world.entity_mut(tab).set_parent_in_place(parent);
    }

    parent
}

fn spawn_tooltip(world: &mut World, r: &Ref) -> Entity {
    let data = TooltipData::default();

    world
        .spawn((
            Name::new("Tooltip"),
            UiWidget {
                widget_type: UiWidgetType::Tooltip,
                locked: false,
            },
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            data.clone(),
            Visibility::Hidden,
            UiFill::solid(data.bg_color),
            UiBorderRadius::all(4.0),
            UiPadding::symmetric(4.0, 8.0),
            BackgroundColor(data.bg_color),
            bevy::ui::widget::Text::new(data.text.clone()),
            TextColor(data.text_color),
            TextFont {
                font_size: 12.0,
                ..default()
            },
        ))
        .id()
}

fn spawn_modal(world: &mut World, r: &Ref) -> Entity {
    let data = ModalData::default();

    // Backdrop
    let backdrop = world
        .spawn((
            Name::new("Modal"),
            UiWidget {
                widget_type: UiWidgetType::Modal,
                locked: false,
            },
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            data.clone(),
            Visibility::Hidden,
            BackgroundColor(data.backdrop_color),
        ))
        .id();

    // Dialog box
    let dialog = world
        .spawn((
            UiWidgetPart::new("dialog"),
            Node {
                width: pct_w(400.0, r),
                height: pct_h(250.0, r),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.18, 0.18, 0.22, 1.0)),
            BorderColor::all(Color::srgba(0.35, 0.35, 0.4, 1.0)),
        ))
        .id();

    // Title
    let title = world
        .spawn((
            UiWidgetPart::new("title"),
            Node {
                margin: UiRect::bottom(Val::Px(12.0)),
                ..default()
            },
            bevy::ui::widget::Text::new(data.title.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 18.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(title).set_parent_in_place(dialog);
    world.entity_mut(dialog).set_parent_in_place(backdrop);
    backdrop
}

fn spawn_draggable_window(world: &mut World, r: &Ref) -> Entity {
    let data = DraggableWindowData::default();

    let window = world
        .spawn((
            Name::new("Window"),
            UiWidget {
                widget_type: UiWidgetType::DraggableWindow,
                locked: false,
            },
            Node {
                width: pct_w(300.0, r),
                height: pct_h(200.0, r),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            data.clone(),
            UiFill::solid(Color::srgba(0.15, 0.15, 0.18, 0.95)),
            UiStroke::new(Color::srgba(0.3, 0.3, 0.35, 1.0), 1.0),
            UiBorderRadius::all(6.0),
            UiClipContent(true),
            BackgroundColor(Color::srgba(0.15, 0.15, 0.18, 0.95)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.35, 1.0)),
        ))
        .id();

    // Title bar
    let title_bar = world
        .spawn((
            UiWidgetPart::new("title_bar"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            BackgroundColor(data.title_bar_color),
            UiCursor::Grab,
        ))
        .id();

    // Title text
    let title_text = world
        .spawn((
            UiWidgetPart::new("title_text"),
            Node::default(),
            bevy::ui::widget::Text::new(data.title.clone()),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 13.0,
                ..default()
            },
        ))
        .id();

    // Content area
    let content = world
        .spawn((
            UiWidgetPart::new("content"),
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .id();

    world.entity_mut(title_text).set_parent_in_place(title_bar);
    world.entity_mut(title_bar).set_parent_in_place(window);
    world.entity_mut(content).set_parent_in_place(window);
    window
}

// ── HUD spawn functions ────────────────────────────────────────────────────

fn spawn_crosshair(world: &mut World, _r: &Ref) -> Entity {
    let data = CrosshairData::default();

    let parent = world
        .spawn((
            Name::new("Crosshair"),
            UiWidget {
                widget_type: UiWidgetType::Crosshair,
                locked: false,
            },
            Node {
                width: Val::Px(data.size),
                height: Val::Px(data.size),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                ..default()
            },
            data,
        ))
        .id();

    // Horizontal line
    world
        .spawn((
            UiWidgetPart::new("h_line"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                position_type: PositionType::Absolute,
                top: Val::Percent(50.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
        ))
        .set_parent_in_place(parent);

    // Vertical line
    world
        .spawn((
            UiWidgetPart::new("v_line"),
            Node {
                width: Val::Px(2.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
        ))
        .set_parent_in_place(parent);

    parent
}

fn spawn_ammo_counter(world: &mut World, r: &Ref) -> Entity {
    let data = AmmoCounterData::default();
    let display = format!("{} / {}", data.current, data.max);

    let parent = world
        .spawn((
            Name::new("Ammo Counter"),
            UiWidget {
                widget_type: UiWidgetType::AmmoCounter,
                locked: false,
            },
            Node {
                width: pct_w(120.0, r),
                height: pct_h(36.0, r),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            data,
            UiFill::solid(Color::srgba(0.1, 0.1, 0.12, 0.8)),
            UiBorderRadius::all(4.0),
            UiPadding::symmetric(4.0, 8.0),
            BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.8)),
        ))
        .id();

    let text = world
        .spawn((
            UiWidgetPart::new("text"),
            Node::default(),
            bevy::ui::widget::Text::new(display),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 18.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(text).set_parent_in_place(parent);
    parent
}

fn spawn_compass(world: &mut World, r: &Ref) -> Entity {
    let data = CompassData::default();

    let parent = world
        .spawn((
            Name::new("Compass"),
            UiWidget {
                widget_type: UiWidgetType::Compass,
                locked: false,
            },
            Node {
                width: pct_w(400.0, r),
                height: pct_h(30.0, r),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                ..default()
            },
            data.clone(),
            UiFill::solid(Color::srgba(0.1, 0.1, 0.12, 0.6)),
            UiBorderRadius::all(2.0),
            UiClipContent(true),
            BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.6)),
        ))
        .id();

    // Spawn marker labels as children
    for (i, marker) in data.markers.iter().enumerate() {
        let child = world
            .spawn((
                UiWidgetPart::new(&format!("marker_{}", i)),
                Node::default(),
                bevy::ui::widget::Text::new(marker.label.clone()),
                TextColor(marker.color),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
            ))
            .id();

        world.entity_mut(child).set_parent_in_place(parent);
    }

    parent
}

fn spawn_status_effect_bar(world: &mut World, r: &Ref) -> Entity {
    let data = StatusEffectBarData::default();

    world
        .spawn((
            Name::new("Status Effects"),
            UiWidget {
                widget_type: UiWidgetType::StatusEffectBar,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(data.icon_size, r),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(data.spacing),
                align_items: AlignItems::Center,
                ..default()
            },
            data,
        ))
        .id()
}

fn spawn_notification_feed(world: &mut World, r: &Ref) -> Entity {
    let data = NotificationFeedData::default();

    world
        .spawn((
            Name::new("Notifications"),
            UiWidget {
                widget_type: UiWidgetType::NotificationFeed,
                locked: false,
            },
            Node {
                width: pct_w(300.0, r),
                height: pct_h(200.0, r),
                position_type: PositionType::Absolute,
                right: Val::Percent(2.0),
                top: Val::Percent(2.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            data,
        ))
        .id()
}

fn spawn_radial_menu(world: &mut World, r: &Ref) -> Entity {
    let data = RadialMenuData::default();

    let parent = world
        .spawn((
            Name::new("Radial Menu"),
            UiWidget {
                widget_type: UiWidgetType::RadialMenu,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(200.0, r),
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            data.clone(),
            Visibility::Hidden,
        ))
        .id();

    // Spawn one wedge shape per item
    let item_count = data.items.len();
    for (i, item) in data.items.iter().enumerate() {
        let angle_step = std::f32::consts::TAU / item_count as f32;
        let start_angle = i as f32 * angle_step;
        let end_angle = start_angle + angle_step;

        let shape = shapes::WedgeShape {
            color: item.color,
            start_angle,
            end_angle,
            inner_radius: data.inner_radius,
        };
        let handle = world
            .resource_mut::<Assets<shapes::WedgeMaterial>>()
            .add(shapes::WedgeMaterial::from_shape(&shape));

        let wedge = world
            .spawn((
                UiWidgetPart::new(&format!("wedge_{}", i)),
                UiShapeWidget,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                MaterialNode(handle),
                shape,
            ))
            .id();

        world.entity_mut(wedge).set_parent_in_place(parent);
    }

    parent
}

fn spawn_minimap(world: &mut World, r: &Ref) -> Entity {
    let data = MinimapData::default();

    let parent = world
        .spawn((
            Name::new("Minimap"),
            UiWidget {
                widget_type: UiWidgetType::Minimap,
                locked: false,
            },
            Node {
                width: pct_w(160.0, r),
                height: pct_h(160.0, r),
                position_type: PositionType::Absolute,
                right: Val::Percent(2.0),
                bottom: Val::Percent(2.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            data.clone(),
        ))
        .id();

    // Circle frame using a CircleShape
    let frame_shape = shapes::CircleShape {
        color: data.bg_color,
        stroke_color: data.border_color,
        stroke_width: data.border_width,
    };
    let handle = world
        .resource_mut::<Assets<shapes::CircleMaterial>>()
        .add(shapes::CircleMaterial::from_shape(&frame_shape));

    let frame = world
        .spawn((
            UiWidgetPart::new("frame"),
            UiShapeWidget,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            MaterialNode(handle),
            frame_shape,
        ))
        .id();

    world.entity_mut(frame).set_parent_in_place(parent);
    parent
}

// ── Shape spawn functions ──────────────────────────────────────────────────

fn spawn_circle(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::CircleShape::default();
    let handle = world
        .resource_mut::<Assets<shapes::CircleMaterial>>()
        .add(shapes::CircleMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Circle"),
            UiWidget {
                widget_type: UiWidgetType::Circle,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(64.0, r),
                height: pct_h(64.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

fn spawn_arc(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::ArcShape::default();
    let handle = world
        .resource_mut::<Assets<shapes::ArcMaterial>>()
        .add(shapes::ArcMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Arc"),
            UiWidget {
                widget_type: UiWidgetType::Arc,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(64.0, r),
                height: pct_h(64.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

fn spawn_radial_progress(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::RadialProgressShape::default();
    let handle = world
        .resource_mut::<Assets<shapes::RadialProgressMaterial>>()
        .add(shapes::RadialProgressMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Radial Progress"),
            UiWidget {
                widget_type: UiWidgetType::RadialProgress,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(64.0, r),
                height: pct_h(64.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

fn spawn_line(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::LineShape::default();
    let handle = world
        .resource_mut::<Assets<shapes::LineMaterial>>()
        .add(shapes::LineMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Line"),
            UiWidget {
                widget_type: UiWidgetType::Line,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(100.0, r),
                height: pct_h(4.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

fn spawn_triangle(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::TriangleShape::default();
    let handle = world
        .resource_mut::<Assets<shapes::TriangleMaterial>>()
        .add(shapes::TriangleMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Triangle"),
            UiWidget {
                widget_type: UiWidgetType::Triangle,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(64.0, r),
                height: pct_h(64.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

fn spawn_polygon(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::PolygonShape::default();
    let handle = world
        .resource_mut::<Assets<shapes::PolygonMaterial>>()
        .add(shapes::PolygonMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Polygon"),
            UiWidget {
                widget_type: UiWidgetType::Polygon,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(64.0, r),
                height: pct_h(64.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

fn spawn_rectangle(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::RectangleShape {
        color: Color::srgba(0.35, 0.55, 0.9, 1.0),
        corner_radius: [8.0; 4],
        ..default()
    };
    let handle = world
        .resource_mut::<Assets<shapes::RectangleMaterial>>()
        .add(shapes::RectangleMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Rectangle"),
            UiWidget {
                widget_type: UiWidgetType::Rectangle,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(96.0, r),
                height: pct_h(64.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

fn spawn_wedge(world: &mut World, r: &Ref) -> Entity {
    let shape = shapes::WedgeShape::default();
    let handle = world
        .resource_mut::<Assets<shapes::WedgeMaterial>>()
        .add(shapes::WedgeMaterial::from_shape(&shape));
    world
        .spawn((
            Name::new("Wedge"),
            UiWidget {
                widget_type: UiWidgetType::Wedge,
                locked: false,
            },
            UiShapeWidget,
            Node {
                width: pct_w(64.0, r),
                height: pct_h(64.0, r),
                ..default()
            },
            MaterialNode(handle),
            shape,
        ))
        .id()
}

// ── Menu widget spawn functions ─────────────────────────────────────────────

fn spawn_inventory_grid(world: &mut World, r: &Ref) -> Entity {
    let data = InventoryGridData::default();
    let cols = data.columns;
    let rows = data.rows;
    let slot_size = data.slot_size;
    let gap = data.gap;
    let slot_bg = data.slot_bg_color;
    let slot_border = data.slot_border_color;
    let slot_border_w = data.slot_border_width;

    let parent = world
        .spawn((
            Name::new("Inventory Grid"),
            UiWidget {
                widget_type: UiWidgetType::InventoryGrid,
                locked: false,
            },
            Node {
                width: pct_w(cols as f32 * (slot_size + gap) + gap, r),
                height: pct_h(rows as f32 * (slot_size + gap) + gap, r),
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::px(cols as u16, slot_size),
                grid_template_rows: RepeatedGridTrack::px(rows as u16, slot_size),
                column_gap: Val::Px(gap),
                row_gap: Val::Px(gap),
                padding: UiRect::all(Val::Px(gap)),
                ..default()
            },
            data,
            UiFill::solid(Color::srgba(0.1, 0.1, 0.12, 0.9)),
            UiBorderRadius::all(4.0),
            BackgroundColor(Color::srgba(0.1, 0.1, 0.12, 0.9)),
        ))
        .id();

    for row in 0..rows {
        for col in 0..cols {
            let slot = world
                .spawn((
                    UiWidgetPart::new("slot"),
                    InventorySlot { col, row },
                    Node {
                        width: Val::Px(slot_size),
                        height: Val::Px(slot_size),
                        border: UiRect::all(Val::Px(slot_border_w)),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(slot_bg),
                    BorderColor::all(slot_border),
                ))
                .id();

            world.entity_mut(slot).set_parent_in_place(parent);
        }
    }

    parent
}

fn spawn_dialog_box(world: &mut World, r: &Ref) -> Entity {
    let data = DialogBoxData::default();
    let speaker_color = data.speaker_color;
    let text_color = data.text_color;
    let bg_color = data.bg_color;
    let speaker_name = data.speaker.clone();

    let parent = world
        .spawn((
            Name::new("Dialog Box"),
            UiWidget {
                widget_type: UiWidgetType::DialogBox,
                locked: false,
            },
            Node {
                width: Val::Percent(80.0),
                position_type: PositionType::Absolute,
                left: Val::Percent(10.0),
                bottom: Val::Percent(5.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            data,
            UiFill::solid(bg_color),
            UiBorderRadius::all(8.0),
            BackgroundColor(bg_color),
        ))
        .id();

    let speaker = world
        .spawn((
            UiWidgetPart::new("speaker"),
            Node::default(),
            bevy::ui::widget::Text::new(speaker_name),
            TextColor(speaker_color),
            TextFont {
                font_size: 18.0,
                ..default()
            },
        ))
        .id();

    let text = world
        .spawn((
            UiWidgetPart::new("text"),
            Node::default(),
            bevy::ui::widget::Text::new(""),
            TextColor(text_color),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(speaker).set_parent_in_place(parent);
    world.entity_mut(text).set_parent_in_place(parent);
    parent
}

fn spawn_objective_tracker(world: &mut World, r: &Ref) -> Entity {
    let data = ObjectiveTrackerData::default();
    let title_text = data.title.clone();
    let title_color = data.title_color;
    let objectives = data.objectives.clone();
    let active_color = data.active_color;
    let completed_color = data.completed_color;
    let failed_color = data.failed_color;

    let parent = world
        .spawn((
            Name::new("Objective Tracker"),
            UiWidget {
                widget_type: UiWidgetType::ObjectiveTracker,
                locked: false,
            },
            Node {
                width: pct_w(250.0, r),
                position_type: PositionType::Absolute,
                right: Val::Percent(2.0),
                top: Val::Percent(10.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(6.0),
                ..default()
            },
            data,
            UiFill::solid(Color::srgba(0.08, 0.08, 0.1, 0.7)),
            UiBorderRadius::all(6.0),
            BackgroundColor(Color::srgba(0.08, 0.08, 0.1, 0.7)),
        ))
        .id();

    let title = world
        .spawn((
            UiWidgetPart::new("title"),
            Node {
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            bevy::ui::widget::Text::new(title_text),
            TextColor(title_color),
            TextFont {
                font_size: 16.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(title).set_parent_in_place(parent);

    for obj in &objectives {
        let color = match obj.status {
            ObjectiveStatus::Active => active_color,
            ObjectiveStatus::Completed => completed_color,
            ObjectiveStatus::Failed => failed_color,
        };
        let prefix = match obj.status {
            ObjectiveStatus::Active => "○ ",
            ObjectiveStatus::Completed => "● ",
            ObjectiveStatus::Failed => "✕ ",
        };
        let progress_str = match obj.progress {
            Some((cur, max)) => format!(" ({}/{})", cur, max),
            None => String::new(),
        };
        let label = format!("{}{}{}", prefix, obj.label, progress_str);

        let child = world
            .spawn((
                UiWidgetPart::new("objective"),
                Node::default(),
                bevy::ui::widget::Text::new(label),
                TextColor(color),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
            ))
            .id();

        world.entity_mut(child).set_parent_in_place(parent);
    }

    parent
}

fn spawn_loading_screen(world: &mut World, r: &Ref) -> Entity {
    let data = LoadingScreenData::default();
    let bg_color = data.bg_color;
    let bar_color = data.bar_color;
    let bar_bg_color = data.bar_bg_color;
    let text_color = data.text_color;
    let message = data.message.clone();
    let progress = data.progress;

    let parent = world
        .spawn((
            Name::new("Loading Screen"),
            UiWidget {
                widget_type: UiWidgetType::LoadingScreen,
                locked: false,
            },
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(24.0),
                ..default()
            },
            data,
            UiFill::solid(bg_color),
            BackgroundColor(bg_color),
        ))
        .id();

    let msg = world
        .spawn((
            UiWidgetPart::new("message"),
            Node::default(),
            bevy::ui::widget::Text::new(message),
            TextColor(text_color),
            TextFont {
                font_size: 18.0,
                ..default()
            },
        ))
        .id();

    let bar_bg = world
        .spawn((
            UiWidgetPart::new("bar_bg"),
            Node {
                width: Val::Percent(40.0),
                height: pct_h(20.0, r),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(bar_bg_color),
        ))
        .id();

    let bar_fill = world
        .spawn((
            UiWidgetPart::new("bar_fill"),
            Node {
                width: Val::Percent(progress.clamp(0.0, 1.0) * 100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(bar_color),
        ))
        .id();

    world.entity_mut(bar_fill).set_parent_in_place(bar_bg);
    world.entity_mut(msg).set_parent_in_place(parent);
    world.entity_mut(bar_bg).set_parent_in_place(parent);
    parent
}

fn spawn_keybind_row(world: &mut World, r: &Ref) -> Entity {
    let data = KeybindRowData::default();
    let action = data.action.clone();
    let binding = data.binding.clone();
    let label_color = data.label_color;
    let key_bg = data.key_bg_color;
    let key_text = data.key_text_color;

    let parent = world
        .spawn((
            Name::new("Keybind Row"),
            UiWidget {
                widget_type: UiWidgetType::KeybindRow,
                locked: false,
            },
            Node {
                width: pct_w(300.0, r),
                height: pct_h(36.0, r),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                ..default()
            },
            data,
        ))
        .id();

    let action_label = world
        .spawn((
            UiWidgetPart::new("action"),
            Node::default(),
            bevy::ui::widget::Text::new(action),
            TextColor(label_color),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    let key_badge = world
        .spawn((
            UiWidgetPart::new("key"),
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(key_bg),
            bevy::ui::widget::Text::new(binding),
            TextColor(key_text),
            TextFont {
                font_size: 13.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(action_label).set_parent_in_place(parent);
    world.entity_mut(key_badge).set_parent_in_place(parent);
    parent
}

fn spawn_settings_row(world: &mut World, r: &Ref) -> Entity {
    let data = SettingsRowData::default();
    let label_text = data.label.clone();
    let label_color = data.label_color;
    let value_text = data.value.clone();

    let parent = world
        .spawn((
            Name::new("Settings Row"),
            UiWidget {
                widget_type: UiWidgetType::SettingsRow,
                locked: false,
            },
            Node {
                width: pct_w(400.0, r),
                height: pct_h(36.0, r),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                ..default()
            },
            data,
        ))
        .id();

    let label = world
        .spawn((
            UiWidgetPart::new("label"),
            Node::default(),
            bevy::ui::widget::Text::new(label_text),
            TextColor(label_color),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    let control = world
        .spawn((
            UiWidgetPart::new("control"),
            Node::default(),
            bevy::ui::widget::Text::new(value_text),
            TextColor(Color::srgba(0.7, 0.7, 0.7, 1.0)),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(label).set_parent_in_place(parent);
    world.entity_mut(control).set_parent_in_place(parent);
    parent
}

// ── Separator ──────────────────────────────────────────────────────────────

fn spawn_separator(world: &mut World, r: &Ref) -> Entity {
    let data = SeparatorData::default();
    let (w, h) = match data.direction {
        SeparatorDirection::Horizontal => (pct_w(200.0, r), Val::Px(data.thickness)),
        SeparatorDirection::Vertical => (Val::Px(data.thickness), pct_h(200.0, r)),
    };
    world
        .spawn((
            Name::new("Separator"),
            UiWidget {
                widget_type: UiWidgetType::Separator,
                locked: false,
            },
            Node {
                width: w,
                height: h,
                margin: UiRect::all(Val::Px(data.margin)),
                ..default()
            },
            UiFill::solid(data.color),
            BackgroundColor(data.color),
            data,
        ))
        .id()
}

// ── Number Input ───────────────────────────────────────────────────────────

fn spawn_number_input(world: &mut World, r: &Ref) -> Entity {
    let data = NumberInputData::default();

    let root = world
        .spawn((
            Name::new("Number Input"),
            UiWidget {
                widget_type: UiWidgetType::NumberInput,
                locked: false,
            },
            Node {
                width: pct_w(160.0, r),
                height: pct_h(32.0, r),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            UiFill::solid(data.bg_color),
            UiBorderRadius::all(4.0),
            BackgroundColor(data.bg_color),
            data.clone(),
        ))
        .id();

    // Decrement button
    let dec = world
        .spawn((
            UiWidgetPart::new("decrement"),
            Node {
                width: Val::Px(28.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(data.button_color),
            Text::new("-"),
            TextColor(data.text_color),
            TextFont {
                font_size: 16.0,
                ..default()
            },
        ))
        .id();

    // Value display
    let val = world
        .spawn((
            UiWidgetPart::new("value"),
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(format!("{:.*}", data.precision as usize, data.value)),
            TextColor(data.text_color),
            TextFont {
                font_size: 14.0,
                ..default()
            },
        ))
        .id();

    // Increment button
    let inc = world
        .spawn((
            UiWidgetPart::new("increment"),
            Node {
                width: Val::Px(28.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(data.button_color),
            Text::new("+"),
            TextColor(data.text_color),
            TextFont {
                font_size: 16.0,
                ..default()
            },
        ))
        .id();

    world.entity_mut(dec).set_parent_in_place(root);
    world.entity_mut(val).set_parent_in_place(root);
    world.entity_mut(inc).set_parent_in_place(root);
    root
}

// ── Vertical Slider ────────────────────────────────────────────────────────

fn spawn_vertical_slider(world: &mut World, r: &Ref) -> Entity {
    let data = VerticalSliderData::default();
    let frac = if (data.max - data.min).abs() > f32::EPSILON {
        ((data.value - data.min) / (data.max - data.min)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let root = world
        .spawn((
            Name::new("Vertical Slider"),
            UiWidget {
                widget_type: UiWidgetType::VerticalSlider,
                locked: false,
            },
            Node {
                width: pct_w(24.0, r),
                height: pct_h(150.0, r),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::End,
                ..default()
            },
            data.clone(),
            Interaction::default(),
            RelativeCursorPosition::default(),
            UiCursor::Pointer,
        ))
        .id();

    // Track
    let track = world
        .spawn((
            UiWidgetPart::new("track"),
            Node {
                width: Val::Px(6.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                overflow: Overflow::clip(),
                align_self: AlignSelf::Center,
                justify_content: JustifyContent::End,
                ..default()
            },
            BackgroundColor(data.track_color),
        ))
        .id();

    // Fill (grows from bottom)
    let fill = world
        .spawn((
            UiWidgetPart::new("fill"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(frac * 100.0),
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
                bottom: Val::Percent(frac * 100.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(data.thumb_color),
        ))
        .id();

    world.entity_mut(fill).set_parent_in_place(track);
    world.entity_mut(track).set_parent_in_place(root);
    world.entity_mut(thumb).set_parent_in_place(root);
    root
}

// ── Scrollbar ──────────────────────────────────────────────────────────────

fn spawn_scrollbar(world: &mut World, r: &Ref) -> Entity {
    let data = ScrollbarData::default();
    let thumb_pct = data.viewport_fraction.clamp(0.05, 1.0) * 100.0;
    let pos_pct = data.position.clamp(0.0, 1.0) * (100.0 - thumb_pct);

    let (root_w, root_h, thumb_w, thumb_h, thumb_top, thumb_left) = match data.orientation {
        ScrollbarOrientation::Vertical => (
            pct_w(14.0, r),
            pct_h(200.0, r),
            Val::Percent(100.0),
            Val::Percent(thumb_pct),
            Val::Percent(pos_pct),
            Val::Auto,
        ),
        ScrollbarOrientation::Horizontal => (
            pct_w(200.0, r),
            pct_h(14.0, r),
            Val::Percent(thumb_pct),
            Val::Percent(100.0),
            Val::Auto,
            Val::Percent(pos_pct),
        ),
    };

    let root = world
        .spawn((
            Name::new("Scrollbar"),
            UiWidget {
                widget_type: UiWidgetType::Scrollbar,
                locked: false,
            },
            Node {
                width: root_w,
                height: root_h,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            UiFill::solid(data.track_color),
            UiBorderRadius::all(4.0),
            BackgroundColor(data.track_color),
            data.clone(),
        ))
        .id();

    let thumb = world
        .spawn((
            UiWidgetPart::new("thumb"),
            Node {
                width: thumb_w,
                height: thumb_h,
                top: thumb_top,
                left: thumb_left,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(data.thumb_color),
        ))
        .id();

    world.entity_mut(thumb).set_parent_in_place(root);
    root
}

// ── List ───────────────────────────────────────────────────────────────────

fn spawn_list(world: &mut World, r: &Ref) -> Entity {
    let data = ListData::default();

    let root = world
        .spawn((
            Name::new("List"),
            UiWidget {
                widget_type: UiWidgetType::List,
                locked: false,
            },
            Node {
                width: pct_w(200.0, r),
                height: pct_h(150.0, r),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            UiFill::solid(data.bg_color),
            UiBorderRadius::all(4.0),
            BackgroundColor(data.bg_color),
            data.clone(),
        ))
        .id();

    for item in &data.items {
        let bg = if item.selected { data.selected_bg_color } else { Color::NONE };
        let row = world
            .spawn((
                UiWidgetPart::new("item"),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(data.item_height),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(bg),
                Text::new(item.label.clone()),
                TextColor(data.text_color),
                TextFont {
                    font_size: 14.0,
                    ..default()
                },
            ))
            .id();
        world.entity_mut(row).set_parent_in_place(root);
    }

    root
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
    // Find or create canvas.
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
    let load_path = if let Some(project) = world.get_resource::<renzora::CurrentProject>() {
        project.make_asset_relative(asset_path)
    } else {
        asset_path.to_string_lossy().replace('\\', "/")
    };

    let image_handle: Handle<Image> = world.resource::<AssetServer>().load(load_path.clone());

    // Read actual image dimensions from disk; fall back to 128×128 if unreadable
    #[cfg(feature = "editor")]
    let (img_w, img_h) = ::image::image_dimensions(asset_path)
        .map(|(w, h)| (w as f32, h as f32))
        .unwrap_or((128.0, 128.0));
    #[cfg(not(feature = "editor"))]
    let (img_w, img_h) = (128.0_f32, 128.0_f32);

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
                width: pct_w(img_w, &r),
                height: pct_h(img_h, &r),
                ..default()
            },
            ImageNode::new(image_handle),
            UiImagePath { path: load_path },
            UiThemed,
        ))
        .id();

    world.entity_mut(entity).set_parent_in_place(canvas_entity);

    #[cfg(feature = "editor")]
    if let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() {
        sel.set(Some(entity));
    }
}
