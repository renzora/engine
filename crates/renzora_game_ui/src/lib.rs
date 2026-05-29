#![allow(unused_variables, unused_assignments, dead_code)]

//! Renzora Game UI — bevy_ui game interface components and (optionally) editor panels.
//!
//! **Runtime** (always available):
//! - `UiCanvas`, `UiWidget`, `UiWidgetType` — serializable marker components
//! - Widget data components (`ProgressBarData`, `SliderData`, etc.)
//! - Runtime systems that drive widget behavior
//! - `GameUiPlugin` — registers types for reflection + runtime systems
//!
//! **Editor** (behind `editor` feature):
//! - Widget Palette, UI Canvas, and UI Inspector panels
//! - Play-mode visibility sync, debug tree logging

pub mod components;
pub mod script_extension;
pub mod shapes;
pub mod spawn;
pub mod systems;

#[cfg(feature = "editor")]
pub mod canvas;
#[cfg(feature = "editor")]
pub mod canvas_render;
#[cfg(feature = "editor")]
pub mod inspector;
#[cfg(feature = "editor")]
pub mod palette;

use bevy::prelude::*;

pub use components::{
    HtmlTemplatePath, HuiBuildOnSelf, UiCanvas, UiTheme, UiThemed, UiWidget, UiWidgetType,
};

#[derive(Default)]
pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        // ── Reflection registration ─────────────────────────────────────
        app.register_type::<components::UiCanvas>();
        app.register_type::<components::UiWidget>();
        app.register_type::<components::HtmlTemplatePath>();
        app.register_type::<components::HuiBuildOnSelf>();
        app.register_type::<components::UiWidgetPart>();
        // Single-entity primitive (replaces ProgressBar / HealthBar / LoadingScreen)
        app.register_type::<components::UiBarFill>();
        app.register_type::<components::ProgressDirection>();
        // Form inputs
        app.register_type::<components::SliderData>();
        app.register_type::<components::CheckboxData>();
        app.register_type::<components::ToggleData>();
        app.register_type::<components::RadioButtonData>();
        app.register_type::<components::DropdownData>();
        app.register_type::<components::TextInputData>();
        app.register_type::<components::NumberInputData>();
        // Layout / overlay primitives
        app.register_type::<components::ScrollViewData>();
        app.register_type::<components::TooltipData>();
        app.register_type::<components::ModalData>();
        app.register_type::<components::DraggableWindowData>();
        app.register_type::<components::SeparatorData>();
        app.register_type::<components::SeparatorDirection>();
        app.register_type::<components::ScrollbarData>();
        app.register_type::<components::ScrollbarOrientation>();
        app.register_type::<components::UiImagePath>();
        // Settings UI rows (used by editor settings panel)
        app.register_type::<components::KeybindRowData>();
        app.register_type::<components::SettingsRowData>();
        app.register_type::<components::SettingsControlType>();
        // Widget style components
        app.register_type::<components::UiFill>();
        app.register_type::<components::UiStroke>();
        app.register_type::<components::UiBorderRadius>();
        app.register_type::<components::UiBoxShadow>();
        app.register_type::<components::UiOpacity>();
        app.register_type::<components::UiClipContent>();
        app.register_type::<components::UiCursor>();
        app.register_type::<components::UiTextStyle>();
        app.register_type::<components::UiPadding>();
        // Interaction & animation
        app.register_type::<components::UiInteractionStyle>();
        app.register_type::<components::UiTransition>();
        app.register_type::<components::UiTween>();
        // Theming
        app.register_type::<components::UiTheme>();
        app.register_type::<components::UiThemed>();

        // ── Default theme resource ────────────────────────────────────
        app.init_resource::<components::UiTheme>();

        // ── Script actions (decoupled — observes ScriptAction events) ──
        app.add_observer(script_extension::handle_ui_script_actions);

        // ── Auto-layout on reparent ────────────────────────────────────
        // When a UI widget is dragged to a new parent in the hierarchy,
        // re-apply parent-aware positioning: Container parent → Relative
        // (flex flow), Canvas parent → Absolute (free placement). The
        // Changed-filtered system covers runtime drag-reparents; the
        // Insert observer covers the scene-load case (reflection inserts
        // bypass change detection).
        app.add_systems(Update, on_widget_reparented);
        app.add_observer(on_childof_inserted);

        // Visibility-mode binding: same dual-path setup as the reparent
        // logic. The Changed system handles inspector edits to the
        // mode dropdown; the observer applies the saved mode on scene
        // load when reflection inserts skip change-tick propagation.
        app.add_observer(on_canvas_inserted);

        // ── Shape primitives ────────────────────────────────────────────
        app.add_plugins(shapes::ShapesPlugin);

        // ── Canvas scaler & visibility-mode ──────────────────────────────
        //
        // `update_ui_scale` adjusts the global `UiScale` to fit the 3D
        // viewport's render target. Useful in standalone runtime (UI
        // scales with window), but in the editor it would also scale the
        // UI rendered to our fixed 1280×720 editor render target — making
        // a Node with `width: Px(100)` show up as some other pixel count
        // depending on the editor window size. So in editor builds we
        // skip it entirely; UiScale stays at the default 1.0 and our
        // canvas tab renders 1:1 with what the user authors.
        #[cfg(not(feature = "editor"))]
        app.add_systems(Update, update_ui_scale);
        app.add_systems(
            Update,
            (
                rehydrate_ui_images,
                sync_ui_zindex,
                apply_canvas_visibility_mode,
            ),
        );

        // ── Runtime widget systems ──────────────────────────────────────
        app.add_systems(
            Update,
            (
                systems::apply_bar_fill,
                systems::slider_system,
                systems::checkbox_system,
                systems::toggle_system,
                systems::radio_button_system,
                systems::tooltip_system,
                systems::dropdown_system,
                systems::dropdown_option_system,
                systems::modal_system,
                systems::draggable_window_system,
                systems::separator_system,
                systems::number_input_system,
                systems::scrollbar_system,
                systems::keybind_row_system,
                systems::settings_row_system,
                systems::interaction_style_system,
                systems::ui_theme_system,
                systems::ui_tween_system,
                systems::ensure_style_components,
                systems::apply_widget_style_system,
            ),
        );

        // ── Editor panels & systems ─────────────────────────────────────
        #[cfg(feature = "editor")]
        {
            use renzora_editor::AppEditorExt;
            info!("[editor] GameUiPlugin (editor panels)");

            app.register_panel(palette::WidgetPalettePanel::default());
            register_ui_presets(app);
            app.init_resource::<canvas::UiCanvasPreviewEnabled>();
            app.init_resource::<canvas::UiCanvasPanel>();
            app.init_resource::<LastSelectionForViewSwitch>();
            // Per-component inspector entries (Phase A of the UI inspector
            // decomposition). Each constituent component gets its own
            // collapsible in the main inspector. Fill/stroke/etc. are still
            // grouped under a "UI Style" lump until Phase B splits them.
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_canvas_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
                type_id: "ui_widget",
                display_name: "UI Widget",
                icon: egui_phosphor::regular::SQUARES_FOUR,
                category: "ui",
                has_fn: |world, entity| world.get::<components::UiWidget>(entity).is_some(),
                add_fn: None,
                remove_fn: None,
                is_enabled_fn: None,
                set_enabled_fn: None,
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_widget_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_layout_inspector),
            });
            // Per-style components — each is individually addable via the
            // Add Component overlay and removable via the trash icon. A text
            // label that doesn't want a border can drop UiStroke; a button
            // that wants a shadow can add UiBoxShadow. (Phase B.)
            app.register_inspector(renzora_editor::InspectorEntry {
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
                custom_ui_fn: Some(inspector::render_fill_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                custom_ui_fn: Some(inspector::render_stroke_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_border_radius_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_text_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_padding_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_opacity_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_shadow_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_clip_content_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_cursor_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                custom_ui_fn: Some(inspector::render_interaction_inspector),
            });
            // Per-widget-type data components — Phase C. Each is its own
            // entry; users can swap a slider's data, drop a tooltip's data,
            // etc. via the Add Component overlay.
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_slider_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_checkbox_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_toggle_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_radio_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                custom_ui_fn: Some(inspector::render_dropdown_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_text_input_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_scroll_view_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_tooltip_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_modal_data_inspector),
            });
            app.register_inspector(renzora_editor::InspectorEntry {
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
                fields: Vec::new(),
                custom_ui_fn: Some(inspector::render_draggable_window_data_inspector),
            });

            // Register hierarchy icons for UI entities
            app.register_component_icon(renzora_editor::ComponentIconEntry {
                type_id: std::any::TypeId::of::<components::UiCanvas>(),
                name: "UI Canvas",
                icon: egui_phosphor::regular::FRAME_CORNERS,
                color: [130, 200, 255],
                priority: 70,
                dynamic_icon_fn: None,
            });
            app.register_component_icon(renzora_editor::ComponentIconEntry {
                type_id: std::any::TypeId::of::<components::UiWidget>(),
                name: "UI Widget",
                icon: egui_phosphor::regular::SQUARES_FOUR,
                color: [130, 200, 255],
                priority: 60,
                dynamic_icon_fn: Some(|world, entity| {
                    world
                        .get::<components::UiWidget>(entity)
                        .map(|w| (w.widget_type.icon(), [130u8, 200, 255]))
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
                    register_ui_image_textures,
                    debug_ui_tree,
                )
                    .chain(),
            );
            app.add_systems(Update, auto_switch_view_on_selection);
        }

        #[cfg(not(feature = "editor"))]
        {
            info!("[runtime] GameUiPlugin");
        }
    }
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
#[cfg(feature = "editor")]
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

// ── Canvas visibility_mode → Visibility ──────────────────────────────────
//
// `UiCanvas.visibility_mode` is the user-facing dropdown ("always",
// "play_only", "editor_only"). Until now it was a hint nothing read.
// This system writes the actual Bevy `Visibility` component from it
// whenever the canvas is freshly added or the dropdown changes.
//
// Runs in both editor and runtime — `PlayModeState` is optional, so in
// runtime builds (no PlayModeState resource) `in_play` defaults to true,
// making "play_only" canvases visible at runtime, "editor_only" hidden,
// and "always" always visible. Scripts can still override via
// `ui_show` / `ui_hide` afterward; the system only fires when the
// canvas component itself changes (`Changed<UiCanvas>`), not every frame.

fn apply_canvas_visibility_mode(
    play_mode: Option<Res<renzora::PlayModeState>>,
    mut canvases: Query<(&UiCanvas, &mut Visibility), Changed<UiCanvas>>,
) {
    let in_play = play_mode.is_none_or(|p| p.is_in_play_mode());
    for (canvas, mut vis) in &mut canvases {
        apply_canvas_visibility_to(in_play, canvas, &mut vis);
    }
}

fn apply_canvas_visibility_to(in_play: bool, canvas: &UiCanvas, vis: &mut Visibility) {
    let visible = match canvas.visibility_mode.as_str() {
        "always" => true,
        "play_only" => in_play,
        "editor_only" => !in_play,
        _ => true,
    };
    let target = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *vis != target {
        *vis = target;
    }
}

/// Lifecycle observer covering the scene-load case the `Changed`-filtered
/// system above misses. Reflection inserts (used by `DynamicScene::write_to_world`)
/// don't propagate Bevy's change ticks, so a saved canvas with
/// `visibility_mode: "play_only"` would render incorrectly in the editor
/// until the user touched it. The observer fires on insert and applies
/// the same logic.
fn on_canvas_inserted(
    trigger: On<Insert, UiCanvas>,
    play_mode: Option<Res<renzora::PlayModeState>>,
    mut canvases: Query<(&UiCanvas, &mut Visibility)>,
) {
    let entity = trigger.entity;
    let in_play = play_mode.is_none_or(|p| p.is_in_play_mode());
    if let Ok((canvas, mut vis)) = canvases.get_mut(entity) {
        apply_canvas_visibility_to(in_play, canvas, &mut vis);
    }
}

// ── Reparent system ────────────────────────────────────────────────────────
//
// Fires when a `ChildOf` is inserted *or* replaced on a UI widget entity
// (drag in hierarchy → Replace; spawn → Insert; both surface as
// `Changed<ChildOf>`). Re-runs the parent-aware layout logic so the moved
// widget switches between Absolute (canvas root) and Relative (Container)
// automatically.
//
// Originally written as an `On<Insert, ChildOf>` observer, which missed
// the drag-in-hierarchy case because that fires `Replace` not `Insert`.
// `Changed` filter catches both.

fn on_widget_reparented(
    mut commands: Commands,
    changed: Query<Entity, (With<UiWidget>, Changed<ChildOf>)>,
) {
    for entity in &changed {
        commands.queue(move |world: &mut World| {
            crate::spawn::reapply_layout_from_parent(world, entity);
        });
    }
}

/// Lifecycle observer covering the scene-load case the `Changed`-filtered
/// system above misses. `DynamicScene::write_to_world` inserts `ChildOf`
/// via reflection without propagating change ticks, so widgets loaded
/// from a saved scene wouldn't have their parent-aware layout applied
/// (Container parent → Relative, Canvas root → Absolute) until the user
/// touched them.
fn on_childof_inserted(
    trigger: On<Insert, ChildOf>,
    mut commands: Commands,
    widgets: Query<(), With<UiWidget>>,
) {
    let entity = trigger.entity;
    if widgets.get(entity).is_err() {
        return;
    }
    commands.queue(move |world: &mut World| {
        crate::spawn::reapply_layout_from_parent(world, entity);
    });
}

// ── Canvas scaler ───────────────────────────────────────────────────────────

/// Scales `Val::Px` values (text size, padding, border-radius) uniformly so
/// they stay proportional to the viewport.
fn update_ui_scale(
    canvases: Query<&UiCanvas>,
    render_target: Option<Res<renzora::ViewportRenderTarget>>,
    images: Res<Assets<Image>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut ui_scale: ResMut<bevy::ui::UiScale>,
) {
    let (ref_w, ref_h) = canvases
        .iter()
        .next()
        .map(|c| (c.reference_width, c.reference_height))
        .unwrap_or((1280.0, 720.0));

    if ref_w <= 0.0 || ref_h <= 0.0 {
        return;
    }

    let actual = render_target
        .as_ref()
        .and_then(|rt| rt.image.as_ref())
        .and_then(|h| images.get(h))
        .map(|img| {
            let s = img.size();
            (s.x as f32, s.y as f32)
        });

    let (actual_w, actual_h) = match actual {
        Some(size) => size,
        None => {
            if let Ok(window) = windows.single() {
                (window.width(), window.height())
            } else {
                return;
            }
        }
    };

    if actual_w <= 0.0 || actual_h <= 0.0 {
        return;
    }

    let scale = (actual_w / ref_w).min(actual_h / ref_h);
    ui_scale.0 = scale;
}

// ── Image rehydration ───────────────────────────────────────────────────────

/// Rehydrates `ImageNode` for UI image widgets after scene deserialization.
///
/// `ImageNode` contains a `Handle<Image>` which fails serialization and gets
/// stripped on save. `UiImagePath` stores the asset-relative path and survives.
/// This system re-loads the image and inserts `ImageNode` on any entity that
/// has `UiImagePath` but no `ImageNode`.
fn rehydrate_ui_images(
    mut commands: Commands,
    query: Query<
        (Entity, &components::UiImagePath),
        (Without<ImageNode>, Added<components::UiImagePath>),
    >,
    asset_server: Res<AssetServer>,
) {
    for (entity, img_path) in &query {
        let path = img_path.path.clone();
        let handle: Handle<Image> = asset_server.load(path);
        commands.entity(entity).try_insert(ImageNode::new(handle));
    }
}

// ── Z-index sync ────────────────────────────────────────────────────────────

/// Syncs `ZIndex` on UI canvas and widget entities so that items higher in the
/// hierarchy (top of the list) render on top — matching the layer order convention
/// used by most editors (Photoshop, Unity, etc.).
fn sync_ui_zindex(
    canvas_entities: Query<Entity, With<UiCanvas>>,
    canvas_data: Query<(&UiCanvas, Option<&GlobalZIndex>)>,
    widgets: Query<Entity, With<UiWidget>>,
    zindex_query: Query<Option<&ZIndex>>,
    children_query: Query<&Children>,
    child_of_query: Query<&ChildOf>,
    mut commands: Commands,
) {
    let mut processed_parents = std::collections::HashSet::new();

    for entity in canvas_entities.iter().chain(widgets.iter()) {
        let parent = match child_of_query.get(entity) {
            Ok(c) => c.parent(),
            Err(_) => continue,
        };

        if !processed_parents.insert(parent) {
            continue;
        }

        let Ok(children) = children_query.get(parent) else {
            continue;
        };

        // Count only UI entities among siblings for correct reverse indexing.
        let ui_count = children
            .iter()
            .filter(|c| canvas_entities.contains(*c) || widgets.contains(*c))
            .count() as i32;

        let mut ui_idx = 0i32;
        for child in children.iter() {
            if canvas_entities.contains(child) || widgets.contains(child) {
                // First child (top of hierarchy) gets highest ZIndex → renders on top.
                let desired = ZIndex(ui_count - 1 - ui_idx);
                let current = zindex_query.get(child).ok().flatten().copied();
                if current != Some(desired) {
                    commands.entity(child).try_insert(desired);
                }
                ui_idx += 1;
            }
        }
    }

    // Root-level canvases (no parent) use GlobalZIndex from sort_order.
    for entity in &canvas_entities {
        if child_of_query.contains(entity) {
            continue;
        }
        if let Ok((canvas, current_gz)) = canvas_data.get(entity) {
            let desired = GlobalZIndex(canvas.sort_order);
            if current_gz.copied() != Some(desired) {
                commands.entity(entity).try_insert(desired);
            }
        }
    }
}

// ── Editor-only systems ─────────────────────────────────────────────────────

/// Tracks the last selection we processed for view-auto-switching, so the
/// switch fires on selection *change* only — not every frame, which would
/// fight a user who explicitly picked a different viewport view while a
/// UI entity was selected.
#[cfg(feature = "editor")]
#[derive(Resource, Default)]
struct LastSelectionForViewSwitch(Option<Entity>);

/// When the selection changes to a UI entity (`UiCanvas`/`UiWidget` or a
/// descendant of one), flip the viewport into UI view. When it changes to
/// a non-UI entity *and* we're currently in UI view, flip back to 3D.
/// Other view transitions (3D ↔ 2D) are left to the user.
#[cfg(feature = "editor")]
fn auto_switch_view_on_selection(world: &mut World) {
    use renzora::core::viewport_types::{ViewportSettings, ViewportView};

    let current_sel = world
        .get_resource::<renzora_editor::EditorSelection>()
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
#[cfg(feature = "editor")]
fn sync_canvas_sort_order_from_hierarchy(
    mut canvases: Query<(&mut UiCanvas, &renzora_editor::HierarchyOrder), Without<ChildOf>>,
) {
    let max_order = canvases.iter().map(|(_, h)| h.0).max().unwrap_or(0) as i32;
    for (mut canvas, order) in &mut canvases {
        let new_order = max_order - order.0 as i32;
        if canvas.sort_order != new_order {
            canvas.sort_order = new_order;
        }
    }
}

#[cfg(feature = "editor")]
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
#[cfg(feature = "editor")]
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

#[cfg(feature = "editor")]
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

/// Registers `ImageNode` handles from UiWidget entities with egui so the canvas
/// Register UI Canvas + all UI widget types as entity presets in the hierarchy
/// "Add Entity" overlay. Each widget preset spawns via `spawn::spawn_widget`,
/// which finds (or creates) a canvas and parents the new widget to it.
#[cfg(feature = "editor")]
fn register_ui_presets(app: &mut App) {
    use renzora_editor::{AppEditorExt, EntityPreset, SceneStarter};

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
            if let Some(selection) = world.get_resource::<renzora_editor::EditorSelection>() {
                selection.set(Some(canvas));
            }
        },
    });

    macro_rules! widget_preset {
        ($variant:ident, $id:literal, $label:literal) => {{
            fn spawn_fn(world: &mut World) -> Entity {
                crate::spawn::spawn_widget(world, &UiWidgetType::$variant, None)
            }
            app.register_entity_preset(EntityPreset {
                id: $id,
                display_name: $label,
                icon: UiWidgetType::$variant.icon(),
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

/// panel can display image previews.
#[cfg(feature = "editor")]
fn register_ui_image_textures(
    widgets: Query<&ImageNode, With<UiWidget>>,
    images: Res<Assets<Image>>,
    mut user_textures: ResMut<bevy_egui::EguiUserTextures>,
) {
    for image_node in &widgets {
        let handle = &image_node.image;
        // Only register once the image is loaded, and only if not already registered
        if images.contains(handle) && user_textures.image_id(handle.id()).is_none() {
            user_textures.add_image(bevy_egui::EguiTextureHandle::Strong(handle.clone()));
        }
    }
}

renzora::add!(GameUiPlugin);
