// Native C++ renderer integration for Babylon Native

use crate::babylon_native_simple::*;
use cxx::UniquePtr;
use serde_json::Value;

pub struct NativeRenderer {
    cpp_renderer: Option<UniquePtr<SimpleRenderer>>,
    initialized: bool,
}

impl NativeRenderer {
    pub fn new() -> Self {
        Self {
            cpp_renderer: None,
            initialized: false,
        }
    }

    pub fn initialize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if self.initialized {
            return Ok(());
        }

        let renderer = create_simple_renderer(width, height);
        self.cpp_renderer = Some(renderer);
        self.initialized = true;
        println!("🏛️ Native C++ renderer initialized {}x{}", width, height);
        Ok(())
    }

    pub fn render_frame(&mut self) -> Result<u64, String> {
        if !self.initialized {
            return Err("Renderer not initialized".to_string());
        }

        if let Some(renderer) = self.cpp_renderer.as_mut() {
            let success = render_frame(renderer.pin_mut());
            if success {
                Ok(get_frame_count(renderer))
            } else {
                Err("Render failed".to_string())
            }
        } else {
            Err("No renderer available".to_string())
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if let Some(renderer) = self.cpp_renderer.as_mut() {
            resize_renderer(renderer.pin_mut(), width, height);
            println!("🏛️ Native renderer resized to {}x{}", width, height);
            Ok(())
        } else {
            Err("No renderer available".to_string())
        }
    }

    pub fn set_clear_color(&mut self, r: f32, g: f32, b: f32) -> Result<(), String> {
        if let Some(renderer) = self.cpp_renderer.as_mut() {
            set_clear_color(renderer.pin_mut(), r, g, b);
            Ok(())
        } else {
            Err("No renderer available".to_string())
        }
    }

    pub fn add_cube(&mut self, x: f32, y: f32, z: f32, size: f32) -> Result<(), String> {
        if let Some(renderer) = self.cpp_renderer.as_mut() {
            add_cube(renderer.pin_mut(), x, y, z, size);
            Ok(())
        } else {
            Err("No renderer available".to_string())
        }
    }

    pub fn enable_native_mode(&mut self) -> Result<(), String> {
        if let Some(renderer) = self.cpp_renderer.as_mut() {
            enable_native_rendering(renderer.pin_mut());
            println!("🏛️ Babylon Native mode ENABLED");
            Ok(())
        } else {
            Err("No renderer available".to_string())
        }
    }

    pub fn get_info(&self) -> Result<String, String> {
        if let Some(renderer) = self.cpp_renderer.as_ref() {
            let info = get_renderer_info(renderer);
            Ok(info)
        } else {
            Ok("Native renderer not initialized".to_string())
        }
    }

    pub fn process_scene_data(&mut self, scene_data: Value) -> Result<(), String> {
        // Extract meshes from Babylon.js scene data
        if let Some(meshes) = scene_data.get("meshes").and_then(|m| m.as_array()) {
            for mesh in meshes {
                if let (Some(pos), Some(name)) = (
                    mesh.get("position").and_then(|p| p.as_object()),
                    mesh.get("name").and_then(|n| n.as_str())
                ) {
                    if name.contains("box") || name.contains("cube") {
                        let x = pos.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let y = pos.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let z = pos.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let size = 1.0f32; // Default size
                        
                        self.add_cube(x, y, z, size)?;
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for NativeRenderer {
    fn default() -> Self {
        Self::new()
    }
}