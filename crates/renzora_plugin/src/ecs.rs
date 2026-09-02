//! The ergonomic layer: Bevy-shaped types over the raw `sys` calls.
//!
//! Everything here exists to make plugin source **identical to Bevy source**.
//! Names and signatures mirror `bevy_ecs` deliberately — a "better" name is a
//! name nobody already knows, and the whole value of this crate is that existing
//! Bevy knowledge transfers without a translation step.
//!
//! ## How a system becomes a C function
//!
//! `App::add_systems` takes any zero-sized callable. Two things are derived from
//! its signature at compile time:
//!
//! * the [`sys::QueryDesc`](crate::sys::QueryDesc) — which components, and how
//!   they're accessed
//! * an `extern "C"` thunk, monomorphised per signature, that unpacks
//!   [`sys::SystemCall`](crate::sys::SystemCall) into typed arguments and calls
//!   the function
//!
//! Because the thunk is generic over the callable's *type*, the function needs
//! no runtime representation at all — see `system::materialize`. That is why a
//! capturing closure is rejected: its captures would need storage the host
//! cannot own.
//!
//! ## Where things live
//!
//! The submodules are an internal split; **every public name is re-exported
//! here** under the name it has always had, so `renzora_plugin::ecs::Query` and
//! the prelude keep working untouched.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`component`] | [`Component`], the `host_component!` markers, the `sys` type re-exports |
//! | [`transform`] | [`Vec3`] / [`Quat`] / [`Transform`] arithmetic |
//! | [`init`] | [`InitCtx`] — resolving a type path to a host component id |
//! | [`math`] | [`Vec2`], [`Color`] — maths types with no boundary presence |
//! | [`query`] | [`QueryData`], [`QueryFilter`], [`Query`] and its iterator |
//! | [`resource`] | [`Resource`], [`Res`] / [`ResMut`], and the [`Time`] / [`Input`] pseudo-resources |
//! | [`commands`] | [`Bundle`], [`Commands`], [`EntityCommands`] |
//! | [`system`] | [`SystemParam`], the thunk machinery, [`Meshes`] / [`Images`] / [`Replies`] |
//! | [`render`] | The stashed interface, the log helpers, [`RenderPass`] |
//! | [`app`] | [`App`], [`Panel`], [`Plugin`] and the schedule labels |

pub mod app;
pub mod commands;
pub mod component;
pub mod init;
pub mod math;
pub mod query;
pub mod render;
pub mod resource;
pub mod system;
pub mod transform;

pub use crate::ecs::app::{
    Action, App, First, Last, Panel, PanelHandler, Plugin, PostUpdate, PreUpdate, Schedule, Update,
};
pub use crate::ecs::commands::{Bundle, Commands, EntityCommands, Scene};
pub use crate::ecs::component::{
    Component, Entity, Mesh3d, Quat, Str256, Transform, Vec3, Visibility,
};
pub use crate::ecs::init::{component_id_of, InitCtx};
pub use crate::ecs::math::{Color, Vec2};
pub use crate::ecs::query::{
    Added, Changed, Or, Query, QueryData, QueryFilter, QueryIter, With, Without,
};
pub use crate::ecs::render::{error, info, log, warn, RenderPass};
pub use crate::ecs::resource::{resource_id_of, Input, Res, ResMut, Resource, ResourceParam, Time};
pub use crate::ecs::system::{
    guarded_build, Images, IntoSystem, IntoSystems, MeshData, Meshes, ParamsMarker,
    RemovedComponents, Replies, SystemBuilder, SystemParam,
};
