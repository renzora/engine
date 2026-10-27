//! The web build's stand-in. A browser has no child processes and no mapped
//! files, so the link can never open there; the same API exists so callers need
//! no `cfg` of their own, and every attempt reports why it failed.

use std::io;
use std::path::Path;

use crate::ViewRequest;

/// Never constructed on the web: both constructors fail.
pub struct Link {
    _never: (),
}

fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "a separate render process is not available on the web",
    )
}

impl Link {
    pub fn create(_path: &Path) -> io::Result<Self> {
        Err(unsupported())
    }

    pub fn open(_path: &Path) -> io::Result<Self> {
        Err(unsupported())
    }

    pub fn write_view(&self, _view: &ViewRequest) {}

    pub fn read_view(&self) -> Option<ViewRequest> {
        None
    }

    pub fn publish_frame(&self, _width: u32, _height: u32, _fill: impl FnOnce(&mut [u8])) {}

    pub fn read_frame(&self, _seen: u64, _out: &mut Vec<u8>) -> Option<(u64, u32, u32)> {
        None
    }
}
