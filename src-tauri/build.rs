
fn main() {
    // Standard Tauri build
    tauri_build::build();
    
    // Copy bridge server executable
    copy_bridge_server();
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