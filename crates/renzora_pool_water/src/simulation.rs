use bevy::prelude::*;
use bevy::image::{Image, ImageSampler, ImageAddressMode, ImageFilterMode};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::asset::RenderAssetUsages;

/// CPU-side heightfield water simulation.
/// Uses the wave equation with velocity verlet integration,
/// matching the webgpu-water approach but computed on CPU.
///
/// Texture layout (RGBA32Float):
///   R = height
///   G = velocity
///   B = normal.x
///   A = normal.z
#[derive(Component)]
pub struct WaterSim {
    pub width: usize,
    pub height: usize,
    /// Current height values
    pub heights: Vec<f32>,
    /// Current velocity values
    pub velocity: Vec<f32>,
    /// Computed normals (x, z) — y is derived
    pub normal_x: Vec<f32>,
    pub normal_z: Vec<f32>,
    /// GPU texture handle
    pub texture_handle: Handle<Image>,
    /// Damping factor (0.99–0.999)
    pub damping: f32,
    /// Wave propagation speed
    pub speed: f32,
}

impl WaterSim {
    pub fn new(width: usize, height: usize, damping: f32, speed: f32, images: &mut Assets<Image>) -> Self {
        let n = width * height;
        let texture_handle = images.add(Self::create_image(width, height));
        Self {
            width,
            height,
            heights: vec![0.0; n],
            velocity: vec![0.0; n],
            normal_x: vec![0.0; n],
            normal_z: vec![0.0; n],
            texture_handle,
            damping,
            speed,
        }
    }

    fn create_image(width: usize, height: usize) -> Image {
        let data = vec![0u8; width * height * 16]; // 4 f32 per pixel
        let mut image = Image::new(
            Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba32Float,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::Descriptor(bevy::image::ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::ClampToEdge,
            address_mode_v: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            ..default()
        });
        image
    }

    /// Inject a drop/ripple at normalized position (0–1, 0–1).
    pub fn add_drop(&mut self, center_x: f32, center_y: f32, radius: f32, strength: f32) {
        let cx = center_x * self.width as f32;
        let cy = center_y * self.height as f32;
        let r = radius * self.width as f32;
        let r2 = r * r;

        let min_x = ((cx - r).floor() as isize).max(0) as usize;
        let max_x = ((cx + r).ceil() as usize).min(self.width - 1);
        let min_y = ((cy - r).floor() as isize).max(0) as usize;
        let max_y = ((cy + r).ceil() as usize).min(self.height - 1);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let dist2 = dx * dx + dy * dy;
                if dist2 < r2 {
                    let dist = dist2.sqrt() / r;
                    let drop = 0.5 - (dist * std::f32::consts::PI).cos() * 0.5;
                    self.heights[y * self.width + x] += drop * strength;
                }
            }
        }
    }

    /// Step the simulation forward (wave equation + normals).
    pub fn step(&mut self) {
        let w = self.width;
        let h = self.height;

        // Wave equation update (run twice per frame like the original)
        for _ in 0..2 {
            for y in 0..h {
                for x in 0..w {
                    let idx = y * w + x;

                    // Sample neighbors with clamped boundaries
                    let left = if x > 0 { self.heights[idx - 1] } else { self.heights[idx] };
                    let right = if x < w - 1 { self.heights[idx + 1] } else { self.heights[idx] };
                    let up = if y > 0 { self.heights[idx - w] } else { self.heights[idx] };
                    let down = if y < h - 1 { self.heights[idx + w] } else { self.heights[idx] };

                    let average = (left + right + up + down) * 0.25;
                    self.velocity[idx] += (average - self.heights[idx]) * self.speed;
                    self.velocity[idx] *= self.damping;
                }
            }

            // Apply velocity to heights (separate pass to avoid read-write conflict)
            for i in 0..w * h {
                self.heights[i] += self.velocity[i];
            }
        }

        // Compute normals from height gradient
        let inv_w = 1.0 / w as f32;
        let inv_h = 1.0 / h as f32;

        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let val = self.heights[idx];

                let val_right = if x < w - 1 { self.heights[idx + 1] } else { val };
                let val_down = if y < h - 1 { self.heights[idx + w] } else { val };

                // Tangent vectors
                let dx = Vec3::new(inv_w, val_right - val, 0.0);
                let dy = Vec3::new(0.0, val_down - val, inv_h);
                let normal = dy.cross(dx).normalize_or_zero();

                self.normal_x[idx] = normal.x;
                self.normal_z[idx] = normal.z;
            }
        }
    }

    /// Upload current simulation state to GPU texture.
    pub fn upload(&self, images: &mut Assets<Image>) {
        let Some(image) = images.get_mut(&self.texture_handle) else { return };
        let data = image.data.as_mut().expect("image data");
        let n = self.width * self.height;

        for i in 0..n {
            let offset = i * 16; // 4 f32s × 4 bytes
            let h_bytes = self.heights[i].to_le_bytes();
            let v_bytes = self.velocity[i].to_le_bytes();
            let nx_bytes = self.normal_x[i].to_le_bytes();
            let nz_bytes = self.normal_z[i].to_le_bytes();

            data[offset..offset + 4].copy_from_slice(&h_bytes);
            data[offset + 4..offset + 8].copy_from_slice(&v_bytes);
            data[offset + 8..offset + 12].copy_from_slice(&nx_bytes);
            data[offset + 12..offset + 16].copy_from_slice(&nz_bytes);
        }
    }
}
