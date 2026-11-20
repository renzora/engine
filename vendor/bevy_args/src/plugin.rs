use std::marker::PhantomData;

use bevy::prelude::*;
use clap::Parser;
use serde::{
    Deserialize,
    Serialize,
};

use crate::parse_args;


pub struct BevyArgsPlugin<R> {
    phantom: PhantomData<fn() -> R>,
}

impl<R> Default for BevyArgsPlugin<R> {
    fn default() -> Self {
        Self {
            phantom: PhantomData,
        }
    }
}

impl<R: Default + Parser + Resource + Serialize + for<'a> Deserialize<'a>> Plugin for BevyArgsPlugin<R> {
    fn build(&self, app: &mut App) {
        app.insert_resource(parse_args::<R>());
    }
}
