//! USDA (text format) parser.
//!
//! Implements a proper recursive descent parser for USDA text,
//! supporting nested prims, properties, metadata, and relationships.

mod tokenizer;
mod parser;

use super::scene::*;
use super::UsdResult;

/// Parse USDA text content into a UsdStage.
pub fn parse(content: &str) -> UsdResult<UsdStage> {
    let tokens = tokenizer::tokenize(content);
    parser::parse_stage(&tokens, content)
}
