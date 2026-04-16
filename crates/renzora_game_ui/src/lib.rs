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
pub mod inspector;
#[cfg(feature = "editor")]
pub mod palette;

use bevy::prelude::*;

pub use components::{UiCanvas, UiTheme, UiThemed, UiWidget, UiWidgetType};

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        // ── Reflection registration ─────────────────────────────────────
        app.register_type::<components::UiCanvas>();
        app.register_type::<components::UiWidget>();
        app.register_type::<components::UiWidgetPart>();
        // Widget data
        app.register_type::<components::ProgressBarData>();
        app.register_type::<components::HealthBarData>();
        app.register_type::<components::SliderData>();
        app.register_type::<components::CheckboxData>();
        app.register_type::<components::ToggleData>();
        app.register_type::<components::RadioButtonData>();
        app.register_type::<components::DropdownData>();
        app.register_type::<components::TextInputData>();
        app.register_type::<components::ScrollViewData>();
        app.register_type::<components::TabBarData>();
        app.register_type::<components::SpinnerData>();
        app.register_type::<components::TooltipData>();
        app.register_type::<components::ModalData>();
        app.register_type::<components::DraggableWindowData>();
        app.register_type::<components::UiImagePath>();
        // HUD widget data
        app.register_type::<components::CrosshairData>();
        app.register_type::<components::CrosshairStyle>();
        app.register_type::<components::AmmoCounterData>();
        app.register_type::<components::AmmoDisplayMode>();
        app.register_type::<components::CompassData>();
        app.register_type::<components::CompassMarker>();
        app.register_type::<components::StatusEffectBarData>();
        app.register_type::<components::StatusEffect>();
        app.register_type::<components::NotificationFeedData>();
        app.register_type::<components::Notification>();
        app.register_type::<components::RadialMenuData>();
        app.register_type::<components::RadialMenuItem>();
        app.register_type::<components::MinimapData>();
        app.register_type::<components::MinimapRotation>();
        app.register_type::<components::MinimapShape>();
        // Menu widget data
        app.register_type::<components::InventoryGridData>();
        app.register_type::<components::InventorySlot>();
        app.register_type::<components::DialogBoxData>();
        app.register_type::<components::ObjectiveTrackerData>();
        app.register_type::<components::ObjectiveStatus>();
        app.register_type::<components::Objective>();
        app.register_type::<components::LoadingScreenData>();
        app.register_type::<components::KeybindRowData>();
        app.register_type::<components::SettingsRowData>();
        app.register_type::<components::SettingsControlType>();
        // Extra widget data
        app.register_type::<components::SeparatorData>();
        app.register_type::<components::SeparatorDirection>();
        app.register_type::<components::NumberInputData>();
        app.register_type::<components::VerticalSliderData>();
        app.register_type::<components::ScrollbarData>();
        app.register_type::<components::ScrollbarOrientation>();
        app.register_type::<components::ListData>();
        app.register_type::<components::ListItem>();
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

        // ── Shape primitives ────────────────────────────────────────────
        app.add_plugins(shapes::ShapesPlugin);

        // ── Canvas scaler ───────────────────────────────────────────────
        app.add_systems(Update, (update_ui_scale, rehydrate_ui_images, sync_ui_zindex));

        // ── Runtime widget systems ──────────────────────────────────────
        app.add_systems(
            Update,
            (
                systems::progress_bar_system,
                systems::health_bar_system,
                systems::slider_system,
                systems::checkbox_system,
                systems::toggle_system,
                systems::radio_button_system,
                systems::tab_bar_system,
                systems::spinner_system,
                systems::tooltip_system,
                systems::dropdown_system,
                systems::dropdown_option_system,
                systems::modal_system,
                systems::draggable_window_system,
                systems::dialog_box_system,
                systems::loading_screen_system,
                systems::objective_tracker_system,
            ),
        );
        app.add_systems(
            Update,
            (
                systems::ammo_counter_system,
                systems::compass_system,
                systems::status_effect_system,
                systems::notification_system,
                systems::separator_system,
                systems::number_input_system,
                systems::vertical_slider_system,
                systems::scrollbar_system,
                systems::list_system,
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
            use renzora_editor_framework::AppEditorExt;
            info!("[editor] GameUiPlugin (editor panels)");

            app.register_panel(palette::WidgetPalettePanel::default());
            register_ui_presets(app);
            app.init_resource::<canvas::UiCanvasPreviewEnabled>();
            app.init_resource::<UiWorkspaceActive>();
            app.register_panel(canvas::UiCanvasPanel::default());
            app.register_panel(inspector::UiInspectorPanel::default());

            // Register hierarchy icons for UI entities
            app.register_component_icon(renzora_editor_framework::ComponentIconEntry {
                type_id: std::any::TypeId::of::<components::UiCanvas>(),
                icon: egui_phosphor::regular::FRAME_CORNERS,
                color: [130, 200, 255],
                priority: 70,
                dynamic_icon_fn: None,
            });
            app.register_component_icon(renzora_editor_framework::ComponentIconEntry {
                type_id: std::any::TypeId::of::<components::UiWidget>(),
                icon: egui_phosphor::regular::SQUARES_FOUR,
                color: [130, 200, 255],
                priority: 60,
                dynamic_icon_fn: Some(|world, entity| {
                    world.get::<components::UiWidget>(entity)
                        .map(|w| (w.widget_type.icon(), [130u8, 200, 255]))
                }),
            });

            app.add_systems(Startup, canvas::setup_canvas_preview);
            app.add_systems(
                Update,
                (
                    canvas::update_canvas_preview,
                    ensure_ui_visibility_components,
                    sync_ui_canvas_visibility,
                    sync_canvas_sort_order_from_hierarchy,
                    sync_hierarchy_filter_for_ui_workspace,
                    register_ui_image_textures,
                    debug_ui_tree,
                )
                    .chain(),
            );
            app.add_systems(Update, auto_switch_to_ui_layout_on_selection);
            app.add_systems(Update, reset_ui_preview_on_layout_enter);
        }

        #[cfg(not(feature = "editor"))]
        {
            info!("[runtime] GameUiPlugin");
        }
    }
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
    query: Query<(Entity, &components::UiImagePath), (Without<ImageNode>, Added<components::UiImagePath>)>,
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

/// Filter the hierarchy by active workspace: UI layout shows only `UiCanvas`
/// subtrees, Scene layout hides them. Other layouts are left alone so their
/// own sync systems (e.g. materials → `Mesh3d`) still apply.
#[cfg(feature = "editor")]
fn sync_hierarchy_filter_for_ui_workspace(
    layout_mgr: Res<renzora_editor_framework::LayoutManager>,
    mut filter: ResMut<renzora_editor_framework::HierarchyFilter>,
) {
    let desired = match layout_mgr.active_name() {
        "UI" => renzora_editor_framework::HierarchyFilter::OnlyWithComponents(vec!["UiCanvas"]),
        "Scene" => renzora_editor_framework::HierarchyFilter::ExcludeDescendantsOf(vec!["UiCanvas"]),
        _ => return,
    };
    if *filter != desired {
        *filter = desired;
    }
}

/// Tracks whether the UI workspace was active last frame, so we can detect
/// transitions into it and reset the preview toggle from settings.
#[cfg(feature = "editor")]
#[derive(Resource, Default)]
struct UiWorkspaceActive(bool);

#[cfg(feature = "editor")]
fn reset_ui_preview_on_layout_enter(
    layout_mgr: Res<renzora_editor_framework::LayoutManager>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    mut last: ResMut<UiWorkspaceActive>,
    mut preview: ResMut<canvas::UiCanvasPreviewEnabled>,
) {
    let is_ui = layout_mgr.active_name() == "UI";
    if is_ui && !last.0 {
        let default_on = settings.map_or(true, |s| s.ui_preview_by_default);
        preview.0 = default_on;
    }
    last.0 = is_ui;
}

/// When a UI entity (UiCanvas or a descendant of one) becomes selected in the
/// Scene workspace, auto-switch to the UI workspace so the user can edit it.
#[cfg(feature = "editor")]
fn auto_switch_to_ui_layout_on_selection(world: &mut World) {
    let active = world.resource::<renzora_editor_framework::LayoutManager>().active_name().to_string();
    if active == "Hub" {
        return;
    }
    let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() else {
        return;
    };
    let Some(entity) = sel.get() else { return };
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
    match (is_ui, active.as_str()) {
        (true, "UI") | (false, "Scene") => {}
        (true, _) => renzora_editor_framework::switch_layout_by_name(world, "UI"),
        (false, _) => renzora_editor_framework::switch_layout_by_name(world, "Scene"),
    }
}

/// In the editor, sync `UiCanvas::sort_order` from `HierarchyOrder` so that
/// reordering canvases in the hierarchy panel updates their z-index.
/// Top of hierarchy (lowest HierarchyOrder) gets the highest sort_order → renders on top.
#[cfg(feature = "editor")]
fn sync_canvas_sort_order_from_hierarchy(
    mut canvases: Query<(&mut UiCanvas, &renzora_editor_framework::HierarchyOrder), Without<ChildOf>>,
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
        commands.entity(entity).try_insert((
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

#[cfg(feature = "editor")]
fn sync_ui_canvas_visibility(
    mut commands: Commands,
    play_mode: Res<renzora::PlayModeState>,
    mut canvases: Query<
        (
            Entity,
            &mut Visibility,
            &Name,
            Option<&bevy::ui::UiTargetCamera>,
        ),
        With<UiCanvas>,
    >,
) {
    let in_play = play_mode.is_in_play_mode();
    let target = if in_play {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let game_camera = play_mode.active_game_camera;

    for (entity, mut vis, name, existing_target_cam) in &mut canvases {
        if *vis != target {
            *vis = target;
        }

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
            commands
                .entity(entity)
                .remove::<bevy::ui::UiTargetCamera>();
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
    static LAST_PLAY: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
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
    use renzora_editor_framework::{AppEditorExt, EntityPreset};

    // UI Canvas — always spawned at root.
    app.register_entity_preset(EntityPreset {
        id: "ui_canvas",
        display_name: "UI Canvas",
        icon: egui_phosphor::regular::FRAME_CORNERS,
        category: "ui",
        spawn_fn: |world| {
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
    widget_preset!(ProgressBar, "ui_progress_bar", "Progress Bar");
    widget_preset!(HealthBar, "ui_health_bar", "Health Bar");
    widget_preset!(Spinner, "ui_spinner", "Spinner");
    widget_preset!(TabBar, "ui_tab_bar", "Tab Bar");
    widget_preset!(Tooltip, "ui_tooltip", "Tooltip");
    widget_preset!(Modal, "ui_modal", "Modal");
    widget_preset!(DraggableWindow, "ui_draggable_window", "Draggable Window");
    widget_preset!(Crosshair, "ui_crosshair", "Crosshair");
    widget_preset!(AmmoCounter, "ui_ammo_counter", "Ammo Counter");
    widget_preset!(Compass, "ui_compass", "Compass");
    widget_preset!(StatusEffectBar, "ui_status_effects", "Status Effects");
    widget_preset!(NotificationFeed, "ui_notifications", "Notifications");
    widget_preset!(RadialMenu, "ui_radial_menu", "Radial Menu");
    widget_preset!(Minimap, "ui_minimap", "Minimap");
    widget_preset!(InventoryGrid, "ui_inventory_grid", "Inventory Grid");
    widget_preset!(DialogBox, "ui_dialog_box", "Dialog Box");
    widget_preset!(ObjectiveTracker, "ui_objective_tracker", "Objective Tracker");
    widget_preset!(LoadingScreen, "ui_loading_screen", "Loading Screen");
    widget_preset!(KeybindRow, "ui_keybind_row", "Keybind Row");
    widget_preset!(SettingsRow, "ui_settings_row", "Settings Row");
    widget_preset!(Separator, "ui_separator", "Separator");
    widget_preset!(NumberInput, "ui_number_input", "Number Input");
    widget_preset!(VerticalSlider, "ui_vertical_slider", "Vertical Slider");
    widget_preset!(Scrollbar, "ui_scrollbar", "Scrollbar");
    widget_preset!(List, "ui_list", "List");
    widget_preset!(Circle, "ui_circle", "Circle");
    widget_preset!(Arc, "ui_arc", "Arc");
    widget_preset!(RadialProgress, "ui_radial_progress", "Radial Progress");
    widget_preset!(Line, "ui_line", "Line");
    widget_preset!(Triangle, "ui_triangle", "Triangle");
    widget_preset!(Polygon, "ui_polygon", "Polygon");
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
