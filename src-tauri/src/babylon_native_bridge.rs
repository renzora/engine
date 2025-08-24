// C++/Rust bridge for Babylon Native integration using cxx

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("babylon_native_bridge.hpp");
        
        // Babylon Native C++ types
        type BabylonNativeEngine;
        type BabylonNativeScene;
        
        // Engine management
        fn create_babylon_engine(width: u32, height: u32) -> UniquePtr<BabylonNativeEngine>;
        fn destroy_babylon_engine(engine: UniquePtr<BabylonNativeEngine>);
        fn engine_render_frame(engine: Pin<&mut BabylonNativeEngine>) -> bool;
        fn engine_resize(engine: Pin<&mut BabylonNativeEngine>, width: u32, height: u32);
        
        // Scene management  
        fn create_babylon_scene(engine: Pin<&mut BabylonNativeEngine>) -> UniquePtr<BabylonNativeScene>;
        fn destroy_babylon_scene(scene: UniquePtr<BabylonNativeScene>);
        fn scene_load_script(scene: Pin<&mut BabylonNativeScene>, script: &CxxString) -> bool;
        
        // Camera operations
        fn scene_update_camera(scene: Pin<&mut BabylonNativeScene>, 
                              pos_x: f32, pos_y: f32, pos_z: f32,
                              target_x: f32, target_y: f32, target_z: f32,
                              fov: f32, near: f32, far: f32) -> bool;
        
        // Object operations
        fn scene_add_mesh(scene: Pin<&mut BabylonNativeScene>, 
                         name: &str, 
                         pos_x: f32, pos_y: f32, pos_z: f32,
                         scale_x: f32, scale_y: f32, scale_z: f32) -> bool;
        
        fn scene_update_mesh(scene: Pin<&mut BabylonNativeScene>,
                            name: &str,
                            pos_x: f32, pos_y: f32, pos_z: f32,
                            rot_x: f32, rot_y: f32, rot_z: f32,
                            scale_x: f32, scale_y: f32, scale_z: f32,
                            visible: bool) -> bool;
        
        fn scene_remove_mesh(scene: Pin<&mut BabylonNativeScene>, name: &str) -> bool;
        
        // Lighting operations
        fn scene_add_light(scene: Pin<&mut BabylonNativeScene>,
                          name: &str,
                          light_type: &str,
                          pos_x: f32, pos_y: f32, pos_z: f32,
                          intensity: f32,
                          r: f32, g: f32, b: f32) -> bool;
        
        fn scene_update_light(scene: Pin<&mut BabylonNativeScene>,
                             name: &str,
                             pos_x: f32, pos_y: f32, pos_z: f32,
                             intensity: f32) -> bool;
        
        // Material operations
        fn scene_update_material(scene: Pin<&mut BabylonNativeScene>,
                                material_id: &str,
                                r: f32, g: f32, b: f32) -> bool;
        
        // Utility functions
        fn babylon_native_get_version() -> String;
        fn babylon_native_is_initialized() -> bool;
    }
}

pub use ffi::*;