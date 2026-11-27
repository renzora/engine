use std::{
    env,
    path::{Path, PathBuf},
};

fn main() {
    if cfg!(feature = "mock") {
        return;
    }

    // Get SDK paths
    let dlss_sdk = env::var("DLSS_SDK")
        .expect("DLSS_SDK environment variable not set. Consult the dlss_wgpu readme.");
    let vulkan_sdk = env::var("VULKAN_SDK").expect("VULKAN_SDK environment variable not set");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Determine platform info
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let platform = if target_os == "windows" {
        format!("Windows_{target_arch}")
    } else {
        format!("Linux_{target_arch}")
    };
    println!("cargo:rustc-env=DLSS_SDK_PLATFORM={platform}");

    // Link to needed libraries
    let mut lib_dir = format!("{dlss_sdk}/lib/{platform}");
    if target_os == "windows" && target_arch == "x86_64" {
        lib_dir.push_str("/x64");
    }
    if !Path::new(&lib_dir).is_dir() {
        panic!("DLSS SDK at {dlss_sdk} has no libraries for {platform}");
    }
    println!("cargo:rustc-link-search=native={lib_dir}");
    if target_os == "windows" {
        #[cfg(not(target_feature = "crt-static"))]
        println!("cargo:rustc-link-lib=static=nvsdk_ngx_d");
        #[cfg(target_feature = "crt-static")]
        println!("cargo:rustc-link-lib=static=nvsdk_ngx_s");
    } else {
        println!("cargo:rustc-link-lib=static=nvsdk_ngx");
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=dl");
    }

    // Generate rust bindings
    #[cfg(not(target_os = "windows"))]
    let vulkan_sdk_include = "include";
    #[cfg(target_os = "windows")]
    let vulkan_sdk_include = "Include";
    let mut bindings = bindgen::Builder::default()
        .header(format!("{}/src/wrapper.h", env!("CARGO_MANIFEST_DIR")))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .wrap_static_fns(true)
        .wrap_static_fns_path(out_dir.join("wrap_static_fns"))
        .clang_arg(format!("-I{dlss_sdk}/include"))
        .clang_arg(format!("-I{vulkan_sdk}/{vulkan_sdk_include}"))
        .allowlist_item(".*NGX.*")
        .blocklist_item("Vk.*")
        .blocklist_item("PFN_vk.*")
        .blocklist_item(".*Cuda.*")
        .blocklist_item(".*CUDA.*");
    // RENZORA: no Direct3D helpers off Windows. `wrapper.h` pulls in the SDK's
    // D3D helper headers on every platform, and `wrap_static_fns` compiles a C
    // wrapper for each of their `static inline` functions, which call
    // `NVSDK_NGX_D3D11/12_CreateFeature` and `_EvaluateFeature_C`. The Linux NGX
    // library has no Direct3D, so those four stay undefined. A static binary
    // drops the unused wrappers, but a shared `libbevy_dylib.so` keeps them,
    // and with BIND_NOW the loader refuses it at startup ("undefined symbol:
    // NVSDK_NGX_D3D11_CreateFeature"). This crate only drives Vulkan, so none of
    // the 34 functions this removes is called.
    if target_os != "windows" {
        bindings = bindings.blocklist_function(".*D3D1[12].*");
    }
    bindings
        .generate()
        .unwrap()
        .write_to_file(out_dir.join("bindings.rs"))
        .unwrap();

    // Generate and link a library for static inline functions
    cc::Build::new()
        .file(out_dir.join("wrap_static_fns.c"))
        .includes([
            format!("{dlss_sdk}/include"),
            format!("{vulkan_sdk}/{vulkan_sdk_include}"),
        ])
        .compile("wrap_static_fns");
}
