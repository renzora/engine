//! Visual blueprints — authored node graphs that **compile to Lua** and run
//! through the script VM (drop a `.blueprint` onto a Script component and Play
//! runs it). There is no live graph interpreter: compilation is the single
//! execution path, mirroring how Unreal compiles Blueprints to bytecode.
//!
//! Node semantics live in [`nodes`] (one self-contained, tested unit per node);
//! [`compiler`] walks the graph and dispatches to them. The `BlueprintGraph`
//! component is the authoring/serialization vehicle, registered for reflection
//! so it round-trips through scenes.

pub mod compiler;
pub mod graph;
pub mod layout;
pub mod nodes;

use bevy::prelude::*;

pub use compiler::compile_to_lua;
pub use graph::{
    BlueprintConnection, BlueprintGraph, BlueprintNode, BlueprintNodeDef, PinDir, PinTemplate,
    PinType, PinValue,
};
pub use nodes::{categories, node_def, nodes_in_category};

#[derive(Default)]
pub struct BlueprintPlugin;

impl Plugin for BlueprintPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] BlueprintPlugin");
        // Register the graph types so blueprints serialize into scene RON. The
        // graph executes by compiling to Lua (see `compiler`), so there are no
        // per-frame interpreter systems to add.
        app.register_type::<BlueprintGraph>()
            .register_type::<BlueprintNode>()
            .register_type::<BlueprintConnection>()
            .register_type::<renzora::GraphComment>();
    }
}

renzora::add!(BlueprintPlugin);
