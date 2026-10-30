//! What each kind of job actually does, on a worker thread.
//!
//! Every handler takes its arguments as the JSON its submitter wrote (the
//! types are in `renzora::job_args`, shared by both ends) and answers with a
//! short result or a reason in words.

use std::time::Duration;

use renzora::job_args;
use serde::de::DeserializeOwned;

use super::jobs::JobCtx;
use crate::protocol::Job;

/// Do `job`. `Ok` carries a short result the editor may use (a path, a
/// summary); `Err` says why it failed, in words for the Console.
pub(crate) fn run(job: Job, ctx: &JobCtx) -> Result<String, String> {
    match job {
        Job::Echo { text, millis } => {
            ctx.progress(None, "waiting");
            std::thread::sleep(Duration::from_millis(millis));
            Ok(text)
        }
        Job::Run { kind, payload, .. } => match kind.as_str() {
            "texture_thumbnail" => texture_thumbnail(args(&payload)?, ctx),
            "rust_script" => rust_script(args(&payload)?, ctx),
            "rvg_bake" => rvg_bake(args(&payload)?, ctx),
            renzora_import::remote::CONVERT_KIND => model_convert(args(&payload)?, ctx),
            renzora_import::remote::BUILD_KIND => {
                let glb: std::path::PathBuf = args(&payload)?;
                ctx.progress(None, "building virtual geometry");
                renzora_import::remote::build_model_file(&glb)
                    .map(str::to_string)
                    .map_err(|e| format!("could not store {}: {e}", glb.display()))
            }
            "upload" => {
                let job: renzora_marketplace::UploadJob = args(&payload)?;
                ctx.progress(None, "uploading");
                let item = renzora_marketplace::run_upload(&job)?;
                serde_json::to_string(&item).map_err(|e| e.to_string())
            }
            other => Err(format!("this companion does not know how to run a {other} job")),
        },
    }
}

fn args<T: DeserializeOwned>(payload: &str) -> Result<T, String> {
    serde_json::from_str(payload).map_err(|e| format!("could not read the job's arguments: {e}"))
}

/// Decode, shrink to fit `size` square with Lanczos3 (the filter the mip
/// baker uses, so a cached thumbnail matches the texture), write a PNG.
fn texture_thumbnail(args: job_args::TextureThumbnail, ctx: &JobCtx) -> Result<String, String> {
    let image = image::open(&args.source).map_err(|e| format!("could not read {}: {e}", args.source.display()))?;
    if ctx.cancelled() {
        return Err("cancelled".into());
    }
    let image = image.to_rgba8();
    let (w, h) = image.dimensions();
    let (tw, th) = fit(w, h, args.size);
    let small = image::imageops::resize(&image, tw, th, image::imageops::FilterType::Lanczos3);
    if let Some(dir) = args.cache.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    }
    // Written beside and renamed, so the editor never loads half a PNG.
    let partial = args.cache.with_extension("png.partial");
    small
        .save_with_format(&partial, image::ImageFormat::Png)
        .map_err(|e| format!("could not write {}: {e}", partial.display()))?;
    std::fs::rename(&partial, &args.cache).map_err(|e| format!("could not finish {}: {e}", args.cache.display()))?;
    Ok(args.cache.display().to_string())
}

/// Compile a Rust script the way the editor did: the same SDK, the same
/// staging and cache. Answers with the library's path.
fn rust_script(args: job_args::RustScriptBuild, ctx: &JobCtx) -> Result<String, String> {
    ctx.progress(None, "compiling");
    let sdk = renzora_plugin_build::Sdk::load(&args.sdk_dir).map_err(|e| e.to_string())?;
    let built = renzora_rust_script::build_to_path(&sdk, &args.project, &args.source)?;
    Ok(built.display().to_string())
}

/// Bring a model's virtual geometry up to date, as the editor's converter did.
/// Answers with an `RvgBakeOutcome` as JSON.
fn rvg_bake(args: job_args::RvgBake, ctx: &JobCtx) -> Result<String, String> {
    use renzora_rvg::bake::Ensured;
    ctx.progress(None, "building clusters");
    let outcome = match renzora_rvg::bake::ensure_current(&args.model)? {
        Ensured::Built { primitives, skipped, bytes, seconds } => job_args::RvgBakeOutcome {
            built: true,
            primitives,
            bytes,
            seconds,
            skipped: skipped
                .into_iter()
                .map(|s| format!("mesh {} primitive {} stays ordinary geometry ({})", s.mesh_index, s.primitive_index, s.reason))
                .collect(),
        },
        Ensured::UpToDate | Ensured::NothingToConvert { .. } | Ensured::Model => job_args::RvgBakeOutcome::default(),
    };
    serde_json::to_string(&outcome).map_err(|e| e.to_string())
}

/// Convert a model for the editor's import: the GLB and its textures go to
/// the folder the editor named, the rest back as JSON. Texture progress goes
/// back as `"done/total name"`, which the import window reads to move its bar.
fn model_convert(args: renzora_import::remote::ConvertArgs, ctx: &JobCtx) -> Result<String, String> {
    ctx.progress(None, "converting");
    // The baker reports from several threads at once; `JobCtx` is a channel
    // sender, so each report is one send.
    let reporter = std::sync::Mutex::new(ctx.clone());
    let progress = move |done: usize, total: usize, name: &str| {
        if let Ok(ctx) = reporter.lock() {
            ctx.progress(Some(done as f32 / total.max(1) as f32), format!("{done}/{total} {name}"));
        }
    };
    let converted = renzora_import::remote::convert_to_files(&args, &progress)?;
    serde_json::to_string(&converted).map_err(|e| e.to_string())
}

/// The size that fits `w`×`h` inside `max` square, never enlarged, never 0.
fn fit(w: u32, h: u32, max: u32) -> (u32, u32) {
    if w >= h {
        let tw = max.min(w);
        (tw, ((tw as f32 * h as f32 / w as f32).round() as u32).max(1))
    } else {
        let th = max.min(h);
        (((th as f32 * w as f32 / h as f32).round() as u32).max(1), th)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnails_fit_without_enlarging() {
        assert_eq!(fit(1024, 512, 256), (256, 128));
        assert_eq!(fit(512, 1024, 256), (128, 256));
        assert_eq!(fit(100, 50, 256), (100, 50));
        assert_eq!(fit(4000, 1, 256), (256, 1));
    }

    #[test]
    fn a_texture_thumbnail_is_written_from_the_source_file() {
        let dir = std::env::temp_dir().join(format!("rz-thumb-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("brick.png");
        image::RgbaImage::from_pixel(512, 256, image::Rgba([200, 80, 40, 255])).save(&source).unwrap();
        let cache = dir.join("cache").join("brick.png");
        let (ctx, _reports) = JobCtx::for_test();
        let result = texture_thumbnail(
            job_args::TextureThumbnail { source, cache: cache.clone(), size: 256 },
            &ctx,
        );
        assert!(result.is_ok(), "{result:?}");
        let written = image::open(&cache).unwrap();
        assert_eq!((written.width(), written.height()), (256, 128));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_kind_fails_in_words() {
        let (ctx, _reports) = JobCtx::for_test();
        let result = run(Job::Run { kind: "teleport".into(), payload: "{}".into(), target: String::new() }, &ctx);
        assert!(result.unwrap_err().contains("teleport"));
    }
}
