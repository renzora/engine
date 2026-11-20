//! The companion's vocabulary, with no Bevy in it.
//!
//! Three programs share these types: the editor, the companion and any game
//! being profiled. They used to live in crates built on Bevy (`renzora`,
//! `renzora_profiler`, `renzora_companion`), which was harmless while the
//! companion was a second copy of the engine. The standalone companion is not,
//! so its vocabulary lives here, in a crate it can link without linking the
//! engine. The old paths re-export from here, so nothing that names them
//! changed.
//!
//! - [`protocol`]: editor and companion, JSON lines.
//! - [`job_args`]: the arguments and answers of each job kind.
//! - [`frame`] and [`wire`]: a game's profile and the binary stream that
//!   carries it.
//! - [`probes`]: the debug reports a process sends the companion.
//! - [`project_files`]: what counts as a project file, the listing the
//!   companion answers `"project_index"` with, and thumbnail cache paths.

pub mod frame;
pub mod job_args;
pub mod probes;
pub mod project_files;
pub mod protocol;
pub mod wire;
