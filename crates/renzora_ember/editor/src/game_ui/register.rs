//! Editor-side registration + systems relocated from `renzora_game_ui`'s old
//! `#[cfg(feature = "editor")]` block.
//!
//! `register_game_ui_editor(app)` reproduces — verbatim — the per-component
//! inspector entries, hierarchy component icons, entity presets, the UI render
//! target setup/sync, and the editor-only sync/debug systems that used to live
//! inside `GameUiPlugin::build` under the `editor` feature. It runs from
//! `GameUiEditorPlugin::build`.
//!
//! Path note: `components::` → `renzora_ember::game_ui::components::`, the moved canvas
//! modules are now local (`crate::game_ui::canvas` / `crate::game_ui::canvas_render` /
//! `crate::game_ui::ui_inspector`), and `UiWidgetType::icon()` became the free fn
//! [`widget_icon`] here (icons are name-based, resolved via the phosphor map).

use bevy::prelude::*;

use renzora::AppEditorExt;
use renzora_ember::game_ui::components::{self};
use renzora_ember::game_ui::{UiCanvas, UiWidget, UiWidgetType};

use crate::game_ui::{canvas, canvas_render, ui_inspector as inspector};

/// Phosphor icon *name* (kebab-case) for a widget type. Replaces the old
/// `UiWidgetType::icon()` inherent method (which lived in `renzora_game_ui`
/// behind the deleted `editor` feature). The name is resolved to a glyph
/// downstream via renzora_ember's phosphor map, so the mapping lives here in
/// the editor crate.
pub fn widget_icon(t: &UiWidgetType) -> &'static str {
    match t {
        UiWidgetType::Container => "squares-four",
        UiWidgetType::Panel => "rectangle",
        UiWidgetType::ScrollView => "scroll",
        UiWidgetType::Text => "text-aa",
        UiWidgetType::Image => "image",
        UiWidgetType::Button => "cursor-click",
        UiWidgetType::Slider => "sliders-horizontal",
        UiWidgetType::Checkbox => "check-square",
        UiWidgetType::Toggle => "toggle-right",
        UiWidgetType::RadioButton => "radio-button",
        UiWidgetType::Dropdown => "caret-circle-down",
        UiWidgetType::TextInput => "text-t",
        UiWidgetType::BarFill => "battery-medium",
        UiWidgetType::Tooltip => "chat-circle-text",
        UiWidgetType::Modal => "browsers",
        UiWidgetType::DraggableWindow => "app-window",
        UiWidgetType::KeybindRow => "keyboard",
        UiWidgetType::SettingsRow => "gear",
        UiWidgetType::Separator => "minus",
        UiWidgetType::NumberInput => "calculator",
        UiWidgetType::Scrollbar => "arrows-down-up",
        UiWidgetType::Circle => "circle",
        UiWidgetType::Arc => "circle-dashed",
        UiWidgetType::RadialProgress => "circle-notch",
        UiWidgetType::Line => "line-segment",
        UiWidgetType::Triangle => "triangle",
        UiWidgetType::Polygon => "hexagon",
        UiWidgetType::Rectangle => "rectangle",
        UiWidgetType::Wedge => "chart-pie-slice",
    }
}

/// Register everything the editor build used to wire up inside
/// `GameUiPlugin::build`'s `#[cfg(feature = "editor")]` block.
pub fn register_game_ui_editor(app: &mut App) {
    info!("[editor] GameUiPlugin (editor panels)");

    register_ui_presets(app);
    app.init_resource::<canvas::UiCanvasPreviewEnabled>();
    // Per-component inspector entries (Phase A of the UI inspector
    // decomposition). Each constituent component gets its own
    // collapsible in the main inspector. Fill/stroke/etc. are still
    // grouped under a "UI Style" lump until Phase B splits them.
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_canvas",
        display_name: "UI Canvas",
        icon: "frame-corners",
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
        // No trash button. Removing the marker left the full-size `Node` behind
        // — an invisible screen-covering entity that is no longer a canvas, no
        // longer holds a template, and reads in the hierarchy as an ordinary
        // empty. Deleting the entity is what you actually wanted, and the
        // hierarchy already does that.
        remove_fn: None,
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
            // How the reference resolution above maps onto a window that isn't
            // that size. `fit` keeps the canvas exactly as composed here and
            // letterboxes it; `expand` lets it re-flow to the window's aspect;
            // `constant` leaves authored pixels alone. See `CanvasScaleMode`.
            renzora::FieldDef {
                name: "Scale Mode",
                field_type: renzora::FieldType::Enum {
                    options: &["fit", "expand", "constant"],
                },
                get_fn: |w, e| {
                    w.get::<components::UiCanvas>(e)
                        .map(|c| renzora::FieldValue::Enum(c.scale_mode.clone()))
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(s), Some(mut c)) =
                        (v, w.get_mut::<components::UiCanvas>(e))
                    {
                        c.scale_mode = s;
                    }
                },
            },
            // Screen (normal fullscreen UI) vs world (projected onto a plane in
            // the 3D scene, placed by the entity's Transform).
            renzora::FieldDef {
                name: "Render Space",
                field_type: renzora::FieldType::Enum {
                    options: &["screen", "world"],
                },
                get_fn: |w, e| {
                    w.get::<components::UiCanvas>(e)
                        .map(|c| renzora::FieldValue::Enum(c.render_space.clone()))
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(s), Some(mut c)) =
                        (v, w.get_mut::<components::UiCanvas>(e))
                    {
                        c.render_space = s;
                    }
                },
            },
            // World space only: RTT texture-on-quad vs Unity-style emitted mesh.
            renzora::FieldDef {
                name: "Render Mode",
                field_type: renzora::FieldType::Enum {
                    options: &["texture", "mesh"],
                },
                get_fn: |w, e| {
                    w.get::<components::UiCanvas>(e)
                        .map(|c| renzora::FieldValue::Enum(c.render_mode.clone()))
                },
                set_fn: |w, e, v| {
                    if let (renzora::FieldValue::Enum(s), Some(mut c)) =
                        (v, w.get_mut::<components::UiCanvas>(e))
                    {
                        c.render_mode = s;
                    }
                },
            },
        ],
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_widget",
        display_name: "UI Widget",
        icon: "squares-four",
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
        icon: "square-half",
        category: "ui",
        has_fn: |world, entity| {
            // Widgets only — never a canvas.
            //
            // A canvas has a `Node`, but it is structural: full-size, absolute,
            // the surface the template's root sizes against. Offering Position /
            // X / Y / Width / Height / Direction / Justify / Align on it invited
            // you to make the canvas not fill the screen, and put a second,
            // competing answer to "what lays this out" next to the template that
            // actually does. The canvas says *how big the design surface is*
            // (Ref Width / Ref Height) and *where it renders* (Render Space);
            // layout belongs to the markup.
            //
            // Restricted to UI entities as well, so Bevy's `Node` on a non-UI
            // usage isn't picked up.
            world.get::<bevy::ui::Node>(entity).is_some()
                && world.get::<components::UiCanvas>(entity).is_none()
                && world.get::<components::UiWidget>(entity).is_some()
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
    //
    // Every `add_fn` and `remove_fn` below writes the `.html` as well as the
    // entity. A component is only half of what these represent: the other half
    // is the attribute the loader built it from, and the template is rebuilt
    // from that file on the next hot-reload. Touching only the entity meant a
    // removed component came back and an added one vanished, at whatever moment
    // the next drag or insert happened to trigger a rebuild.
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_fill",
        display_name: "UI Fill",
        icon: "drop-half",
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiFill>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiFill::Solid(Color::srgba(0.2, 0.2, 0.2, 1.0)));
            set_ui_attrs(world, entity, &[("background", "#333333")]);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiFill>();
            drop_ui_attrs(world, entity, &["background", "gradient"]);
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_stroke",
        display_name: "UI Border",
        icon: "bounding-box",
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiStroke>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(components::UiStroke::new(
                Color::srgba(0.4, 0.4, 0.4, 1.0),
                1.0,
            ));
            set_ui_attrs(
                world,
                entity,
                &[("border", "1px"), ("border_color", "#666666")],
            );
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiStroke>();
            drop_ui_attrs(world, entity, &["border", "border_color"]);
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_border_radius",
        display_name: "UI Border Radius",
        icon: "frame-corners",
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiBorderRadius>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiBorderRadius::default());
            set_ui_attrs(world, entity, &[("border_radius", "0")]);
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<components::UiBorderRadius>();
            drop_ui_attrs(world, entity, &["border_radius"]);
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::border_radius_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_text",
        display_name: "UI Text",
        icon: "text-aa",
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiTextStyle>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiTextStyle::default());
            set_ui_attrs(world, entity, &[("font_size", "14")]);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiTextStyle>();
            drop_ui_attrs(world, entity, &["font_size", "font_color", "font"]);
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::text_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_padding",
        display_name: "UI Padding",
        icon: "columns",
        category: "ui",
        has_fn: |world, entity| world.get::<components::UiPadding>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(components::UiPadding::default());
            set_ui_attrs(world, entity, &[("padding", "0")]);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<components::UiPadding>();
            drop_ui_attrs(world, entity, &["padding"]);
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: inspector::padding_fields(),
    });
    app.register_inspector(renzora::InspectorEntry {
        type_id: "ui_opacity",
        display_name: "UI Opacity",
        icon: "circle-half",
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
        icon: "sun-dim",
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
        icon: "crop",
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
        icon: "cursor",
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
        icon: "cursor-click",
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
        icon: "sliders-horizontal",
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
        icon: "check-square",
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
        icon: "toggle-left",
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
        icon: "radio-button",
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
        icon: "caret-circle-down",
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
        icon: "textbox",
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
        icon: "scroll",
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
        icon: "chat-circle",
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
        icon: "browser",
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
        icon: "app-window",
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
        // Outranks `HtmlTemplatePath` (66): a canvas holding a template is
        // still a canvas, and holding one is the entity's whole purpose — so
        // letting the path win meant every canvas rendered as a template.
        type_id: std::any::TypeId::of::<components::UiCanvas>(),
        name: "UI Canvas",
        icon: "frame-corners",
        color: [130, 200, 255],
        priority: 70,
        dynamic_icon_fn: None,
    });
    app.register_component_icon(renzora::ComponentIconEntry {
        type_id: std::any::TypeId::of::<components::UiWidget>(),
        name: "UI Widget",
        icon: "squares-four",
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
    // (not an egui simulation). The scene backdrop behind it is
    // borrowed from `ViewportRenderTarget` (the slot-0 editor
    // camera image — 3D, or 2D when UI view was entered from
    // 2D), so we don't spawn or maintain a second preview camera.
    app.add_systems(Startup, canvas_render::setup_ui_canvas_render);
    app.add_systems(Update, canvas_render::sync_render_target_to_reference);
    app.add_systems(
        Update,
        (
            ensure_ui_visibility_components,
            sync_ui_canvas_target_camera,
            // After the routing decision it reads — a frame behind would show
            // the unscaled canvas for one frame every time the Game UI toggle
            // or the UI editor panel opens.
            scale_canvas_for_viewport_preview,
            sync_canvas_sort_order_from_hierarchy,
            debug_ui_tree,
        )
            .chain(),
    );
    // Two systems used to live here — `auto_switch_view_on_selection` and
    // `switch_to_3d_on_world_canvas` — whose whole job was steering
    // `ViewportView::Ui`: flip the viewport into UI view when a widget was
    // selected, flip it back to 3D on a camera or a world-space canvas. That
    // variant is gone with the in-viewport editor, and so are they. Selecting a
    // widget no longer changes what the viewport is looking at, which is the
    // point of the canvas being its own panel: the two surfaces stop reaching
    // into each other.
}

// ── Canvas reference resolution ─────────────────────────────────────────
//
// Matching the editor's bevy_ui render target to the active canvas's
// reference resolution is handled by `canvas_render::sync_render_target_to_
// reference` — it resizes the offscreen texture so the canvas always renders
// 1:1 in design space. The earlier approach wrote the *global* `UiScale` to
// fit a fixed-size texture, which scaled the entire editor shell (issue #55),
// since the chrome is native bevy_ui under that same global scale.

// ── Editor-only systems ─────────────────────────────────────────────────────

// `LastSelectionForViewSwitch`, `auto_switch_view_on_selection` and
// `switch_to_3d_on_world_canvas` stood here. All three existed only to steer
// `ViewportView::Ui` — flip the viewport into UI view when a widget was
// selected, flip it back to 3D on a camera, a light, or a canvas switched to
// world space. That variant went with the in-viewport editor, and they went with
// it. See the note at the end of `register_game_ui_editor`.

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

/// Route every UI canvas to the one camera it should render through this frame.
///
/// This is the **single authority** on canvas `UiTargetCamera` in the editor,
/// for both modes — it previously shared the job with a second system
/// (`sync_canvases_to_editor_camera`) that re-added the edit-mode target after
/// this one removed it. That remove-then-re-add churn left a window where a
/// canvas perturbed by a reparent/reorder (or a freshly spawned one) could be
/// left with *no* target and fall back to Bevy's `IsDefaultUiCamera` — the
/// editor's own chrome camera — so game UI bled into the editor interface. One
/// system that only ever *sets* the correct camera, never removes, closes that
/// window: a canvas keeps a valid target at all times and merely switches it on
/// a mode change.
///
/// - **Edit mode, UI editor open** → the offscreen UI render camera
///   (`UiCanvasRender`), whose image the canvas tab displays.
/// - **Edit mode, UI editor closed** → the editor viewport camera, when the
///   viewport's `show_game_ui` switch is on, so the game's UI composites over
///   the scene you are editing. Without this the switch appeared to do nothing:
///   canvases were routed to an offscreen camera that is only *active* while the
///   Viewport is in UI view, so in the 3D viewport they were simply never drawn,
///   and toggling their `Visibility` changed nothing anyone could see.
///   With the switch off they stay on the offscreen route, which is the same as
///   not being drawn — no second mechanism needed to hide them.
/// - **Play mode** → the editor viewport camera that renders the running game
///   into the viewport image, so the UI composites on top. A 2D game plays
///   through the editor 2D camera (the 3D editor camera is parked on a token
///   render target then — UI hung off it would rasterize into a 64² image
///   nobody displays). Play mode never renders through the *authored* scene
///   camera, so we deliberately don't target it.
///
/// **Does not touch `Visibility`** — that's the user's / the script's concern.
/// Earlier versions force-hid every canvas outside play mode, which polluted
/// saved scenes and broke shipped runtime visibility.
fn sync_ui_canvas_target_camera(
    mut commands: Commands,
    play_mode: Res<renzora::PlayModeState>,
    render: Option<Res<canvas_render::UiCanvasRender>>,
    editor_cam: Query<Entity, With<renzora::core::EditorCamera>>,
    editor_cam_2d: Query<Entity, With<renzora::core::EditorCamera2d>>,
    kind_2d: Query<(), With<bevy::camera::Camera2d>>,
    canvases: Query<(Entity, Option<&bevy::ui::UiTargetCamera>, &UiCanvas)>,
    settings: Option<Res<renzora::core::viewport_types::ViewportSettings>>,
    dock: Option<Res<renzora_ember::dock::Dock>>,
    fixed: Option<Res<renzora_ember::dock::FixedDock>>,
    wins: Option<Res<renzora_ember::dock::DockWindows>>,
) {
    let offscreen = render.as_ref().map(|r| r.camera_entity);
    let viewport_cam = || {
        let game_is_2d = play_mode
            .active_game_camera
            .is_some_and(|e| kind_2d.get(e).is_ok());
        if game_is_2d {
            editor_cam_2d.iter().next()
        } else {
            editor_cam.iter().next()
        }
    };
    let target = if play_mode.is_in_play_mode() {
        viewport_cam()
    } else {
        // The UI editor owns the canvases while it is on screen — it displays
        // the offscreen render, so routing them elsewhere would blank the panel
        // whose whole job is to show them.
        let ui_editor_open = renzora_ember::dock::panel_visible_anywhere(
            "ui_canvas",
            dock.as_deref(),
            fixed.as_deref(),
            wins.as_deref(),
        );
        let show_over_scene = settings.is_some_and(|s| s.show_game_ui);
        if !ui_editor_open && show_over_scene {
            viewport_cam().or(offscreen)
        } else {
            offscreen
        }
    };

    // No camera resolved yet (startup, or the render target not spawned) — leave
    // canvases as they are rather than stripping a target they already hold.
    let Some(target) = target else {
        return;
    };

    for (entity, existing_target_cam, canvas) in &canvases {
        // A world-space canvas is a 3D object routed to its OWN offscreen camera
        // (see `world_panel::sync_world_ui_canvases`); it must not be pointed at
        // the screen UI camera.
        if canvas.is_world() {
            continue;
        }
        let needs_insert = existing_target_cam.is_none_or(|tc| tc.entity() != target);
        if needs_insert {
            commands
                .entity(entity)
                .insert(bevy::ui::UiTargetCamera(target));
        }
    }
}

/// Scale a canvas that is being previewed *over the 3D viewport* down to fit
/// it, the way the shipped game will.
///
/// The game does this with the global `UiScale`, which the editor can't touch:
/// one resource drives every `bevy_ui` tree in the process, so moving it would
/// resize the dock, the panels and the menu bar along with the preview. So the
/// preview uses a per-entity `UiTransform` instead. It resamples rather than
/// re-rasterizing, which is why the game doesn't do it this way — but this is a
/// preview, and being the wrong *size* is a worse lie than being slightly soft.
///
/// Only canvases routed away from the UI editor's own camera need it. That
/// target is resized to the canvas's reference resolution
/// (`sync_render_target_to_reference`), so there the design box already fills
/// it exactly and the identity transform is correct.
fn scale_canvas_for_viewport_preview(
    mut commands: Commands,
    render: Option<Res<canvas_render::UiCanvasRender>>,
    render_target: Option<Res<renzora::ViewportRenderTarget>>,
    images: Res<Assets<Image>>,
    canvases: Query<
        (
            Entity,
            &UiCanvas,
            Option<&bevy::ui::UiTargetCamera>,
            Option<&bevy::ui::UiTransform>,
        ),
        Without<renzora::HideInHierarchy>,
    >,
) {
    let offscreen = render.as_ref().map(|r| r.camera_entity);
    let viewport_size = render_target
        .as_ref()
        .and_then(|rt| rt.image.as_ref())
        .and_then(|h| images.get(h))
        .map(|img| img.size())
        .map(|s| Vec2::new(s.x as f32, s.y as f32));

    for (entity, canvas, target_cam, existing) in &canvases {
        if canvas.is_world() {
            continue;
        }
        let on_ui_editor_camera = match (target_cam, offscreen) {
            (Some(tc), Some(off)) => tc.entity() == off,
            // Not routed anywhere yet — assume the editor's own target rather
            // than scaling against a viewport it may never be shown in.
            _ => true,
        };

        let want = if on_ui_editor_camera {
            1.0
        } else {
            match viewport_size {
                Some(size) if size.x > 0.0 && size.y > 0.0 => canvas.scale_mode().scale_for(
                    canvas.reference_width.max(1.0),
                    canvas.reference_height.max(1.0),
                    size.x,
                    size.y,
                ),
                _ => 1.0,
            }
        };

        let current = existing.map(|t| t.scale.x).unwrap_or(1.0);
        if (current - want).abs() <= f32::EPSILON {
            continue;
        }
        let mut next = existing.copied().unwrap_or(bevy::ui::UiTransform::IDENTITY);
        next.scale = Vec2::splat(want);
        commands.entity(entity).insert(next);
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

/// Spawn a bare UI Canvas: the marker, a name, and the full-size absolute
/// `Node` that makes it a UI root. No template and no file written — pick one in
/// the inspector's UI Template slot, or make one there with "+".
///
/// Module-level (rather than nested in `register_ui_presets`) because three
/// things create a canvas now: the Add Entity preset, the "New UI Canvas"
/// starter, and the UI editor's own empty state.
/// Write attributes onto the markup a node came from, alongside the component
/// the inspector just inserted.
///
/// A no-op on anything without `MarkupSource`, so a widget spawned outside a
/// template is unaffected.
fn set_ui_attrs(world: &mut World, entity: Entity, attrs: &[(&str, &str)]) {
    for (key, value) in attrs {
        renzora_ember::markup::writeback::write_attr_to_markup(world, entity, key, value);
    }
}

/// Delete attributes from the markup, alongside the component the inspector just
/// removed. Removing an attribute a node never had is a no-op, so a component
/// that maps to several can list them all without checking which are present.
fn drop_ui_attrs(world: &mut World, entity: Entity, attrs: &[&str]) {
    for key in attrs {
        renzora_ember::markup::writeback::remove_attr_from_markup(world, entity, key);
    }
}

pub(crate) fn spawn_ui_canvas(world: &mut World) -> Entity {
    let canvas = components::UiCanvas::default();
    // Not written out here: `heal_canvas_root_geometry` re-establishes exactly
    // this every frame, and two copies of the definition is how they drift.
    let node = components::canvas_root_node(&canvas);
    let entity = world.spawn((canvas, node)).id();
    // The engine's one-id-per-entity rule, applied here rather than left to the
    // caller. Spawning through Add Entity gets it for free —
    // `renzora_context_menu` re-ids every preset it spawns — but the "New UI
    // Canvas" starter and the UI editor's own empty state call this directly,
    // and they were producing a second entity called "UI Canvas". Three rows you
    // cannot tell apart, all racing for `ui/ui_canvas.html`, since the template
    // "+" names the file after the canvas.
    let id = renzora::unique_entity_name(world, "UI Canvas", entity);
    world.entity_mut(entity).insert(Name::new(id));
    entity
}

/// Register the UI Canvas entity preset and the "New UI Canvas" scene starter.
fn register_ui_presets(app: &mut App) {
    use renzora::{AppEditorExt, EntityPreset, SceneStarter};

    // UI Canvas — always spawned at root.
    app.register_entity_preset(EntityPreset {
        id: "ui_canvas",
        display_name: "UI Canvas",
        icon: "frame-corners",
        category: "ui",
        spawn_fn: spawn_ui_canvas,
    });

    // "New UI" scene starter — spawns a canvas and selects it, so the inspector
    // opens on the template slot that is the next thing to fill.
    app.register_scene_starter(SceneStarter {
        id: "ui",
        title: "New UI Canvas",
        description: "A canvas to mount a UI template on",
        icon: "frame-corners",
        // The one starter that still makes sense when the hierarchy is scoped
        // to UI — it is the thing that scope is looking for.
        produces: &["UiCanvas"],
        spawn_fn: |world: &mut World| {
            let canvas = spawn_ui_canvas(world);
            if let Some(selection) = world.get_resource::<renzora::EditorSelection>() {
                selection.set(Some(canvas));
            }
        },
    });

    // ── The 29 widget presets are gone ───────────────────────────────────────
    //
    // Container, Panel, Button, Slider, … each spawned a `UiWidget` entity under
    // the canvas via `spawn_widget`. Three things were wrong with that, in
    // increasing order of seriousness:
    //
    // 1. Building a UI by clicking Add Entity once per element is slow, and 29
    //    entries made the UI category the largest thing in that menu.
    // 2. Those entities carry no `MarkupSource`, so nothing they contain is
    //    written to the template — the `.html` never learns they exist.
    // 3. **They are destroyed.** `finalize_pending_templates` despawns every
    //    `Node`-bearing child of the canvas before rebuilding from the file, so
    //    a hot-reload, a scene reload or a template change silently deletes
    //    everything added this way. It looked like an authoring tool and behaved
    //    like a scratchpad.
    //
    // The `.html` is the source of truth for a canvas's contents, so the way to
    // add a widget is to add a node to the template. `spawn_widget` and the
    // `UiWidgetType` vocabulary stay in `renzora_ember::game_ui::spawn` — they
    // are what a markup-inserting palette will need to describe — but nothing
    // reaches them from Add Entity any more.
}
