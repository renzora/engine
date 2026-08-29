use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub struct IndicesOverflowError;

impl Error for IndicesOverflowError {}

impl Display for IndicesOverflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Indices overflow in mesh generation: Please reduce amount of sections, segments or leaves or enable the u32_indices feature.")
    }
}