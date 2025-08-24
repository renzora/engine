
fn main() {
    // Standard Tauri build
    tauri_build::build();

    // Build Babylon Native C++ integration
    build_babylon_native();
    
    // Copy bridge server executable
    copy_bridge_server();
}

fn build_babylon_native() {
    use std::env;
    use std::path::Path;
    
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let babylon_native_dir = Path::new(&manifest_dir).join("babylon-native");
    let build_dir = babylon_native_dir.join("build").join("win32");
    
    println!("cargo:rerun-if-changed=src/babylon_native_simple.cpp");
    println!("cargo:rerun-if-changed=babylon-native");
    
    // Build simple C++ bridge
    let mut bridge = cxx_build::bridge("src/babylon_native_simple.rs");
    bridge
        .file("src/babylon_native_simple.cpp")
        .include("src")
        .flag_if_supported("-std=c++17")
        .flag_if_supported("/std:c++17"); // MSVC
    
    // Note: Simplified DirectX integration without full Babylon Native
    // Full integration requires complex dependency chain
    
    bridge.compile("babylon_native_simple");
    
    // Link Babylon Native libraries if built
    if build_dir.join("Release").exists() {
        println!("cargo:rustc-link-search=native={}", build_dir.join("Core").join("Graphics").join("Release").display());
        println!("cargo:rustc-link-search=native={}", build_dir.join("Core").join("AppRuntime").join("Release").display());
        println!("cargo:rustc-link-search=native={}", build_dir.join("Polyfills").join("Canvas").join("Release").display());
        println!("cargo:rustc-link-search=native={}", build_dir.join("lib").join("Release").display());
        
        // Link core Babylon Native libraries
        println!("cargo:rustc-link-lib=static=Graphics");
        println!("cargo:rustc-link-lib=static=AppRuntime");
        println!("cargo:rustc-link-lib=static=Canvas");
        println!("cargo:rustc-link-lib=static=DirectXTK");
        
        println!("cargo:warning=Linking with full Babylon Native libraries");
    } else {
        println!("cargo:warning=Babylon Native not built yet, using simplified integration");
    }
    
    // Link system libraries for graphics
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "windows" => {
            println!("cargo:rustc-link-lib=user32");
            println!("cargo:rustc-link-lib=gdi32");
            println!("cargo:rustc-link-lib=opengl32");
            println!("cargo:rustc-link-lib=d3d11");
            println!("cargo:rustc-link-lib=dxgi");
        },
        _ => {}
    }
}

fn copy_bridge_server() {
    use std::env;
    use std::path::Path;
    use std::fs;
    
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root_dir = Path::new(&manifest_dir).parent().unwrap();
    let bridge_exe = root_dir.join("bridge").join("target").join("release").join("bridge-server.exe");
    
    if bridge_exe.exists() {
        let target_dir = Path::new(&manifest_dir).join("target").join("release");
        let target_exe = target_dir.join("bridge-server.exe");
        
        if let Err(e) = fs::copy(&bridge_exe, &target_exe) {
            println!("cargo:warning=Failed to copy bridge server: {}", e);
        } else {
            println!("cargo:warning=Bridge server copied successfully");
        }
    } else {
        println!("cargo:warning=Bridge server not found, run: cargo build --release --manifest-path bridge/Cargo.toml --bin bridge-server");
    }
}