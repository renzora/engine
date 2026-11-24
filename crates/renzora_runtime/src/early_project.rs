//! The game's `project.toml`, read before the `App` exists.
//!
//! Most of a game's settings reach it through `CurrentProject`, which
//! `renzora_engine::RuntimePlugin` inserts while the engine plugins build. Two
//! decisions are made earlier than that, while `main` is still choosing which
//! plugin set to assemble, and both belong to the game rather than to whoever
//! is running it:
//!
//! - **The renderer backend.** wgpu picks it when `RenderPlugin` is built,
//!   which happens inside `DefaultPlugins`, before any engine plugin and so
//!   before `CurrentProject`. A shipped game used to read the editor user's
//!   `~/.renzora` preference instead, so on the developer's machine it
//!   inherited the editor's backend and on a player's it fell back to Auto.
//! - **The server's `[network]` section.** `--server` needs the tick rate to
//!   build its headless runner, and that is chosen before any plugin exists.
//!   It used to read `./project.toml` off disk, which a packed game does not
//!   have, so a packed server ignored its own config.
//!
//! Both now read the project from the same places `RuntimePlugin` does, in the
//! same order: the rpak (embedded, adjacent, or `--rpak`), then `--project`,
//! then a `project.toml` in the working directory. Reading it twice costs an
//! mmap and an index parse, not a copy of the archive, and keeps
//! `renzora_engine` the single owner of the `CurrentProject` it inserts.
//!
//! Never called in an editor session: the editor's project is picked later, on
//! the splash screen, and its own window keeps its own preferences.

use std::path::PathBuf;
use std::sync::OnceLock;

use renzora_project_api::ProjectConfig;

/// The game's project config, or `None` when none was found or it does not
/// parse. Read once per process and cached, because both callers run during
/// startup and neither should pay for a second read.
pub fn early_project_config() -> Option<&'static ProjectConfig> {
    static CONFIG: OnceLock<Option<ProjectConfig>> = OnceLock::new();
    CONFIG.get_or_init(|| read_project_toml().and_then(|text| parse_config(&text))).as_ref()
}

/// The text of the game's `project.toml`, from the first source that has one.
fn read_project_toml() -> Option<String> {
    let vfs = renzora_engine::vfs::Vfs::detect();
    if vfs.has_archive() {
        // An archive is authoritative even when it lacks the file: that is the
        // shipped game, and a stray `project.toml` in the working directory is
        // not its config. `RuntimePlugin` makes the same call.
        return vfs.read_string("project.toml");
    }
    let path = project_toml_path(&std::env::args().collect::<Vec<_>>())
        .or_else(|| Some(PathBuf::from("project.toml")).filter(|p| p.exists()))?;
    std::fs::read_to_string(path).ok()
}

/// The `project.toml` that `--project <path>` names, where `<path>` may be the
/// project directory or the file itself. Mirrors `renzora_engine`'s own
/// argument parse, which is private to it.
fn project_toml_path(args: &[String]) -> Option<PathBuf> {
    let index = args.iter().position(|a| a == "--project")?;
    let path = PathBuf::from(args.get(index + 1)?);
    Some(if path.is_dir() { path.join("project.toml") } else { path })
}

/// Parse a `project.toml`. A malformed file is `None` here rather than an
/// error, because `RuntimePlugin` parses the same text moments later and is the
/// one that reports it; logging it here would say it twice, and before the log
/// subscriber exists it would not be seen anyway.
fn parse_config(text: &str) -> Option<ProjectConfig> {
    toml::from_str(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn project_flag_names_the_file() {
        let path = project_toml_path(&args(&["renzora", "--project", "no/such/game.toml"]));
        assert_eq!(path, Some(PathBuf::from("no/such/game.toml")));
    }

    #[test]
    fn project_flag_on_a_directory_means_its_project_toml() {
        let dir = std::env::temp_dir();
        let path = project_toml_path(&args(&["renzora", "--project", dir.to_str().unwrap()]));
        assert_eq!(path, Some(dir.join("project.toml")));
    }

    #[test]
    fn no_project_flag_or_no_value_is_none() {
        assert_eq!(project_toml_path(&args(&["renzora", "--no-editor"])), None);
        assert_eq!(project_toml_path(&args(&["renzora", "--project"])), None);
    }

    #[test]
    fn parses_backend_and_network() {
        let text = r#"
name = "Game"
version = "0.1.0"
main_scene = "scenes/main.bsn"

[window]
width = 640
height = 360

[rendering]
backend = "vulkan"

[network]
port = 9000
tick_rate = 30
"#;
        let config = parse_config(text).expect("parses");
        assert_eq!(config.rendering.backend, renzora_project_api::RendererBackend::Vulkan);
        let net = config.network.expect("network section");
        assert_eq!((net.port, net.tick_rate), (9000, 30));
    }

    #[test]
    fn malformed_toml_is_none() {
        assert!(parse_config("name = ").is_none());
    }
}
