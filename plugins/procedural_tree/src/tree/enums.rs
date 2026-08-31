/*
* Inspiration taken with great thanks from: https://github.com/dgreenheck/ez-tree
*/

use bevy::reflect::Reflect;
use renzora::serde::{Deserialize, Serialize};

// #[derive(Reflect, Clone, Copy, Debug, PartialEq)]
// pub enum BarkType {
//   Birch,
//   Oak,
//   Pine,
//   Willow
// }

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(crate = "renzora::serde")]
pub enum LeafBillboard {
  Single,
  Double,
}

// #[derive(Reflect, Clone, Copy, Debug, PartialEq)]
// pub enum LeafType {
//   Ash,
//   Aspen,
//   Pine,
//   Oak,
// }

#[derive(Reflect, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(crate = "renzora::serde")]
pub enum TreeType {
  Deciduous,
  Evergreen,
}
