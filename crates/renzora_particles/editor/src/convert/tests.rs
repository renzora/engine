//! Writing conversions to disk. The conversion itself is tested in the runtime
//! crate (`renzora_particles::convert`).

use std::path::{Path, PathBuf};

use super::format::HanabiEffectDefinition;
use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("renzora_convert_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_particle(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, ron::to_string(&HanabiEffectDefinition::default()).unwrap()).unwrap();
    path
}

#[test]
fn converting_beside_writes_once_then_reuses() {
    let dir = temp_dir("reuse");
    let p = write_particle(&dir, "fire.particle");
    let first = convert_beside(&p).unwrap();
    assert_eq!(first.outcome, Outcome::Written);
    assert_eq!(first.path, dir.join("fire.vfx"));
    let second = convert_beside(&p).unwrap();
    assert_eq!(second.outcome, Outcome::Reused);
    assert_eq!(second.path, first.path);
    assert!(p.exists(), "the .particle is never removed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_different_effect_under_the_name_is_never_overwritten() {
    let dir = temp_dir("taken");
    let p = write_particle(&dir, "fire.particle");
    std::fs::write(dir.join("fire.vfx"), "(name: \"mine\")").unwrap();
    let c = convert_beside(&p).unwrap();
    assert_eq!(c.path, dir.join("fire (converted).vfx"));
    assert_eq!(std::fs::read_to_string(dir.join("fire.vfx")).unwrap(), "(name: \"mine\")");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_broken_particle_file_is_an_error_not_a_default_effect() {
    let dir = temp_dir("broken");
    let p = dir.join("bad.particle");
    std::fs::write(&p, "(this is not an effect").unwrap();
    assert!(convert_beside(&p).is_err());
    assert!(convert_if_missing(&p).is_err());
    assert!(!dir.join("bad.vfx").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_open_pass_writes_only_a_missing_sibling() {
    let dir = temp_dir("missing");
    let p = write_particle(&dir, "smoke.particle");
    let original = std::fs::read(&p).unwrap();
    let written = convert_if_missing(&p).unwrap().expect("nothing was there, so it writes");
    assert_eq!(written.path, dir.join("smoke.vfx"));
    assert!(std::fs::read_to_string(&written.path).unwrap().starts_with("// Converted from smoke.particle"));
    // A second pass, or a user's own file under the name, is left exactly as is.
    std::fs::write(&written.path, "(name: \"edited\")").unwrap();
    assert!(convert_if_missing(&p).unwrap().is_none());
    assert_eq!(std::fs::read_to_string(&written.path).unwrap(), "(name: \"edited\")");
    assert!(!dir.join("smoke (converted).vfx").exists(), "never a second copy");
    assert_eq!(std::fs::read(&p).unwrap(), original, "the .particle is never touched");
    let _ = std::fs::remove_dir_all(&dir);
}
