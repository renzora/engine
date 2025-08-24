use std::sync::Arc;
use vulkano::device::{Device, DeviceCreateInfo, QueueCreateInfo, QueueFlags, Queue};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::memory::allocator::StandardMemoryAllocator;
use vulkano::command_buffer::allocator::StandardCommandBufferAllocator;
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::VulkanLibrary;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraData {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialData {
    pub diffuse_color: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectData {
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scaling: [f32; 3],
    pub is_visible: bool,
    pub material: Option<MaterialData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightData {
    pub name: String,
    pub light_type: String,
    pub position: Option<[f32; 3]>,
    pub direction: Option<[f32; 3]>,
    pub intensity: f32,
    pub diffuse: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneData {
    pub camera: CameraData,
    pub objects: Vec<ObjectData>,
    pub lights: Vec<LightData>,
    pub timestamp: f64,
}

pub struct VulkanRenderer {
    _instance: Arc<Instance>,
    _device: Arc<Device>,
    _queue: Arc<Queue>,
    _memory_allocator: Arc<StandardMemoryAllocator>,
    _command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    _descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    pub frame_count: u64,
    background_color: [f32; 3],
    current_scene: Option<SceneData>,
}

impl VulkanRenderer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let library = VulkanLibrary::new().map_err(|e| format!("Vulkan library not available: {}", e))?;
        
        let instance = Instance::new(
            library,
            InstanceCreateInfo::default(),
        ).map_err(|e| format!("Failed to create Vulkan instance: {}", e))?;

        let physical_device = instance
            .enumerate_physical_devices()
            .map_err(|e| format!("Failed to enumerate devices: {}", e))?
            .next()
            .ok_or("No Vulkan-compatible physical device found")?;

        println!("🎯 Using device: {}", physical_device.properties().device_name);

        // Create device with a simple queue
        let queue_family_index = physical_device
            .queue_family_properties()
            .iter()
            .enumerate()
            .find_map(|(index, props)| {
                if props.queue_flags.intersects(QueueFlags::GRAPHICS) {
                    Some(index as u32)
                } else {
                    None
                }
            })
            .ok_or("No graphics queue family found")?;

        let (device, mut queues) = Device::new(
            physical_device.clone(),
            DeviceCreateInfo {
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        ).map_err(|e| format!("Failed to create device: {}", e))?;

        let queue = queues.next().ok_or("Failed to get queue")?;

        let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(device.clone(), Default::default()));
        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(device.clone(), Default::default()));

        println!("🔥 Native Vulkan renderer initialized successfully!");
        
        Ok(VulkanRenderer {
            _instance: instance,
            _device: device,
            _queue: queue,
            _memory_allocator: memory_allocator,
            _command_buffer_allocator: command_buffer_allocator,
            _descriptor_set_allocator: descriptor_set_allocator,
            frame_count: 0,
            background_color: [0.1, 0.1, 0.15],
            current_scene: None,
        })
    }

    pub fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.frame_count += 1;
        
        // Log render progress and scene data
        if self.frame_count % 60 == 0 {
            if let Some(scene) = &self.current_scene {
                println!("🎮 Native Vulkan renderer: {} frames rendered with {} objects, {} lights", 
                    self.frame_count, scene.objects.len(), scene.lights.len());
            } else {
                println!("🎮 Native Vulkan renderer: {} frames rendered (no scene data)", self.frame_count);
            }
        }

        // Basic Vulkan rendering simulation
        if let Some(scene) = &self.current_scene {
            // Process scene objects into render data
            let visible_objects: Vec<_> = scene.objects.iter()
                .filter(|obj| obj.is_visible)
                .collect();
            
            if self.frame_count % 300 == 0 && !visible_objects.is_empty() {
                println!("🎨 Vulkan: Processing {} visible objects for rendering", visible_objects.len());
                
                for (i, obj) in visible_objects.iter().enumerate() {
                    if i < 3 { // Log first 3 objects
                        let material_color = obj.material.as_ref()
                            .map(|m| format!("RGB({:.2}, {:.2}, {:.2})", m.diffuse_color[0], m.diffuse_color[1], m.diffuse_color[2]))
                            .unwrap_or_else(|| "Default".to_string());
                        
                        println!("🔺 Vulkan: Object '{}' - Pos:[{:.1}, {:.1}, {:.1}] Scale:[{:.1}, {:.1}, {:.1}] Material:{}", 
                            obj.name, 
                            obj.position[0], obj.position[1], obj.position[2],
                            obj.scaling[0], obj.scaling[1], obj.scaling[2],
                            material_color);
                    }
                }
                
                // Log camera for MVP matrix calculation
                let cam = &scene.camera;
                println!("📷 Vulkan: Camera - Pos:[{:.1}, {:.1}, {:.1}] Target:[{:.1}, {:.1}, {:.1}] FOV:{:.2}", 
                    cam.position[0], cam.position[1], cam.position[2],
                    cam.target[0], cam.target[1], cam.target[2],
                    cam.fov);
            }
        }
        
        Ok(())
    }

    pub fn update_scene(&mut self, scene_data_json: &str) -> Result<(), Box<dyn std::error::Error>> {
        let scene_data: SceneData = serde_json::from_str(scene_data_json)
            .map_err(|e| format!("Failed to parse scene data: {}", e))?;
        
        // Log scene updates occasionally
        if self.frame_count % 300 == 0 {
            println!("🔄 Vulkan renderer: Scene updated - {} objects, {} lights", 
                scene_data.objects.len(), scene_data.lights.len());
        }
        
        self.current_scene = Some(scene_data);
        Ok(())
    }

    pub fn _resize(&mut self, _new_size: [u32; 2]) -> Result<(), Box<dyn std::error::Error>> {
        println!("🔄 Vulkan renderer: Window resized to {:?}", _new_size);
        Ok(())
    }

    pub fn set_background_color(&mut self, r: f32, g: f32, b: f32) -> Result<(), Box<dyn std::error::Error>> {
        self.background_color = [r, g, b];
        println!("🎨 Vulkan renderer: Background color set to RGB({:.2}, {:.2}, {:.2})", r, g, b);
        Ok(())
    }

    // Method to create surface once we have window handle
    pub fn _create_surface_and_swapchain(&mut self, _window_handle: &dyn std::any::Any, window_size: [u32; 2]) -> Result<(), Box<dyn std::error::Error>> {
        // This will be implemented when we integrate with Tauri's window
        // For now, just acknowledge the call
        println!("🪟 Vulkan renderer: Surface creation requested for window size {:?}", window_size);
        Ok(())
    }
}

// Cleanup implementation
impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        // Cleanup would happen here in full implementation
    }
}