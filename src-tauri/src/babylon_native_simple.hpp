// Header for Babylon Native C++ renderer

#pragma once
#include <memory>
#include <cstdint>
#include <vector>
#include <string>
#include "rust/cxx.h"

// DirectX includes for native rendering
// Note: Babylon Native integration requires complex dependency chain
// Using direct DirectX for now

#ifdef _WIN32
#include <windows.h>
#include <d3d11.h>
#endif

class SimpleRenderer {
public:
    uint32_t width, height;
    uint64_t frame_count = 0;
    float clear_color[3] = {0.1f, 0.1f, 0.15f};
    bool native_rendering_enabled = false;
    
    struct Cube {
        float x, y, z, size;
        float color[3] = {1.0f, 0.0f, 0.0f}; // Default red
        float rotation_speed = 1.0f;
        float animation_time = 0.0f;
    };
    std::vector<Cube> cubes;
    
    // Animation and camera state
    float animation_time = 0.0f;
    float camera_orbit_angle = 0.0f;
    float camera_distance = 10.0f;
    float camera_height = 5.0f;
    bool animation_enabled = true;
    
    // Native DirectX components
    bool directx_initialized = false;
    
#ifdef _WIN32
    HWND window_handle;
    ID3D11Device* d3d_device;
    ID3D11DeviceContext* d3d_context;
    IDXGISwapChain* swap_chain;
#endif
    
    SimpleRenderer(uint32_t w, uint32_t h);
    ~SimpleRenderer();
    bool render_frame();
    void resize(uint32_t w, uint32_t h);
    void set_clear_color(float r, float g, float b);
    void add_cube(float x, float y, float z, float size);
    void enable_native_rendering();
    void set_window_handle(uint64_t handle);
    bool initialize_d3d11();
    uint64_t get_frame_count() const;
    rust::String get_renderer_info() const;
    
    // Animation and camera controls
    void update_animation(float delta_time);
    void set_camera_orbit(float angle, float distance, float height);
    void toggle_animation();
    void move_object(size_t index, float x, float y, float z);
};

std::unique_ptr<SimpleRenderer> create_simple_renderer(uint32_t width, uint32_t height);
bool render_frame(SimpleRenderer& renderer);
void resize_renderer(SimpleRenderer& renderer, uint32_t width, uint32_t height);
void set_clear_color(SimpleRenderer& renderer, float r, float g, float b);
void add_cube(SimpleRenderer& renderer, float x, float y, float z, float size);
void enable_native_rendering(SimpleRenderer& renderer);
void set_window_handle(SimpleRenderer& renderer, uint64_t handle);
bool initialize_d3d11(SimpleRenderer& renderer);
uint64_t get_frame_count(const SimpleRenderer& renderer);
rust::String get_renderer_info(const SimpleRenderer& renderer);

// Animation and camera controls
void update_animation(SimpleRenderer& renderer, float delta_time);
void set_camera_orbit(SimpleRenderer& renderer, float angle, float distance, float height);
void toggle_animation(SimpleRenderer& renderer);
void move_object(SimpleRenderer& renderer, size_t index, float x, float y, float z);