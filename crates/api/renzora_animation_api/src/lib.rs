//! Animation vocabulary: the `.anim` clip format and property keyframes, how a
//! skeleton is drawn, and multi-sheet sprites for 2D animation.
//!
//! A layer of the contract hierarchy, above `renzora_scripting_api` (property
//! tracks key `PropertyValue`s). Plugins reach every item at the path it always
//! had (`renzora::AnimClip`, `renzora::SpriteImages`, ...) through `renzora_sdk`.
//! The engine's own crates depend on this crate directly.

pub mod animation; // .anim clip format + property keyframes
pub mod bone_style; // how a skeleton is drawn, everywhere one is (not flattened)
pub mod sprite_anim; // multi-sheet sprites (SpriteImages) for 2D animation

pub use animation::*;
pub use sprite_anim::*;
