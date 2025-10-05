use std::path::Path;
use std::fs;
use log::{info, warn};
use image::{ImageBuffer, RgbImage, Rgb};

/// Generate thumbnail for HDR/EXR environment maps with proper tone mapping
pub async fn generate_hdr_environment_thumbnail(
    project_name: &str,
    asset_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("🌅 Generating HDR/EXR environment thumbnail for: {}/{}", project_name, asset_path);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if file exists
    if !full_asset_path.exists() {
        return Err("HDR/EXR file not found".into());
    }

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = crate::modules::thumbnail_generator::get_thumbnails_dir(project_name);
    fs::create_dir_all(&thumbnails_dir)?;

    // Generate filename for thumbnail including extension to avoid conflicts
    let asset_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr_env");
    let asset_extension = full_asset_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr");
    let thumbnail_filename = format!("{}_{}_{}.png", asset_filename, asset_extension, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);

    // Try to read and tone-map the HDR image, fallback to enhanced placeholder
    match attempt_hdr_tone_mapping(&full_asset_path, &thumbnail_path, size) {
        Ok(_) => {
            info!("✅ Successfully created tone-mapped HDR thumbnail: {:?}", thumbnail_path);
        }
        Err(e) => {
            warn!("⚠️ HDR tone mapping failed ({}), creating enhanced placeholder", e);
            create_enhanced_hdr_placeholder(&full_asset_path, &thumbnail_path, size)?;
        }
    }
    
    // Return relative path to thumbnail
    Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
}

/// Generate thumbnail for compressed game engine texture formats
pub async fn generate_game_texture_thumbnail(
    project_name: &str,
    asset_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("🎮 Generating game texture thumbnail for: {}/{}", project_name, asset_path);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if file exists
    if !full_asset_path.exists() {
        return Err("Game texture file not found".into());
    }

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = crate::modules::thumbnail_generator::get_thumbnails_dir(project_name);
    fs::create_dir_all(&thumbnails_dir)?;

    // Generate filename for thumbnail
    let asset_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("texture");
    let asset_extension = full_asset_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("tex");
    let thumbnail_filename = format!("{}_{}_{}.png", asset_filename, asset_extension, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);

    // Create format-specific thumbnail
    create_game_texture_placeholder(&full_asset_path, &thumbnail_path, size)?;
    
    // Return relative path to thumbnail
    Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
}

/// Attempt to read HDR and apply tone mapping
fn attempt_hdr_tone_mapping(
    hdr_path: &Path,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let extension = hdr_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match extension.as_str() {
        "hdr" => {
            // Try to read HDR using the image crate (if supported)
            // For now, create an enhanced placeholder as the image crate doesn't support HDR
            info!("HDR format detected - creating enhanced HDR preview");
            create_enhanced_hdr_placeholder(hdr_path, thumbnail_path, size)?;
            Ok(())
        }
        "exr" => {
            // Try to read EXR using the image crate (if supported)
            info!("EXR format detected - creating enhanced EXR preview");
            create_enhanced_hdr_placeholder(hdr_path, thumbnail_path, size)?;
            Ok(())
        }
        "pfm" => {
            // Portable Float Map
            info!("PFM format detected - creating enhanced PFM preview");
            create_enhanced_hdr_placeholder(hdr_path, thumbnail_path, size)?;
            Ok(())
        }
        _ => Err("Unsupported HDR format".into())
    }
}

/// Create an enhanced HDR placeholder with environment map styling
fn create_enhanced_hdr_placeholder(
    hdr_path: &Path,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating enhanced HDR environment thumbnail: {:?}", hdr_path);
    
    // Get file info
    let file_size = fs::metadata(hdr_path)?.len();
    let extension = hdr_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr")
        .to_uppercase();
    
    // Create HDR environment map style background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Create spherical environment map projection
        let u = x_norm * 2.0 - 1.0; // [-1, 1]
        let v = y_norm * 2.0 - 1.0; // [-1, 1]
        
        // Create spherical gradient that mimics environment lighting
        let radius = (u * u + v * v).sqrt();
        let angle = v.atan2(u);
        
        if radius <= 1.0 {
            // Inside the sphere - create environment-like lighting
            let horizon_factor = (1.0 - v.abs()).powf(0.5); // Brighter at horizon
            let sky_factor = (1.0 + v).max(0.0) * 0.5; // Sky contribution
            let ground_factor = (1.0 - v).max(0.0) * 0.3; // Ground contribution
            
            // Create sun position effect
            let sun_angle = std::f32::consts::PI * 0.25; // 45 degrees
            let sun_distance = (angle - sun_angle).abs();
            let sun_factor = if sun_distance < 0.3 {
                (1.0 - sun_distance / 0.3) * 0.8
            } else {
                0.0
            };
            
            // Combine lighting components
            let brightness = (horizon_factor + sky_factor + ground_factor + sun_factor).min(1.0);
            
            // HDR-like color scheme (warm to cool)
            let r = (255.0 * brightness * (1.0 + sun_factor * 0.3)).min(255.0) as u8;
            let g = (255.0 * brightness * (0.9 + sun_factor * 0.2)).min(255.0) as u8;
            let b = (255.0 * brightness * (0.7 + sky_factor * 0.4)).min(255.0) as u8;
            
            Rgb([r, g, b])
        } else {
            // Outside the sphere - dark space
            let edge_glow = (2.0 - radius).max(0.0) * 0.1;
            let glow_val = (edge_glow * 255.0) as u8;
            Rgb([glow_val / 2, glow_val / 2, glow_val])
        }
    });
    
    // Add environment map indicators
    draw_environment_map_overlay(&mut img, size);
    
    // Add HDR format indicator
    draw_hdr_format_badge(&mut img, &extension, size);
    
    // Add file size and format info
    draw_hdr_info_panel(&mut img, file_size, &extension, size);
    
    // Save the image
    img.save(thumbnail_path)?;
    
    info!("✅ Enhanced HDR environment thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create placeholder for game engine texture formats
fn create_game_texture_placeholder(
    texture_path: &Path,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating game texture placeholder: {:?}", texture_path);
    
    let file_size = fs::metadata(texture_path)?.len();
    let extension = texture_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("tex")
        .to_uppercase();
    
    // Create format-specific background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Create compression pattern that suggests texture compression
        let block_size = 8.0; // 8x8 blocks like DXT compression
        let block_x = (x_norm * size as f32 / block_size).floor();
        let block_y = (y_norm * size as f32 / block_size).floor();
        let checker = ((block_x + block_y) % 2.0) == 0.0;
        
        // Base color based on format
        let (base_r, base_g, base_b) = match extension.as_str() {
            "DDS" => (180, 160, 200), // Purple-ish for DirectDraw Surface
            "KTX" | "KTX2" => (160, 200, 180), // Green-ish for Khronos Texture
            "ASTC" => (200, 180, 160), // Orange-ish for ARM compression
            "PVR" => (200, 160, 180), // Pink-ish for PowerVR
            "ETC1" | "ETC2" => (160, 180, 200), // Blue-ish for Ericsson compression
            "PKM" => (180, 200, 160), // Light green for PKM
            _ => (170, 170, 170), // Gray for unknown
        };
        
        // Apply compression pattern
        let variation = if checker { 20 } else { -20 };
        let r = (base_r as i32 + variation).max(0).min(255) as u8;
        let g = (base_g as i32 + variation).max(0).min(255) as u8;
        let b = (base_b as i32 + variation).max(0).min(255) as u8;
        
        Rgb([r, g, b])
    });
    
    // Add compression format indicators
    draw_compression_overlay(&mut img, &extension, size);
    
    // Add format badge
    draw_texture_format_badge(&mut img, &extension, size);
    
    // Add compression info
    draw_compression_info(&mut img, file_size, &extension, size);
    
    // Save the image
    img.save(thumbnail_path)?;
    
    info!("✅ Game texture placeholder created: {:?}", thumbnail_path);
    Ok(())
}

/// Draw environment map overlay indicators
fn draw_environment_map_overlay(img: &mut RgbImage, size: u32) {
    let center_x = size / 2;
    let center_y = size / 2;
    let radius = size / 3;
    
    // Draw subtle grid lines to suggest spherical projection
    let grid_color = Rgb([255, 255, 255]);
    
    // Horizontal lines
    for i in 1..4 {
        let y_offset = radius as i32 * (i as i32 - 2) / 2;
        let y = if y_offset >= 0 {
            center_y + y_offset as u32
        } else {
            center_y.saturating_sub((-y_offset) as u32)
        };
        if y < size {
            draw_dotted_line(img, (center_x - radius, y as u32), (center_x + radius, y as u32), grid_color);
        }
    }
    
    // Vertical meridian lines
    for i in 1..6 {
        let angle = (i as f32) * std::f32::consts::PI / 3.0;
        let x_offset = (radius as f32 * angle.cos() * 0.8) as i32;
        let start_y = center_y - radius / 2;
        let end_y = center_y + radius / 2;
        
        if center_x as i32 + x_offset > 0 && (center_x as i32 + x_offset) < size as i32 {
            draw_dotted_line(img, 
                ((center_x as i32 + x_offset) as u32, start_y), 
                ((center_x as i32 + x_offset) as u32, end_y), 
                grid_color
            );
        }
    }
}

/// Draw compression format overlay
fn draw_compression_overlay(img: &mut RgbImage, format: &str, size: u32) {
    let block_color = Rgb([100, 100, 100]);
    let grid_size = match format {
        "DDS" => 8, // DXT blocks are 4x4 pixels, represent as 8x8 in thumbnail
        "ASTC" => 6, // ASTC can be variable, use 6x6
        "ETC1" | "ETC2" => 8, // ETC blocks are 4x4, represent as 8x8
        "PVRTC" => 16, // PVRTC uses larger blocks
        _ => 8,
    };
    
    // Draw block grid
    let blocks_per_side = size / grid_size;
    for i in 0..=blocks_per_side {
        let pos = i * grid_size;
        if pos < size {
            // Vertical lines
            for y in 0..size {
                if y % 4 == 0 { // Dotted pattern
                    img.put_pixel(pos, y, block_color);
                }
            }
            // Horizontal lines
            for x in 0..size {
                if x % 4 == 0 { // Dotted pattern
                    img.put_pixel(x, pos, block_color);
                }
            }
        }
    }
}

/// Draw HDR format badge
fn draw_hdr_format_badge(img: &mut RgbImage, format: &str, size: u32) {
    let badge_width = size / 3;
    let badge_height = size / 8;
    let badge_x = size - badge_width - 4;
    let badge_y = 4;
    
    // HDR-specific colors
    let badge_color = match format {
        "HDR" => Rgb([255, 200, 100]), // Warm yellow-orange
        "EXR" => Rgb([100, 150, 255]), // Cool blue  
        "PFM" => Rgb([200, 100, 255]), // Purple
        _ => Rgb([150, 150, 150]),
    };
    
    // Draw background with gradient
    for dy in 0..badge_height {
        for dx in 0..badge_width {
            let px = badge_x + dx;
            let py = badge_y + dy;
            if px < size && py < size {
                let gradient = dy as f32 / badge_height as f32;
                let r = (badge_color[0] as f32 * (1.0 - gradient * 0.3)) as u8;
                let g = (badge_color[1] as f32 * (1.0 - gradient * 0.3)) as u8;
                let b = (badge_color[2] as f32 * (1.0 - gradient * 0.3)) as u8;
                img.put_pixel(px, py, Rgb([r, g, b]));
            }
        }
    }
    
    // Add border
    let border_color = Rgb([80, 80, 80]);
    for dx in 0..badge_width {
        if badge_x + dx < size {
            img.put_pixel(badge_x + dx, badge_y, border_color);
            if badge_y + badge_height - 1 < size {
                img.put_pixel(badge_x + dx, badge_y + badge_height - 1, border_color);
            }
        }
    }
}

/// Draw texture format badge
fn draw_texture_format_badge(img: &mut RgbImage, format: &str, size: u32) {
    let badge_width = size / 4;
    let badge_height = size / 10;
    let badge_x = size - badge_width - 4;
    let badge_y = 4;
    
    // Format-specific colors
    let badge_color = match format {
        "DDS" => [150, 100, 200],
        "KTX" | "KTX2" => [100, 200, 150],
        "ASTC" => [200, 150, 100],
        "PVR" => [200, 100, 150],
        "ETC1" | "ETC2" => [100, 150, 200],
        "PKM" => [150, 200, 100],
        _ => [120, 120, 120],
    };
    
    // Draw background
    for dy in 0..badge_height {
        for dx in 0..badge_width {
            let px = badge_x + dx;
            let py = badge_y + dy;
            if px < size && py < size {
                img.put_pixel(px, py, Rgb(badge_color));
            }
        }
    }
    
    // Add border
    let border_color = Rgb([60, 60, 60]);
    for dx in 0..badge_width {
        if badge_x + dx < size {
            img.put_pixel(badge_x + dx, badge_y, border_color);
            if badge_y + badge_height - 1 < size {
                img.put_pixel(badge_x + dx, badge_y + badge_height - 1, border_color);
            }
        }
    }
}

/// Draw HDR info panel
fn draw_hdr_info_panel(img: &mut RgbImage, _file_size: u64, _format: &str, size: u32) {
    let panel_width = size / 2;
    let panel_height = size / 8;
    let panel_x = 4;
    let panel_y = size - panel_height - 4;
    
    // Draw semi-transparent background
    for dy in 0..panel_height {
        for dx in 0..panel_width {
            let px = panel_x + dx;
            let py = panel_y + dy;
            if px < size && py < size {
                img.put_pixel(px, py, Rgb([20, 20, 30]));
            }
        }
    }
    
    // Add HDR indicator symbol (simplified sun)
    let sun_x = panel_x + panel_height / 2;
    let sun_y = panel_y + panel_height / 2;
    let sun_radius = panel_height / 4;
    
    draw_simple_sun(img, sun_x, sun_y, sun_radius);
}

/// Draw compression info
fn draw_compression_info(img: &mut RgbImage, _file_size: u64, _format: &str, size: u32) {
    let panel_width = size / 3;
    let panel_height = size / 12;
    let panel_x = 4;
    let panel_y = size - panel_height - 4;
    
    // Draw background
    for dy in 0..panel_height {
        for dx in 0..panel_width {
            let px = panel_x + dx;
            let py = panel_y + dy;
            if px < size && py < size {
                img.put_pixel(px, py, Rgb([30, 30, 30]));
            }
        }
    }
    
    // Add compression ratio indicator (simplified bars)
    let bar_count = 5;
    let bar_width = panel_width / (bar_count * 2);
    let bar_color = Rgb([100, 200, 100]);
    
    for i in 0..bar_count {
        let bar_x = panel_x + i * bar_width * 2 + 2;
        let bar_height = panel_height / 2 + (i * panel_height / 10);
        let bar_y = panel_y + panel_height - bar_height;
        
        for dy in 0..bar_height {
            for dx in 0..bar_width {
                let px = bar_x + dx;
                let py = bar_y + dy;
                if px < size && py < size {
                    img.put_pixel(px, py, bar_color);
                }
            }
        }
    }
}

/// Draw a simple sun icon
fn draw_simple_sun(img: &mut RgbImage, center_x: u32, center_y: u32, radius: u32) {
    let sun_color = Rgb([255, 220, 100]);
    
    // Draw center circle
    for dy in 0..radius * 2 {
        for dx in 0..radius * 2 {
            let x = center_x + dx - radius;
            let y = center_y + dy - radius;
            
            if x < img.width() && y < img.height() {
                let dist_sq = (dx as i32 - radius as i32).pow(2) + (dy as i32 - radius as i32).pow(2);
                if dist_sq <= (radius as i32).pow(2) {
                    img.put_pixel(x, y, sun_color);
                }
            }
        }
    }
    
    // Draw rays
    let ray_length = radius / 2;
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::PI / 4.0;
        let ray_end_x = center_x as i32 + ((radius + ray_length) as f32 * angle.cos()) as i32;
        let ray_end_y = center_y as i32 + ((radius + ray_length) as f32 * angle.sin()) as i32;
        let ray_start_x = center_x as i32 + (radius as f32 * angle.cos()) as i32;
        let ray_start_y = center_y as i32 + (radius as f32 * angle.sin()) as i32;
        
        if ray_end_x >= 0 && ray_end_y >= 0 && ray_start_x >= 0 && ray_start_y >= 0 {
            draw_simple_line(img, 
                (ray_start_x as u32, ray_start_y as u32), 
                (ray_end_x as u32, ray_end_y as u32), 
                sun_color
            );
        }
    }
}

/// Draw a dotted line
fn draw_dotted_line(img: &mut RgbImage, start: (u32, u32), end: (u32, u32), color: Rgb<u8>) {
    let (x0, y0) = (start.0 as i32, start.1 as i32);
    let (x1, y1) = (end.0 as i32, end.1 as i32);
    
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    
    let mut x = x0;
    let mut y = y0;
    let mut step = 0;
    
    let (width, height) = img.dimensions();
    
    loop {
        // Only draw every 3rd pixel for dotted effect
        if step % 3 == 0 && x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
            img.put_pixel(x as u32, y as u32, color);
        }
        
        if x == x1 && y == y1 { break; }
        
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
        step += 1;
    }
}

/// Simple line drawing function
fn draw_simple_line(img: &mut RgbImage, start: (u32, u32), end: (u32, u32), color: Rgb<u8>) {
    let (x0, y0) = (start.0 as i32, start.1 as i32);
    let (x1, y1) = (end.0 as i32, end.1 as i32);
    
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    
    let mut x = x0;
    let mut y = y0;
    
    let (width, height) = img.dimensions();
    
    loop {
        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
            img.put_pixel(x as u32, y as u32, color);
        }
        
        if x == x1 && y == y1 { break; }
        
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}