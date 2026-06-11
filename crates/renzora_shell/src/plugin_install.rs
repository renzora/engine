//! "File → Install Plugin…" — editor-only installer for distribution
//! plugins.
//!
//! Flow: native file picker (filtered to this platform's library extension:
//! `.dll` / `.so` / `.dylib`) → a security confirmation modal (plugins are
//! native code with full editor privileges) → copy into the engine's
//! `plugins/` directory next to the executable → "restart to load" notice
//! (plugins are dlopen'd once at startup).
//!
//! Validation here is deliberately shallow: extension + native-library magic
//! bytes, *without* dlopen-ing the file — loading it would already execute
//! its static initializers, which defeats the point of asking first. The
//! deep check (`plugin_bevy_hash` ABI match, `plugin_scope`) happens in
//! `dynamic_plugin_loader` on next startup, which rejects incompatible files.

use std::path::{Path, PathBuf};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{rgb, text_muted, text_primary};
use renzora_ember::widgets::{button, overlay_sized};

#[cfg(target_os = "windows")]
const PLUGIN_EXT: &str = "dll";
#[cfg(target_os = "macos")]
const PLUGIN_EXT: &str = "dylib";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const PLUGIN_EXT: &str = "so";

/// The picked file awaiting confirmation. Lives only while the confirm
/// overlay is up; its buttons despawn with the overlay, so a stale resource
/// (overlay dismissed via Escape/backdrop) is inert and simply replaced by
/// the next "Install Plugin…" invocation.
#[derive(Resource)]
pub(crate) struct PendingPluginInstall {
    source: PathBuf,
    overlay: Entity,
}

#[derive(Component)]
pub(crate) struct InstallConfirmBtn;

/// Closes the given overlay root (Cancel / OK buttons).
#[derive(Component)]
pub(crate) struct DismissOverlayBtn(Entity);

/// `File → Install Plugin…` menu action.
pub(crate) fn open_install_dialog(world: &mut World) {
    let Some(source) = rfd::FileDialog::new()
        .set_title("Select a Renzora plugin")
        .add_filter("Renzora plugin", &[PLUGIN_EXT])
        .pick_file()
    else {
        return;
    };

    if source.extension().and_then(|e| e.to_str()) != Some(PLUGIN_EXT) {
        notice_overlay(
            world,
            "Incompatible Plugin",
            &format!(
                "{} is not a plugin library for this platform (expected a .{PLUGIN_EXT} file).",
                file_name(&source)
            ),
        );
        return;
    }
    if !looks_like_native_lib(&source) {
        notice_overlay(
            world,
            "Not a Plugin Library",
            &format!(
                "{} doesn't look like a native .{PLUGIN_EXT} library for this platform.",
                file_name(&source)
            ),
        );
        return;
    }

    confirm_overlay(world, source);
}

/// The security prompt: what's being installed, where it goes, and what that
/// means, with explicit Cancel / Install buttons.
fn confirm_overlay(world: &mut World, source: PathBuf) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let dest_dir = plugins_dir();
    let replaces = dest_dir
        .as_ref()
        .is_some_and(|d| d.join(source.file_name().unwrap_or_default()).exists());

    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);

    let (root, content) = overlay_sized(&mut commands, &fonts, "Install Plugin", 560.0, 300.0, true);

    let mut kids = vec![
        info_row(&mut commands, &fonts, "Plugin", &file_name(&source)),
        info_row(&mut commands, &fonts, "From", &source.display().to_string()),
        info_row(
            &mut commands,
            &fonts,
            "Install to",
            &dest_dir
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|| "(plugins directory unavailable)".into()),
        ),
    ];
    if replaces {
        kids.push(paragraph(
            &mut commands,
            &fonts,
            "A plugin with this name is already installed and will be replaced.",
            rgb((230, 200, 110)),
        ));
    }
    kids.push(paragraph(
        &mut commands,
        &fonts,
        "Plugins are native code and run with the same privileges as the \
         editor — they can read and write anything you can. Only install \
         plugins from sources you trust. Compatibility with this engine \
         build is verified the next time the editor starts; incompatible \
         plugins are rejected at load time.",
        rgb((230, 110, 110)),
    ));

    let buttons = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .id();
    let cancel = button(&mut commands, &fonts.ui, "Cancel");
    commands.entity(cancel).insert(DismissOverlayBtn(root));
    let install = button(&mut commands, &fonts.ui, "Install Plugin");
    commands.entity(install).insert(InstallConfirmBtn);
    commands.entity(buttons).add_children(&[cancel, install]);
    kids.push(buttons);

    commands.entity(content).add_children(&kids);
    queue.apply(world);

    world.insert_resource(PendingPluginInstall { source, overlay: root });
}

/// Confirm / dismiss button handling. Runs every frame in the editor shell;
/// the queries are empty unless an installer overlay is open.
pub(crate) fn install_buttons(
    confirm: Query<&Interaction, (With<InstallConfirmBtn>, Changed<Interaction>)>,
    dismiss: Query<(&Interaction, &DismissOverlayBtn), Changed<Interaction>>,
    pending: Option<Res<PendingPluginInstall>>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    for (interaction, btn) in &dismiss {
        if *interaction == Interaction::Pressed {
            commands.entity(btn.0).despawn();
            commands.remove_resource::<PendingPluginInstall>();
        }
    }

    if !confirm.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let (Some(pending), Some(fonts)) = (pending, fonts) else {
        return;
    };
    commands.entity(pending.overlay).despawn();
    commands.remove_resource::<PendingPluginInstall>();

    let (title, body) = match install(&pending.source) {
        Ok(dest) => {
            renzora::core::console_log::console_info(
                "Plugins",
                format!("Installed {}", dest.display()),
            );
            (
                "Plugin Installed".to_string(),
                format!(
                    "{} was installed. Restart the editor to load it.",
                    file_name(&pending.source)
                ),
            )
        }
        Err(e) => (
            "Install Failed".to_string(),
            format!("Couldn't install {}: {e}", file_name(&pending.source)),
        ),
    };
    spawn_notice(&mut commands, &fonts, &title, &body);
}

/// Copy the library into `<exe dir>/plugins/`, creating the directory.
fn install(source: &Path) -> std::io::Result<PathBuf> {
    let dir = plugins_dir().ok_or_else(|| {
        std::io::Error::other("can't resolve the plugins directory next to the executable")
    })?;
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(source.file_name().ok_or_else(|| {
        std::io::Error::other("source path has no file name")
    })?);
    std::fs::copy(source, &dest)?;
    Ok(dest)
}

/// Same resolution the startup loader uses: `plugins/` beside the executable.
fn plugins_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(|d| d.join("plugins"))
}

/// First-bytes sanity check that this is a native library for the running
/// platform (ELF / Mach-O / PE magic). Deliberately NOT a dlopen — loading
/// would execute the library's initializers before the user consented.
fn looks_like_native_lib(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 4];
    if f.read_exact(&mut magic).is_err() {
        return false;
    }
    match magic {
        [0x7f, b'E', b'L', b'F'] => cfg!(target_os = "linux"),
        // Mach-O 64-bit (both endiannesses) + universal binaries.
        [0xcf, 0xfa, 0xed, 0xfe] | [0xfe, 0xed, 0xfa, 0xcf] | [0xca, 0xfe, 0xba, 0xbe] => {
            cfg!(target_os = "macos")
        }
        [b'M', b'Z', _, _] => cfg!(target_os = "windows"),
        _ => false,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

// ── Small UI helpers ─────────────────────────────────────────────────────────

fn info_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: &str) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(70.0),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    let v = commands
        .spawn((
            Text::new(value),
            ui_font(&fonts.mono, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(row).add_children(&[l, v]);
    row
}

fn paragraph(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: Color) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(color),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
        ))
        .id()
}

/// A small info/error overlay with an OK button (exclusive-world entry).
fn notice_overlay(world: &mut World, title: &str, body: &str) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    spawn_notice(&mut commands, &fonts, title, body);
    queue.apply(world);
}

fn spawn_notice(commands: &mut Commands, fonts: &EmberFonts, title: &str, body: &str) {
    let (root, content) = overlay_sized(commands, fonts, title, 460.0, 170.0, true);
    let text = paragraph(commands, fonts, body, rgb(text_primary()));
    let buttons = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            margin: UiRect::top(Val::Px(8.0)),
            ..default()
        })
        .id();
    let ok = button(commands, &fonts.ui, "OK");
    commands.entity(ok).insert(DismissOverlayBtn(root));
    commands.entity(buttons).add_child(ok);
    commands.entity(content).add_children(&[text, buttons]);
}
