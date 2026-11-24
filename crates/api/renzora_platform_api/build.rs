//! Sets `renzora_build` for this package's own targets.
//!
//! `renzora::plugin!` refuses to compile without that cfg, because outside the
//! Renzora driver (which passes `--cfg renzora_build`) a plugin is being built by
//! a plain `cargo build` against a second Bevy. This crate's integration tests
//! expand `plugin!` the way a plugin does, standing in for the driver, so they
//! need the cfg the driver would give them. Nothing else here reads it: the cfg
//! is checked where the macro expands, in the crate using it.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(renzora_build)");
    println!("cargo::rustc-check-cfg=cfg(renzora_no_editor)");
    println!("cargo::rustc-cfg=renzora_build");
}
