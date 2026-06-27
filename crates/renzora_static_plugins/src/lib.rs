//! Static-plugin aggregator for lean single-binary game exports.
//!
//! A distribution plugin normally ships as a `cdylib` that the engine `dlopen`s
//! from `plugins/` at startup. A lean export is **fully static** — it can't
//! `dlopen` anything — so the plugins a game uses must instead be *compiled into*
//! the binary. The `renzora::add!` macro already emits an `inventory::submit!`
//! registration that works in any static build; the only requirement is that the
//! plugin crate is actually **linked** into the binary (an unreferenced rlib is
//! dead-stripped, ctor and all).
//!
//! This crate is that link anchor. The lean exporter
//! (`renzora_export::build`) regenerates this file with one `extern crate <plugin>;`
//! line per selected plugin and adds those crates to `Cargo.toml`; the root
//! binary force-links this aggregator behind its `static_plugins` feature. Each
//! `extern crate` pulls the plugin's object — and thus its `inventory::submit!`
//! ctor — into the binary, where `renzora::for_each_static_plugin` finds and
//! installs it at boot. After the build, the exporter restores this file to its
//! empty committed state.
//!
//! Empty here by design: with no plugins selected there is nothing to link.
