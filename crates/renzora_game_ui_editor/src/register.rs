//! Editor-side registration + systems relocated from `renzora_game_ui`'s old
//! `#[cfg(feature = "editor")]` block.
//!
//! `register_game_ui_editor(app)` reproduces — verbatim — the per-component
//! inspector entries, hierarchy component icons, entity presets, the UI render
//! target setup/sync, and the editor-only sync/debug systems that used to live
//! inside `GameUiPlugin::build` under the `editor` feature. It runs from
//! `GameUiEditorPlugin::build`.
//!
//! Path note: `components::` → `renzora_game_ui::components::`, the moved canvas
//! modules are now local (`crate::canvas` / `crate::canvas_render` /
//! `crate::ui_inspector`), and `UiWidgetType::icon()` became the free fn
//! [`widget_icon`] here (egui-phosphor is an editor-only dep).

use bevy::prelude::*;

use renzora::AppEditorExt;
use renzora_game_ui::components::{self};
use renzora_game_ui::{UiCanvas, UiWidget, UiWidgetType};

use crate::{canvas, canvas_render, ui_inspector as inspector};

/// Phosphor icon glyph for a widget type. Replaces the old
/// `UiWidgetType::icon()` inherent method (which lived in `renzora_game_ui`
/// behind the deleted `editor` feature). Egui-phosphor is editor-only, so the
/// mapping lives here in the editor crate.
pub fn widget_icon(t: &UiWidgetType) -> &'static str {
    use egui_phosphor::regular::*;
    match t {
        UiWidgetType::Container => SQUARES_FOUR,
        UiWidgetType::Panel => RECTANGLE,
        UiWidgetType::ScrollView => SCROLL,
        UiWidgetType::Text => TEXT_AA,
        UiWidgetType::Image => IMAGE,
        UiWidgetType::Button => CURSOR_CLICK,
        UiWidgetType::Slider => SLIDERS_HORIZONTAL,
        UiWidgetType::Checkbox => CHECK_SQUARE,
        UiWidgetType::Toggle => TOGGLE_RIGHT,
        UiWidgetType::RadioButton => RADIO_BUTTON,
        UiWidgetType::Dropdown => CARET_CIRCLE_DOWN,
        UiWidgetType::TextInput => TEXT_T,
        UiWidgetType::BarFill => BATTERY_MEDIUM,
        UiWidgetType::Tooltip => CHAT_CIRCLE_TEXT,
        UiWidgetType::Modal => BROWSERS,
        UiWidgetType::DraggableWindow => APP_WINDOW,
        UiWidgetType::KeybindRow => KEYBOARD,
        UiWidgetType::SettingsRow => GEAR,
        UiWidgetType::Separator => MINUS,
        UiWidgetType::NumberInput => CALCULATOR,
        UiWidgetType::Scrollbar => ARROWS_DOWN_UP,
        UiWidgetType::Circle => CIRCLE,
        UiWidgetType::Arc => CIRCLE_DASHED,
        UiWidgetType::RadialProgress => CIRCLE_NOTCH,
        UiWidgetType::Line => LINE_SEGMENT,
        UiWidgetType::Triangle => TRIANGLE,
        UiWidgetType::Polygon => HEXAGON,
        UiWidgetType::Rectangle => RECTANGLE,
        UiWidgetType::Wedge => CHART_PIE_SLICE,
    }
}

/// Register everything the editor build used to wire up inside
/// `GameUiPlugin::build`'s `#[cfg(feature = "editor")]` block.
pub fn register_game_ui_editor(app: &mut App) {
    info!("[editor] GameUiPlugin (editor panels)");

    register_ui_presets(app);
    app.init_resource::<canvas::UiCanvasPreviewEnabled>();
    app.init_resource::<LastSelectionForViewSwitch>();
    // Per-component inspector entries (Phase A of the UI inspector
    // decomposition). Each constituent component gets its own
    // collapsible in the main inspector. Fill/stroke/etc. are still
    // grouped under a "UI Style" lump until Phase B splits them.
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_canvas",
        display_name: "UI Canvas",
        icon: egui_phosphor::regular::FRAME_CORNERS,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiCanvas>(entity).is_some(),
        // Addable to any entity: insert the canvas marker plus a
        // full-size root `Node` so it renders / camera-targets like a
        // canvas spawned through the normal path.
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert((
                components::UiCanvas::default(),
                bevy::ui::Node {
                    width: bevy::ui::Val::Percent(100.0),
                    height: bevy::ui::Val::Percent(100.0),
                    position_type: bevy::ui::PositionType::Absolute,
                    ..Default::default()
                },
            ));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiCanvas>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            renzora::int_field!("Sort Order", components::UiCanvas, sort_order, i32, 1.0, -100.0, 100.0),
            renzora::FieldDef {
                name: "Visibility",
                field_type: renzora::FieldType::Enum {
                    options: &["always", "play_only", "editor_only"],
                },
                get_fn: |w, e| {
                    w.get::<components::UiCanvas>(e)
                        .map(|c| renzora::FieldValue::Enum(c.visibility_mode.clone()))
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(s), Some(mut c)) =
                        (v, w.get_mut::<components::UiCanvas>(e))
                    {
                        c.visibility_mode = s;
                    }
                },
            },
            renzora::float_field!("Ref Width", components::UiCanvas, reference_width, 1.0, 1.0, 7680.0),
            renzora::float_field!("Ref Height", components::UiCanvas, reference_height, 1.0, 1.0, 4320.0),
        ],
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_widget",
        display_name: "UI Widget",
        icon: egui_phosphor::regular::SQUARES_FOUR,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiWidget>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::widget_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_layout",
        display_name: "Layout",
        icon: egui_phosphor::regular::SQUARE_HALF,
        category: "ui",
        has_fn: |world, entity| {
            // Restrict to UI entities so Bevy's Node component on
            // non-UI usages isn't picked up.
            world.get::<bevy::ui::Node>(entity).is_some()
                && (world.get::<components::UiCanvas>(entity).is_some()
                    || world.get::<components::UiWidget>(entity).is_some())
        },
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::layout_fields(),
    });
    // Per-style components — each is individually addable via the
    // Add Component overlay and removable via the trash icon. A text
    // label that doesn't want a border can drop UiStroke; a button
    // that wants a shadow can add UiBoxShadow. (Phase B.)
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_fill",
        display_name: "UI Fill",
        icon: egui_phosphor::regular::DROP_HALF,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiFill>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiFill::Solid(Color::srgba(0.2, 0.2, 0.2, 1.0)));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiFill>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_stroke",
        display_name: "UI Border",
        icon: egui_phosphor::regular::BOUNDING_BOX,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiStroke>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(components::UiStroke::new(
                Color::srgba(0.4, 0.4, 0.4, 1.0),
                1.0,
            ));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiStroke>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_border_radius",
        display_name: "UI Border Radius",
        icon: egui_phosphor::regular::FRAME_CORNERS,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiBorderRadius>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiBorderRadius::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::UiBorderRadius>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::border_radius_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_text",
        display_name: "UI Text",
        icon: egui_phosphor::regular::TEXT_AA,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiTextStyle>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiTextStyle::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiTextStyle>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::text_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_padding",
        display_name: "UI Padding",
        icon: egui_phosphor::regular::COLUMNS,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiPadding>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiPadding::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiPadding>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::padding_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_opacity",
        display_name: "UI Opacity",
        icon: egui_phosphor::regular::CIRCLE_HALF,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiOpacity>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(components::UiOpacity(1.0));
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiOpacity>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::opacity_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_shadow",
        display_name: "UI Shadow",
        icon: egui_phosphor::regular::SUN_DIM,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiBoxShadow>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiBoxShadow::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiBoxShadow>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::shadow_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_clip",
        display_name: "UI Clip Content",
        icon: egui_phosphor::regular::CROP,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiClipContent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiClipContent(true));
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::UiClipContent>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::clip_content_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_cursor",
        display_name: "UI Cursor",
        icon: egui_phosphor::regular::CURSOR,
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiCursor>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiCursor::Pointer);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiCursor>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::cursor_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_interaction",
        display_name: "UI Interaction States",
        icon: egui_phosphor::regular::CURSOR_CLICK,
        category: "ui",
        has_fn: |world, entity| {
            world
                .get::<components::UiInteractionStyle>(entity)
                .is_some()
        },
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiInteractionStyle::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::UiInteractionStyle>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    });
    // Per-widget-type data components — Phase C. Each is its own
    // entry; users can swap a slider's data, drop a tooltip's data,
    // etc. via the Add Component overlay.
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_slider_data",
        display_name: "Slider",
        icon: egui_phosphor::regular::SLIDERS_HORIZONTAL,
        category: "ui",
        has_fn: |world, entity| world.get::<components::SliderData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::SliderData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::SliderData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::slider_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_checkbox_data",
        display_name: "Checkbox",
        icon: egui_phosphor::regular::CHECK_SQUARE,
        category: "ui",
        has_fn: |world, entity| world.get::<components::CheckboxData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::CheckboxData::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::CheckboxData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::checkbox_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_toggle_data",
        display_name: "Toggle",
        icon: egui_phosphor::regular::TOGGLE_LEFT,
        category: "ui",
        has_fn: |world, entity| world.get::<components::ToggleData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::ToggleData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::ToggleData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::toggle_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_radio_data",
        display_name: "Radio Button",
        icon: egui_phosphor::regular::RADIO_BUTTON,
        category: "ui",
        has_fn: |world, entity| world.get::<components::RadioButtonData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::RadioButtonData::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::RadioButtonData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::radio_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_dropdown_data",
        display_name: "Dropdown",
        icon: egui_phosphor::regular::CARET_CIRCLE_DOWN,
        category: "ui",
        has_fn: |world, entity| world.get::<components::DropdownData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::DropdownData::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::DropdownData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_text_input_data",
        display_name: "Text Input",
        icon: egui_phosphor::regular::TEXTBOX,
        category: "ui",
        has_fn: |world, entity| world.get::<components::TextInputData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::TextInputData::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::TextInputData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::text_input_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_scroll_view_data",
        display_name: "Scroll View",
        icon: egui_phosphor::regular::SCROLL,
        category: "ui",
        has_fn: |world, entity| world.get::<components::ScrollViewData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::ScrollViewData::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::ScrollViewData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::scroll_view_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_tooltip_data",
        display_name: "Tooltip",
        icon: egui_phosphor::regular::CHAT_CIRCLE,
        category: "ui",
        has_fn: |world, entity| world.get::<components::TooltipData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::TooltipData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::TooltipData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::tooltip_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_modal_data",
        display_name: "Modal",
        icon: egui_phosphor::regular::BROWSER,
        category: "ui",
        has_fn: |world, entity| world.get::<components::ModalData>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::ModalData::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::ModalData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::modal_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_draggable_window_data",
        display_name: "Draggable Window",
        icon: egui_phosphor::regular::APP_WINDOW,
        category: "ui",
        has_fn: |world, entity| {
            world
                .get::<components::DraggableWindowData>(entity)
                .is_some()
        },
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::DraggableWindowData::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::DraggableWindowData>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::draggable_window_fields(),
    });

    // Register hierarchy icons for UI entities
    app.register_component_icon(renzora::ComponentIconEntry {
        type_id: std::any::TypeId::of::<components::UiCanvas>(),
        name: "UI Canvas",
        icon: egui_phosphor::regular::FRAME_CORNERS,
        color: [130, 200, 255],
        priority: 70,
        dynamic_icon_fn: None,
    });
    app.register_component_icon(renzora::ComponentIconEntry {
        type_id: std::any::TypeId::of::<components::UiWidget>(),
        name: "UI Widget",
        icon: egui_phosphor::regular::SQUARES_FOUR,
        color: [130, 200, 255],
        priority: 60,
        dynamic_icon_fn: Some(|world, entity| {
            world
                .get::<components::UiWidget>(entity)
                .map(|w| (widget_icon(&w.widget_type), [130u8, 200, 255]))
        }),
    });

    // Editor's dedicated bevy_ui render target — what the UI
    // viewport mode displays for the *real* bevy_ui render
    // (not an egui simulation). The 3D backdrop behind it is
    // borrowed from `ViewportRenderTarget` (editor camera), so
    // we don't spawn or maintain a second 3D preview camera.
    app.add_systems(Startup, canvas_render::setup_ui_canvas_render);
    app.add_systems(
        Update,
        canvas_render::sync_canvases_to_editor_camera.after(sync_ui_canvas_target_camera),
    );
    app.add_systems(Update, sync_ui_scale_to_canvas_reference);
    app.add_systems(
        Update,
        (
            ensure_ui_visibility_components,
            sync_ui_canvas_target_camera,
            sync_canvas_sort_order_from_hierarchy,
            debug_ui_tree,
        )
            .chain(),
    );
    app.add_systems(Update, auto_switch_view_on_selection);
}

// ── UiScale ↔ canvas reference sync ─────────────────────────────────────
//
// The editor renders bevy_ui to a fixed-size texture (`UI_RENDER_WIDTH ×
// UI_RENDER_HEIGHT`), then displays it in the canvas tab at the active
// canvas's reference resolution. If those two don't match — say, a
// 1920×1080 canvas reference into a 1280×720 render target — every
// `Val::Px(400)` would render at 400 texture-pixels (= 600 design-pixels
// on display), and selection handles authored in design space would sit
// at the wrong place over the rendered widget.
//
// Fix: scale `UiScale` so `design_pixels × UiScale = render_pixels`
// matches the texture's resolution. Then bevy_ui rasterises at the
// correct fraction of the render target, the texture stretches cleanly
// to the display, and design-space coordinates line up everywhere.
//
// Single global UiScale means we use the *first* canvas's reference; if
// you have multiple canvases at different references, only the first
// will match. That's fine for the common single-canvas authoring case.
fn sync_ui_scale_to_canvas_reference(
    canvases: Query<&UiCanvas>,
    mut ui_scale: ResMut<bevy::ui::UiScale>,
) {
    let Some(canvas) = canvases.iter().next() else {
        return;
    };
    let ref_w = canvas.reference_width.max(1.0);
    let target = canvas_render::UI_RENDER_WIDTH as f32 / ref_w;
    if (ui_scale.0 - target).abs() > 0.001 {
        ui_scale.0 = target;
    }
}

// ── Editor-only systems ─────────────────────────────────────────────────────

/// Tracks the last selection we processed for view-auto-switching, so the
/// switch fires on selection *change* only — not every frame, which would
/// fight a user who explicitly picked a different viewport view while a
/// UI entity was selected.
#[derive(Resource, Default)]
struct LastSelectionForViewSwitch(Option<Entity>);

/// When the selection changes to a UI entity (`UiCanvas`/`UiWidget` or a
/// descendant of one), flip the viewport into UI view. When it changes to
/// a non-UI entity *and* we're currently in UI view, flip back to 3D.
/// Other view transitions (3D ↔ 2D) are left to the user.
fn auto_switch_view_on_selection(world: &mut World) {
    use renzora::core::viewport_types::{ViewportSettings, ViewportView};

    let current_sel = world
        .get_resource::<renzora::EditorSelection>()
        .and_then(|s| s.get());
    let last_sel = world
        .get_resource::<LastSelectionForViewSwitch>()
        .map(|l| l.0)
        .unwrap_or(None);
    if current_sel == last_sel {
        return;
    }
    if let Some(mut last) = world.get_resource_mut::<LastSelectionForViewSwitch>() {
        last.0 = current_sel;
    }
    let Some(entity) = current_sel else { return };

    // Hybrid entity (a 3D mesh that *also* carries a `UiCanvas` to render UI
    // onto itself): don't auto-switch either way. Yanking the viewport to UI
    // every time you click a cube-with-a-canvas would make it impossible to
    // manipulate its transform in 3D. The user toggles the view manually when
    // they want to edit that entity's UI.
    if world.get::<bevy::prelude::Mesh3d>(entity).is_some() {
        return;
    }

    let mut check = entity;
    let is_ui = loop {
        if world.get::<UiCanvas>(check).is_some() || world.get::<UiWidget>(check).is_some() {
            break true;
        }
        match world.get::<ChildOf>(check) {
            Some(c) => check = c.parent(),
            None => break false,
        }
    };

    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    let target = match (is_ui, view) {
        (true, ViewportView::Ui) => return,
        (true, _) => ViewportView::Ui,
        (false, ViewportView::Ui) => ViewportView::Three,
        (false, _) => return,
    };
    if let Some(mut settings) = world.get_resource_mut::<ViewportSettings>() {
        settings.viewport_view = target;
    }
}

/// In the editor, sync `UiCanvas::sort_order` from `HierarchyOrder` so that
/// reordering canvases in the hierarchy panel updates their z-index.
/// Top of hierarchy (lowest HierarchyOrder) gets the highest sort_order → renders on top.
fn sync_canvas_sort_order_from_hierarchy(
    mut canvases: Query<(&mut UiCanvas, &renzora::HierarchyOrder), Without<ChildOf>>,
) {
    let max_order = canvases.iter().map(|(_, h)| h.0).max().unwrap_or(0) as i32;
    for (mut canvas, order) in &mut canvases {
        let new_order = max_order - order.0 as i32;
        if canvas.sort_order != new_order {
            canvas.sort_order = new_order;
        }
    }
}

fn ensure_ui_visibility_components(
    mut commands: Commands,
    canvases_no_iv: Query<Entity, (With<UiCanvas>, Without<InheritedVisibility>)>,
    widgets_no_iv: Query<Entity, (With<UiWidget>, Without<InheritedVisibility>)>,
) {
    for entity in canvases_no_iv.iter().chain(widgets_no_iv.iter()) {
        commands
            .entity(entity)
            .try_insert((InheritedVisibility::default(), ViewVisibility::default()));
    }
}

/// Route UI canvases to the right camera in the editor.
///
/// The editor has both an editor camera (rendering to the viewport image)
/// and a play-mode game camera. Without an explicit target, Bevy UI
/// picks "the first Camera it finds," which is non-deterministic. This
/// system inserts `UiTargetCamera` pointing at the active game camera
/// while in play mode, and removes it otherwise so edit-mode renders go
/// through whatever default Bevy picks (typically the editor camera).
///
/// **Does not touch `Visibility`** — that's the user's / the script's
/// concern. Earlier versions of this system also force-hid every canvas
/// outside of play mode, which polluted saved scenes and broke shipped
/// runtime visibility.
fn sync_ui_canvas_target_camera(
    mut commands: Commands,
    play_mode: Res<renzora::PlayModeState>,
    canvases: Query<(Entity, Option<&bevy::ui::UiTargetCamera>), With<UiCanvas>>,
) {
    let in_play = play_mode.is_in_play_mode();
    let game_camera = play_mode.active_game_camera;

    for (entity, existing_target_cam) in &canvases {
        if in_play {
            if let Some(cam_entity) = game_camera {
                let needs_insert = match existing_target_cam {
                    Some(tc) => tc.entity() != cam_entity,
                    None => true,
                };
                if needs_insert {
                    commands
                        .entity(entity)
                        .insert(bevy::ui::UiTargetCamera(cam_entity));
                }
            }
        } else if existing_target_cam.is_some() {
            commands.entity(entity).remove::<bevy::ui::UiTargetCamera>();
        }
    }
}

fn debug_ui_tree(
    play_mode: Res<renzora::PlayModeState>,
    canvases: Query<
        (
            Entity,
            &Name,
            &Node,
            &Visibility,
            Option<&InheritedVisibility>,
            Option<&ViewVisibility>,
        ),
        With<UiCanvas>,
    >,
    widgets: Query<
        (
            Entity,
            &Name,
            &Node,
            &Visibility,
            Option<&InheritedVisibility>,
            Option<&ViewVisibility>,
            Option<&ChildOf>,
        ),
        With<UiWidget>,
    >,
    cameras: Query<(Entity, &Camera, Option<&Name>)>,
) {
    static LAST_PLAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let in_play = play_mode.is_in_play_mode();
    let was_playing = LAST_PLAY.swap(in_play, std::sync::atomic::Ordering::Relaxed);
    if in_play == was_playing {
        return;
    }

    info!("[ui_editor] === UI TREE DUMP (play_mode={}) ===", in_play);

    for (entity, name, node, vis, inh_vis, view_vis) in &canvases {
        info!(
            "[ui_editor]   CANVAS {:?} name={} vis={:?} inherited={:?} view={:?} w={:?} h={:?} pos={:?}",
            entity, name, vis, inh_vis, view_vis, node.width, node.height, node.position_type,
        );
    }

    for (entity, name, node, vis, inh_vis, view_vis, parent) in &widgets {
        info!(
            "[ui_editor]   WIDGET {:?} name={} parent={:?} vis={:?} inherited={:?} view={:?} w={:?} h={:?}",
            entity,
            name,
            parent.map(|p| p.parent()),
            vis,
            inh_vis,
            view_vis,
            node.width,
            node.height,
        );
    }

    for (entity, camera, name) in &cameras {
        info!(
            "[ui_editor]   CAMERA {:?} name={:?} active={} order={}",
            entity,
            name.map(|n| n.as_str()),
            camera.is_active,
            camera.order,
        );
    }

    info!("[ui_editor] === END UI TREE DUMP ===");
}

/// Register UI Canvas + all UI widget types as entity presets in the hierarchy
/// "Add Entity" overlay. Each widget preset spawns via `spawn::spawn_widget`,
/// which finds (or creates) a canvas and parents the new widget to it.
fn register_ui_presets(app: &mut App) {
    use renzora::{AppEditorExt, EntityPreset, SceneStarter};

    fn spawn_ui_canvas(world: &mut World) -> Entity {
        world
            .spawn((
                Name::new("UI Canvas"),
                components::UiCanvas::default(),
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ))
            .id()
    }

    // UI Canvas — always spawned at root.
    app.register_entity_preset(EntityPreset {
        id: "ui_canvas",
        display_name: "UI Canvas",
        icon: egui_phosphor::regular::FRAME_CORNERS,
        category: "ui",
        spawn_fn: spawn_ui_canvas,
    });

    // "New UI" scene starter — spawns a canvas and selects it so the next
    // click already targets the right parent for new widgets.
    app.register_scene_starter(SceneStarter {
        id: "ui",
        title: "New UI",
        description: "An empty canvas, ready for widgets",
        icon: egui_phosphor::regular::FRAME_CORNERS,
        spawn_fn: |world: &mut World| {
            let canvas = spawn_ui_canvas(world);
            if let Some(selection) = world.get_resource::<renzora::EditorSelection>() {
                selection.set(Some(canvas));
            }
        },
    });

    macro_rules! widget_preset {
        ($variant:ident, $id:literal, $label:literal) => {{
            fn spawn_fn(world: &mut World) -> Entity {
                let e =
                    renzora_game_ui::spawn::spawn_widget(world, &UiWidgetType::$variant, None);
                // Editor follow-up that used to live inside `spawn_widget`'s
                // `#[cfg(feature = "editor")]` block: expand the parent in the
                // hierarchy panel + select the freshly-spawned widget.
                if let Some(parent) = world.get::<ChildOf>(e).map(|c| c.parent()) {
                    if let Some(requests) =
                        world.get_resource::<renzora::HierarchyExpandRequests>()
                    {
                        requests.push(parent);
                    }
                }
                if let Some(sel) = world.get_resource::<renzora::EditorSelection>() {
                    sel.set(Some(e));
                }
                e
            }
            app.register_entity_preset(EntityPreset {
                id: $id,
                display_name: $label,
                icon: widget_icon(&UiWidgetType::$variant),
                category: "ui",
                spawn_fn,
            });
        }};
    }

    widget_preset!(Container, "ui_container", "Container");
    widget_preset!(Panel, "ui_panel", "Panel");
    widget_preset!(ScrollView, "ui_scroll_view", "Scroll View");
    widget_preset!(Text, "ui_text", "Text");
    widget_preset!(Image, "ui_image", "Image");
    widget_preset!(Button, "ui_button", "Button");
    widget_preset!(Slider, "ui_slider", "Slider");
    widget_preset!(Checkbox, "ui_checkbox", "Checkbox");
    widget_preset!(Toggle, "ui_toggle", "Toggle");
    widget_preset!(RadioButton, "ui_radio_button", "Radio Button");
    widget_preset!(Dropdown, "ui_dropdown", "Dropdown");
    widget_preset!(TextInput, "ui_text_input", "Text Input");
    widget_preset!(BarFill, "ui_bar_fill", "Bar Fill");
    widget_preset!(Tooltip, "ui_tooltip", "Tooltip");
    widget_preset!(Modal, "ui_modal", "Modal");
    widget_preset!(DraggableWindow, "ui_draggable_window", "Draggable Window");
    widget_preset!(KeybindRow, "ui_keybind_row", "Keybind Row");
    widget_preset!(SettingsRow, "ui_settings_row", "Settings Row");
    widget_preset!(Separator, "ui_separator", "Separator");
    widget_preset!(NumberInput, "ui_number_input", "Number Input");
    widget_preset!(Scrollbar, "ui_scrollbar", "Scrollbar");
    widget_preset!(Circle, "ui_circle", "Circle");
    widget_preset!(Arc, "ui_arc", "Arc");
    widget_preset!(RadialProgress, "ui_radial_progress", "Radial Progress");
    widget_preset!(Line, "ui_line", "Line");
    widget_preset!(Triangle, "ui_triangle", "Triangle");
    widget_preset!(Polygon, "ui_polygon", "Polygon");
    widget_preset!(Rectangle, "ui_rectangle", "Rectangle");
    widget_preset!(Wedge, "ui_wedge", "Wedge");
}
