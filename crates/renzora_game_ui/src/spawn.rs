#![allow(unused_variables, unused_assignments, dead_code)]

//! Widget spawn functions — each creates the correct entity hierarchy for a widget type.

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
/// What kind of layout context a widget is being placed into.
///
/// Determines whether the new widget gets free (Absolute) or flowed
/// (Relative) positioning — the same split Figma / Webflow / UMG draw
/// between "free placement on a frame" and "auto-layout containers."
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParentLayout {
    /// Parent is a `UiCanvas` root → child uses `position_type: Absolute`
    /// and gets a sensible canvas-relative position. Drag = freely move.
    Canvas,
    /// Parent is a `UiWidget` Container with `display: Flex` → child uses
    /// `position_type: Relative` so flex flow positions it. Drag = reorder.
    Container,
}

fn classify_parent(world: &World, parent: Option<Entity>) -> (Option<Entity>, ParentLayout) {
    match parent {
        Some(e) => {
            // Direct UiCanvas → free placement.
            if world.get::<UiCanvas>(e).is_some() {
                return (Some(e), ParentLayout::Canvas);
            }
            // UiWidget with display: Flex → auto-layout container.
            if world.get::<UiWidget>(e).is_some() {
                let is_flex = world
                    .get::<Node>(e)
                    .map(|n| matches!(n.display, Display::Flex))
                    .unwrap_or(false);
                if is_flex {
                    return (Some(e), ParentLayout::Container);
                }
            }
            // Unknown — treat as canvas root.
            (Some(e), ParentLayout::Canvas)
        }
        None => (None, ParentLayout::Canvas),
    }
}

/// Walk up the parent chain to find the enclosing `UiCanvas` so we can
/// look up its reference width/height for `pct_*` math.
fn find_ancestor_canvas(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if world.get::<UiCanvas>(entity).is_some() {
            return Some(entity);
        }
        match world.get::<ChildOf>(entity) {
            Some(co) => entity = co.parent(),
            None => return None,
        }
    }
}

pub fn spawn_widget(
    world: &mut World,
    widget_type: &UiWidgetType,
    parent: Option<Entity>,
) -> Entity {
    let (resolved_parent, layout) = classify_parent(world, parent);

    // Resolve the actual parent entity. If none provided, find any canvas;
    // if still none, spawn a default canvas.
    let parent_entity = resolved_parent
        .or_else(|| {
            let mut q = world.query_filtered::<Entity, With<UiCanvas>>();
            q.iter(world).next()
        })
        .unwrap_or_else(|| {
            world
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
                .id()
        });

    // Reference dimensions for pct_w/pct_h come from the enclosing canvas.
    // Container children still use canvas-reference percentages so the
    // initial sizes are sensible (a 100×24 widget at 1280×720 reference is
    // ~7.8% × 3.3%; flex flow will then constrain it inside the parent).
    let canvas_for_ref = find_ancestor_canvas(world, parent_entity).unwrap_or(parent_entity);
    let r = world
        .get::<UiCanvas>(canvas_for_ref)
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
        UiWidgetType::BarFill => spawn_bar_fill(world, &r),
        UiWidgetType::Slider => spawn_slider(world, &r),
        UiWidgetType::Checkbox => spawn_checkbox(world, &r),
        UiWidgetType::Toggle => spawn_toggle(world, &r),
        UiWidgetType::RadioButton => spawn_radio_button(world, &r),
        UiWidgetType::Dropdown => spawn_dropdown(world, &r),
        UiWidgetType::TextInput => spawn_text_input(world, &r),
        UiWidgetType::ScrollView => spawn_scroll_view(world, &r),
        UiWidgetType::Tooltip => spawn_tooltip(world, &r),
        UiWidgetType::Modal => spawn_modal(world, &r),
        UiWidgetType::DraggableWindow => spawn_draggable_window(world, &r),
        UiWidgetType::KeybindRow => spawn_keybind_row(world, &r),
        UiWidgetType::SettingsRow => spawn_settings_row(world, &r),
        UiWidgetType::Separator => spawn_separator(world, &r),
        UiWidgetType::NumberInput => spawn_number_input(world, &r),
        UiWidgetType::Scrollbar => spawn_scrollbar(world, &r),
        UiWidgetType::Circle => spawn_circle(world, &r),
        UiWidgetType::Arc => spawn_arc(world, &r),
        UiWidgetType::RadialProgress => spawn_radial_progress(world, &r),
        UiWidgetType::Line => spawn_line(world, &r),
        UiWidgetType::Triangle => spawn_triangle(world, &r),
        UiWidgetType::Polygon => spawn_polygon(world, &r),
        UiWidgetType::Rectangle => spawn_rectangle(world, &r),
        UiWidgetType::Wedge => spawn_wedge(world, &r),
    };

    // Apply the parent-aware layout. Widgets dropped on the canvas root
    // get free Absolute positioning; widgets dropped into a Container get
    // Relative + flex flow. Each individual `spawn_*` may have set its
    // own Node defaults, but the parent context is the authority on
    // *positioning* so we re-write those fields here.
    apply_parent_layout(world, entity, layout);

    // Default every widget to clip its children. Renzora's UI kit treats
    // hierarchies as nested frames — children that exceed their parent's
    // bounds get masked, recursively, at every level. Users can opt out
    // per-widget via the `UiClipContent` toggle in the inspector
    // (e.g. dropdowns or tooltips that need to extend past their parent).
    //
    // Border defaults: insert zero-width `UiStroke` and zero-radius
    // `UiBorderRadius` if missing so the inspector's Stroke + Border
    // Radius sections always render. Default values render as no-op
    // (invisible border, sharp corners) — users edit them to taste.
    if let Ok(mut em) = world.get_entity_mut(entity) {
        if em.get::<UiClipContent>().is_none() {
            em.insert(UiClipContent(true));
        }
        if em.get::<UiStroke>().is_none() {
            em.insert(UiStroke {
                color: Color::srgba(0.0, 0.0, 0.0, 0.0),
                width: 0.0,
                sides: UiSides::all(),
            });
        }
        if em.get::<UiBorderRadius>().is_none() {
            em.insert(UiBorderRadius::all(0.0));
        }
        // Underlying bevy_ui `BorderColor` so the live border-color edits
        // reach rendering. The widget-style system only writes to it if
        // it exists; without this insert the inspector's color picker
        // would be a no-op until the user manually adds the component.
        if em.get::<BorderColor>().is_none() {
            em.insert(BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.0)));
        }
        if let Some(mut node) = em.get_mut::<Node>() {
            // Only set if currently default; respects per-widget overrides
            // a `spawn_*` already wrote (e.g. ScrollView already does this).
            if matches!(node.overflow.x, OverflowAxis::Visible)
                && matches!(node.overflow.y, OverflowAxis::Visible)
            {
                node.overflow = Overflow::clip();
            }
        }
    }

    world.entity_mut(entity).insert(UiThemed);
    world.entity_mut(entity).set_parent_in_place(parent_entity);

    #[cfg(feature = "editor")]
    {
        if let Some(requests) = world.get_resource::<renzora_editor::HierarchyExpandRequests>() {
            requests.push(parent_entity);
        }
        if let Some(sel) = world.get_resource::<renzora_editor::EditorSelection>() {
            sel.set(Some(entity));
        }
    }

    entity
}

/// Set `position_type` and `left`/`top` on an entity to match its parent
/// context. Called both by `spawn_widget` after a fresh spawn and by the
/// re-parent observer when a widget moves between parents in the
/// hierarchy.
fn apply_parent_layout(world: &mut World, entity: Entity, layout: ParentLayout) {
    let Ok(mut em) = world.get_entity_mut(entity) else {
        return;
    };
    let Some(mut node) = em.get_mut::<Node>() else {
        return;
    };
    match layout {
        ParentLayout::Canvas => {
            node.position_type = PositionType::Absolute;
            // Free-placement default: a small offset from origin so the
            // widget appears on screen rather than at (0,0). The user
            // drags afterward.
            if matches!(node.left, Val::Auto) {
                node.left = Val::Percent(10.0);
            }
            if matches!(node.top, Val::Auto) {
                node.top = Val::Percent(10.0);
            }
        }
        ParentLayout::Container => {
            node.position_type = PositionType::Relative;
            // Auto-layout flow doesn't use offsets — clear any leftover
            // canvas-positioning values so the parent's flex algorithm
            // takes over.
            node.left = Val::Auto;
            node.right = Val::Auto;
            node.top = Val::Auto;
            node.bottom = Val::Auto;
        }
    }
}

/// Re-apply [`apply_parent_layout`] to an entity based on its current
/// parent in the world. Public so the editor's reparent observer can
/// call it when a widget is dragged to a new parent in the hierarchy.
/// Spawn an HTML-template instance as a child of `parent` (or the first existing
/// / a freshly-created `UiCanvas`). Returns the instance entity.
///
/// The instance is a **transparent 100% × 100% layout host** (not a `UiWidget`),
/// so it provides the `100%` sizing reference markup roots need but doesn't catch
/// canvas clicks. `renzora_hui` tags the bevy_hui-built markup nodes themselves
/// as `UiWidget`s — that's what the canvas selects, drags, and hit-tests, so
/// clicks land on the *visible* element and transparent gaps fall through.
/// `renzora_hui`'s observer builds the actual markup under a *child* `HtmlNode`,
/// so bevy_hui's per-build/hot-reload work never disturbs the instance itself.
pub fn spawn_html_template_at(
    world: &mut World,
    asset_path: &std::path::Path,
    parent: Option<Entity>,
) -> Entity {
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

    let load_path = if let Some(project) = world.get_resource::<renzora::CurrentProject>() {
        project.make_asset_relative(asset_path)
    } else {
        asset_path.to_string_lossy().replace('\\', "/")
    };

    let name = asset_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "HTML Template".to_string());
    let instance = world
        .spawn((
            Name::new(name),
            UiWidget::default(),
            HtmlTemplatePath(load_path),
            // Dedicated UI entity dropped onto the canvas — build the markup
            // tree directly onto it, not as a child.
            crate::HuiBuildOnSelf,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                ..default()
            },
        ))
        .id();
    // Insert at index 0 so newly-dropped templates land at the top of the
    // canvas's child list in the hierarchy panel (Bevy renders later siblings
    // on top, so this places the template *behind* existing siblings — the
    // hierarchy ordering is what the user is asking for here).
    world.entity_mut(canvas_entity).insert_children(0, &[instance]);
    instance
}

pub fn reapply_layout_from_parent(world: &mut World, entity: Entity) {
    let parent = world.get::<ChildOf>(entity).map(|co| co.parent());
    let (_, layout) = classify_parent(world, parent);
    apply_parent_layout(world, entity, layout);
}

/// True when an entity's parent is a flex-layout *container widget*
/// (Container, Panel, etc. with `display: Flex`) — **not** a UiCanvas.
///
/// `UiCanvas` itself has `display: Flex` by default (Bevy's default), so a
/// naive "parent has Flex display" check would mis-classify every
/// canvas-root widget as a flex child. The canvas is conceptually a
/// free-placement surface; only nested Container/Panel widgets count as
/// auto-layout parents.
///
/// Used by the canvas editor's drag-handlers to skip writes that would
/// convert a flex-flow child to absolute positioning. Without this guard,
/// dragging or resizing a widget that's nested inside a Container
/// silently force-converts it back to `position_type: Absolute` with
/// canvas-relative coordinates — undoing the auto-layout entirely.
pub fn is_flex_child(world: &World, entity: Entity) -> bool {
    let Some(parent) = world.get::<ChildOf>(entity).map(|c| c.parent()) else {
        return false;
    };
    // Direct child of canvas → free placement, not flex flow.
    if world.get::<UiCanvas>(parent).is_some() {
        return false;
    }
    // Only widget parents with explicit Flex display count as auto-layout.
    if world.get::<UiWidget>(parent).is_none() {
        return false;
    }
    world
        .get::<Node>(parent)
        .map(|n| matches!(n.display, Display::Flex))
        .unwrap_or(false)
}

/// Choose the parent to use for a "Add Widget" action in the editor.
///
/// Order of preference:
/// 1. Current `EditorSelection` if it's a UiCanvas or a UiWidget Container —
///    the user expects new widgets to land "inside what I'm working in."
/// 2. The provided `active_canvas` fallback — the canvas tab passes its
///    active canvas here.
/// 3. `None` — `spawn_widget` will fall back to "any canvas, or spawn one."
#[cfg(feature = "editor")]
pub fn pick_spawn_parent(world: &World, active_canvas: Option<Entity>) -> Option<Entity> {
    if let Some(sel_res) = world.get_resource::<renzora_editor::EditorSelection>() {
        if let Some(sel) = sel_res.get() {
            // Container/Panel: a layout-mode parent for nested widgets.
            let is_container = world
                .get::<UiWidget>(sel)
                .map(|w| matches!(w.widget_type, UiWidgetType::Container | UiWidgetType::Panel))
                .unwrap_or(false);
            if is_container {
                return Some(sel);
            }
            // Canvas: a free-placement parent.
            if world.get::<UiCanvas>(sel).is_some() {
                return Some(sel);
            }
        }
    }
    active_canvas
}

fn spawn_container(world: &mut World, r: &Ref) -> Entity {
    // Containers default to **auto-layout frames**: Flex display, clipped
    // overflow, dim translucent fill so the frame is visible without
    // setup. Children dropped into a Container flow along the flex axis
    // (Row by default) and are clipped to the container's bounds —
    // matching how Figma's "auto-layout" frames or Webflow's flex
    // containers behave.
    //
    // `overflow: clip` is set directly on `Node` (not via `UiClipContent`
    // alone) so masking is in effect immediately on spawn. Otherwise the
    // first frame between spawn and the `widget_style` system running
    // would render children unmasked.
    let bg = Color::srgba(0.15, 0.15, 0.18, 0.4);
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
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                overflow: Overflow::clip(),
                ..default()
            },
            UiFill::solid(bg),
            UiStroke::new(Color::srgba(0.3, 0.3, 0.35, 0.5), 1.0),
            UiBorderRadius::all(2.0),
            UiClipContent(true),
            BackgroundColor(bg),
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

    world
        .entity_mut(checkmark)
        .set_parent_in_place(checkbox_box);
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

// ── Bar Fill ───────────────────────────────────────────────────────────────
//
// Single-entity primitive — drag inside a Container to use it as the fill of a
// bar. The Container's size is the "track"; this widget's `Node` width/height
// is rewritten by `apply_bar_fill` from `UiBarFill::value`.
//
// The defaults give 50% fill, full height, green-ish color so it's obviously
// "alive" in the editor preview without needing the inspector to tweak.

fn spawn_bar_fill(world: &mut World, _r: &Ref) -> Entity {
    // Bar Fill defaults to **fixed pixel** sizing rather than percent of
    // parent. Reasons:
    //
    // - Bevy/Taffy's percent-on-cross-axis interaction with flex parents
    //   that themselves use percent sizing doesn't always resolve
    //   against the immediate parent — the bar would inherit the canvas's
    //   full height instead of the container's.
    // - Pixel sizes are predictable and self-contained: a fresh bar is
    //   100×20 px regardless of where it lands. Users can resize the bar
    //   itself or switch to percent mode in inspector.
    //
    // `apply_bar_fill` rewrites width per `value * max_px` each tick (or
    // per percent if `max_px == 0`); height is left alone unless direction
    // is vertical.
    let data = UiBarFill::default();
    let fill_color = Color::srgba(0.3, 0.7, 0.3, 1.0);
    let initial_width = data.fraction() * data.max_px.max(1.0);
    world
        .spawn((
            Name::new("Bar Fill"),
            UiWidget {
                widget_type: UiWidgetType::BarFill,
                locked: false,
            },
            Node {
                width: Val::Px(initial_width),
                height: Val::Px(20.0),
                // `flex_shrink: 0` — Bevy's default would let the parent
                // squash this bar down if it doesn't fit, making the
                // inspector's height value lie about what's rendered.
                // The bar always shows at the authored size; if it
                // exceeds the parent it gets clipped (which is what
                // `UiClipContent` is for).
                flex_shrink: 0.0,
                ..default()
            },
            data,
            UiFill::solid(fill_color),
            UiBorderRadius::all(2.0),
            BackgroundColor(fill_color),
        ))
        .id()
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

// ── Image-at-position (drag-drop from asset browser) ──────────────────────
//
// Called by canvas.rs when an image file is drag-dropped onto the UI canvas.
// Converts the file path to an asset-relative path and spawns an Image
// widget at the drop coordinates, snapped to grid if enabled.

pub fn spawn_image_at(
    world: &mut World,
    asset_path: &std::path::Path,
    x: f32,
    y: f32,
    snap: bool,
    grid: f32,
    parent: Option<Entity>,
) {
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

    let load_path = if let Some(project) = world.get_resource::<renzora::CurrentProject>() {
        project.make_asset_relative(asset_path)
    } else {
        asset_path.to_string_lossy().replace('\\', "/")
    };

    let image_handle: Handle<Image> = world.resource::<AssetServer>().load(load_path.clone());

    #[cfg(feature = "editor")]
    let (img_w, img_h) = ::image::image_dimensions(asset_path)
        .map(|(w, h)| (w as f32, h as f32))
        .unwrap_or((128.0, 128.0));
    #[cfg(not(feature = "editor"))]
    let (img_w, img_h) = (128.0_f32, 128.0_f32);

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
    if let Some(sel) = world.get_resource::<renzora_editor::EditorSelection>() {
        sel.set(Some(entity));
    }
}
