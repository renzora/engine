// Babylon Native C++ integration

#include "babylon_native_simple.hpp"
#include <iostream>
#include <memory>
#include <vector>
#include <chrono>
#include <cmath>

#ifdef _WIN32
#include <windows.h>
#include <gl/GL.h>
#include <dxgi.h>
#pragma comment(lib, "opengl32.lib")
#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")
#endif

// CXX bridge types
#include "rust/cxx.h"

// Implementation of SimpleRenderer methods
SimpleRenderer::SimpleRenderer(uint32_t w, uint32_t h) : width(w), height(h) {
    std::cout << "🏛️ C++ SimpleRenderer created " << w << "x" << h << std::endl;
    
#ifdef _WIN32
    window_handle = nullptr;
    d3d_device = nullptr;
    d3d_context = nullptr;
    swap_chain = nullptr;
#endif

    // Add a default animated cube
    add_cube(0.0f, 0.0f, 0.0f, 1.0f);
}

SimpleRenderer::~SimpleRenderer() {
    std::cout << "🏛️ C++ SimpleRenderer destroyed" << std::endl;
    
#ifdef _WIN32
    if (swap_chain) swap_chain->Release();
    if (d3d_context) d3d_context->Release();
    if (d3d_device) d3d_device->Release();
#endif
}

bool SimpleRenderer::render_frame() {
    frame_count++;
    
    // Update animation
    if (animation_enabled) {
        static auto last_time = std::chrono::high_resolution_clock::now();
        auto current_time = std::chrono::high_resolution_clock::now();
        float delta_time = std::chrono::duration<float>(current_time - last_time).count();
        last_time = current_time;
        update_animation(delta_time);
    }
    
    if (native_rendering_enabled && directx_initialized) {
        // Native DirectX rendering
#ifdef _WIN32
        if (d3d_context && swap_chain) {
            ID3D11RenderTargetView* render_target = nullptr;
            ID3D11Texture2D* back_buffer = nullptr;
            
            swap_chain->GetBuffer(0, __uuidof(ID3D11Texture2D), (void**)&back_buffer);
            d3d_device->CreateRenderTargetView(back_buffer, nullptr, &render_target);
            
            float clear_color_d3d[4] = { clear_color[0], clear_color[1], clear_color[2], 1.0f };
            d3d_context->ClearRenderTargetView(render_target, clear_color_d3d);
            d3d_context->OMSetRenderTargets(1, &render_target, nullptr);
            
            // TODO: Add actual mesh rendering here
            // For now just clearing with color
            
            // Present the frame
            swap_chain->Present(1, 0);
            
            // Cleanup
            if (render_target) render_target->Release();
            if (back_buffer) back_buffer->Release();
        }
#endif
        
        if (frame_count % 60 == 0) {
            std::cout << "🏛️ Native DirectX rendering frame " << frame_count 
                      << " with " << cubes.size() << " cubes" << std::endl;
        }
    } else if (native_rendering_enabled) {
        if (frame_count % 60 == 0) {
            std::cout << "🏛️ Native DirectX initializing... frame " << frame_count << std::endl;
        }
    } else {
        // Fallback OpenGL rendering
        glClearColor(clear_color[0], clear_color[1], clear_color[2], 1.0f);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
        
        for (const auto& cube : cubes) {
            glPushMatrix();
            glTranslatef(cube.x, cube.y, cube.z);
            glScalef(cube.size, cube.size, cube.size);
            glColor3f(cube.color[0], cube.color[1], cube.color[2]);
            
            // Draw wireframe cube
            glBegin(GL_LINES);
            // Front face
            glVertex3f(-0.5f, -0.5f,  0.5f); glVertex3f( 0.5f, -0.5f,  0.5f);
            glVertex3f( 0.5f, -0.5f,  0.5f); glVertex3f( 0.5f,  0.5f,  0.5f);
            glVertex3f( 0.5f,  0.5f,  0.5f); glVertex3f(-0.5f,  0.5f,  0.5f);
            glVertex3f(-0.5f,  0.5f,  0.5f); glVertex3f(-0.5f, -0.5f,  0.5f);
            // Back face
            glVertex3f(-0.5f, -0.5f, -0.5f); glVertex3f( 0.5f, -0.5f, -0.5f);
            glVertex3f( 0.5f, -0.5f, -0.5f); glVertex3f( 0.5f,  0.5f, -0.5f);
            glVertex3f( 0.5f,  0.5f, -0.5f); glVertex3f(-0.5f,  0.5f, -0.5f);
            glVertex3f(-0.5f,  0.5f, -0.5f); glVertex3f(-0.5f, -0.5f, -0.5f);
            // Connecting lines
            glVertex3f(-0.5f, -0.5f,  0.5f); glVertex3f(-0.5f, -0.5f, -0.5f);
            glVertex3f( 0.5f, -0.5f,  0.5f); glVertex3f( 0.5f, -0.5f, -0.5f);
            glVertex3f( 0.5f,  0.5f,  0.5f); glVertex3f( 0.5f,  0.5f, -0.5f);
            glVertex3f(-0.5f,  0.5f,  0.5f); glVertex3f(-0.5f,  0.5f, -0.5f);
            glEnd();
            
            glPopMatrix();
        }
        
        if (frame_count % 60 == 0) {
            std::cout << "🏛️ OpenGL fallback rendering frame " << frame_count 
                      << " with " << cubes.size() << " cubes" << std::endl;
        }
    }
    
    return true;
}

void SimpleRenderer::resize(uint32_t w, uint32_t h) {
    width = w;
    height = h;
    std::cout << "🏛️ C++ Renderer resized to " << w << "x" << h << std::endl;
}

void SimpleRenderer::set_clear_color(float r, float g, float b) {
    clear_color[0] = r;
    clear_color[1] = g;
    clear_color[2] = b;
    std::cout << "🏛️ C++ Clear color set to RGB(" << r << ", " << g << ", " << b << ")" << std::endl;
}

void SimpleRenderer::add_cube(float x, float y, float z, float size) {
    Cube cube;
    cube.x = x;
    cube.y = y;
    cube.z = z;
    cube.size = size;
    cube.color[0] = 1.0f;
    cube.color[1] = 0.0f;
    cube.color[2] = 0.0f;
    cube.rotation_speed = 1.0f + (static_cast<float>(rand()) / RAND_MAX) * 2.0f; // Random speed 1-3
    cube.animation_time = 0.0f;
    cubes.push_back(cube);
    std::cout << "🏛️ C++ Added animated cube at (" << x << ", " << y << ", " << z << ") size " << size << std::endl;
}

void SimpleRenderer::enable_native_rendering() {
    native_rendering_enabled = true;
    std::cout << "🏛️ Babylon Native rendering ENABLED" << std::endl;
    
    if (initialize_d3d11()) {
        directx_initialized = true;
        std::cout << "🏛️ Native DirectX Device created!" << std::endl;
    } else {
        native_rendering_enabled = false;
    }
}

void SimpleRenderer::set_window_handle(uint64_t handle) {
#ifdef _WIN32
    window_handle = reinterpret_cast<HWND>(handle);
    std::cout << "🏛️ Window handle set for D3D11 integration" << std::endl;
#endif
}

bool SimpleRenderer::initialize_d3d11() {
#ifdef _WIN32
    if (!window_handle) {
        std::cout << "❌ No window handle for D3D11" << std::endl;
        return false;
    }

    DXGI_SWAP_CHAIN_DESC desc = {};
    desc.BufferCount = 1;
    desc.BufferDesc.Width = width;
    desc.BufferDesc.Height = height;
    desc.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    desc.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    desc.OutputWindow = window_handle;
    desc.SampleDesc.Count = 1;
    desc.Windowed = TRUE;

    D3D_FEATURE_LEVEL levels[] = { D3D_FEATURE_LEVEL_11_0 };
    D3D_FEATURE_LEVEL level;

    HRESULT hr = D3D11CreateDeviceAndSwapChain(
        nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, 0,
        levels, 1, D3D11_SDK_VERSION, &desc,
        &swap_chain, &d3d_device, &level, &d3d_context
    );

    if (SUCCEEDED(hr)) {
        std::cout << "🏛️ D3D11 device created successfully!" << std::endl;
        return true;
    } else {
        std::cout << "❌ D3D11 creation failed: " << std::hex << hr << std::endl;
        return false;
    }
#else
    return false;
#endif
}

uint64_t SimpleRenderer::get_frame_count() const {
    return frame_count;
}

rust::String SimpleRenderer::get_renderer_info() const {
    if (native_rendering_enabled && directx_initialized) {
        return rust::String("Native DirectX C++ Renderer (Animated)");
    } else if (native_rendering_enabled) {
        return rust::String("Native DirectX C++ Renderer (Initializing...)");
    } else {
        return rust::String("OpenGL Fallback C++ Renderer");
    }
}

void SimpleRenderer::update_animation(float delta_time) {
    animation_time += delta_time;
    
    // Update camera orbit
    camera_orbit_angle += delta_time * 0.5f; // Orbit speed
    if (camera_orbit_angle > 2.0f * 3.14159f) {
        camera_orbit_angle -= 2.0f * 3.14159f;
    }
    
    // Update cube animations
    for (auto& cube : cubes) {
        cube.animation_time += delta_time * cube.rotation_speed;
        
        // Animated position with sine wave
        float base_y = cube.y;
        cube.y = base_y + sin(cube.animation_time) * 0.5f;
        
        // Animated color cycling
        cube.color[0] = 0.5f + 0.5f * sin(cube.animation_time);
        cube.color[1] = 0.5f + 0.5f * sin(cube.animation_time + 2.094f); // 120 degrees
        cube.color[2] = 0.5f + 0.5f * sin(cube.animation_time + 4.188f); // 240 degrees
    }
}

void SimpleRenderer::set_camera_orbit(float angle, float distance, float height) {
    camera_orbit_angle = angle;
    camera_distance = distance;
    camera_height = height;
    std::cout << "🏛️ C++ Camera orbit: angle=" << angle << ", distance=" << distance << ", height=" << height << std::endl;
}

void SimpleRenderer::toggle_animation() {
    animation_enabled = !animation_enabled;
    std::cout << "🏛️ C++ Animation " << (animation_enabled ? "enabled" : "disabled") << std::endl;
}

void SimpleRenderer::move_object(size_t index, float x, float y, float z) {
    if (index < cubes.size()) {
        cubes[index].x = x;
        cubes[index].y = y;
        cubes[index].z = z;
        std::cout << "🏛️ C++ Moved object " << index << " to (" << x << ", " << y << ", " << z << ")" << std::endl;
    }
}

// Bridge implementations
std::unique_ptr<SimpleRenderer> create_simple_renderer(uint32_t width, uint32_t height) {
    return std::make_unique<SimpleRenderer>(width, height);
}

bool render_frame(SimpleRenderer& renderer) {
    return renderer.render_frame();
}

void resize_renderer(SimpleRenderer& renderer, uint32_t width, uint32_t height) {
    renderer.resize(width, height);
}

void set_clear_color(SimpleRenderer& renderer, float r, float g, float b) {
    renderer.set_clear_color(r, g, b);
}

void add_cube(SimpleRenderer& renderer, float x, float y, float z, float size) {
    renderer.add_cube(x, y, z, size);
}

void enable_native_rendering(SimpleRenderer& renderer) {
    renderer.enable_native_rendering();
}

void set_window_handle(SimpleRenderer& renderer, uint64_t handle) {
    renderer.set_window_handle(handle);
}

bool initialize_d3d11(SimpleRenderer& renderer) {
    return renderer.initialize_d3d11();
}

uint64_t get_frame_count(const SimpleRenderer& renderer) {
    return renderer.get_frame_count();
}

rust::String get_renderer_info(const SimpleRenderer& renderer) {
    return renderer.get_renderer_info();
}

// Animation and camera control bridge functions
void update_animation(SimpleRenderer& renderer, float delta_time) {
    renderer.update_animation(delta_time);
}

void set_camera_orbit(SimpleRenderer& renderer, float angle, float distance, float height) {
    renderer.set_camera_orbit(angle, distance, height);
}

void toggle_animation(SimpleRenderer& renderer) {
    renderer.toggle_animation();
}

void move_object(SimpleRenderer& renderer, size_t index, float x, float y, float z) {
    renderer.move_object(index, x, y, z);
}