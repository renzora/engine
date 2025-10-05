use std::path::Path;
use std::fs;
use log::{info, error};
use image::{ImageBuffer, RgbImage, Rgb, DynamicImage, ImageFormat, GenericImageView};

/// Generate thumbnail for regular image formats (PNG, JPG, GIF, BMP, TIFF, WebP)
pub async fn generate_image_thumbnail(
    project_name: &str,
    asset_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("🖼️ Generating image thumbnail for: {}/{}", project_name, asset_path);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if file exists
    if !full_asset_path.exists() {
        return Err("Image file not found".into());
    }

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = crate::modules::thumbnail_generator::get_thumbnails_dir(project_name);
    fs::create_dir_all(&thumbnails_dir)?;

    // Generate filename for thumbnail
    let asset_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let asset_extension = full_asset_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("img");
    let thumbnail_filename = format!("{}_{}_{}.png", asset_filename, asset_extension, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);

    // Try to load and resize the image
    match load_and_resize_image(&full_asset_path, &thumbnail_path, size) {
        Ok(_) => {
            info!("✅ Successfully generated image thumbnail: {:?}", thumbnail_path);
            Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
        }
        Err(e) => {
            error!("❌ Failed to generate image thumbnail for {:?}: {}", full_asset_path, e);
            
            // Create fallback thumbnail
            create_image_fallback_thumbnail(&full_asset_path, &thumbnail_path, size)?;
            Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
        }
    }
}

/// Load an image file and resize it to create a thumbnail
fn load_and_resize_image(
    source_path: &Path,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("📸 Loading and resizing image: {:?}", source_path);
    
    // Try to load the image
    let img = image::open(source_path)?;
    
    // Get original dimensions
    let (original_width, original_height) = img.dimensions();
    info!("📏 Original dimensions: {}x{}", original_width, original_height);
    
    // Calculate new dimensions maintaining aspect ratio
    let (new_width, new_height) = calculate_thumbnail_dimensions(
        original_width, 
        original_height, 
        size
    );
    
    info!("🔄 Resizing to: {}x{}", new_width, new_height);
    
    // Resize the image using a high-quality filter
    let resized = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);
    
    // If the resized image is not square, pad it to make it square
    let final_image = if new_width != new_height || new_width != size || new_height != size {
        pad_to_square(resized, size)
    } else {
        resized
    };
    
    // Save as PNG
    final_image.save_with_format(thumbnail_path, ImageFormat::Png)?;
    
    info!("💾 Thumbnail saved: {:?}", thumbnail_path);
    Ok(())
}


/// Calculate thumbnail dimensions while maintaining aspect ratio
fn calculate_thumbnail_dimensions(
    original_width: u32,
    original_height: u32,
    target_size: u32,
) -> (u32, u32) {
    if original_width == 0 || original_height == 0 {
        return (target_size, target_size);
    }
    
    let aspect_ratio = original_width as f32 / original_height as f32;
    
    if aspect_ratio > 1.0 {
        // Landscape - fit to width
        let new_width = target_size;
        let new_height = (target_size as f32 / aspect_ratio) as u32;
        (new_width, new_height)
    } else {
        // Portrait or square - fit to height
        let new_height = target_size;
        let new_width = (target_size as f32 * aspect_ratio) as u32;
        (new_width, new_height)
    }
}

/// Pad an image to make it square with a centered crop
fn pad_to_square(img: DynamicImage, target_size: u32) -> DynamicImage {
    let (width, height) = img.dimensions();
    
    if width == target_size && height == target_size {
        return img;
    }
    
    // Create a new square image with a neutral background
    let mut square_img = DynamicImage::new_rgb8(target_size, target_size);
    
    // Fill with a light gray background
    if let DynamicImage::ImageRgb8(ref mut buffer) = square_img {
        for pixel in buffer.pixels_mut() {
            *pixel = image::Rgb([240, 240, 240]);
        }
    }
    
    // Calculate position to center the image
    let x_offset = (target_size - width) / 2;
    let y_offset = (target_size - height) / 2;
    
    // Overlay the resized image onto the square background
    image::imageops::overlay(&mut square_img, &img, x_offset as i64, y_offset as i64);
    
    square_img
}

/// Create a fallback thumbnail when image loading fails
fn create_image_fallback_thumbnail(
    source_path: &Path,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating fallback thumbnail for image: {:?}", source_path);
    
    // Get file info
    let file_size = fs::metadata(source_path)?.len();
    let extension = source_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("img")
        .to_uppercase();
    
    // Create a placeholder image with file info
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Create a subtle gradient background
        let gradient = (x_norm + y_norm) / 2.0;
        let gray_val = (200.0 + gradient * 40.0) as u8;
        Rgb([gray_val, gray_val, gray_val])
    });
    
    // Draw a simple image icon in the center
    draw_image_icon(&mut img, size);
    
    // Add format indicator
    draw_format_badge(&mut img, &extension, size);
    
    // Add file size indicator
    let size_text = format_file_size(file_size);
    draw_size_indicator(&mut img, &size_text, size);
    
    // Save the placeholder
    img.save(thumbnail_path)?;
    
    info!("✅ Fallback thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Draw a simple image icon
fn draw_image_icon(img: &mut RgbImage, size: u32) {
    let center_x = size / 2;
    let center_y = size / 2;
    let icon_size = size / 4;
    let half_icon = icon_size / 2;
    
    let icon_color = Rgb([100, 100, 100]);
    let highlight_color = Rgb([150, 150, 150]);
    
    // Draw image frame
    let frame_x = center_x - half_icon;
    let frame_y = center_y - half_icon;
    let frame_width = icon_size;
    let frame_height = icon_size;
    
    // Frame border
    for x in frame_x..frame_x + frame_width {
        for y in [frame_y, frame_y + frame_height - 1] {
            if x < size && y < size {
                img.put_pixel(x, y, icon_color);
            }
        }
    }
    
    for y in frame_y..frame_y + frame_height {
        for x in [frame_x, frame_x + frame_width - 1] {
            if x < size && y < size {
                img.put_pixel(x, y, icon_color);
            }
        }
    }
    
    // Draw mountain-like shape inside frame (simple image icon)
    let mountain_points = [
        (frame_x + frame_width / 4, frame_y + frame_height * 3 / 4),
        (frame_x + frame_width / 2, frame_y + frame_height / 3),
        (frame_x + frame_width * 3 / 4, frame_y + frame_height * 2 / 3),
        (frame_x + frame_width - 2, frame_y + frame_height * 3 / 4),
    ];
    
    // Simple mountain silhouette
    for i in 0..mountain_points.len() - 1 {
        draw_simple_line(img, mountain_points[i], mountain_points[i + 1], highlight_color);
    }
    
    // Draw sun circle
    let sun_x = frame_x + frame_width * 3 / 4;
    let sun_y = frame_y + frame_height / 4;
    let sun_radius = frame_width / 8;
    
    for dy in 0..sun_radius * 2 {
        for dx in 0..sun_radius * 2 {
            let px = sun_x - sun_radius + dx;
            let py = sun_y - sun_radius + dy;
            
            if px < size && py < size {
                let dist_sq = (dx as i32 - sun_radius as i32).pow(2) + (dy as i32 - sun_radius as i32).pow(2);
                if dist_sq <= (sun_radius as i32).pow(2) {
                    img.put_pixel(px, py, highlight_color);
                }
            }
        }
    }
}

/// Draw a format badge in the top-right corner
fn draw_format_badge(img: &mut RgbImage, _format: &str, size: u32) {
    let badge_size = size / 6;
    let badge_x = size - badge_size - 4;
    let badge_y = 4;
    
    // Draw background
    for dy in 0..badge_size {
        for dx in 0..badge_size {
            let px = badge_x + dx;
            let py = badge_y + dy;
            
            if px < size && py < size {
                img.put_pixel(px, py, Rgb([60, 60, 60]));
            }
        }
    }
    
    // Add border
    for dx in 0..badge_size {
        if badge_x + dx < size {
            img.put_pixel(badge_x + dx, badge_y, Rgb([120, 120, 120]));
            if badge_y + badge_size - 1 < size {
                img.put_pixel(badge_x + dx, badge_y + badge_size - 1, Rgb([120, 120, 120]));
            }
        }
    }
    
    for dy in 0..badge_size {
        if badge_y + dy < size {
            img.put_pixel(badge_x, badge_y + dy, Rgb([120, 120, 120]));
            if badge_x + badge_size - 1 < size {
                img.put_pixel(badge_x + badge_size - 1, badge_y + dy, Rgb([120, 120, 120]));
            }
        }
    }
}

/// Draw a file size indicator in the bottom-left corner
fn draw_size_indicator(img: &mut RgbImage, _size_text: &str, size: u32) {
    let indicator_height = size / 12;
    let indicator_width = size / 3;
    let indicator_x = 4;
    let indicator_y = size - indicator_height - 4;
    
    // Draw semi-transparent background
    for dy in 0..indicator_height {
        for dx in 0..indicator_width {
            let px = indicator_x + dx;
            let py = indicator_y + dy;
            
            if px < size && py < size {
                img.put_pixel(px, py, Rgb([40, 40, 40]));
            }
        }
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

/// Format file size for display
fn format_file_size(size: u64) -> String {
    if size > 1_000_000 {
        format!("{:.1}MB", size as f32 / 1_000_000.0)
    } else if size > 1_000 {
        format!("{:.1}KB", size as f32 / 1_000.0)
    } else {
        format!("{}B", size)
    }
}

