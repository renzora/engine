//! Blender `.blend` → GLB converter via Blender shell-out.
//!
//! Detects a local Blender installation and invokes it headlessly to export
//! the `.blend` file as GLB, which is then read back through the standard
//! glTF passthrough pipeline.
//!
//! Blender is located via (in order):
//! 1. `BLENDER_PATH` environment variable
//! 2. Common install directories (Windows / Linux / macOS)
//! 3. `PATH` lookup

use std::path::{Path, PathBuf};

use crate::convert::{ImportError, ImportResult};
use crate::settings::{ImportSettings, UpAxis};

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let blender = find_blender()?;

    let tmp_glb = std::env::temp_dir().join(format!(
        "renzora_blend_{}.glb",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));

    let y_up = settings.up_axis != UpAxis::ZUp;

    // Build the Python export expression.
    // We use --factory-startup to ignore user preferences/addons and get a
    // clean, reproducible export.
    let python_script = format!(
        concat!(
            "import bpy\n",
            "bpy.ops.export_scene.gltf(",
            "filepath=r'{filepath}',",
            "export_format='GLB',",
            "export_yup={yup},",
            "export_apply=True,",
            "export_animations=True,",
            "export_skins=True,",
            "export_morph=True,",
            "export_lights=True,",
            "export_cameras=True,",
            "export_materials='EXPORT',",
            "export_colors=True",
            ")\n",
        ),
        filepath = tmp_glb.display().to_string().replace('\\', "\\\\"),
        yup = if y_up { "True" } else { "False" },
    );

    // Write script to a temp file (more reliable than --python-expr across
    // Blender versions)
    let tmp_script = std::env::temp_dir().join(format!(
        "renzora_blend_export_{}.py",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));

    std::fs::write(&tmp_script, &python_script).map_err(|e| {
        ImportError::ConversionError(format!("Failed to write export script: {}", e))
    })?;

    let output = std::process::Command::new(&blender)
        .args([
            "--background",
            "--factory-startup",
            path.to_str().unwrap_or(""),
            "--python",
            tmp_script.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp_script);
            ImportError::ConversionError(format!("Failed to launch Blender: {}", e))
        })?;

    let _ = std::fs::remove_file(&tmp_script);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let _ = std::fs::remove_file(&tmp_glb);
        return Err(ImportError::ConversionError(format!(
            "Blender export failed (exit code {}):\n{}\n{}",
            output.status.code().unwrap_or(-1),
            stderr,
            stdout
        )));
    }

    if !tmp_glb.exists() {
        return Err(ImportError::ConversionError(
            "Blender completed but no GLB file was produced. \
             The .blend file may be empty or contain no exportable data."
                .into(),
        ));
    }

    // Apply scale post-export (Blender's glTF exporter doesn't have a
    // reliable global scale parameter across all versions)
    let result = crate::gltf_pass::convert_glb(&tmp_glb, settings);
    let _ = std::fs::remove_file(&tmp_glb);
    result
}

/// Locate the Blender executable.
fn find_blender() -> Result<PathBuf, ImportError> {
    // 1. Environment variable
    if let Ok(path) = std::env::var("BLENDER_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. Common install paths
    #[cfg(target_os = "windows")]
    {
        let base = r"C:\Program Files\Blender Foundation";
        if let Ok(entries) = std::fs::read_dir(base) {
            // Find the highest version installed
            let mut candidates: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().join("blender.exe"))
                .filter(|p| p.exists())
                .collect();
            candidates.sort();
            candidates.reverse();
            if let Some(best) = candidates.into_iter().next() {
                return Ok(best);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let app_path = "/Applications/Blender.app/Contents/MacOS/Blender";
        if Path::new(app_path).exists() {
            return Ok(PathBuf::from(app_path));
        }
    }

    #[cfg(target_os = "linux")]
    {
        for candidate in &["/usr/bin/blender", "/usr/local/bin/blender", "/snap/bin/blender"] {
            if Path::new(candidate).exists() {
                return Ok(PathBuf::from(*candidate));
            }
        }
    }

    // 3. Try PATH
    if let Ok(output) = std::process::Command::new("blender").arg("--version").output() {
        if output.status.success() {
            return Ok(PathBuf::from("blender"));
        }
    }

    Err(ImportError::ConversionError(
        "Blender not found. Install Blender or set the BLENDER_PATH environment variable \
         to the path of the Blender executable."
            .into(),
    ))
}
