fn main() {
    // `intel_tex_2` (bake feature) ships precompiled ISPC texture-compression
    // kernels containing C++ object code, which needs the C++ runtime for its
    // exception personality (`__gxx_personality_v0`). The editor binary links
    // it transitively through other native deps, but this crate's standalone
    // test binary doesn't — without this, `cargo test --workspace` fails to
    // link on Linux. MSVC pulls its C++ runtime in automatically.
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var_os("CARGO_FEATURE_BAKE").is_none() {
        return;
    }
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("linux") {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    } else if target.contains("apple") {
        println!("cargo:rustc-link-lib=dylib=c++");
    }
}
