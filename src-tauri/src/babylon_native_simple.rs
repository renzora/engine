// Simplified C++/Rust bridge for native graphics

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("babylon_native_simple.hpp");
        
        // Simple native renderer
        type SimpleRenderer;
        
        // Core functions
        fn create_simple_renderer(width: u32, height: u32) -> UniquePtr<SimpleRenderer>;
        fn render_frame(renderer: Pin<&mut SimpleRenderer>) -> bool;
        fn resize_renderer(renderer: Pin<&mut SimpleRenderer>, width: u32, height: u32);
        fn set_clear_color(renderer: Pin<&mut SimpleRenderer>, r: f32, g: f32, b: f32);
        fn add_cube(renderer: Pin<&mut SimpleRenderer>, x: f32, y: f32, z: f32, size: f32);
        fn enable_native_rendering(renderer: Pin<&mut SimpleRenderer>);
        fn set_window_handle(renderer: Pin<&mut SimpleRenderer>, handle: u64);
        fn initialize_d3d11(renderer: Pin<&mut SimpleRenderer>) -> bool;
        fn get_frame_count(renderer: &SimpleRenderer) -> u64;
        fn get_renderer_info(renderer: &SimpleRenderer) -> String;
        
        // Animation and camera controls
        fn update_animation(renderer: Pin<&mut SimpleRenderer>, delta_time: f32);
        fn set_camera_orbit(renderer: Pin<&mut SimpleRenderer>, angle: f32, distance: f32, height: f32);
        fn toggle_animation(renderer: Pin<&mut SimpleRenderer>);
        fn move_object(renderer: Pin<&mut SimpleRenderer>, index: usize, x: f32, y: f32, z: f32);
    }
}

pub use ffi::*;