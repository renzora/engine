use renzora_rpak::pack_project_with_progress;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let project = if args.len() > 1 {
        std::path::PathBuf::from(&args[1])
    } else {
        std::path::PathBuf::from(".")
    };

    let mut packed = Vec::new();
    let packer = pack_project_with_progress(&project, None, |key| {
        packed.push(key.to_string());
    })
    .expect("pack");

    println!("Total packed: {}", packer.len());
    for k in &packed {
        println!("  {}", k);
    }

    // Also dump every asset-path-looking quoted string the scene contains so
    // we can see what the BFS *should* have found.
    let scene_path = project.join("scenes").join("main.ron");
    if let Ok(text) = std::fs::read_to_string(&scene_path) {
        let mut found: Vec<&str> = Vec::new();
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'"' {
                let start = i + 1;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < bytes.len() {
                    let s = &text[start..i];
                    if !s.is_empty()
                        && !s.contains("::")
                        && (s.ends_with(".glb")
                            || s.ends_with(".png")
                            || s.ends_with(".material")
                            || s.ends_with(".ron")
                            || s.ends_with(".lua"))
                    {
                        found.push(s);
                    }
                }
                i += 1;
            } else {
                i += 1;
            }
        }
        println!("\nQuoted asset-like paths in scenes/main.ron:");
        for s in &found {
            println!("  {}", s);
        }
    }
}
