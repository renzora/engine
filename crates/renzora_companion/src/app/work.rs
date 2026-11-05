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
            "texture_bake" => {
                use renzora_rmip::bake::{bake_image, BakeParams, TextureRole};
                let job: job_args::TextureBake = args(&payload)?;
                let role = match job.role.as_str() {
                    "color" => TextureRole::Color,
                    "normal" => TextureRole::NormalMap,
                    "linear" => TextureRole::LinearData,
                    other => return Err(format!("no texture role is called {other}")),
                };
                ctx.progress(None, "baking mipmaps");
                let source = std::fs::read(&job.source).map_err(|e| format!("could not read {}: {e}", job.source.display()))?;
                let baked = bake_image(&source, BakeParams { role, compress: true, ..Default::default() })?;
                if let Some(dir) = job.target.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
                }
                // Written beside and renamed, so the editor never loads half a file.
                let partial = job.target.with_extension("rmip.partial");
                std::fs::write(&partial, baked).map_err(|e| format!("could not write {}: {e}", partial.display()))?;
                std::fs::rename(&partial, &job.target).map_err(|e| format!("could not write {}: {e}", job.target.display()))?;
                Ok(job.target.display().to_string())
            }
            "rvg_migrate" => {
                use job_args::RvgMigrateOutcome as Out;
                use renzora_rvg::bake::Converted;
                let job: job_args::RvgMigrate = args(&payload)?;
                ctx.progress(None, "building the model");
                let outcome = match renzora_rvg::bake::convert_to_model(&job.glb)? {
                    Converted::Written { rvg, bytes, before } => Out::Written { rvg, bytes, before },
                    Converted::AlreadyConverted { rvg } => Out::AlreadyConverted { rvg },
                    Converted::NameTaken { rvg } => Out::NameTaken { rvg },
                };
                serde_json::to_string(&outcome).map_err(|e| e.to_string())
            }
            renzora_import::remote::CONVERT_KIND => model_convert(args(&payload)?, ctx),
            renzora_import::remote::BUILD_KIND => {
                let glb: std::path::PathBuf = args(&payload)?;
                ctx.progress(None, "building virtual geometry");
                renzora_import::remote::build_model_file(&glb)
                    .map(str::to_string)
                    .map_err(|e| format!("could not store {}: {e}", glb.display()))
            }
            renzora_export::remote::KIND => {
                ctx.progress(None, "Packing assets...");
                renzora_export::remote::run_in_companion(&payload, &mut |line| ctx.progress(None, line), &|| {
                    ctx.cancelled()
                })
            }
            renzora_marketplace::install_remote::KIND => {
                renzora_marketplace::install_remote::run_in_companion(&payload, &mut |fraction, note| {
                    ctx.progress(fraction, note)
                })
            }
            renzora_marketplace::thumbs::KIND => {
                let job: renzora_marketplace::thumbs::HubThumbnail = args(&payload)?;
                renzora_marketplace::thumbs::fetch_hub_thumbnail(&job).map(|()| job.image.display().to_string())
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
    let ext = args.source.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let image = if ext == "svg" || ext == "svgz" {
        // Drawn at thumbnail size by the engine's own rasterizer; it is already
        // small, so the resize below changes nothing.
        let (w, h, pixels) = renzora_svg::raster::thumbnail(&args.source, args.size)?;
        image::RgbaImage::from_raw(w, h, pixels)
            .map(image::DynamicImage::ImageRgba8)
            .ok_or_else(|| format!("{} drew to the wrong number of pixels", args.source.display()))?
    } else {
        image::open(&args.source).map_err(|e| format!("could not read {}: {e}", args.source.display()))?
    };
    if ctx.cancelled() {
        return Err("cancelled".into());
    }
    let (w, h) = (image.width(), image.height());
    let (tw, th) = fit(w, h, args.size);
    let small = if matches!(image, image::DynamicImage::ImageRgb32F(_) | image::DynamicImage::ImageRgba32F(_)) {
        // An HDR: tone-mapped the way the editor maps one (`c / (1 + c)`, then
        // sRGB), so a bright sky rolls off instead of clipping to white and the
        // tile matches one made there. Mapped before the shrink, still linear:
        // `resize` clamps float pixels to 1.0, so shrinking first clipped
        // every bright texel to white before the mapping ever saw it.
        let mut mapped = image.to_rgba32f();
        for p in mapped.pixels_mut() {
            for c in &mut p.0[..3] {
                let v = c.max(0.0);
                *c = v / (1.0 + v);
            }
            p.0[3] = 1.0;
        }
        let small = image::imageops::resize(&mapped, tw, th, image::imageops::FilterType::Lanczos3);
        let mut out = image::RgbaImage::new(tw, th);
        for (dst, src) in out.pixels_mut().zip(small.pixels()) {
            let [r, g, b, _] = src.0;
            let encode = |c: f32| {
                let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
                (s.clamp(0.0, 1.0) * 255.0).round() as u8
            };
            *dst = image::Rgba([encode(r), encode(g), encode(b), 255]);
        }
        out
    } else {
        image::imageops::resize(&image.to_rgba8(), tw, th, image::imageops::FilterType::Lanczos3)
    };
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
    fn an_svg_thumbnail_is_drawn_at_thumbnail_size() {
        let dir = std::env::temp_dir().join(format!("rz-svg-thumb-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("icon.svg");
        // A 24 px icon, half red: drawn up to the tile, not left tiny.
        std::fs::write(
            &source,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="12"><rect width="12" height="12" fill="red"/></svg>"#,
        )
        .unwrap();
        let cache = dir.join("cache").join("icon.png");
        let (ctx, _reports) = JobCtx::for_test();
        let result = texture_thumbnail(job_args::TextureThumbnail { source, cache: cache.clone(), size: 256 }, &ctx);
        assert!(result.is_ok(), "{result:?}");
        let written = image::open(&cache).unwrap().to_rgba8();
        assert_eq!(written.dimensions(), (256, 128));
        assert_eq!(written.get_pixel(10, 64).0, [255, 0, 0, 255]);
        assert_eq!(written.get_pixel(250, 64).0[3], 0, "the right half is transparent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_hdr_thumbnail_is_tone_mapped_not_clipped() {
        let dir = std::env::temp_dir().join(format!("rz-hdr-thumb-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("sky.hdr");
        // Four times brighter than white in red, nothing in the others.
        let sky = image::Rgb32FImage::from_pixel(1024, 512, image::Rgb([4.0, 0.0, 0.0]));
        image::DynamicImage::ImageRgb32F(sky).save(&source).unwrap();
        let cache = dir.join("cache").join("sky.png");
        let (ctx, _reports) = JobCtx::for_test();
        let result = texture_thumbnail(job_args::TextureThumbnail { source, cache: cache.clone(), size: 256 }, &ctx);
        assert!(result.is_ok(), "{result:?}");
        let written = image::open(&cache).unwrap().to_rgba8();
        assert_eq!(written.dimensions(), (256, 128));
        let red = written.get_pixel(10, 10).0[0];
        assert!(red > 200 && red < 255, "4.0 maps to 0.8 linear, below white: got {red}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_kind_fails_in_words() {
        let (ctx, _reports) = JobCtx::for_test();
        let result = run(Job::Run { kind: "teleport".into(), payload: "{}".into(), target: String::new() }, &ctx);
        assert!(result.unwrap_err().contains("teleport"));
    }
}
