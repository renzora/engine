//! VR integration bridge for the editor
//!
//! Connects renzora_xr systems with the editor's play mode, audio, and UI.
//! Includes sync systems that copy VR resource data into editor panel states.
//! This module is only compiled when the `xr` feature is enabled.

use bevy::prelude::*;
use bevy::core_pipeline::Skybox;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy_egui::{EguiContext, EguiInput, PrimaryEguiContext};

use crate::core::PlayModeState;
use crate::core::resources::console::{console_log, LogLevel};
use crate::core::EditorEntity;
use crate::component_system::{CameraNodeData, CameraRigData};
use crate::ui::HandSnapshot;

/// System: when VR play mode starts, spawn the VR camera rig at the scene's
/// default camera position.
pub fn enter_vr_play_mode(
    play_mode: Res<PlayModeState>,
    cameras: Query<(&CameraNodeData, &Transform), Without<CameraRigData>>,
    camera_rigs: Query<(&CameraRigData, &Transform), Without<CameraNodeData>>,
    mut spawn_events: MessageWriter<renzora_xr::camera::SpawnVrCameraRigEvent>,
    mut entered: Local<bool>,
) {
    if play_mode.is_playing() && !*entered {
        // Find the default camera position
        let spawn_pos = cameras
            .iter()
            .find(|(data, _)| data.is_default_camera)
            .map(|(_, tf)| (tf.translation, tf.rotation))
            .or_else(|| {
                camera_rigs
                    .iter()
                    .find(|(data, _)| data.is_default_camera)
                    .map(|(_, tf)| (tf.translation, tf.rotation))
            })
            .or_else(|| cameras.iter().next().map(|(_, tf)| (tf.translation, tf.rotation)))
            .or_else(|| camera_rigs.iter().next().map(|(_, tf)| (tf.translation, tf.rotation)))
            .unwrap_or((Vec3::new(0.0, 1.7, 0.0), Quat::IDENTITY));

        spawn_events.write(renzora_xr::camera::SpawnVrCameraRigEvent {
            position: spawn_pos.0,
            rotation: spawn_pos.1,
        });

        *entered = true;
        info!("VR play mode: spawning camera rig at {:?}", spawn_pos.0);
    }

    if !play_mode.is_in_play_mode() && *entered {
        *entered = false;
    }
}

/// System: when VR play mode exits, despawn the VR camera rig.
pub fn exit_vr_play_mode(
    play_mode: Res<PlayModeState>,
    mut despawn_events: MessageWriter<renzora_xr::camera::DespawnVrCameraRigEvent>,
    mut was_playing: Local<bool>,
) {
    if play_mode.is_in_play_mode() {
        *was_playing = true;
    }

    if !play_mode.is_in_play_mode() && *was_playing {
        despawn_events.write(renzora_xr::camera::DespawnVrCameraRigEvent);
        *was_playing = false;
        info!("VR play mode: despawning camera rig");
    }
}

/// System: drain VR log buffer entries into the editor console each frame.
pub fn drain_vr_logs_to_console() {
    let Some(buffer) = renzora_xr::get_vr_log_buffer() else {
        return;
    };

    for entry in buffer.drain() {
        let level = match entry.level {
            renzora_xr::VrLogLevel::Info => LogLevel::Info,
            renzora_xr::VrLogLevel::Success => LogLevel::Success,
            renzora_xr::VrLogLevel::Warning => LogLevel::Warning,
            renzora_xr::VrLogLevel::Error => LogLevel::Error,
        };
        console_log(level, "VR", &entry.message);
    }
}

// ============================================================================
// VR Entity Selection — laser raycast selects scene entities
// ============================================================================

/// Walk up the `ChildOf` hierarchy to find an `EditorEntity` ancestor.
fn find_editor_entity_ancestor(
    entity: Entity,
    editor_entities: &Query<(Entity, &GlobalTransform, &EditorEntity)>,
    parent_query: &Query<&ChildOf>,
) -> Option<Entity> {
    if editor_entities.get(entity).is_ok() {
        return Some(entity);
    }
    let mut current = entity;
    while let Ok(child_of) = parent_query.get(current) {
        let parent = child_of.0;
        if editor_entities.get(parent).is_ok() {
            return Some(parent);
        }
        current = parent;
    }
    None
}

/// System: cast rays from controller aim poses to select scene entities.
///
/// Skips when the hand is pointing at a VR panel (checked via `VrPointerHit`).
/// On trigger rising edge, selects the closest `EditorEntity` ancestor; on
/// trigger release with no hit, clears the selection.
pub fn vr_entity_selection(
    controllers: Option<Res<renzora_xr::VrControllerState>>,
    pointer_hit: Res<renzora_vr_editor::VrPointerHit>,
    tracking_root: Query<&GlobalTransform, With<renzora_xr::reexports::XrTrackingRoot>>,
    mut selection: ResMut<crate::core::SelectionState>,
    mut mesh_ray_cast: MeshRayCast,
    editor_entities: Query<(Entity, &GlobalTransform, &EditorEntity)>,
    parent_query: Query<&ChildOf>,
    mut prev_trigger: Local<[bool; 2]>,
) {
    let Some(controllers) = controllers else { return };
    let root_tf = tracking_root.single().copied().unwrap_or(GlobalTransform::IDENTITY);

    let hands = [
        (&controllers.left, &pointer_hit.left, 0usize),
        (&controllers.right, &pointer_hit.right, 1usize),
    ];

    for (hand, hand_ray, idx) in hands {
        if !hand.tracked {
            prev_trigger[idx] = hand.trigger_pressed;
            continue;
        }

        // Rising edge detection
        let rising = hand.trigger_pressed && !prev_trigger[idx];
        prev_trigger[idx] = hand.trigger_pressed;

        if !rising {
            continue;
        }

        // Skip if hand is hitting a VR panel
        if hand_ray.hit_distance.is_some() {
            continue;
        }

        // Build world-space ray from aim pose
        let aim_pos = root_tf.transform_point(hand.aim_position);
        let aim_dir = (root_tf.affine().matrix3 * (hand.aim_rotation * Vec3::NEG_Z)).normalize();
        let ray = Ray3d::new(aim_pos, Dir3::new(aim_dir).unwrap_or(Dir3::NEG_Z));

        let hits = mesh_ray_cast.cast_ray(ray, &MeshRayCastSettings::default());

        // Find closest hit that belongs to an EditorEntity
        let mut closest_entity: Option<Entity> = None;
        let mut closest_distance = f32::MAX;

        for (hit_entity, hit) in hits.iter() {
            if let Some(editor_entity) = find_editor_entity_ancestor(*hit_entity, &editor_entities, &parent_query) {
                if let Ok((_, _, ee)) = editor_entities.get(editor_entity) {
                    if ee.locked {
                        continue;
                    }
                }
                if hit.distance < closest_distance {
                    closest_distance = hit.distance;
                    closest_entity = Some(editor_entity);
                }
            }
        }

        if let Some(entity) = closest_entity {
            selection.select(entity);
        } else {
            selection.clear();
        }
    }
}

// ============================================================================
// Session control — manual Start/Stop from editor UI
// ============================================================================

/// Manual session handler — replaces bevy_xr's `auto_handle_session`.
///
/// Handles the OpenXR state machine transitions (Ready→begin, Stopping→end,
/// Exiting→destroy) but does NOT auto-create sessions on Available. The user
/// must click "Start VR" to create a session, and "Stop VR" to exit.
///
/// Headset removal (Focused→Visible) does NOT stop VR — the session keeps
/// running so the user can debug via the desktop editor.
pub fn handle_vr_session_commands(
    mut panel: ResMut<crate::ui::VrSessionPanelState>,
    mut state_changed: MessageReader<renzora_xr::reexports::XrStateChanged>,
    mut create_session: MessageWriter<renzora_xr::reexports::XrCreateSessionMessage>,
    mut begin_session: MessageWriter<renzora_xr::reexports::XrBeginSessionMessage>,
    mut end_session: MessageWriter<renzora_xr::reexports::XrEndSessionMessage>,
    mut destroy_session: MessageWriter<renzora_xr::reexports::XrDestroySessionMessage>,
    mut request_exit: MessageWriter<renzora_xr::reexports::XrRequestExitMessage>,
) {
    // Handle user button presses
    if panel.start_requested {
        panel.start_requested = false;
        create_session.write_default();
        console_log(LogLevel::Info, "VR", "Starting VR session...");
    }
    if panel.stop_requested {
        panel.stop_requested = false;
        request_exit.write_default();
        console_log(LogLevel::Info, "VR", "Stopping VR session...");
    }

    // Handle OpenXR state machine transitions (everything except Available→create)
    for renzora_xr::reexports::XrStateChanged(state) in state_changed.read() {
        match state {
            renzora_xr::reexports::XrState::Available => {
                // Do NOT auto-create — user must click Start VR
            }
            renzora_xr::reexports::XrState::Ready => {
                begin_session.write_default();
            }
            renzora_xr::reexports::XrState::Stopping => {
                end_session.write_default();
            }
            renzora_xr::reexports::XrState::Exiting { .. } => {
                destroy_session.write_default();
            }
            _ => {}
        }
    }
}

// ============================================================================
// Panel sync systems — copy VR resources into editor panel state each frame
// ============================================================================

/// Sync VrConfig + VrSessionState + VrCapabilities → VrSettingsState panel
pub fn sync_vr_settings_panel(
    vr_config: Option<Res<renzora_xr::VrConfig>>,
    session_state: Option<Res<renzora_xr::resources::VrSessionState>>,
    capabilities: Option<Res<renzora_xr::VrCapabilities>>,
    mut panel: ResMut<crate::ui::VrSettingsState>,
) {
    // On first frame (or if not yet initialized), populate from VrConfig
    if !panel.initialized {
        if let Some(ref config) = vr_config {
            panel.render_scale = config.render_scale;
            panel.comfort_vignette = config.comfort_vignette;
            panel.snap_turn_angle = config.snap_turn_angle;
            panel.locomotion_mode = match config.locomotion_mode {
                renzora_xr::LocomotionMode::Teleport => 0,
                renzora_xr::LocomotionMode::Smooth => 1,
                renzora_xr::LocomotionMode::Both => 2,
            };
            panel.move_speed = config.move_speed;
            panel.hand_tracking_enabled = config.hand_tracking_enabled;
            panel.seated_mode = config.seated_mode;
            panel.locomotion_hand = match config.locomotion_hand {
                renzora_xr::VrHand::Left => 0,
                renzora_xr::VrHand::Right => 1,
            };
            panel.thumbstick_deadzone = config.thumbstick_deadzone;
            panel.passthrough_enabled = config.passthrough_enabled;
            panel.blend_mode = match config.blend_mode {
                renzora_xr::BlendMode::Opaque => 0,
                renzora_xr::BlendMode::Additive => 1,
                renzora_xr::BlendMode::AlphaBlend => 2,
            };
            panel.foveated_rendering = config.foveated_rendering;
            panel.reference_space = if config.seated_mode { "local".into() } else { "stage".into() };
            panel.initialized = true;
        }
    }

    // Always sync read-only status fields
    if let Some(ref session) = session_state {
        panel.status_text = format!("{:?}", session.status);
        panel.headset_name = session.headset_name.clone();
        panel.refresh_rate = session.refresh_rate;
        panel.available_refresh_rates = session.available_refresh_rates.clone();
        panel.left_battery = session.left_battery;
        panel.right_battery = session.right_battery;
    }

    if let Some(ref caps) = capabilities {
        panel.hand_tracking_supported = caps.hand_tracking_supported;
        panel.passthrough_supported = caps.passthrough_supported;
        panel.eye_tracking_supported = caps.eye_tracking_supported;
        panel.foveation_supported = caps.foveation_supported;
    }
}

/// When VR settings panel is dirty, write changes back to VrConfig
pub fn apply_vr_settings(
    mut vr_config: Option<ResMut<renzora_xr::VrConfig>>,
    mut session_state: Option<ResMut<renzora_xr::resources::VrSessionState>>,
    mut panel: ResMut<crate::ui::VrSettingsState>,
) {
    if !panel.dirty {
        return;
    }
    panel.dirty = false;

    // If the user changed the refresh rate dropdown, request it from the runtime
    if let Some(ref mut session) = session_state {
        if panel.selected_refresh_rate_idx < panel.available_refresh_rates.len() {
            let selected = panel.available_refresh_rates[panel.selected_refresh_rate_idx];
            // Only request if it differs from current rate
            if (selected - session.refresh_rate).abs() > 1.0 {
                session.requested_refresh_rate = selected;
            }
        }
    }

    let Some(ref mut config) = vr_config else { return };

    config.render_scale = panel.render_scale;
    config.comfort_vignette = panel.comfort_vignette;
    config.snap_turn_angle = panel.snap_turn_angle;
    config.locomotion_mode = match panel.locomotion_mode {
        1 => renzora_xr::LocomotionMode::Smooth,
        2 => renzora_xr::LocomotionMode::Both,
        _ => renzora_xr::LocomotionMode::Teleport,
    };
    config.move_speed = panel.move_speed;
    config.hand_tracking_enabled = panel.hand_tracking_enabled;
    config.seated_mode = panel.seated_mode;
    config.locomotion_hand = match panel.locomotion_hand {
        1 => renzora_xr::VrHand::Right,
        _ => renzora_xr::VrHand::Left,
    };
    config.thumbstick_deadzone = panel.thumbstick_deadzone;
    config.passthrough_enabled = panel.passthrough_enabled;
    config.blend_mode = match panel.blend_mode {
        1 => renzora_xr::BlendMode::Additive,
        2 => renzora_xr::BlendMode::AlphaBlend,
        _ => renzora_xr::BlendMode::Opaque,
    };
    config.foveated_rendering = panel.foveated_rendering;
}

/// Sync VrControllerState + VrHandTrackingState → VrInputDebugState panel
pub fn sync_vr_input_debug_panel(
    controllers: Option<Res<renzora_xr::VrControllerState>>,
    hand_tracking: Option<Res<renzora_xr::VrHandTrackingState>>,
    mut panel: ResMut<crate::ui::VrInputDebugState>,
) {
    if let Some(ref ctrl) = controllers {
        let snap_hand = |src: &renzora_xr::input::ControllerHandState| -> HandSnapshot {
            HandSnapshot {
                tracked: src.tracked,
                trigger: src.trigger,
                trigger_pressed: src.trigger_pressed,
                grip: src.grip,
                grip_pressed: src.grip_pressed,
                thumbstick_x: src.thumbstick_x,
                thumbstick_y: src.thumbstick_y,
                thumbstick_clicked: src.thumbstick_clicked,
                button_a: src.button_a,
                button_b: src.button_b,
                menu: src.menu,
                grip_position: src.grip_position,
                grip_rotation: src.grip_rotation,
                aim_position: src.aim_position,
                aim_rotation: src.aim_rotation,
                hand_tracked: false,
                pinch_strength: 0.0,
                grab_strength: 0.0,
            }
        };
        panel.left = snap_hand(&ctrl.left);
        panel.right = snap_hand(&ctrl.right);

        // Overlay hand tracking data
        if let Some(ref ht) = hand_tracking {
            panel.left.hand_tracked = ht.left.tracked;
            panel.left.pinch_strength = ht.left.pinch_strength;
            panel.left.grab_strength = ht.left.grab_strength;
            panel.right.hand_tracked = ht.right.tracked;
            panel.right.pinch_strength = ht.right.pinch_strength;
            panel.right.grab_strength = ht.right.grab_strength;
        }

        // Track grip position history for motion trail
        if panel.position_history.len() >= 60 {
            panel.position_history.pop_front();
        }
        panel.position_history.push_back([ctrl.left.grip_position, ctrl.right.grip_position]);
    }
}

/// Sync VrSessionState + VrCapabilities → VrSessionPanelState panel
pub fn sync_vr_session_panel(
    session_state: Option<Res<renzora_xr::resources::VrSessionState>>,
    capabilities: Option<Res<renzora_xr::VrCapabilities>>,
    vr_config: Option<Res<renzora_xr::VrConfig>>,
    time: Res<Time>,
    mut panel: ResMut<crate::ui::VrSessionPanelState>,
    mut frame_count: Local<u32>,
    mut fps_timer: Local<f32>,
    mut fps_frames: Local<u32>,
) {
    if let Some(ref session) = session_state {
        panel.status = session.status.as_str().to_string();
        panel.headset_name = session.headset_name.clone();
        panel.runtime_name = session.headset_name.clone(); // Runtime name from properties
        panel.target_fps = session.refresh_rate;
        panel.should_render = session.status.is_active();
    }

    if let Some(ref caps) = capabilities {
        panel.hand_tracking = caps.hand_tracking_supported;
        panel.passthrough = caps.passthrough_supported;
        panel.eye_tracking = caps.eye_tracking_supported;
        panel.foveation = caps.foveation_supported;
        panel.overlay = caps.overlay_supported;
        panel.spatial_anchors = caps.spatial_anchors_supported;
    }

    if let Some(ref config) = vr_config {
        panel.reference_space = if config.seated_mode { "local".into() } else { "stage".into() };
    }

    // Calculate actual FPS (sample over 1 second)
    *fps_frames += 1;
    *fps_timer += time.delta_secs();
    *frame_count += 1;
    if *fps_timer >= 1.0 {
        panel.actual_fps = *fps_frames as f32 / *fps_timer;
        *fps_timer = 0.0;
        *fps_frames = 0;
    }
}

/// Sync frame timing → VrPerformanceState panel
pub fn sync_vr_performance_panel(
    session_state: Option<Res<renzora_xr::resources::VrSessionState>>,
    vr_config: Option<Res<renzora_xr::VrConfig>>,
    time: Res<Time>,
    mut panel: ResMut<crate::ui::VrPerformanceState>,
) {
    if let Some(ref session) = session_state {
        panel.target_framerate = session.refresh_rate;
    }

    if let Some(ref config) = vr_config {
        panel.render_scale = config.render_scale;
        panel.foveation_active = config.foveated_rendering;
    }

    // Track frame times
    let frame_ms = time.delta_secs() * 1000.0;
    if panel.frame_time_history.len() >= 120 {
        panel.frame_time_history.pop_front();
    }
    panel.frame_time_history.push_back(frame_ms);
    panel.total_frames += 1;

    // Count dropped frames (over budget)
    let budget_ms = if panel.target_framerate > 0.0 { 1000.0 / panel.target_framerate } else { 11.1 };
    if frame_ms > budget_ms * 1.1 {
        panel.dropped_frames += 1;
    }
}

/// Sync VrControllerState + VrSessionState → VrDevicesState panel
pub fn sync_vr_devices_panel(
    controllers: Option<Res<renzora_xr::VrControllerState>>,
    session_state: Option<Res<renzora_xr::resources::VrSessionState>>,
    mut panel: ResMut<crate::ui::VrDevicesState>,
) {
    use crate::ui::TrackingQuality;

    if let Some(ref session) = session_state {
        panel.headset.name = session.headset_name.clone();
        panel.headset.connected = session.status.is_active()
            || matches!(session.status, renzora_xr::resources::VrStatus::Ready | renzora_xr::resources::VrStatus::Initializing);
        panel.headset.tracked = session.status.is_active();
        panel.headset.tracking_quality = if session.status == renzora_xr::resources::VrStatus::Focused {
            TrackingQuality::Good
        } else if session.status == renzora_xr::resources::VrStatus::Visible {
            TrackingQuality::Degraded
        } else if panel.headset.connected {
            TrackingQuality::Unknown
        } else {
            TrackingQuality::Lost
        };
        // Battery from controller states (headset battery not exposed by OpenXR directly)
        panel.headset.battery = -1.0;
    }

    if let Some(ref ctrl) = controllers {
        // Left controller
        panel.left_controller.name = "Left Controller".to_string();
        panel.left_controller.connected = ctrl.left.tracked;
        panel.left_controller.tracked = ctrl.left.tracked;
        panel.left_controller.tracking_quality = if ctrl.left.tracked {
            TrackingQuality::Good
        } else {
            TrackingQuality::Lost
        };

        // Right controller
        panel.right_controller.name = "Right Controller".to_string();
        panel.right_controller.connected = ctrl.right.tracked;
        panel.right_controller.tracked = ctrl.right.tracked;
        panel.right_controller.tracking_quality = if ctrl.right.tracked {
            TrackingQuality::Good
        } else {
            TrackingQuality::Lost
        };

        // Battery from session state
        if let Some(ref session) = session_state {
            panel.left_controller.battery = session.left_battery;
            panel.right_controller.battery = session.right_battery;
        }
    }
}

/// Sync VR state → VrSetupWizardState live data
pub fn sync_vr_setup_wizard(
    session_state: Option<Res<renzora_xr::resources::VrSessionState>>,
    controllers: Option<Res<renzora_xr::VrControllerState>>,
    mut panel: ResMut<crate::ui::VrSetupWizardState>,
) {
    if let Some(ref session) = session_state {
        // Runtime is detected if we have a headset name (OpenXR is running)
        panel.runtime_detected = !session.headset_name.is_empty()
            || session.status != renzora_xr::resources::VrStatus::Disconnected;
        panel.runtime_name = if session.headset_name.is_empty() {
            "OpenXR".to_string()
        } else {
            session.headset_name.clone()
        };
        panel.headset_connected = session.status.is_active()
            || matches!(session.status, renzora_xr::resources::VrStatus::Ready | renzora_xr::resources::VrStatus::Initializing);
        panel.headset_name = session.headset_name.clone();
        panel.session_focused = session.status == renzora_xr::resources::VrStatus::Focused;
        panel.refresh_rate = session.refresh_rate;
    }

    if let Some(ref ctrl) = controllers {
        panel.left_tracked = ctrl.left.tracked;
        panel.right_tracked = ctrl.right.tracked;
        // Use grip position for head tracking test visualization
        // (head position would need XrPrimaryReferenceSpace, approximate with controller midpoint)
    }
}

// ============================================================================
// VR Editor Mode — in-headset floating panel rendering
// ============================================================================

/// System: inject VR pointer events into `EguiInput` on VR panel context entities.
///
/// This MUST run during `Update` (after `vr_panel_interaction` writes `VrPanelInput`)
/// so that by the time bevy_egui's `run_egui_context_pass_loop_system` runs in
/// `PostUpdate::EguiPostUpdateSet::EndPass`, the `EguiInput` component already
/// contains pointer events. This is the only correct injection point — the
/// multipass context loop calls `ctx.run(input.take(), ...)` which begins the
/// egui frame with the raw input BEFORE our schedule systems run.
pub fn inject_vr_panel_egui_input(
    panel_input: Res<renzora_vr_editor::VrPanelInput>,
    panels: Query<&renzora_vr_editor::VrPanel>,
    mut egui_inputs: Query<(Entity, &mut EguiInput)>,
    time: Res<Time>,
) {
    use bevy_egui::egui;

    // Collect VR panel context entities and their dimensions
    let vr_panels: Vec<(Entity, f32, f32)> = panels
        .iter()
        .map(|p| (p.context_entity, p.width_meters * 512.0, p.height_meters * 512.0))
        .collect();

    for (entity, mut egui_input) in egui_inputs.iter_mut() {
        // Only process VR panel context entities
        let Some(&(_, px_w, px_h)) = vr_panels.iter().find(|(ctx, _, _)| *ctx == entity) else {
            continue;
        };

        // Set screen_rect so egui knows the viewport bounds — without this,
        // egui cannot determine if pointer events are within the UI area.
        egui_input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::new(px_w, px_h),
        ));
        egui_input.time = Some(time.elapsed_secs_f64());
        egui_input.focused = true;

        if panel_input.hovered_context == Some(entity) {
            let pos = egui::Pos2::new(panel_input.pointer_pos.x, panel_input.pointer_pos.y);

            // Always send pointer position for hover highlighting
            egui_input.events.push(egui::Event::PointerMoved(pos));

            // ── Right trigger = click/drag ──
            if panel_input.click_just_pressed {
                egui_input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                });
            } else if panel_input.click_just_released {
                egui_input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                });
            } else if panel_input.click_pressed {
                // Continuous hold: re-send pressed state so egui maintains
                // drag across frames (sliders, scroll, etc.)
                egui_input.events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                });
            }

            // ── Left trigger = drag-to-scroll ──
            // While left trigger is held, pointer Y movement is converted to
            // scroll events. Speed-responsive: faster pointer movement = larger scroll.
            if panel_input.scroll_active {
                let dy = panel_input.pointer_pos.y - panel_input.prev_pointer_pos.y;
                if dy.abs() > 0.5 {
                    // Dragging pointer down (dy > 0) → scroll content down (positive Y)
                    // Speed-responsive: square the delta for natural acceleration
                    let speed_factor = (dy.abs() / 5.0).clamp(0.2, 4.0);
                    let scroll_amount = dy.signum() * dy.abs() * speed_factor;
                    let trigger_scale = panel_input.scroll_trigger_value.max(0.3);
                    egui_input.events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::Vec2::new(0.0, scroll_amount * trigger_scale),
                        modifiers: Default::default(),
                    });
                }
            }
        } else {
            // Pointer not on this panel — tell egui the pointer is gone
            egui_input.events.push(egui::Event::PointerGone);
        }
    }
}

/// Marker: fonts (including phosphor icons) have been initialized on this VR panel context.
#[derive(Component)]
struct VrFontsReady;

/// Exclusive-system renderer for VR panels.
///
/// Takes `&mut World` directly so each panel type can pull only the resources
/// it needs — no hardcoded parameter list. Called from per-panel schedule
/// systems registered by `register_vr_panel_render_systems`.
fn render_vr_panel_exclusive(world: &mut World, context_entity: Entity, panel_type: &str) {
    use bevy_egui::egui;

    // Clone the egui context (Arc-based, cheap) to release the world borrow
    let ctx = {
        let mut entity = world.entity_mut(context_entity);
        let Some(mut egui_ctx) = entity.get_mut::<EguiContext>() else {
            return;
        };
        egui_ctx.get_mut().clone()
    };

    // Initialize fonts once per VR panel context — each EguiMultipassSchedule context
    // is independent and does NOT inherit font definitions from the primary context,
    // so phosphor icons (and custom fonts) would be missing without this.
    if !world.entity(context_entity).contains::<VrFontsReady>() {
        crate::ui::style::init_fonts(&ctx);
        world.entity_mut(context_entity).insert(VrFontsReady);
    }

    // NOTE: VR pointer events are injected into `EguiInput` by
    // `inject_vr_panel_egui_input` during Update, BEFORE bevy_egui's
    // multipass loop consumes them. Do NOT inject via ctx.input_mut()
    // here — it runs too late (after begin_pass has computed pointer state).

    // Apply dark visuals for VR readability
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = egui::Color32::from_rgba_unmultiplied(30, 30, 35, 240);
    visuals.panel_fill = egui::Color32::from_rgba_unmultiplied(30, 30, 35, 240);
    ctx.set_visuals(visuals);

    // Clone theme to avoid holding a world borrow across the panel render
    let theme = world.resource::<renzora_theme::ThemeManager>().active_theme.clone();

    // ── Title bar with panel name and close button ──
    // Skip for wrist_menu and toolbar which have their own compact UIs.
    let mut close_requested = false;
    if panel_type != "wrist_menu" && panel_type != "toolbar" {
        let display_name = vr_panel_display_name(panel_type);
        let bar_bg = egui::Color32::from_rgb(22, 22, 26);
        let title_frame = egui::Frame::NONE
            .fill(bar_bg)
            .inner_margin(egui::Margin::symmetric(8, 4));

        egui::TopBottomPanel::top("vr_title_bar")
            .frame(title_frame)
            .show_separator_line(false)
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(display_name)
                            .size(12.0)
                            .strong()
                            .color(egui::Color32::from_rgb(200, 200, 210)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let close_btn = ui.add(
                            egui::Button::new(
                                egui::RichText::new("\u{2715}") // ✕
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(160, 160, 170)),
                            )
                            .frame(false),
                        );
                        if close_btn.clicked() {
                            close_requested = true;
                        }
                    });
                });
            });
    }

    // Handle close request — find quad entity and queue for despawn
    if close_requested {
        let quad_entity: Option<Entity> = world
            .query::<(Entity, &renzora_vr_editor::VrPanel)>()
            .iter(world)
            .find(|(_, p)| p.context_entity == context_entity)
            .map(|(e, _)| e);
        if let Some(quad) = quad_entity {
            world
                .resource_mut::<renzora_vr_editor::panel_spawner::VrPanelMenu>()
                .pending_close = Some(quad);
        }
    }

    match panel_type {
        // ── VR-specific panels (simple: single state resource) ──
        "vr_session" => {
            let mut state = world.resource_mut::<crate::ui::VrSessionPanelState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_vr_session_content(ui, &mut state, &theme);
            });
        }
        "vr_settings" => {
            let mut state = world.resource_mut::<crate::ui::VrSettingsState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_vr_settings_content(ui, &mut state, &theme);
            });
        }
        "vr_devices" => {
            let mut state = world.resource_mut::<crate::ui::VrDevicesState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_vr_devices_content(ui, &mut state, &theme);
            });
        }
        "vr_performance" => {
            let mut state = world.resource_mut::<crate::ui::VrPerformanceState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_vr_performance_content(ui, &mut state, &theme);
            });
        }
        "vr_input_debug" => {
            let mut state = world.resource_mut::<crate::ui::VrInputDebugState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_vr_input_debug_content(ui, &mut state, &theme);
            });
        }
        "vr_setup_wizard" => {
            let mut state = world.resource_mut::<crate::ui::VrSetupWizardState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_vr_setup_wizard_content(ui, &mut state, &theme);
            });
        }

        // ── Console — uses resource_scope for two resources ──
        "console" => {
            world.resource_scope(|world, mut console: Mut<crate::core::ConsoleState>| {
                let rhai = world.resource::<crate::scripting::RhaiScriptEngine>();
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_console_content(ui, &mut console, &theme, rhai, None);
                });
            });
        }

        // ── Desktop panels available in VR (simple: single state resource) ──
        "performance" => {
            let diagnostics = world.resource::<crate::core::DiagnosticsState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_performance_content(ui, diagnostics, &theme);
            });
        }
        "ecs_stats" => {
            let mut state = world.resource_mut::<crate::core::EcsStatsState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_ecs_stats_content(ui, &mut state, &theme);
            });
        }
        "render_stats" => {
            let stats = world.resource::<crate::core::RenderStats>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_render_stats_content(ui, stats, &theme);
            });
        }
        "mixer" => {
            let mut mixer = world.resource_mut::<crate::audio::MixerState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_mixer_content(ui, &mut mixer, &theme);
            });
        }
        "physics_debug" => {
            let mut state = world.resource_mut::<crate::core::PhysicsDebugState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_physics_debug_content(ui, &mut state, &theme);
            });
        }
        "camera_debug" => {
            let mut state = world.resource_mut::<crate::core::CameraDebugState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_camera_debug_content(ui, &mut state, &theme);
            });
        }
        "shape_library" => {
            let mut state = world.resource_mut::<crate::ui::ShapeLibraryState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_shape_library_content(ui, &mut state, &theme);
            });
        }
        "history" => {
            let mut history = world.resource_mut::<crate::commands::CommandHistory>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_history_content(ui, &mut history);
            });
        }

        // ── Hierarchy — needs queries + multiple resources via SystemState ──
        "hierarchy" => {
            use bevy::ecs::system::SystemState;

            let mut sys = SystemState::<(
                crate::ui::HierarchyQueries,
                Commands,
                ResMut<Assets<Mesh>>,
                ResMut<Assets<StandardMaterial>>,
                ResMut<crate::core::SelectionState>,
                ResMut<crate::core::HierarchyState>,
                Res<crate::component_system::ComponentRegistry>,
                Res<crate::plugin_core::PluginHost>,
                ResMut<crate::core::AssetBrowserState>,
                Res<crate::core::DefaultCameraEntity>,
                ResMut<crate::commands::CommandHistory>,
            )>::new(world);

            {
                let (hq, mut cmds, mut meshes, mut mats, mut sel, mut hier,
                     reg, plugins, mut assets, default_cam, mut cmd_hist) = sys.get_mut(world);

                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_hierarchy_content(
                        ui, &ctx, &mut sel, &mut hier, &hq, &mut cmds,
                        &mut meshes, &mut mats, &reg, 0, &plugins, &mut assets,
                        false, false, false, &default_cam, &mut cmd_hist, &theme,
                    );
                });
            }

            sys.apply(world);
        }

        // ── Inspector — exclusive world access with nested resource_scope ──
        "inspector" => {
            use crate::component_system::AddComponentPopupState;
            use crate::ui_api::renderer::UiRenderer;

            // Bridge VR drag state: copy dragging_asset into InspectorPanelRenderState
            // so the inspector can detect asset drops via egui pointer release.
            {
                let dragging = world.resource::<crate::core::AssetBrowserState>().dragging_asset.clone();
                world.resource_mut::<crate::core::InspectorPanelRenderState>().dragging_asset_path = dragging;
            }

            world.resource_scope(|world, mut add_popup: Mut<AddComponentPopupState>| {
                world.resource_scope(|world, mut assets: Mut<crate::core::AssetBrowserState>| {
                    world.resource_scope(|world, mut thumbs: Mut<crate::core::ThumbnailCache>| {
                        let mut ui_renderer = UiRenderer::default();

                        let selection = {
                            let s = world.resource::<crate::core::SelectionState>();
                            crate::core::SelectionState {
                                selected_entity: s.selected_entity,
                                multi_selection: s.multi_selection.clone(),
                                context_menu_entity: s.context_menu_entity,
                                selection_anchor: s.selection_anchor,
                            }
                        };

                        let (_events, scene_changed) = egui::CentralPanel::default()
                            .show(&ctx, |ui| {
                                crate::ui::render_inspector_content_world(
                                    ui,
                                    world,
                                    &selection,
                                    None,
                                    &mut ui_renderer,
                                    &mut add_popup,
                                    &mut assets,
                                    &mut thumbs,
                                    &theme,
                                )
                            })
                            .inner;

                        if scene_changed {
                            world.resource_mut::<crate::core::SceneManagerState>().mark_modified();
                        }

                        // If inspector consumed the drag, clear the source
                        if world.resource::<crate::core::InspectorPanelRenderState>().drag_accepted {
                            assets.dragging_asset = None;
                            world.resource_mut::<crate::core::InspectorPanelRenderState>().drag_accepted = false;
                        }
                    });
                });
            });
        }

        // ── Blueprint — needs multiple resources via SystemState ──
        "blueprint" => {
            use bevy::ecs::system::SystemState;

            let mut sys = SystemState::<(
                ResMut<crate::blueprint::BlueprintEditorState>,
                ResMut<crate::blueprint::BlueprintCanvasState>,
                Res<crate::blueprint::nodes::NodeRegistry>,
                Option<Res<crate::project::CurrentProject>>,
                ResMut<crate::core::AssetBrowserState>,
                ResMut<crate::core::ThumbnailCache>,
                ResMut<crate::core::SceneManagerState>,
            )>::new(world);

            {
                let (mut editor, mut canvas, nodes, project, mut assets, mut thumbs, mut scene) =
                    sys.get_mut(world);

                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_blueprint_panel(
                        ui, &ctx, &mut editor, &mut canvas, &nodes,
                        project.as_deref(), &mut assets, &mut thumbs, &mut scene,
                    );
                });
            }

            sys.apply(world);
        }

        // ── Node Library — 3 resources ──
        "node_library" => {
            use bevy::ecs::system::SystemState;

            let mut sys = SystemState::<(
                ResMut<crate::blueprint::BlueprintEditorState>,
                Res<crate::blueprint::BlueprintCanvasState>,
                Res<crate::blueprint::nodes::NodeRegistry>,
            )>::new(world);

            {
                let (mut editor, canvas, nodes) = sys.get_mut(world);

                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_node_library_panel(
                        ui, &mut editor, &canvas, &nodes,
                    );
                });
            }

            sys.apply(world);
        }

        // ── Assets — multiple resources via SystemState ──
        "assets" => {
            use bevy::ecs::system::SystemState;

            let mut sys = SystemState::<(
                Option<Res<crate::project::CurrentProject>>,
                ResMut<crate::core::AssetBrowserState>,
                ResMut<crate::core::SceneManagerState>,
                ResMut<crate::core::ThumbnailCache>,
                ResMut<crate::viewport::ModelPreviewCache>,
                ResMut<crate::shader_thumbnail::ShaderThumbnailCache>,
            )>::new(world);

            {
                let (project, mut assets, mut scene, mut thumbs, mut model_prev, mut shader_prev) =
                    sys.get_mut(world);

                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_assets_content(
                        ui, project.as_deref(), &mut assets, &mut scene,
                        &mut thumbs, &mut model_prev, &mut shader_prev, &theme,
                    );
                });
            }

            sys.apply(world);
        }

        // ── VR Toolbar — tool mode selection ──
        "toolbar" => {
            let mut gizmo = world.resource_mut::<crate::gizmo::GizmoState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                render_vr_toolbar(ui, &mut gizmo, &theme);
            });
        }

        // ── VR Wrist Menu — panel spawner ──
        "wrist_menu" => {
            let mut menu = world.resource_mut::<renzora_vr_editor::panel_spawner::VrPanelMenu>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                render_vr_wrist_menu(ui, &mut menu, &theme);
            });
        }

        // ── Physics panels (simple: single state resource) ──
        "physics_properties" => {
            let mut state = world.resource_mut::<crate::core::PhysicsPropertiesState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_physics_properties_content(ui, &mut state, &theme);
            });
        }
        "physics_forces" => {
            let mut state = world.resource_mut::<crate::core::PhysicsForcesState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_physics_forces_content(ui, &mut state, &theme);
            });
        }
        "physics_metrics" => {
            let mut state = world.resource_mut::<crate::core::PhysicsMetricsState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_physics_metrics_content(ui, &mut state, &theme);
            });
        }
        "physics_playground" => {
            let mut state = world.resource_mut::<crate::core::PlaygroundState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_physics_playground_content(ui, &mut state, &theme);
            });
        }
        "physics_scenarios" => {
            let mut state = world.resource_mut::<crate::core::PhysicsScenariosState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_physics_scenarios_content(ui, &mut state, &theme);
            });
        }
        "collision_viz" => {
            let mut state = world.resource_mut::<crate::core::CollisionVizState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_collision_viz_content(ui, &mut state, &theme);
            });
        }

        // ── Debug / diagnostics panels ──
        "culling_debug" => {
            let mut state = world.resource_mut::<crate::core::CullingDebugState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_culling_debug_content(ui, &mut state, &theme);
            });
        }
        "movement_trails" => {
            let mut state = world.resource_mut::<crate::core::MovementTrailsState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_movement_trails_content(ui, &mut state, &theme);
            });
        }
        "state_recorder" => {
            let mut state = world.resource_mut::<crate::core::StateRecorderState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_state_recorder_content(ui, &mut state, &theme);
            });
        }
        "stress_test" => {
            let mut state = world.resource_mut::<crate::core::StressTestState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_stress_test_content(ui, &mut state, &theme);
            });
        }
        "memory_profiler" => {
            let mut state = world.resource_mut::<crate::core::MemoryProfilerState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_memory_profiler_content(ui, &mut state, &theme);
            });
        }
        "arena_presets" => {
            let mut state = world.resource_mut::<crate::core::ArenaPresetsState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_arena_presets_content(ui, &mut state, &theme);
            });
        }
        "gamepad" => {
            let mut state = world.resource_mut::<crate::core::GamepadDebugState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_gamepad_content(ui, &mut state, &theme);
            });
        }

        // ── System profiler — two read-only resources ──
        "system_profiler" => {
            let diagnostics = world.resource::<crate::core::DiagnosticsState>();
            let timing = world.resource::<crate::core::SystemTimingState>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_system_profiler_content(ui, diagnostics, timing, &theme);
            });
        }

        // ── Render pipeline — mutable graph + read-only stats ──
        "render_pipeline" => {
            use bevy::ecs::system::SystemState;
            let mut sys = SystemState::<(
                ResMut<crate::core::RenderPipelineGraphData>,
                Res<crate::core::RenderStats>,
            )>::new(world);
            {
                let (mut graph, stats) = sys.get_mut(world);
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_render_pipeline_content(ui, &mut graph, &stats, &theme);
                });
            }
            sys.apply(world);
        }

        // ── Animation panel — needs queries via SystemState ──
        "animation" => {
            use bevy::ecs::system::SystemState;
            use crate::component_system::data::components::animation::{AnimationData, GltfAnimations};
            let mut sys = SystemState::<(
                Res<crate::core::SelectionState>,
                Query<&mut GltfAnimations>,
                Query<&AnimationData>,
            )>::new(world);
            {
                let (sel, mut gltf_q, anim_q) = sys.get_mut(world);
                let mut panel_state = crate::ui::AnimationPanelState::default();
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_animation_content(
                        ui, &sel, &mut gltf_q, &anim_q, &mut panel_state, &theme,
                    );
                });
            }
            sys.apply(world);
        }

        // ── Timeline — needs queries via SystemState ──
        "timeline" => {
            use bevy::ecs::system::SystemState;
            use crate::component_system::data::components::animation::{AnimationData, GltfAnimations};
            let mut sys = SystemState::<(
                ResMut<crate::core::AnimationTimelineState>,
                Res<crate::core::SelectionState>,
                Query<&mut GltfAnimations>,
                Query<&AnimationData>,
            )>::new(world);
            {
                let (mut timeline, sel, gltf_q, anim_q) = sys.get_mut(world);
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_timeline_content(
                        ui, &mut timeline, &sel, &gltf_q, &anim_q, &theme,
                    );
                });
            }
            sys.apply(world);
        }

        // ── Script variables — read-only resources ──
        "script_variables" => {
            let scene = world.resource::<crate::core::SceneManagerState>();
            let rhai = world.resource::<crate::scripting::RhaiScriptEngine>();
            let project = world.get_resource::<crate::project::CurrentProject>();
            egui::CentralPanel::default().show(&ctx, |ui| {
                crate::ui::render_script_variables_content(ui, scene, rhai, project);
            });
        }

        // ── Code editor — needs mutable SceneManagerState + optional project ──
        "code_editor" => {
            use bevy::ecs::system::SystemState;
            let mut sys = SystemState::<(
                ResMut<crate::core::SceneManagerState>,
                Option<Res<crate::project::CurrentProject>>,
            )>::new(world);
            {
                let (mut scene, project) = sys.get_mut(world);
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_code_editor_content(
                        ui, &ctx, &mut scene, project.as_deref(),
                    );
                });
            }
            sys.apply(world);
        }

        // ── Particle editor — 3 mutable resources ──
        "particle_editor" => {
            use bevy::ecs::system::SystemState;
            let mut sys = SystemState::<(
                ResMut<crate::particles::ParticleEditorState>,
                ResMut<crate::core::SceneManagerState>,
                ResMut<crate::core::AssetBrowserState>,
            )>::new(world);
            {
                let (mut editor, mut scene, mut assets) = sys.get_mut(world);
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_particle_editor_content(
                        ui, &mut editor, &mut scene, &mut assets, &theme,
                    );
                });
            }
            sys.apply(world);
        }

        // ── Level tools — multiple resources via SystemState ──
        "level_tools" => {
            use bevy::ecs::system::SystemState;
            let mut sys = SystemState::<(
                ResMut<crate::gizmo::GizmoState>,
                ResMut<crate::brushes::BrushSettings>,
                Res<crate::brushes::BlockEditState>,
                ResMut<crate::terrain::TerrainSettings>,
                ResMut<crate::surface_painting::SurfacePaintSettings>,
                ResMut<crate::surface_painting::SurfacePaintState>,
                ResMut<crate::core::AssetBrowserState>,
            )>::new(world);
            {
                let (mut gizmo, mut brush, block_edit, mut terrain,
                     mut paint_settings, mut paint_state, mut assets) = sys.get_mut(world);
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_level_tools_content(
                        ui, &mut gizmo, &mut brush, &block_edit, &mut terrain,
                        &mut paint_settings, &mut paint_state, &mut assets, &theme,
                    );
                });
            }
            sys.apply(world);
        }

        // ── Settings — many resources via SystemState ──
        "settings" => {
            use bevy::ecs::system::SystemState;
            let mut sys = SystemState::<(
                ResMut<crate::core::EditorSettings>,
                ResMut<crate::core::KeyBindings>,
                ResMut<renzora_theme::ThemeManager>,
                ResMut<crate::project::AppConfig>,
                ResMut<crate::update::UpdateState>,
                ResMut<crate::update::UpdateDialogState>,
                ResMut<crate::core::SceneManagerState>,
                ResMut<crate::plugin_core::PluginHost>,
                ResMut<crate::locale::LocaleResource>,
            )>::new(world);
            {
                let (mut settings, mut keybinds, mut theme_mgr, mut config,
                     mut update, mut dialog, mut scene, mut plugins, mut locale) = sys.get_mut(world);
                egui::CentralPanel::default().show(&ctx, |ui| {
                    crate::ui::render_settings_content(
                        ui, &ctx, &mut settings, &mut keybinds, &mut theme_mgr,
                        &mut config, &mut update, &mut dialog, &mut scene,
                        &mut plugins, &mut locale,
                    );
                });
            }
            sys.apply(world);
        }

        // ── Wildcard fallback ──
        _ => {
            egui::CentralPanel::default().show(&ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("Panel '{}' not yet available in VR", panel_type));
                });
            });
        }
    }
}

/// Map panel type ID to a human-readable display name for the title bar.
fn vr_panel_display_name(panel_type: &str) -> &str {
    match panel_type {
        // Core editor
        "hierarchy" => "Hierarchy",
        "inspector" => "Inspector",
        "console" => "Console",
        "assets" => "Assets",
        "history" => "History",
        "settings" => "Settings",
        // Blueprint
        "blueprint" => "Blueprint",
        "node_library" => "Node Library",
        // Animation
        "animation" => "Animation",
        "timeline" => "Timeline",
        // Scripting
        "code_editor" => "Code Editor",
        "script_variables" => "Script Variables",
        // Audio
        "mixer" => "Audio Mixer",
        // Level tools
        "level_tools" => "Level Tools",
        "shape_library" => "Shape Library",
        "particle_editor" => "Particle Editor",
        // Physics
        "physics_debug" => "Physics Debug",
        "physics_properties" => "Physics Properties",
        "physics_forces" => "Physics Forces",
        "physics_metrics" => "Physics Metrics",
        "physics_playground" => "Physics Playground",
        "physics_scenarios" => "Physics Scenarios",
        "collision_viz" => "Collision Viz",
        // Performance / debug
        "performance" => "Performance",
        "ecs_stats" => "ECS Stats",
        "render_stats" => "Render Stats",
        "render_pipeline" => "Render Pipeline",
        "system_profiler" => "System Profiler",
        "memory_profiler" => "Memory Profiler",
        "camera_debug" => "Camera Debug",
        "culling_debug" => "Culling Debug",
        "stress_test" => "Stress Test",
        "movement_trails" => "Movement Trails",
        "state_recorder" => "State Recorder",
        "arena_presets" => "Arena Presets",
        "gamepad" => "Gamepad",
        // VR-specific
        "vr_session" => "VR Session",
        "vr_settings" => "VR Settings",
        "vr_devices" => "VR Devices",
        "vr_performance" => "VR Performance",
        "vr_input_debug" => "Input Debug",
        "vr_setup_wizard" => "VR Setup",
        other => other,
    }
}

/// Render the VR toolbar: Select, Move, Rotate, Scale buttons.
fn render_vr_toolbar(
    ui: &mut bevy_egui::egui::Ui,
    gizmo: &mut crate::gizmo::GizmoState,
    theme: &renzora_theme::Theme,
) {
    use bevy_egui::egui;
    use crate::gizmo::{EditorTool, GizmoMode};

    let accent = theme.semantic.accent.to_color32();
    let inactive = egui::Color32::from_rgb(60, 60, 65);
    let text_color = theme.text.primary.to_color32();

    ui.vertical_centered(|ui| {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Tools").size(14.0).color(text_color));
        ui.add_space(6.0);

        let buttons: &[(&str, EditorTool, Option<GizmoMode>)] = &[
            ("Select", EditorTool::Select, None),
            ("Move", EditorTool::Transform, Some(GizmoMode::Translate)),
            ("Rotate", EditorTool::Transform, Some(GizmoMode::Rotate)),
            ("Scale", EditorTool::Transform, Some(GizmoMode::Scale)),
        ];

        for (label, tool, mode) in buttons {
            let is_active = if let Some(m) = mode {
                gizmo.tool == *tool && gizmo.mode == *m
            } else {
                gizmo.tool == *tool
            };

            let fill = if is_active { accent } else { inactive };
            let btn = ui.add_sized(
                [120.0, 32.0],
                egui::Button::new(
                    egui::RichText::new(*label).size(14.0).color(egui::Color32::WHITE),
                )
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(4)),
            );

            if btn.clicked() {
                gizmo.tool = *tool;
                if let Some(m) = mode {
                    gizmo.mode = *m;
                }
            }

            ui.add_space(3.0);
        }
    });
}

/// Render the VR wrist menu: list of available panels to spawn.
fn render_vr_wrist_menu(
    ui: &mut bevy_egui::egui::Ui,
    menu: &mut renzora_vr_editor::panel_spawner::VrPanelMenu,
    theme: &renzora_theme::Theme,
) {
    use bevy_egui::egui;

    let text_color = theme.text.primary.to_color32();
    let btn_bg = egui::Color32::from_rgb(50, 55, 60);

    ui.vertical_centered(|ui| {
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Add Panel").size(14.0).color(text_color));
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for &(type_id, display_name) in renzora_vr_editor::panel_spawner::AVAILABLE_PANELS {
                    let btn = ui.add_sized(
                        [140.0, 26.0],
                        egui::Button::new(
                            egui::RichText::new(display_name).size(12.0).color(egui::Color32::WHITE),
                        )
                        .fill(btn_bg)
                        .corner_radius(egui::CornerRadius::same(3)),
                    );
                    if btn.clicked() {
                        menu.pending_spawn = Some(type_id.to_string());
                    }
                    ui.add_space(2.0);
                }
            });
    });
}

/// Exclusive system (Update): detect newly-spawned VR panels and register a
/// render system into each panel's unique schedule. Handles both initial
/// panels (from Startup) and dynamically-spawned panels (from the wrist menu).
pub fn register_vr_panel_render_systems(world: &mut World) {
    use bevy::ecs::schedule::Schedule;
    use renzora_vr_editor::{VrPanelRegistered, VrPanelPass};

    let new_panels: Vec<(Entity, Entity, String, u32)> = world
        .query_filtered::<(Entity, &renzora_vr_editor::VrPanel), Without<VrPanelRegistered>>()
        .iter(world)
        .map(|(e, p)| (e, p.context_entity, p.panel_type.clone(), p.schedule_id))
        .collect();

    if new_panels.is_empty() {
        return;
    }

    for &(_, context_entity, ref panel_type, schedule_id) in &new_panels {
        let panel_type = panel_type.clone();
        // Ensure the schedule exists — bevy_egui creates it lazily on first
        // pass loop tick, but our system may run before that happens.
        let label = VrPanelPass(schedule_id);
        if world.try_schedule_scope(label.clone(), |_, _| {}).is_err() {
            world.add_schedule(Schedule::new(label.clone()));
        }
        world.schedule_scope(label, |_, schedule| {
            schedule.add_systems(
                move |world: &mut World| {
                    render_vr_panel_exclusive(world, context_entity, &panel_type);
                },
            );
        });
    }

    // Mark panels as registered so we don't re-register on the next frame
    for (quad_entity, _, _, _) in new_panels {
        world.entity_mut(quad_entity).insert(VrPanelRegistered);
    }
}

/// Desktop companion UI: shows "VR Editor Active" with a "Stop VR" button
/// when `VrCompanionMode` is active, or a "Start VR" button in the status
/// bar area when VR hardware is detected but not active.
pub fn vr_companion_ui(
    mut contexts: bevy_egui::EguiContexts,
    mut commands: Commands,
    app_state: Res<bevy::prelude::State<crate::core::AppState>>,
    session_state: Res<renzora_xr::resources::VrSessionState>,
    console: Res<crate::core::ConsoleState>,
    theme_manager: Res<renzora_theme::ThemeManager>,
    companion_mode: Option<Res<crate::ui::VrCompanionMode>>,
) {
    use bevy_egui::egui;

    if *app_state.get() != crate::core::AppState::Editor {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else { return };
    let theme = &theme_manager.active_theme;

    if companion_mode.is_some() {
        // ── Full-screen VR companion view ──
        let bg = theme.surfaces.window.to_color32();
        let text_color = theme.text.primary.to_color32();
        let dim_color = theme.text.secondary.to_color32();
        let accent = theme.semantic.accent.to_color32();

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(bg))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.30);

                    ui.label(
                        egui::RichText::new("VR Editor Active")
                            .size(32.0)
                            .color(text_color),
                    );

                    ui.add_space(12.0);

                    let status_text = format!(
                        "Session: {}",
                        match session_state.status {
                            renzora_xr::resources::VrStatus::Disconnected => "Disconnected",
                            renzora_xr::resources::VrStatus::Initializing => "Initializing...",
                            renzora_xr::resources::VrStatus::Ready => "Ready",
                            renzora_xr::resources::VrStatus::Focused => "Active",
                            renzora_xr::resources::VrStatus::Visible => "Visible",
                            renzora_xr::resources::VrStatus::Stopping => "Stopping...",
                            renzora_xr::resources::VrStatus::Stopped => "Stopped",
                        }
                    );
                    ui.label(
                        egui::RichText::new(status_text)
                            .size(16.0)
                            .color(dim_color),
                    );

                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new("Put on your headset to use the editor")
                            .size(14.0)
                            .color(dim_color),
                    );

                    ui.add_space(24.0);

                    let stop_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new("Stop VR").size(16.0),
                        )
                        .fill(accent)
                        .min_size(egui::Vec2::new(120.0, 36.0))
                        .corner_radius(egui::CornerRadius::same(6)),
                    );
                    if stop_btn.clicked() {
                        commands.remove_resource::<crate::ui::VrCompanionMode>();
                    }

                    // ── Console log ──
                    ui.add_space(24.0);
                    ui.separator();
                    ui.label(
                        egui::RichText::new("Console")
                            .size(14.0)
                            .color(dim_color),
                    );
                    ui.add_space(4.0);

                    let available = ui.available_height().max(120.0);
                    egui::ScrollArea::vertical()
                        .max_height(available)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            let entries: Vec<_> = console.entries.iter().collect();
                            let start = entries.len().saturating_sub(30);
                            for entry in &entries[start..] {
                                let color = match entry.level {
                                    crate::core::resources::console::LogLevel::Info => egui::Color32::from_rgb(140, 180, 220),
                                    crate::core::resources::console::LogLevel::Success => egui::Color32::from_rgb(100, 200, 120),
                                    crate::core::resources::console::LogLevel::Warning => egui::Color32::from_rgb(230, 180, 80),
                                    crate::core::resources::console::LogLevel::Error => egui::Color32::from_rgb(220, 80, 80),
                                };
                                let prefix = match entry.level {
                                    crate::core::resources::console::LogLevel::Info => "INFO",
                                    crate::core::resources::console::LogLevel::Success => " OK ",
                                    crate::core::resources::console::LogLevel::Warning => "WARN",
                                    crate::core::resources::console::LogLevel::Error => " ERR",
                                };
                                ui.label(
                                    egui::RichText::new(format!(
                                        "[{}] [{}] {}",
                                        prefix, entry.category, entry.message
                                    ))
                                    .size(11.0)
                                    .color(color)
                                    .family(egui::FontFamily::Monospace),
                                );
                            }

                            if entries.is_empty() {
                                ui.label(
                                    egui::RichText::new("No log entries")
                                        .size(11.0)
                                        .color(egui::Color32::GRAY)
                                        .italics(),
                                );
                            }
                        });
                });
            });
    } else {
        // ── Check if the status bar "Start VR" button was clicked ──
        let clicked = ctx.data_mut(|d| {
            let id = egui::Id::new("start_vr_clicked");
            let val = d.get_temp::<bool>(id).unwrap_or(false);
            if val {
                d.insert_temp(id, false);
            }
            val
        });
        if clicked {
            commands.insert_resource(crate::ui::VrCompanionMode);
        }
    }
}

/// System: when `VrCompanionMode` is inserted, start the XR session and
/// spawn VR editor panels.
pub fn handle_start_vr(
    companion_mode: Option<Res<crate::ui::VrCompanionMode>>,
    mut started: Local<bool>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut create_session: MessageWriter<renzora_xr::reexports::XrCreateSessionMessage>,
) {
    if companion_mode.is_some() && !*started {
        // Request XR session creation (the runtime will transition through
        // Available → Ready, then our auto_begin system sends BeginSession)
        create_session.write_default();

        renzora_vr_editor::layout::spawn_default_layout(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
            Vec3::new(0.0, 1.5, 0.0),
            1.5,
        );
        *started = true;
        console_log(LogLevel::Info, "VR Editor", "VR editor started — session created, panels spawned");
    }

    if companion_mode.is_none() && *started {
        *started = false;
    }
}

/// System: when `VrCompanionMode` is removed, stop the XR session and
/// despawn VR editor panels + controller models.
pub fn handle_stop_vr(
    companion_mode: Option<Res<crate::ui::VrCompanionMode>>,
    mut stopped: Local<bool>,
    mut commands: Commands,
    panels: Query<Entity, With<renzora_vr_editor::VrPanel>>,
    hand_bones: Query<Entity, With<renzora_vr_editor::VrHandBone>>,
    hand_palms: Query<Entity, With<renzora_vr_editor::VrHandPalm>>,
    fallbacks: Query<Entity, With<renzora_vr_editor::controller_model::VrControllerFallback>>,
    lasers: Query<Entity, With<renzora_vr_editor::controller_model::VrLaserPointer>>,
    mut model_state: ResMut<renzora_vr_editor::controller_model::ControllerModelState>,
    mut request_exit: MessageWriter<renzora_xr::reexports::XrRequestExitMessage>,
) {
    if companion_mode.is_none() && !*stopped {
        let mut count = 0;
        for entity in panels.iter() {
            commands.entity(entity).despawn();
            count += 1;
        }
        // Despawn hand models and laser pointers
        for entity in hand_bones.iter() {
            commands.entity(entity).despawn();
        }
        for entity in hand_palms.iter() {
            commands.entity(entity).despawn();
        }
        for entity in fallbacks.iter() {
            commands.entity(entity).despawn();
        }
        for entity in lasers.iter() {
            commands.entity(entity).despawn();
        }
        model_state.spawned = false;

        if count > 0 {
            // Request graceful session exit
            request_exit.write_default();
            console_log(LogLevel::Info, "VR Editor", &format!("VR editor stopped — {count} panels despawned"));
        }
        *stopped = true;
    }

    if companion_mode.is_some() && *stopped {
        *stopped = false;
    }
}

/// System: automatically begin the XR session when it reaches Ready state,
/// and handle the Stopping/Exiting lifecycle.
pub fn auto_manage_xr_session(
    mut state_changed: MessageReader<renzora_xr::reexports::XrStateChanged>,
    companion_mode: Option<Res<crate::ui::VrCompanionMode>>,
    mut begin_session: MessageWriter<renzora_xr::reexports::XrBeginSessionMessage>,
    mut end_session: MessageWriter<renzora_xr::reexports::XrEndSessionMessage>,
    mut destroy_session: MessageWriter<renzora_xr::reexports::XrDestroySessionMessage>,
) {
    for renzora_xr::reexports::XrStateChanged(state) in state_changed.read() {
        match state {
            renzora_xr::reexports::XrState::Ready => {
                if companion_mode.is_some() {
                    begin_session.write_default();
                    console_log(LogLevel::Info, "VR Editor", "XR session ready — beginning render");
                }
            }
            renzora_xr::reexports::XrState::Stopping => {
                end_session.write_default();
            }
            renzora_xr::reexports::XrState::Exiting { .. } => {
                destroy_session.write_default();
            }
            _ => {}
        }
    }
}

/// Classify an asset file path by extension into the appropriate `pending_*_drop`
/// field on `AssetBrowserState`.
fn set_pending_drop_for_asset(
    assets: &mut crate::core::AssetBrowserState,
    path: std::path::PathBuf,
    position: Vec3,
) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        // 3D models
        "glb" | "gltf" | "obj" | "fbx" | "usd" | "usdz" => {
            assets.pending_asset_drop = Some((path, position));
        }
        // Images
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "dds" | "webp" => {
            assets.pending_image_drop = Some(crate::core::PendingImageDrop {
                path,
                position,
                is_2d_mode: false,
            });
        }
        // Audio
        "wav" | "ogg" | "mp3" | "flac" => {
            assets.pending_audio_drop = Some((path, position));
        }
        // Particle effects
        "particle" => {
            assets.pending_effect_drop = Some((path, position));
        }
        // Skybox HDR/EXR
        "hdr" | "exr" => {
            assets.pending_skybox_drop = Some(path);
        }
        _ => {
            warn!("[VR Drop] Unsupported asset extension: {ext}");
        }
    }
}

/// System: handle drag-and-drop across VR panels.
///
/// Desktop drag-and-drop works because all panels share one egui context —
/// the cursor moves between panels and egui tracks the drag natively. In VR,
/// each panel has an isolated egui context so when the controller pointer
/// leaves a panel, that panel's context loses the drag. This system bridges
/// the gap by watching `AssetBrowserState.dragging_asset` and detecting
/// trigger release to set the appropriate `pending_*_drop` field.
pub fn vr_drag_drop_system(
    controllers: Res<renzora_xr::VrControllerState>,
    pointer_hit: Res<renzora_vr_editor::VrPointerHit>,
    tracking_root: Query<&GlobalTransform, With<renzora_xr::reexports::XrTrackingRoot>>,
    mut assets: ResMut<crate::core::AssetBrowserState>,
    mut drag_state: ResMut<renzora_vr_editor::VrDragState>,
    mut inspector_render: ResMut<crate::core::InspectorPanelRenderState>,
) {
    let trigger_now = controllers.right.trigger_pressed;

    // ── Detect drag start ──
    if !drag_state.active {
        if assets.dragging_asset.is_some() && trigger_now {
            drag_state.active = true;
            drag_state.kind = renzora_vr_editor::VrDragKind::Asset;
        }
    }

    // ── Bridge: copy dragging_asset into inspector render state ──
    if drag_state.active {
        if let renzora_vr_editor::VrDragKind::Asset = drag_state.kind {
            inspector_render.dragging_asset_path = assets.dragging_asset.clone();
        }
    }

    // ── Detect trigger release ──
    let trigger_released = drag_state.prev_trigger_pressed && !trigger_now;

    if drag_state.active && trigger_released {
        let on_panel = pointer_hit.right.hit_entity.is_some()
            || pointer_hit.left.hit_entity.is_some();

        match drag_state.kind {
            renzora_vr_editor::VrDragKind::Asset => {
                if !on_panel {
                    // Scene drop — raycast aim ray against ground plane (Y=0)
                    if let Some(path) = assets.dragging_asset.take() {
                        let root_tf = tracking_root.single().copied()
                            .unwrap_or(GlobalTransform::IDENTITY);
                        let aim_pos = root_tf.transform_point(controllers.right.aim_position);
                        let aim_dir = (root_tf.affine().matrix3
                            * (controllers.right.aim_rotation * Vec3::NEG_Z))
                            .normalize();

                        let ground_pos = if aim_dir.y.abs() > 1e-6 {
                            let t = -aim_pos.y / aim_dir.y;
                            if t > 0.0 && t < 100.0 {
                                aim_pos + aim_dir * t
                            } else {
                                aim_pos + aim_dir * 3.0
                            }
                        } else {
                            aim_pos + aim_dir * 3.0
                        };

                        set_pending_drop_for_asset(&mut assets, path, ground_pos);
                    }
                }
                // Panel drop: inspector reads dragging_asset_path via egui pointer release
                // Check if inspector consumed the drop
                if inspector_render.drag_accepted {
                    inspector_render.drag_accepted = false;
                }
            }
            _ => {}
        }

        // Cleanup
        assets.dragging_asset = None;
        inspector_render.dragging_asset_path = None;
        drag_state.active = false;
        drag_state.kind = renzora_vr_editor::VrDragKind::None;
    }

    // ── Update edge ──
    drag_state.prev_trigger_pressed = trigger_now;
}

/// System: copy `Skybox` and `Camera.clear_color` from the editor viewport camera
/// to XR eye cameras so the VR headset sees the same sky as the desktop editor.
pub fn sync_skybox_to_xr_cameras(
    mut commands: Commands,
    viewport_cameras: Query<
        (Option<&Skybox>, &Camera),
        (With<crate::core::ViewportCamera>, Without<renzora_xr::reexports::XrCamera>),
    >,
    xr_cameras: Query<
        (Entity, Option<&Skybox>),
        With<renzora_xr::reexports::XrCamera>,
    >,
    mut xr_cam_settings: Query<
        &mut Camera,
        With<renzora_xr::reexports::XrCamera>,
    >,
) {
    // Get the skybox and clear color from the first viewport camera
    let Some((viewport_skybox, viewport_cam)) = viewport_cameras.iter().next() else {
        return;
    };
    let viewport_clear_color = viewport_cam.clear_color.clone();

    for (xr_entity, existing_skybox) in xr_cameras.iter() {
        // Sync skybox component
        match (viewport_skybox, existing_skybox) {
            (Some(skybox), Some(existing)) => {
                if existing.image != skybox.image || existing.brightness != skybox.brightness {
                    commands.entity(xr_entity).insert(skybox.clone());
                }
            }
            (Some(skybox), None) => {
                commands.entity(xr_entity).insert(skybox.clone());
            }
            (None, Some(_)) => {
                commands.entity(xr_entity).remove::<Skybox>();
            }
            (None, None) => {}
        }

        // Sync clear color
        if let Ok(mut xr_cam) = xr_cam_settings.get_mut(xr_entity) {
            xr_cam.clear_color = viewport_clear_color.clone();
        }
    }
}

