use bevy::prelude::*;
use bevy_args::{Deserialize, Parser, Serialize};

use crate::gaussian::settings::{GaussianMode, PlaybackMode, RadixSortDepthBits, RasterizeMode};

#[derive(Debug, Resource, Serialize, Deserialize, Parser)]
#[command(about = "bevy_gaussian_splatting viewer", version, long_about = None)]
pub struct GaussianSplattingViewer {
    #[arg(long, default_value = "true")]
    pub editor: bool,

    #[arg(long, default_value = "true")]
    pub press_esc_close: bool,

    #[arg(long, default_value = "true")]
    pub press_s_screenshot: bool,

    #[arg(long, default_value = "false")]
    pub show_axes: bool,

    #[arg(long, default_value = "true")]
    pub show_fps: bool,

    #[arg(long, default_value = "1920.0")]
    pub width: f32,

    #[arg(long, default_value = "1080.0")]
    pub height: f32,

    #[arg(long, default_value = "bevy_gaussian_splatting")]
    pub name: String,

    #[arg(long, default_value = "1")]
    pub msaa_samples: u8,

    #[arg(long, default_value = None, help = "input file path (or url/base64_url if web_asset feature is enabled)")]
    pub input_cloud: Option<String>,

    #[arg(
        long,
        default_value = None,
        help = "secondary input file used when morph_interpolate is enabled",
    )]
    pub input_cloud_target: Option<String>,

    #[arg(long, default_value = None, help = "input glTF/GLB scene path (or url/base64_url if web_asset feature is enabled)")]
    pub input_scene: Option<String>,

    #[arg(long, default_value = None, help = "cloud translation as x,y,z")]
    pub cloud_translation: Option<String>,

    #[arg(long, default_value = None, help = "cloud rotation in degrees as x,y,z")]
    pub cloud_rotation: Option<String>,

    #[arg(long, default_value = None, help = "cloud scale as uniform or x,y,z")]
    pub cloud_scale: Option<String>,

    #[arg(long, default_value = "0")]
    pub gaussian_count: usize,

    #[arg(long, default_value = None, help = "seed for random gaussian generation")]
    pub gaussian_seed: Option<u64>,

    #[arg(long, value_enum, default_value_t = GaussianMode::Gaussian3d)]
    pub gaussian_mode: GaussianMode,

    #[arg(long, value_enum, default_value_t = PlaybackMode::Still)]
    pub playback_mode: PlaybackMode,

    #[arg(long, value_enum, default_value_t = RasterizeMode::Color)]
    pub rasterization_mode: RasterizeMode,

    #[arg(long, value_enum, default_value_t = RadixSortDepthBits::Bits32)]
    pub radix_sort_depth_bits: RadixSortDepthBits,

    #[arg(long, default_value = "0")]
    pub particle_count: usize,
}

impl Default for GaussianSplattingViewer {
    fn default() -> GaussianSplattingViewer {
        GaussianSplattingViewer {
            editor: true,
            press_esc_close: true,
            press_s_screenshot: true,
            show_axes: false,
            show_fps: true,
            width: 1920.0,
            height: 1080.0,
            name: "bevy_gaussian_splatting".to_string(),
            msaa_samples: 1,
            input_cloud: None,
            input_cloud_target: None,
            input_scene: None,
            cloud_translation: None,
            cloud_rotation: None,
            cloud_scale: None,
            gaussian_count: 0,
            gaussian_seed: None,
            gaussian_mode: GaussianMode::Gaussian3d,
            playback_mode: PlaybackMode::Still,
            rasterization_mode: RasterizeMode::Color,
            radix_sort_depth_bits: RadixSortDepthBits::Bits32,
            particle_count: 0,
        }
    }
}

impl GaussianSplattingViewer {
    pub fn cloud_transform(&self) -> Transform {
        let mut transform = Transform::default();

        if let Some(translation) = self.cloud_translation.as_deref().and_then(parse_vec3) {
            transform.translation = translation;
        }

        if let Some(rotation) = self.cloud_rotation.as_deref().and_then(parse_vec3) {
            transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                rotation.x.to_radians(),
                rotation.y.to_radians(),
                rotation.z.to_radians(),
            );
        }

        if let Some(scale) = self.cloud_scale.as_deref().and_then(parse_scale) {
            transform.scale = scale;
        }

        transform
    }
}

fn parse_vec3(value: &str) -> Option<Vec3> {
    let parts: Vec<&str> = value
        .split(&[',', ' ', '\t'][..])
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() != 3 {
        return None;
    }

    let x = parts[0].parse::<f32>().ok()?;
    let y = parts[1].parse::<f32>().ok()?;
    let z = parts[2].parse::<f32>().ok()?;

    Some(Vec3::new(x, y, z))
}

fn parse_scale(value: &str) -> Option<Vec3> {
    let parts: Vec<&str> = value
        .split(&[',', ' ', '\t'][..])
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }

    if parts.len() == 1 {
        let v = parts[0].parse::<f32>().ok()?;
        return Some(Vec3::splat(v));
    }

    if parts.len() != 3 {
        return None;
    }

    let x = parts[0].parse::<f32>().ok()?;
    let y = parts[1].parse::<f32>().ok()?;
    let z = parts[2].parse::<f32>().ok()?;

    Some(Vec3::new(x, y, z))
}

pub fn setup_hooks() {
    #[cfg(debug_assertions)]
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
    }
}

pub fn log(_msg: &str) {
    #[cfg(debug_assertions)]
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::log_1(&_msg.into());
    }
    #[cfg(debug_assertions)]
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("{_msg}");
    }
}
