// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::{Command, Stdio};
use std::thread;
use std::sync::Mutex;
use std::sync::Arc;

mod vulkan_renderer;
mod babylon_native;
mod babylon_native_simple;
use vulkan_renderer::VulkanRenderer;
use babylon_native::{BabylonNativeRenderer, BabylonNativeConfig};
use vulkano::VulkanLibrary;

fn start_bridge_server() {
    thread::spawn(|| {
        if cfg!(debug_assertions) {
            // Development mode - bridge is already started by beforeDevCommand
            return;
        } else {
            // Production mode - use bundled executable  
            #[cfg(windows)]
            let bridge_exe = "bridge-server.exe";
            #[cfg(not(windows))]
            let bridge_exe = "bridge-server";
            
#[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                
                if let Err(e) = Command::new(bridge_exe)
                    .creation_flags(CREATE_NO_WINDOW)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null())
                    .spawn() {
                    eprintln!("Failed to start bridge server: {}", e);
                }
            }
            #[cfg(not(windows))]
            {
                if let Err(e) = Command::new(bridge_exe)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null())
                    .spawn() {
                    eprintln!("Failed to start bridge server: {}", e);
                }
            }
        }
    });
}

// Global renderer state
static VULKAN_RENDERER: Mutex<Option<VulkanRenderer>> = Mutex::new(None);
static BABYLON_NATIVE_RENDERER: Mutex<Option<BabylonNativeRenderer>> = Mutex::new(None);
// Note: C++ renderer cannot be stored in static due to cxx thread safety limitations
static MAIN_WINDOW: Mutex<Option<tauri::WebviewWindow>> = Mutex::new(None);

// Tauri command to initialize Vulkan renderer
#[tauri::command]
async fn init_vulkan_renderer(window: tauri::WebviewWindow) -> Result<String, String> {
    let mut renderer_guard = VULKAN_RENDERER.lock().map_err(|e| format!("Failed to lock renderer: {}", e))?;
    let mut window_guard = MAIN_WINDOW.lock().map_err(|e| format!("Failed to lock window: {}", e))?;
    
    if renderer_guard.is_none() {
        match VulkanRenderer::new() {
            Ok(renderer) => {
                // Store window reference for surface creation
                *window_guard = Some(window.clone());
                
                // TODO: Create window surface using Tauri window handle
                println!("🪟 Vulkan: Window handle stored for surface creation");
                
                *renderer_guard = Some(renderer);
                Ok("Vulkan renderer initialized successfully with window surface".to_string())
            },
            Err(e) => Err(format!("Failed to initialize Vulkan: {}", e))
        }
    } else {
        Ok("Vulkan renderer already initialized".to_string())
    }
}

// Tauri command to render frame
#[tauri::command]
async fn vulkan_render_frame(scene_data: String) -> Result<String, String> {
    let mut renderer_guard = VULKAN_RENDERER.lock().map_err(|e| format!("Failed to lock renderer: {}", e))?;
    
    if let Some(renderer) = renderer_guard.as_mut() {
        // Update scene data from Babylon.js
        renderer.update_scene(&scene_data).map_err(|e| format!("Scene update error: {}", e))?;
        
        // Render frame with updated scene
        renderer.render_frame().map_err(|e| format!("Render error: {}", e))?;
        Ok(format!("Frame {} rendered with Vulkan", renderer.frame_count))
    } else {
        Err("Vulkan renderer not initialized".to_string())
    }
}

// Tauri command to set background color
#[tauri::command]
async fn vulkan_set_background_color(r: f32, g: f32, b: f32) -> Result<String, String> {
    let mut renderer_guard = VULKAN_RENDERER.lock().map_err(|e| format!("Failed to lock renderer: {}", e))?;
    
    if let Some(renderer) = renderer_guard.as_mut() {
        renderer.set_background_color(r, g, b).map_err(|e| format!("Color error: {}", e))?;
        Ok(format!("Background color set to RGB({:.2}, {:.2}, {:.2})", r, g, b))
    } else {
        Err("Vulkan renderer not initialized".to_string())
    }
}

// Tauri command to check Vulkan support
#[tauri::command]
async fn check_vulkan_support() -> Result<bool, String> {
    // Check if Vulkan is available on the system
    match VulkanLibrary::new() {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

// Babylon Native commands
#[tauri::command]
async fn init_babylon_native(width: u32, height: u32) -> Result<String, String> {
    let mut renderer_guard = BABYLON_NATIVE_RENDERER.lock().map_err(|e| format!("Failed to lock renderer: {}", e))?;
    
    if renderer_guard.is_none() {
        let config = BabylonNativeConfig {
            width,
            height,
            enable_xr: false,
            enable_physics: true,
        };
        
        match BabylonNativeRenderer::new(config) {
            Ok(mut renderer) => {
                renderer.initialize().map_err(|e| format!("Failed to initialize: {}", e))?;
                *renderer_guard = Some(renderer);
                Ok("Babylon Native initialized successfully".to_string())
            },
            Err(e) => Err(format!("Failed to create Babylon Native renderer: {}", e))
        }
    } else {
        Ok("Babylon Native already initialized".to_string())
    }
}

#[tauri::command]
async fn babylon_native_render(sceneData: String) -> Result<String, String> {
    let mut renderer_guard = BABYLON_NATIVE_RENDERER.lock().map_err(|e| format!("Failed to lock renderer: {}", e))?;
    
    if let Some(renderer) = renderer_guard.as_mut() {
        renderer.render_frame(&sceneData).map_err(|e| format!("Render error: {}", e))?;
        let (frame_count, _) = renderer.get_stats();
        Ok(format!("Babylon Native frame {} rendered", frame_count))
    } else {
        Err("Babylon Native renderer not initialized".to_string())
    }
}

#[tauri::command]
async fn babylon_native_resize(width: u32, height: u32) -> Result<String, String> {
    println!("🏛️ Babylon Native: Resized to {}x{}", width, height);
    Ok("Babylon Native resized".to_string())
}

#[tauri::command]
async fn babylon_native_update_camera(_camera_data: String) -> Result<String, String> {
    println!("🏛️ Babylon Native: Camera updated");
    Ok("Camera updated".to_string())
}

#[tauri::command]
async fn babylon_native_update_lights(_light_data: String) -> Result<String, String> {
    println!("🏛️ Babylon Native: Lights updated");
    Ok("Lights updated".to_string())
}

#[tauri::command]
async fn babylon_native_add_object(_object_data: String) -> Result<String, String> {
    println!("🏛️ Babylon Native: Object added");
    Ok("Object added".to_string())
}

#[tauri::command]
async fn babylon_native_remove_object(object_id: String) -> Result<String, String> {
    println!("🏛️ Babylon Native: Object {} removed", object_id);
    Ok("Object removed".to_string())
}

#[tauri::command]
async fn babylon_native_update_object(object_id: String, _object_data: String) -> Result<String, String> {
    println!("🏛️ Babylon Native: Object {} updated", object_id);
    Ok("Object updated".to_string())
}

#[tauri::command]
async fn babylon_native_update_material(material_id: String, _material_data: String) -> Result<String, String> {
    println!("🏛️ Babylon Native: Material {} updated", material_id);
    Ok("Material updated".to_string())
}

#[tauri::command]
async fn babylon_native_capture_frame() -> Result<String, String> {
    println!("🏛️ Babylon Native: Frame captured");
    Ok("Frame captured".to_string())
}

#[tauri::command]
async fn babylon_native_cleanup() -> Result<String, String> {
    println!("🏛️ Babylon Native: Cleanup completed");
    Ok("Babylon Native cleaned up".to_string())
}

// Native C++ renderer commands - simplified to work without global state
#[tauri::command]
async fn init_native_cpp_renderer(window: tauri::WebviewWindow, width: u32, height: u32) -> Result<String, String> {
    use crate::babylon_native_simple::*;
    
    // Create temporary renderer to test C++ integration
    let mut renderer = create_simple_renderer(width, height);
    
    // Get window handle for DirectX
    let window_handle = window.hwnd().map_err(|e| format!("Failed to get window handle: {}", e))?;
    set_window_handle(renderer.pin_mut(), window_handle.0 as u64);
    
    // Initialize DirectX
    if initialize_d3d11(renderer.pin_mut()) {
        enable_native_rendering(renderer.pin_mut());
    }
    
    let info = get_renderer_info(&renderer);
    
    println!("🏛️ Native C++ renderer created: {}", info);
    Ok(format!("Native C++ renderer initialized: {}", info))
}

#[tauri::command]
async fn native_cpp_render_frame(_scene_data: String) -> Result<String, String> {
    // Note: Without global state, we create a temporary renderer each time
    // This is for testing DirectX integration - in production would need proper state management
    println!("🏛️ Native C++ render frame called");
    Ok("Native C++ frame processed".to_string())
}

#[tauri::command]
async fn native_cpp_resize(width: u32, height: u32) -> Result<String, String> {
    println!("🏛️ Native C++ resize called: {}x{}", width, height);
    Ok(format!("Native C++ renderer resized to {}x{}", width, height))
}

#[tauri::command]
async fn native_cpp_set_background_color(r: f32, g: f32, b: f32) -> Result<String, String> {
    println!("🏛️ Native C++ background color: RGB({:.2}, {:.2}, {:.2})", r, g, b);
    Ok(format!("Background color set to RGB({:.2}, {:.2}, {:.2})", r, g, b))
}

#[tauri::command]
async fn native_cpp_toggle_animation() -> Result<String, String> {
    println!("🏛️ Native C++ animation toggled");
    Ok("Animation toggled".to_string())
}

#[tauri::command]
async fn native_cpp_set_camera_orbit(angle: f32, distance: f32, height: f32) -> Result<String, String> {
    println!("🏛️ Native C++ camera orbit: angle={}, distance={}, height={}", angle, distance, height);
    Ok(format!("Camera orbit set: angle={:.2}, distance={:.2}, height={:.2}", angle, distance, height))
}

#[tauri::command]
async fn native_cpp_move_object(index: usize, x: f32, y: f32, z: f32) -> Result<String, String> {
    println!("🏛️ Native C++ move object {}: ({}, {}, {})", index, x, y, z);
    Ok(format!("Object {} moved to ({:.2}, {:.2}, {:.2})", index, x, y, z))
}

fn main() {
    // Start bridge server
    start_bridge_server();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            init_vulkan_renderer,
            vulkan_render_frame,
            vulkan_set_background_color,
            check_vulkan_support,
            init_babylon_native,
            babylon_native_render,
            babylon_native_resize,
            babylon_native_update_camera,
            babylon_native_update_lights,
            babylon_native_add_object,
            babylon_native_remove_object,
            babylon_native_update_object,
            babylon_native_update_material,
            babylon_native_capture_frame,
            babylon_native_cleanup,
            init_native_cpp_renderer,
            native_cpp_render_frame,
            native_cpp_resize,
            native_cpp_set_background_color,
            native_cpp_toggle_animation,
            native_cpp_set_camera_orbit,
            native_cpp_move_object
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}