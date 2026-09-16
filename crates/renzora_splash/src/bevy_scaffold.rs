//! The files a new Bevy project starts life as.
//!
//! A Renzora project is a `project.toml` and an empty scene, because the editor
//! is where it gets filled in. A Bevy project is the opposite: the editor cannot
//! author anything into it until there is a crate to author into, so New Bevy
//! Project has to write a game that already builds and already runs.
//!
//! What it writes is deliberately small and deliberately *not* a framework. Four
//! files, no modules, no engine dependency: a camera, a light, a ground plane, a
//! cube you drive with WASD and a few spinning pickups. Somebody who has never
//! opened Renzora can `cargo run` it, and somebody who has can open it here and
//! see the same world in the viewport.
//!
//! # Why the boilerplate derives `Reflect`
//!
//! Because the alternative is a starting point that silently withholds most of
//! the editor. `#[derive(Reflect)]` plus `#[reflect(Component)]` is what turns
//! an inspector row from a name into editable fields, and the same two lines on
//! a `Resource` put it in the Resources panel. A scaffold that left them out
//! would produce a project whose components the editor can name and nothing
//! more, and the author would have no reason to suspect there was anything
//! missing. Writing them in means the first thing you do in the editor is edit
//! something, and the pattern to copy is already on screen.
//!
//! # Why the `Name`s
//!
//! Nothing in Bevy asks for them, and most Bevy projects have none, which the
//! engine handles: `renzora_engine::named_entities` labels an unnamed entity
//! from its own components. But a generated `Player 3` is a worse hierarchy than
//! an authored `player`, and this is the one project whose spawn code is ours to
//! write.

use std::path::Path;

/// Write a new Bevy crate into `path`.
///
/// `path` is the crate root: the folder that ends up holding `Cargo.toml`. It is
/// created if it does not exist, and **an existing file is never overwritten**,
/// which is the only safe reading of "New Project" landing on somebody's work.
/// A folder that already has a `Cargo.toml` is refused outright rather than
/// half-populated.
pub fn create_bevy_project(path: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if path.join("Cargo.toml").exists() {
        return Err(format!(
            "{} already holds a Cargo.toml. Use Import Bevy Project to open it.",
            path.display()
        )
        .into());
    }
    std::fs::create_dir_all(path.join("src"))?;
    std::fs::create_dir_all(path.join("assets"))?;

    let krate = crate_name(name);
    write_new(&path.join("Cargo.toml"), &manifest(&krate))?;
    write_new(&path.join("src").join("main.rs"), MAIN_RS)?;
    write_new(&path.join(".gitignore"), GITIGNORE)?;
    // Bevy's `AssetPlugin` wants its folder to exist, and an empty directory does
    // not survive a `git clone`. One tracked file is the usual answer.
    write_new(&path.join("assets").join(".gitkeep"), "")?;
    Ok(())
}

/// Write `contents` only if nothing is there. See [`create_bevy_project`].
fn write_new(path: &Path, contents: &str) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    std::fs::write(path, contents)
}

/// A folder name as a cargo package name.
///
/// Cargo accepts far less than a filesystem does: `My Game (2)` is a perfectly
/// ordinary folder and not a package name at all. Lowercased, every run of
/// anything else collapsed to one `_`, and a leading digit prefixed, because
/// `2d_game` is not an identifier.
fn crate_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending_sep = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('_');
            }
            pending_sep = false;
            out.extend(ch.to_lowercase());
        } else {
            pending_sep = true;
        }
    }
    if out.is_empty() {
        return "game".to_string();
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert_str(0, "game_");
    }
    out
}

fn manifest(krate: &str) -> String {
    format!(
        r#"[package]
name = "{krate}"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy = "0.19"

# Bevy is unusably slow compiled without optimisation, and compiling *your* code
# optimised is unusably slow to iterate on. These two blocks are the usual split:
# your crate stays quick to rebuild, everything it depends on stays quick to run.
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
"#
    )
}

const GITIGNORE: &str = "/target\n\
                         # Renzora's staged crate root and the library it builds from it.\n\
                         /.renzora\n";

const MAIN_RS: &str = r#"//! A small Bevy game: drive the orange cube with WASD.
//!
//! `cargo run` plays it. Opening this folder in the Renzora editor shows the
//! same world in the viewport, with every entity below in the hierarchy.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_resource::<Score>()
        .add_systems(Startup, setup)
        .add_systems(Update, (move_player, spin_pickups))
        .run();
}

/// The cube you drive.
///
/// `Reflect` and `#[reflect(Component)]` are what let an editor read and write
/// the fields: without them `speed` below is just bytes. Deriving `Default` and
/// reflecting it as well is what lets a field be reset to it.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Player {
    /// World units per second.
    pub speed: f32,
}

/// Something to collect. Spins so it reads as pick-up-able.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
pub struct Pickup {
    /// Radians per second.
    pub spin: f32,
}

/// Game state that belongs to the world rather than to any one entity.
///
/// A reflected `Resource` shows up in the editor's Resources panel, which is
/// where to watch a value like this change while the game runs.
#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct Score {
    pub collected: u32,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("camera"),
        Camera3d::default(),
        Transform::from_xyz(0.0, 12.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Name::new("sun"),
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 12.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Name::new("ground"),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(40.0, 40.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.30, 0.35, 0.30))),
    ));

    commands.spawn((
        Name::new("player"),
        Player { speed: 8.0 },
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.90, 0.40, 0.20))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    for (i, x) in [-6.0f32, -2.0, 2.0, 6.0].into_iter().enumerate() {
        commands.spawn((
            Name::new(format!("pickup-{i}")),
            Pickup { spin: 1.5 },
            Mesh3d(meshes.add(Cuboid::new(0.6, 0.6, 0.6))),
            MeshMaterial3d(materials.add(Color::srgb(0.95, 0.85, 0.25))),
            Transform::from_xyz(x, 0.6, -5.0),
        ));
    }
}

/// WASD, framerate-independent.
fn move_player(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut players: Query<(&Player, &mut Transform)>,
) {
    let mut direction = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        direction.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        direction.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }
    // `try_normalize` rather than `normalize`: pressing nothing is a zero vector,
    // and normalising that is NaN, which would put the player nowhere at all.
    let Some(direction) = direction.try_normalize() else {
        return;
    };
    for (player, mut transform) in &mut players {
        transform.translation += direction * player.speed * time.delta_secs();
    }
}

fn spin_pickups(time: Res<Time>, mut pickups: Query<(&Pickup, &mut Transform)>) {
    for (pickup, mut transform) in &mut pickups {
        transform.rotate_y(pickup.spin * time.delta_secs());
    }
}
"#;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn a_folder_name_becomes_a_cargo_package_name() {
        assert_eq!(crate_name("My Game"), "my_game");
        assert_eq!(crate_name("starfall-grove"), "starfall_grove");
        assert_eq!(crate_name("My Game (2)"), "my_game_2");
        assert_eq!(crate_name("already_fine"), "already_fine");
    }

    /// Cargo rejects a package name starting with a digit, and a folder called
    /// `2d-platformer` is an entirely reasonable thing to have.
    #[test]
    fn a_leading_digit_is_prefixed() {
        assert_eq!(crate_name("2d-platformer"), "game_2d_platformer");
    }

    /// A name with nothing usable in it still has to produce *something*, or the
    /// manifest is invalid and the error arrives from cargo instead of here.
    #[test]
    fn a_nameless_folder_still_gets_a_package_name() {
        assert_eq!(crate_name("???"), "game");
        assert_eq!(crate_name(""), "game");
    }

    /// The refusal that keeps New Project from landing on existing work.
    #[test]
    fn an_existing_crate_is_refused_rather_than_merged() {
        let dir = std::env::temp_dir().join("renzora_scaffold_existing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"theirs\"\n").unwrap();

        assert!(create_bevy_project(&dir, "Mine").is_err());
        // Untouched: still their manifest, and no `src/` alongside it.
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("theirs"));
        assert!(!dir.join("src").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_project_gets_a_crate_that_names_itself_after_the_folder() {
        let dir = std::env::temp_dir().join("renzora_scaffold_new");
        let _ = std::fs::remove_dir_all(&dir);

        create_bevy_project(&dir, "Tower Defence").unwrap();

        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("name = \"tower_defence\""));
        assert!(manifest.contains("bevy = \"0.19\""));
        let main = std::fs::read_to_string(dir.join("src").join("main.rs")).unwrap();
        assert!(main.contains("fn main()"));
        assert!(main.contains("DefaultPlugins"));
        assert!(dir.join("assets").is_dir());
        assert!(dir.join(".gitignore").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
