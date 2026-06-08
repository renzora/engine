//! Lumen diagnostics state.
//!
//! The snapshot types (`LumenDiagState`, `LumenCameraEntry`, and the
//! `LumenBakeSnapshot` it holds) live in the shared `renzora` contract: the GI
//! plugin (`renzora_lumen`) is a cdylib that can't be statically linked here, so
//! it produces the snapshot through the contract and the native Lumen panel
//! reads it across the dlopen boundary.
//!
//! Rendered by the native (ember) Lumen panel in [`crate::native`].

pub use renzora::{LumenCameraEntry, LumenDiagState};
