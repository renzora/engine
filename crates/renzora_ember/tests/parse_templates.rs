//! Parse every shipped `.html` template through bevy_hui's own parser so markup
//! syntax errors are caught in CI without needing the GPU editor.

use bevy::prelude::*;
use bevy_hui::prelude::{parse_template, AssetServerAdaptor, VerboseHtmlError};

/// Collect every `.html` under a directory (recursively).
fn html_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            html_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
            out.push(path);
        }
    }
}

#[test]
fn all_ui_templates_parse() {
    // crates/renzora_ember → repo root → assets/ui
    let ui_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/ui");
    assert!(ui_dir.is_dir(), "assets/ui not found at {ui_dir:?}");

    let mut files = Vec::new();
    html_files(&ui_dir, &mut files);
    assert!(!files.is_empty(), "no .html templates found under {ui_dir:?}");

    // bevy_hui's parser resolves referenced asset paths through an
    // AssetLoadAdaptor; AssetServerAdaptor needs a real AssetServer, so spin up
    // the minimal app that provides one.
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    let server = app.world().resource::<AssetServer>().clone();

    let mut failures = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).expect("read template");
        let mut adaptor = AssetServerAdaptor { server: &server };
        match parse_template::<VerboseHtmlError>(&bytes, &mut adaptor) {
            Ok((rest, _template)) => {
                // The parser should consume the whole document (modulo trailing
                // whitespace) — leftover bytes usually mean a malformed tag.
                if rest.iter().any(|b| !b.is_ascii_whitespace()) {
                    failures.push(format!(
                        "{}: trailing unparsed input: {:?}",
                        path.display(),
                        String::from_utf8_lossy(&rest[..rest.len().min(80)])
                    ));
                }
            }
            Err(e) => failures.push(format!("{}: {e:?}", path.display())),
        }
    }

    assert!(failures.is_empty(), "template parse failures:\n{}", failures.join("\n"));
}
