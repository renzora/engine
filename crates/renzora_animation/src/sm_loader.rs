//! Asset loader for `.animsm` (Animation State Machine) RON files.

use std::fmt;

use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;

use crate::state_machine::AnimationStateMachine;

#[derive(Debug)]
pub enum SmLoadError {
    Io(std::io::Error),
    Ron(ron::de::SpannedError),
}

impl fmt::Display for SmLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmLoadError::Io(e) => write!(f, "IO error: {e}"),
            SmLoadError::Ron(e) => write!(f, "RON parse error: {e}"),
        }
    }
}

impl std::error::Error for SmLoadError {}

impl From<std::io::Error> for SmLoadError {
    fn from(e: std::io::Error) -> Self {
        SmLoadError::Io(e)
    }
}

impl From<ron::de::SpannedError> for SmLoadError {
    fn from(e: ron::de::SpannedError) -> Self {
        SmLoadError::Ron(e)
    }
}

#[derive(Default, TypePath)]
pub struct AnimSmLoader;

impl AssetLoader for AnimSmLoader {
    type Asset = AnimationStateMachine;
    type Settings = ();
    type Error = SmLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let sm: AnimationStateMachine = ron::de::from_bytes(&bytes)?;
        Ok(sm)
    }

    fn extensions(&self) -> &[&str] {
        &["animsm"]
    }
}
