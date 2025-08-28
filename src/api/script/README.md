# RenScript - Complete 3D Game Scripting API

RenScript is a comprehensive, modular scripting system built on **Babylon.js** that provides a complete API for 3D game development. It features 16 specialized modules covering everything from basic transforms to advanced physics, VR/AR, and visual effects.

## Table of Contents

- [Overview](#overview)
- [Basic Syntax](#basic-syntax)
- [Complete API Reference](#complete-api-reference)
- [Examples](#examples)
- [Best Practices](#best-practices)

## Overview

RenScript provides **550+ methods** across **16 specialized modules**, offering production-ready capabilities for:

- **3D Graphics** - Complete mesh creation, materials, textures, lighting
- **Animation** - Keyframe, skeletal, morph targets, procedural animation
- **Physics** - Full physics simulation with Havok/Cannon.js/Ammo.js
- **Audio** - 3D spatial audio with effects and synthesis
- **Input** - Keyboard, mouse, touch, gamepad support
- **UI/GUI** - Complete interface system with controls and layouts
- **Visual Effects** - Particle systems and post-processing
- **XR Support** - VR/AR with hand tracking and spatial anchors
- **Asset Management** - Loading and optimization of 3D assets
- **Developer Tools** - Debugging and performance monitoring

## Basic Syntax

```renscript
script MyGameObject {
  # Properties (appear in editor UI)
  props movement {
    speed: float {
      default: 2.0,
      min: 0.1,
      max: 10.0,
      description: "Movement speed"
    }
  }
  
  # Variables
  health = 100
  
  # Lifecycle methods
  start {
    log("Object initialized!")
    set_color(1.0, 0.0, 0.0)
  }
  
  update(dt) {
    rotate_by(0, dt * speed, 0)
  }
  
  destroy {
    log("Cleanup complete")
  }
}
```

## Complete API Reference

### 🔧 CoreAPI - Essential Functions (50+ methods)
*Priority: HIGHEST - Used in almost every script*

#### Transform & Movement
```renscript
# Position Control
get_position()                    # Returns [x, y, z]
set_position(x, y, z)            # Set absolute position
move_by(x, y, z)                 # Move relative to current
get_world_position()             # World space position
move_to(x, y, z)                 # Move to target over time

# Rotation Control  
get_rotation()                   # Returns [x, y, z] in radians
set_rotation(x, y, z)           # Set absolute rotation
rotate_by(x, y, z)              # Rotate relative to current
get_world_rotation()            # World space rotation
look_at(x, y, z)               # Orient towards target

# Scale Control
get_scale()                     # Returns [x, y, z]
set_scale(x, y, z)             # Set scale factors
```

#### Visibility & State
```renscript
is_visible()                    # Check visibility state
set_visible(visible)           # Show/hide object
is_enabled()                   # Check enabled state
set_enabled(enabled)           # Enable/disable object
```

#### Tagging System
```renscript
add_tag(tag)                   # Add tag to object
remove_tag(tag)                # Remove specific tag
has_tag(tag)                   # Check if tagged
get_tags()                     # Get all tags
```

#### Math Utilities
```renscript
# Basic Math
random()                       # Random 0-1
random_range(min, max)         # Random in range
clamp(value, min, max)         # Constrain value
lerp(start, end, t)           # Linear interpolation

# Vector Math
distance(x1,y1,z1, x2,y2,z2)  # Distance between points
normalize(x, y, z)             # Normalize vector
dot(x1,y1,z1, x2,y2,z2)       # Dot product
cross(x1,y1,z1, x2,y2,z2)     # Cross product

# Angle Conversion
to_radians(degrees)            # Convert degrees to radians
to_degrees(radians)            # Convert radians to degrees
```

#### Time & Logging
```renscript
get_time()                     # Current time (milliseconds)
get_delta_time()               # Frame time delta
log(message, ...)              # Console logging
```

---

### 🎨 MaterialAPI - Materials & Appearance (40+ methods)
*Priority: HIGH - Essential for visual appearance*

#### Basic Materials
```renscript
create_standard_material(name, options)    # Basic material
create_pbr_material(name, options)         # Physically-based material
create_unlit_material(name, options)       # Performance material
```

#### Advanced Materials
```renscript
create_pbr_metallic_roughness(name, opts)  # Metal/rough workflow
create_pbr_specular_glossiness(name, opts) # Spec/gloss workflow
create_background_material(name, opts)      # Skybox material
create_node_material(name, opts)           # Node-based material
create_shader_material(name, opts)         # Custom shader
```

#### Specialized Materials
```renscript
create_cell_material(name, opts)          # Toon shading
create_sky_material(name, opts)           # Sky dome
create_water_material(name, opts)         # Water simulation
create_terrain_material(name, opts)       # Terrain blending
create_grid_material(name, opts)          # Grid overlay
create_fire_material(name, opts)          # Fire effect
create_fur_material(name, opts)           # Fur/hair
create_gradient_material(name, opts)      # Color gradients
```

#### Material Properties
```renscript
set_material_property(prop, value)        # Set any material property
get_material_property(prop)               # Get property value
set_color(r, g, b, a)                    # Set diffuse color
get_color()                              # Get current color
set_alpha(alpha)                         # Set transparency
set_emissive_color(r, g, b)             # Set glow color
enable_material_transparency(material)    # Enable alpha blending
```

---

### 🎭 MeshAPI - 3D Geometry Creation (45+ methods)
*Priority: HIGH - Core 3D object creation*

#### Basic Shapes
```renscript
create_box(name, options)                # Box/cube
create_sphere(name, options)             # Sphere
create_cylinder(name, options)           # Cylinder
create_plane(name, options)              # Flat plane
create_ground(name, options)             # Terrain ground
create_capsule(name, options)            # Capsule shape
create_torus(name, options)              # Donut shape
```

#### Advanced Geometry
```renscript
create_ribbon(name, options)             # Custom ribbon mesh
create_tube(name, options)               # Tube along path
create_extrusion(name, shape, path)      # Extrude shape along path
create_lathe(name, shape, options)       # Rotate shape around axis
create_polygon(name, shape, options)     # 2D polygon mesh
create_text(name, text, options)         # 3D text mesh
```

#### Mesh Operations
```renscript
clone_mesh(mesh, name)                   # Duplicate mesh
merge_meshes(meshes, name)               # Combine meshes
union_meshes(mesh1, mesh2)               # CSG union
subtract_meshes(mesh1, mesh2)            # CSG subtraction
intersect_meshes(mesh1, mesh2)           # CSG intersection
```

#### Mesh Properties
```renscript
set_mesh_visibility(mesh, visible)       # Show/hide mesh
set_mesh_enabled(mesh, enabled)          # Enable/disable
set_mesh_pickable(mesh, pickable)        # Mouse interaction
set_mesh_cast_shadows(mesh, cast)        # Shadow casting
```

#### Instancing & Performance
```renscript
create_mesh_instance(source)             # Create instance
create_mesh_instances(source, count)     # Batch instances
create_thin_instances(source, matrices)  # High-performance instances
optimize_mesh(mesh)                      # Optimize geometry
simplify_mesh(mesh, quality)             # Reduce polygons
```

---

### 🎬 AnimationAPI - Animation System (55+ methods)  
*Priority: HIGH - Animation system*

#### Basic Animation
```renscript
create_animation(name, property, frameRate) # Create keyframe animation
play_animation(target, animations)          # Play animation
stop_animation(target)                      # Stop animation
pause_animation(target)                     # Pause animation
resume_animation(target)                    # Resume animation
```

#### Keyframe Management
```renscript
add_animation_keys(animation, keyframes)    # Add keyframes
create_vector_animation(name, property)     # Vector3 animation
create_color_animation(name, property)      # Color animation
create_quaternion_animation(name, property) # Rotation animation
```

#### Easing Functions
```renscript
create_bezier_ease(x1, y1, x2, y2)        # Bezier curve easing
create_bounce_ease(bounces, bounciness)    # Bounce effect
create_elastic_ease(oscillations, spring) # Elastic effect
create_exponential_ease(exponent)          # Exponential curve
```

#### Skeleton Animation
```renscript
create_skeleton(name, bones)               # Create bone hierarchy
play_skeleton_animation(skeleton, name)    # Play bone animation
create_animation_range(skeleton, name)     # Define anim range
get_bone_by_name(skeleton, name)           # Find specific bone
play_idle_animation(mesh)                  # Play idle preset
play_walk_animation(mesh)                  # Play walk preset
play_run_animation(mesh)                   # Play run preset
play_jump_animation(mesh)                  # Play jump preset
```

#### Morph Target Animation
```renscript
create_morph_target_manager(mesh)          # Create morph system
add_morph_target(mesh, name, positions)    # Add morph target
set_morph_target_influence(mesh, idx, val) # Control influence
animate_morph_target(mesh, idx, influence) # Animate influence
```

#### Procedural Animation
```renscript
animate_along_path(mesh, path, duration)   # Path following
animate_rotation_around_axis(mesh, axis)   # Orbital rotation
animate_scale(mesh, fromScale, toScale)    # Scale animation
animate_opacity(mesh, fromAlpha, toAlpha)  # Fade animation
```

---

### ⚡ PhysicsAPI - Physics Simulation (35+ methods)
*Priority: HIGH - Realistic object behavior*

#### Physics Engine
```renscript
enable_physics(engine, gravity)           # Initialize physics
disable_physics()                         # Disable physics
is_physics_enabled()                      # Check if active
set_gravity(x, y, z)                     # Set world gravity
```

#### Physics Impostors
```renscript
set_physics_impostor(type, options)       # Add physics body
remove_physics_impostor()                 # Remove physics
has_physics_impostor()                    # Check if has physics
set_mass(mass)                           # Set object mass
set_friction(friction)                   # Set surface friction
set_restitution(bounce)                  # Set bounciness
```

#### Forces & Movement
```renscript
apply_impulse(force, contactPoint)        # Apply instant force
apply_force(force, contactPoint)          # Apply continuous force  
set_linear_velocity(x, y, z)             # Set movement velocity
set_angular_velocity(x, y, z)            # Set rotation velocity
get_linear_velocity()                     # Get current velocity
get_angular_velocity()                    # Get rotation speed
```

#### Collision Detection
```renscript
on_collision_enter(callback)              # Collision start event
on_collision_exit(callback)               # Collision end event
physics_raycast(origin, direction, max)   # Physics ray test
```

#### Joints & Constraints
```renscript
create_physics_joint(type, options)       # Connect objects
remove_physics_joint(joint)               # Remove connection
create_distance_joint(target, distance)   # Distance constraint
create_hinge_joint(target, pivot, axis)   # Hinge constraint
```

---

### 🎮 InputAPI - Input Handling (40+ methods)
*Priority: HIGH - User interaction*

#### Keyboard Input
```renscript
is_key_pressed(key)                      # Check if key pressed this frame
is_key_down(key)                         # Check if key held down
get_pressed_keys()                       # Get all pressed keys
is_key_combo_pressed(keys)               # Check key combination
is_ctrl_pressed()                        # Check Ctrl modifier
is_shift_pressed()                       # Check Shift modifier
is_alt_pressed()                         # Check Alt modifier
```

#### Mouse Input
```renscript
is_mouse_button_pressed(button)          # Check mouse button
get_mouse_position()                     # Screen coordinates
get_mouse_normalized()                   # Normalized coordinates
get_mouse_delta()                        # Movement delta
request_pointer_lock()                   # Lock mouse cursor
exit_pointer_lock()                      # Release mouse
is_pointer_locked()                      # Check lock state
```

#### Touch Input
```renscript
get_touch_count()                        # Number of touches
get_touches()                           # All touch data
is_touching()                           # Any touch active
get_pinch_distance()                    # Pinch gesture distance
get_touch_center()                      # Center of touches
```

#### Gamepad Input
```renscript
get_gamepads()                          # All connected gamepads
is_gamepad_connected(index)             # Check if connected
is_gamepad_button_pressed(idx, button)  # Check button
get_left_stick(index)                   # Left analog stick
get_right_stick(index)                  # Right analog stick
get_left_trigger(index)                 # Left trigger value
get_right_trigger(index)                # Right trigger value
vibrate_gamepad(index, weak, strong)    # Haptic feedback
```

---

### 🖼️ TextureAPI - Texture Management (35+ methods)
*Priority: MEDIUM - Visual enhancement*

#### Basic Textures
```renscript
create_texture(url, options)             # Load texture from file
create_cube_texture(url, options)        # Skybox texture
create_dynamic_texture(name, options)    # Programmable texture
create_video_texture(url, options)       # Video as texture
```

#### Procedural Textures
```renscript
create_wood_texture(name, options)       # Wood pattern
create_cloud_texture(name, options)      # Cloud pattern
create_fire_texture(name, options)       # Fire pattern
create_grass_texture(name, options)      # Grass pattern
create_marble_texture(name, options)     # Marble pattern
create_perlin_noise_texture(name, opts)  # Perlin noise
```

#### Texture Properties
```renscript
set_texture_wrap_mode(texture, mode)     # Wrap/repeat mode
set_texture_filtering(texture, filter)   # Filter quality
set_texture_offset(texture, u, v)        # UV offset
set_texture_scale(texture, u, v)         # UV scale
set_texture_rotation(texture, angle)     # UV rotation
```

#### Dynamic Texture Drawing
```renscript
draw_text_on_texture(texture, text, opts) # Draw text
clear_dynamic_texture(texture)            # Clear content
draw_rect_on_texture(texture, x, y, w, h) # Draw rectangle
draw_circle_on_texture(texture, x, y, r)  # Draw circle
```

#### Texture Animation
```renscript
animate_texture_offset(texture, u, v, dur) # Animate UV offset
animate_texture_rotation(texture, angle)   # Animate UV rotation
```

---

### ✨ ParticleAPI - Visual Effects (30+ methods)
*Priority: MEDIUM - Visual effects*

#### Particle System Creation
```renscript
create_particle_system(name, capacity)    # Basic particle system
create_gpu_particle_system(name, cap)     # GPU-accelerated particles
```

#### Emitter Shapes
```renscript
set_box_emitter(system, min, max)         # Box-shaped emission
set_sphere_emitter(system, radius)        # Sphere emission
set_cone_emitter(system, radius, angle)   # Cone emission
set_cylinder_emitter(system, r1, r2, h)   # Cylinder emission
set_point_emitter(system)                 # Point emission
```

#### Particle Properties
```renscript
set_particle_lifetime(system, min, max)   # Particle lifespan
set_particle_size(system, min, max)       # Particle scale
set_particle_speed(system, min, max)      # Emission speed
set_particle_colors(system, start, end)   # Color over lifetime
set_emission_rate(system, rate)           # Particles per second
```

#### Forces & Effects
```renscript
set_gravity(system, x, y, z)              # Gravity effect
add_particle_force(system, force)         # Custom force
set_wind_force(system, direction, strength) # Wind effect
```

#### Preset Effects
```renscript
create_fire_particles(name, intensity)    # Fire effect
create_smoke_particles(name, density)     # Smoke effect
create_rain_particles(name, intensity)    # Rain effect
create_snow_particles(name, intensity)    # Snow effect
create_spark_particles(name, count)       # Electric sparks
create_explosion_effect(pos, intensity)   # Explosion burst
create_magic_effect(name, color, power)   # Magical effects
```

---

### 🔊 AudioAPI - 3D Audio System (40+ methods)
*Priority: MEDIUM - Audio system*

#### Sound Creation
```renscript
create_sound(name, url, options)          # Basic sound
create_3d_sound(name, url, options)       # Spatial sound
create_spatial_sound(name, url, opts)     # Advanced 3D sound
```

#### Sound Playback
```renscript
play_sound(name)                          # Play audio
stop_sound(name)                          # Stop audio
pause_sound(name)                         # Pause audio
set_sound_volume(name, volume)            # Volume control
set_sound_playback_rate(name, rate)       # Playback speed
```

#### 3D Positioning
```renscript
set_sound_position(name, x, y, z)         # Position in 3D space
attach_sound_to_mesh(sound, mesh)         # Follow object
set_sound_max_distance(name, distance)    # Hearing range
set_sound_cone(name, angle, outerAngle)   # Directional audio
```

#### Audio Effects
```renscript
set_sound_lowpass(name, frequency)        # Low-pass filter
set_sound_highpass(name, frequency)       # High-pass filter
set_sound_reverb(name, options)           # Reverb effect
```

#### Sound Synthesis
```renscript
create_tone_sound(frequency, duration)    # Generate tone
create_noise_sound(type, duration)        # Generate noise
create_beep_sound(frequency, duration)    # Simple beep
create_chord_sound(frequencies, duration) # Musical chord
```

#### Advanced Features
```renscript
create_audio_analyser(sound)              # Audio analysis
get_audio_frequency_data(analyser)        # Frequency spectrum
get_audio_level(analyser)                 # Volume level
create_music_playlist(sounds)             # Music playlist
cross_fade_sounds(sound1, sound2, time)   # Smooth transition
```

---

### 🖥️ GUIAPI - User Interface (50+ methods)
*Priority: MEDIUM - User interface*

#### UI Setup
```renscript
create_fullscreen_ui(name)                # Full-screen UI layer
create_texture_ui(name, size)             # Texture-based UI
create_mesh_ui(name, mesh)                # 3D mesh UI
```

#### Basic Controls
```renscript
create_button(name, text)                 # Interactive button
create_text_block(name, text)             # Text display
create_image(name, url)                   # Image display
create_rectangle(name)                    # Rectangle shape
```

#### Input Controls
```renscript
create_slider(name, min, max, value)      # Value slider
create_checkbox(name, checked)            # Checkbox toggle
create_input_text(name, placeholder)      # Text input field
create_color_picker(name, color)          # Color selection
```

#### Layout Controls
```renscript
create_stack_panel(name, vertical)        # Stack layout
create_grid(name, rows, cols)             # Grid layout
create_scroll_viewer(name)                # Scrollable area
create_container(name)                    # Generic container
```

#### Control Properties
```renscript
set_control_position(ctrl, x, y)          # Position control
set_control_size(ctrl, width, height)     # Size control
set_control_alignment(ctrl, h, v)         # Alignment
set_control_background(ctrl, color)       # Background color
set_control_border(ctrl, width, color)    # Border styling
```

#### Events & Animation
```renscript
on_button_click(button, callback)         # Button click event
on_slider_change(slider, callback)        # Slider value change
animate_control_property(ctrl, prop, val) # Animate control
fade_in_control(control, duration)        # Fade in effect
slide_in_control(control, direction)      # Slide in effect
```

---

### 🎥 PostProcessAPI - Visual Effects (25+ methods)
*Priority: MEDIUM - Visual enhancement*

#### Basic Post-Processing
```renscript
create_blur_post_process(name, direction)  # Blur effect
create_fxaa_post_process(name)            # Anti-aliasing
create_highlights_post_process(name)       # Highlight extraction
create_image_processing(name)              # Color processing
```

#### Advanced Effects
```renscript
create_depth_of_field_effect(name, opts)  # DOF blur
create_volumetric_light_scattering(name)  # God rays
create_motion_blur_post_process(name)      # Motion blur
create_screen_space_reflections(name)      # SSR reflections
```

#### Custom Effects
```renscript
create_custom_post_process(name, shader)  # Custom shader
create_grayscale_post_process(name)       # B&W conversion
create_sepia_post_process(name)           # Sepia tone
create_vignette_post_process(name, opts)  # Vignette effect
```

#### Rendering Pipeline
```renscript
create_default_rendering_pipeline(name)   # Standard pipeline
enable_basic_pipeline()                   # Basic presets
enable_cinematic_pipeline()               # Cinematic look
enable_retro_pixel_pipeline()             # Retro aesthetic
```

---

### 🥽 XRAPI - VR/AR Support (25+ methods)
*Priority: LOW - Extended reality*

#### XR Initialization
```renscript
initialize_xr(options)                    # Setup WebXR
enter_vr()                               # Enter VR mode
enter_ar()                               # Enter AR mode
exit_xr()                                # Exit XR mode
is_in_xr()                               # Check XR state
```

#### Controller Tracking
```renscript
get_controller_position(index)            # Controller position
get_controller_rotation(index)            # Controller rotation
get_controller_button_state(idx, btn)     # Button state
get_controller_trigger_value(idx, trigger) # Trigger pressure
```

#### Hand Tracking
```renscript
enable_hand_tracking()                    # Enable hand detection
get_hand_joint_position(hand, joint)      # Joint position
is_hand_tracked(hand)                     # Hand visibility
```

#### AR Features
```renscript
enable_plane_detection()                  # Detect surfaces
get_detected_planes()                     # Surface list
create_anchor(position, rotation)         # Spatial anchor
perform_hit_test(x, y)                   # AR hit testing
```

---

### 🔍 DebugAPI - Development Tools (35+ methods)
*Priority: LOW - Development tools*

#### Debug Visualization
```renscript
show_inspector()                          # Babylon.js inspector
show_fps()                               # FPS counter
show_frame_time()                        # Frame timing
show_memory_usage()                      # Memory monitor
show_draw_calls()                        # Render statistics
```

#### Visual Helpers
```renscript
show_axes(mesh, size)                    # XYZ axes display
show_bounding_box(mesh, color)           # Bounding box
show_wireframe(mesh, color)              # Wireframe mode
show_ray(origin, direction, length)      # Ray visualization
```

#### Gizmos
```renscript
show_position_gizmo(mesh)                # Position manipulator
show_rotation_gizmo(mesh)                # Rotation manipulator
show_scale_gizmo(mesh)                   # Scale manipulator
show_bounding_box_gizmo(mesh)            # Bounding box gizmo
```

#### Debug Information
```renscript
get_scene_info()                         # Complete scene stats
show_scene_info()                        # Display scene info
show_mesh_info(mesh)                     # Display mesh details
show_camera_info(camera)                 # Display camera info
log_mesh_hierarchy()                     # Console hierarchy dump
take_screenshot(width, height)           # Capture screen
```

---

### 📦 AssetAPI - Asset Management (25+ methods)
*Priority: MEDIUM - Content management*

#### Asset Loading
```renscript
load_mesh(name, url, options)            # Load 3D mesh
load_gltf(name, url, options)            # Load GLTF/GLB
load_fbx(name, url, options)             # Load FBX
load_obj(name, url, options)             # Load OBJ
load_stl(name, url, options)             # Load STL
```

#### Asset Manager
```renscript
create_assets_manager()                   # Batch loading
add_mesh_task(manager, name, url)         # Add mesh to batch
add_texture_task(manager, name, url)      # Add texture to batch
load_all_assets(manager)                 # Execute batch load
```

#### Asset Operations
```renscript
get_loaded_asset(name)                   # Retrieve asset
get_loaded_mesh(name)                    # Get specific mesh
get_loaded_animations(name)              # Get animations
instantiate_asset(name)                  # Create instance
clone_asset(name, newName)               # Duplicate asset
```

#### Asset Management
```renscript
dispose_asset(name)                      # Remove from memory
dispose_all_assets()                     # Clear all assets
get_asset_info(name)                     # Asset details
is_asset_loaded(name)                    # Check load status
get_total_memory_usage()                 # Memory usage
```

---

### 🔧 UtilityAPI - Helper Functions (30+ methods)
*Priority: LOW - Helper functions*

#### Advanced Math
```renscript
smooth_step(edge0, edge1, x)             # Smooth interpolation
remap(value, oldMin, oldMax, newMin, newMax) # Remap range
random_choice(array)                     # Random array element
```

#### Vector Operations
```renscript
create_vector3(x, y, z)                  # Create vector
vector_distance(v1, v2)                  # Distance between vectors
vector_lerp(v1, v2, t)                  # Vector interpolation
vector_normalize(vector)                 # Normalize vector
vector_cross(v1, v2)                    # Cross product
angle_between_vectors(v1, v2)            # Angle between vectors
```

#### Color Utilities
```renscript
create_color3(r, g, b)                   # Create color
color_lerp(color1, color2, t)            # Color interpolation
color_from_hex(hexString)                # Convert from hex
color_to_hex(color)                      # Convert to hex
color_from_hsv(h, s, v)                  # Convert from HSV
```

#### Transformation Utilities
```renscript
quaternion_from_euler(x, y, z)           # Euler to quaternion
quaternion_to_euler(quaternion)          # Quaternion to euler
transform_point(point, matrix)           # Transform coordinate
```

#### Screen & World Conversion
```renscript
world_to_screen(position)                # 3D to screen coords
screen_to_world(x, y, depth)             # Screen to 3D coords
get_screen_size()                        # Screen dimensions
```

---

### 🎯 SceneAPI - Scene Queries & Management (20+ methods)
*Priority: HIGH - Object interaction*

#### Object Finding
```renscript
find_object_by_name(name)                # Find by name
find_objects_by_tag(tag)                 # Find by tag
get_all_meshes()                         # All scene meshes
get_all_lights()                         # All scene lights
get_all_cameras()                        # All scene cameras
```

#### Spatial Queries
```renscript
raycast(origin, direction, maxDistance)   # Ray intersection
raycast_from_camera(x, y)                # Camera ray
get_objects_in_radius(pos, radius)       # Proximity search
get_objects_in_box(min, max)             # Box intersection
get_closest_object(position, tag)        # Find nearest
```

#### Object Operations
```renscript
pick_object(x, y)                        # Mouse picking
intersects_mesh(mesh1, mesh2)            # Collision test
clone_object(object, name)               # Duplicate object
dispose_object(object)                   # Remove object
```

---

## Complete Method Count by Module

| Module | Methods | Priority | Description |
|--------|---------|----------|-------------|
| **CoreAPI** | 50+ | HIGHEST | Transform, visibility, math, time, logging |
| **AnimationAPI** | 55+ | HIGH | Keyframe, skeleton, morph, procedural animation |
| **PhysicsAPI** | 35+ | HIGH | Physics simulation, forces, collision |
| **InputAPI** | 40+ | HIGH | Keyboard, mouse, touch, gamepad input |
| **MaterialAPI** | 40+ | HIGH | Materials, colors, shaders, textures |
| **MeshAPI** | 45+ | HIGH | 3D geometry creation and manipulation |
| **SceneAPI** | 20+ | HIGH | Scene queries, raycasting, object finding |
| **TextureAPI** | 35+ | MEDIUM | Texture creation, procedural, dynamic |
| **ParticleAPI** | 30+ | MEDIUM | Particle effects, emitters, forces |
| **AudioAPI** | 40+ | MEDIUM | 3D audio, spatial sound, synthesis |
| **GUIAPI** | 50+ | MEDIUM | UI controls, layouts, events |
| **PostProcessAPI** | 25+ | MEDIUM | Visual effects, post-processing |
| **AssetAPI** | 25+ | MEDIUM | Asset loading, management, optimization |
| **XRAPI** | 25+ | LOW | VR/AR support, hand tracking |
| **DebugAPI** | 35+ | LOW | Debug tools, performance monitoring |
| **UtilityAPI** | 30+ | LOW | Helper functions, utilities |

**Total: 580+ methods across 16 modules**

## Examples

### Complete Feature Demo Script
```renscript
script CompleteDemo {
  props movement {
    speed: float {
      default: 2.0,
      min: 0.1,
      max: 10.0,
      description: "Movement speed"
    }
    
    auto_rotate: boolean {
      default: true,
      description: "Enable rotation"
    }
  }
  
  props appearance {
    color_cycle: range {
      default: 1.0,
      min: 0.1,
      max: 5.0,
      description: "Color cycle speed"
    }
    
    particle_intensity: range {
      default: 0.5,
      min: 0.0,
      max: 1.0,
      description: "Particle effect intensity"
    }
  }
  
  props physics {
    enable_physics: boolean {
      default: false,
      description: "Enable physics simulation"
    }
    
    bounce_factor: range {
      default: 0.8,
      min: 0.0,
      max: 1.0,
      description: "Physics bounciness"
    }
  }
  
  # Variables
  time_offset = 0
  particle_system = null
  
  start {
    log("Complete demo script started!")
    
    # Setup materials
    set_color(1.0, 0.5, 0.2)
    set_emissive_color(0.1, 0.05, 0.02)
    
    # Setup physics if enabled
    if enable_physics {
      set_physics_impostor("box", {mass: 1, restitution: bounce_factor})
    }
    
    # Create particle effect
    particle_system = create_fire_particles("demo_fire", particle_intensity)
    attach_particles_to_mesh(particle_system, this)
    
    # Setup 3D audio
    create_3d_sound("ambient", "assets/ambient.wav", {loop: true, volume: 0.3})
    attach_sound_to_mesh("ambient", this)
    play_sound("ambient")
    
    # Add tags
    add_tag("demo")
    add_tag("interactive")
    
    # Enable debug info for this object
    show_mesh_info(this)
  }
  
  update(dt) {
    time_offset = time_offset + dt
    
    # Animated rotation
    if auto_rotate {
      rotate_by(0, dt * speed, dt * speed * 0.5)
    }
    
    # Dynamic color cycling
    time_factor = get_time() * 0.001 * color_cycle
    red = sin(time_factor) * 0.5 + 0.5
    green = sin(time_factor + 2.0) * 0.5 + 0.5
    blue = sin(time_factor + 4.0) * 0.5 + 0.5
    set_color(red, green, blue)
    
    # Floating animation
    y_offset = sin(time_factor * 2.0) * 0.5
    current_pos = get_position()
    set_position(current_pos[0], current_pos[1] + y_offset * dt, current_pos[2])
    
    # Input handling
    if is_key_pressed("Space") {
      apply_impulse([0, 10, 0], get_position())
      log("Jump!")
    }
    
    if is_key_pressed("R") {
      # Reset position
      set_position(0, 5, 0)
      log("Reset position")
    }
  }
  
  destroy {
    # Cleanup
    stop_sound("ambient")
    stop_particle_system(particle_system)
    log("Demo script destroyed")
  }
}
```

### Animation Controller
```renscript
script AnimationController {
  props character {
    walk_speed: float {
      default: 2.0,
      min: 0.5,
      max: 8.0,
      description: "Walking speed"
    }
    
    run_multiplier: range {
      default: 2.5,
      min: 1.0,
      max: 5.0,
      description: "Running speed multiplier"
    }
    
    jump_height: float {
      default: 5.0,
      min: 1.0,
      max: 15.0,
      description: "Jump force"
    }
  }
  
  # State variables
  current_animation = "idle"
  is_grounded = true
  
  start {
    # Setup physics for character
    set_physics_impostor("capsule", {mass: 1})
    
    # Load animation presets
    play_idle_animation(this)
    
    # Setup sound effects
    create_3d_sound("footsteps", "assets/footsteps.wav", {loop: true})
    create_sound("jump_sound", "assets/jump.wav")
    
    add_tag("character")
    log("Character controller initialized")
  }
  
  update(dt) {
    # Input-driven movement
    move_x = 0
    move_z = 0
    
    if is_key_down("W") { move_z = move_z + 1 }
    if is_key_down("S") { move_z = move_z - 1 }
    if is_key_down("A") { move_x = move_x - 1 }
    if is_key_down("D") { move_x = move_x + 1 }
    
    # Calculate movement
    speed_multiplier = is_key_down("Shift") ? run_multiplier : 1.0
    actual_speed = walk_speed * speed_multiplier
    
    # Apply movement
    if move_x != 0 || move_z != 0 {
      move_by(move_x * actual_speed * dt, 0, move_z * actual_speed * dt)
      
      # Animation state
      new_anim = speed_multiplier > 1.5 ? "run" : "walk"
      if current_animation != new_anim {
        if new_anim == "run" {
          play_run_animation(this)
        } else {
          play_walk_animation(this)
        }
        current_animation = new_anim
      }
      
      # Sound effects
      if !is_sound_playing("footsteps") {
        play_sound("footsteps")
      }
    } else {
      # Idle state
      if current_animation != "idle" {
        play_idle_animation(this)
        current_animation = "idle"
        stop_sound("footsteps")
      }
    }
    
    # Jump
    if is_key_pressed("Space") && is_grounded {
      apply_impulse([0, jump_height, 0], get_position())
      play_jump_animation(this)
      play_sound("jump_sound")
      is_grounded = false
    }
    
    # Ground detection (simplified)
    pos = get_position()
    if pos[1] <= 1.0 && !is_grounded {
      is_grounded = true
    }
  }
}
```

## Architecture Benefits

1. **Modular Design** - Each API module is self-contained and focused
2. **Babylon.js Foundation** - Built on industry-standard 3D engine
3. **Complete Feature Set** - 580+ methods covering all aspects of 3D development
4. **Performance Optimized** - GPU acceleration, instancing, LOD systems
5. **Developer Friendly** - Extensive debugging tools and helpers
6. **Future Ready** - VR/AR support, modern web standards

## System Requirements

- **Babylon.js** 6.0+ (core 3D engine)
- **@babylonjs/gui** (UI components)
- **@babylonjs/materials** (advanced materials)
- **@babylonjs/inspector** (debugging tools)
- **@babylonjs/post-processes** (visual effects)
- Modern browser with **WebGL 2.0** support
- Optional: **WebXR** for VR/AR features

---

*This documentation covers the complete RenScript API with all 580+ available methods across 16 specialized modules. The system provides professional-grade 3D development capabilities comparable to commercial game engines.*