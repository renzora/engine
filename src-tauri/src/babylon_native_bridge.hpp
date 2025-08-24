// Header file for Babylon Native C++ bridge

#pragma once
#include <memory>
#include <string>
#include "rust/cxx.h"

// Forward declarations for Babylon Native types
class BabylonNativeEngine;
class BabylonNativeScene;

// Engine management functions
std::unique_ptr<BabylonNativeEngine> create_babylon_engine(uint32_t width, uint32_t height);
void destroy_babylon_engine(std::unique_ptr<BabylonNativeEngine> engine);
bool engine_render_frame(BabylonNativeEngine& engine);
void engine_resize(BabylonNativeEngine& engine, uint32_t width, uint32_t height);

// Scene management functions
std::unique_ptr<BabylonNativeScene> create_babylon_scene(BabylonNativeEngine& engine);
void destroy_babylon_scene(std::unique_ptr<BabylonNativeScene> scene);
bool scene_load_script(BabylonNativeScene& scene, const std::string& script);

// Camera functions
bool scene_update_camera(BabylonNativeScene& scene, 
                        float pos_x, float pos_y, float pos_z,
                        float target_x, float target_y, float target_z,
                        float fov, float near, float far);

// Mesh functions
bool scene_add_mesh(BabylonNativeScene& scene, 
                   const std::string& name, 
                   float pos_x, float pos_y, float pos_z,
                   float scale_x, float scale_y, float scale_z);

bool scene_update_mesh(BabylonNativeScene& scene,
                      const std::string& name,
                      float pos_x, float pos_y, float pos_z,
                      float rot_x, float rot_y, float rot_z,
                      float scale_x, float scale_y, float scale_z,
                      bool visible);

bool scene_remove_mesh(BabylonNativeScene& scene, const std::string& name);

// Light functions
bool scene_add_light(BabylonNativeScene& scene,
                    const std::string& name,
                    const std::string& light_type,
                    float pos_x, float pos_y, float pos_z,
                    float intensity,
                    float r, float g, float b);

bool scene_update_light(BabylonNativeScene& scene,
                       const std::string& name,
                       float pos_x, float pos_y, float pos_z,
                       float intensity);

// Material functions
bool scene_update_material(BabylonNativeScene& scene,
                          const std::string& material_id,
                          float r, float g, float b);

// Utility functions
std::string babylon_native_get_version();
bool babylon_native_is_initialized();