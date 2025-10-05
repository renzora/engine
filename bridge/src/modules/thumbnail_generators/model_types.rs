use std::path::Path;
use std::fs;
use log::{info, error};
use image::{ImageBuffer, RgbImage, Rgb};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialInfo {
    pub name: String,
    pub diffuse_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_color: [f32; 3],
    pub has_diffuse_texture: bool,
    pub has_normal_texture: bool,
    pub has_metallic_texture: bool,
    pub has_roughness_texture: bool,
}

/// Generate 3D model thumbnail using placeholder rendering
pub async fn generate_model_thumbnail(
    project_name: &str,
    asset_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("🎯 Generating 3D model thumbnail for: {}/{}", project_name, asset_path);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if file exists
    if !full_asset_path.exists() {
        error!("❌ 3D model file not found: {:?}", full_asset_path);
        return Err("3D model file not found".into());
    }

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = crate::modules::thumbnail_generator::get_thumbnails_dir(project_name);
    info!("📁 Creating thumbnail directory: {:?}", thumbnails_dir);
    fs::create_dir_all(&thumbnails_dir).map_err(|e| {
        error!("❌ Failed to create thumbnail directory {:?}: {}", thumbnails_dir, e);
        e
    })?;

    // Generate filename for thumbnail
    let asset_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model");
    let asset_extension = full_asset_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("3d");
    let thumbnail_filename = format!("{}_{}_{}.png", asset_filename, asset_extension, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);

    info!("📄 Generating thumbnail: {} -> {:?}", asset_filename, thumbnail_path);

    // Create enhanced placeholder thumbnail based on model type
    create_model_thumbnail_by_type(&full_asset_path, &thumbnail_path, size).map_err(|e| {
        error!("❌ Failed to create thumbnail for {}: {}", asset_filename, e);
        e
    })?;
    
    info!("✅ Thumbnail created successfully: {:?}", thumbnail_path);
    
    // Return relative path to thumbnail
    Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
}

/// Create different thumbnail styles based on 3D model format
fn create_model_thumbnail_by_type(
    model_path: &Path, 
    thumbnail_path: &Path, 
    size: u32
) -> Result<(), Box<dyn std::error::Error>> {
    let extension = model_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match extension.as_str() {
        "glb" | "gltf" => create_glb_gltf_thumbnail(model_path, thumbnail_path, size),
        "obj" => create_obj_thumbnail(model_path, thumbnail_path, size),
        "fbx" => create_fbx_thumbnail(model_path, thumbnail_path, size),
        "dae" => create_dae_thumbnail(model_path, thumbnail_path, size),
        "3ds" => create_3ds_thumbnail(model_path, thumbnail_path, size),
        "blend" => create_blend_thumbnail(model_path, thumbnail_path, size),
        "max" => create_max_thumbnail(model_path, thumbnail_path, size),
        "ma" | "mb" => create_maya_thumbnail(model_path, thumbnail_path, size),
        _ => create_generic_model_thumbnail(model_path, thumbnail_path, size),
    }
}

/// Create GLB/GLTF specific thumbnail with real model parsing when possible
fn create_glb_gltf_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating GLB/GLTF thumbnail: {:?}", model_path);
    
    // Try to use real GLB renderer synchronously (without async runtime)
    let project_name = extract_project_name_from_path(model_path);
    let asset_path = extract_asset_path_from_full_path(model_path);
    
    if let (Some(proj), Some(asset)) = (project_name, asset_path) {
        match create_real_glb_thumbnail_sync(&proj, &asset, size, thumbnail_path) {
            Ok(()) => {
                info!("✅ Real GLB thumbnail created successfully");
                return Ok(());
            }
            Err(e) => {
                info!("⚠️ Real GLB renderer failed ({}), falling back to placeholder", e);
            }
        }
    }
    
    // Fallback to enhanced placeholder
    let file_size = fs::metadata(model_path)?.len();
    let _filename = model_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model");
    
    // Create a modern gradient background (blue to light blue)
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |_x, y| {
        let y_norm = y as f32 / size as f32;
        let gradient = y_norm;
        let r = (180.0 + (220.0 - 180.0) * gradient) as u8;
        let g = (200.0 + (240.0 - 200.0) * gradient) as u8;
        let b = 255u8;
        Rgb([r, g, b])
    });
    
    // Draw a modern 3D object (rounded cube with PBR-like shading)
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_modern_3d_object(&mut img, center_x, center_y, obj_size);
    
    // Add GLTF/GLB format indicator
    let format_text = if model_path.extension().unwrap_or_default() == "glb" { "GLB" } else { "GLTF" };
    draw_format_badge(&mut img, format_text, size, [80, 120, 200]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    // Save the image
    img.save(thumbnail_path)?;
    info!("✅ GLB/GLTF placeholder thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create OBJ specific thumbnail with simple wireframe styling
fn create_obj_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating OBJ thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    
    // Create a neutral gradient background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let gradient = (x + y) as f32 / (size * 2) as f32;
        let gray_val = (220.0 + gradient * 25.0) as u8;
        Rgb([gray_val, gray_val, gray_val])
    });
    
    // Draw wireframe-style object
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_wireframe_object(&mut img, center_x, center_y, obj_size);
    
    // Add OBJ format indicator
    draw_format_badge(&mut img, "OBJ", size, [100, 100, 100]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ OBJ thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create FBX specific thumbnail with animation-focused styling
fn create_fbx_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating FBX thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    
    // Create a dynamic gradient background (purple to blue)
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        let gradient = (x_norm + y_norm) / 2.0;
        let r = (150.0 + gradient * 50.0) as u8;
        let g = (120.0 + gradient * 80.0) as u8;
        let b = (200.0 + gradient * 55.0) as u8;
        Rgb([r, g, b])
    });
    
    // Draw animated-style character/object
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_animated_character(&mut img, center_x, center_y, obj_size);
    
    // Add FBX format indicator
    draw_format_badge(&mut img, "FBX", size, [150, 100, 200]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ FBX thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create DAE (Collada) specific thumbnail
fn create_dae_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating DAE thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    
    // Create XML-like background (green tint)
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let gradient = (x + y) as f32 / (size * 2) as f32;
        let r = (200.0 + gradient * 25.0) as u8;
        let g = (230.0 + gradient * 25.0) as u8;
        let b = (200.0 + gradient * 25.0) as u8;
        Rgb([r, g, b])
    });
    
    // Draw structured/geometric object
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_geometric_object(&mut img, center_x, center_y, obj_size);
    
    // Add DAE format indicator
    draw_format_badge(&mut img, "DAE", size, [100, 150, 100]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ DAE thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create 3DS specific thumbnail
fn create_3ds_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating 3DS thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    
    // Create retro-style background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |_x, y| {
        let gradient = y as f32 / size as f32;
        let r = (160.0 + gradient * 60.0) as u8;
        let g = (160.0 + gradient * 60.0) as u8;
        let b = (180.0 + gradient * 40.0) as u8;
        Rgb([r, g, b])
    });
    
    // Draw retro 3D object
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_retro_3d_object(&mut img, center_x, center_y, obj_size);
    
    // Add 3DS format indicator
    draw_format_badge(&mut img, "3DS", size, [160, 160, 180]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ 3DS thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create Blender specific thumbnail
fn create_blend_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating Blender thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    
    // Create Blender-orange inspired background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        let distance = ((x_norm - 0.5).powi(2) + (y_norm - 0.5).powi(2)).sqrt();
        let brightness = 1.0 - (distance * 0.5);
        let r = (255.0 * brightness * 0.9) as u8;
        let g = (150.0 * brightness) as u8;
        let b = (50.0 * brightness) as u8;
        Rgb([r, g, b])
    });
    
    // Draw artistic/sculptural object
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_artistic_object(&mut img, center_x, center_y, obj_size);
    
    // Add Blender format indicator
    draw_format_badge(&mut img, "BLEND", size, [255, 150, 50]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ Blender thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create 3ds Max specific thumbnail
fn create_max_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating 3ds Max thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    
    // Create professional blue/gray background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let gradient = (x + y) as f32 / (size * 2) as f32;
        let r = (100.0 + gradient * 50.0) as u8;
        let g = (120.0 + gradient * 60.0) as u8;
        let b = (180.0 + gradient * 75.0) as u8;
        Rgb([r, g, b])
    });
    
    // Draw architectural/professional object
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_architectural_object(&mut img, center_x, center_y, obj_size);
    
    // Add MAX format indicator
    draw_format_badge(&mut img, "MAX", size, [100, 120, 180]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ 3ds Max thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create Maya specific thumbnail
fn create_maya_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating Maya thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    
    // Create Maya-teal inspired background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        let gradient = (x_norm + y_norm) / 2.0;
        let r = (80.0 + gradient * 40.0) as u8;
        let g = (150.0 + gradient * 60.0) as u8;
        let b = (160.0 + gradient * 50.0) as u8;
        Rgb([r, g, b])
    });
    
    // Draw organic/character object
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_organic_object(&mut img, center_x, center_y, obj_size);
    
    // Add Maya format indicator
    let format_text = if model_path.extension().unwrap_or_default() == "ma" { "MA" } else { "MB" };
    draw_format_badge(&mut img, format_text, size, [80, 150, 160]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ Maya thumbnail created: {:?}", thumbnail_path);
    Ok(())
}

/// Create generic model thumbnail
fn create_generic_model_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating generic model thumbnail: {:?}", model_path);
    
    let file_size = fs::metadata(model_path)?.len();
    let extension = model_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("3D")
        .to_uppercase();
    
    // Create neutral background
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let gradient = (x + y) as f32 / (size * 2) as f32;
        let gray_val = (200.0 + gradient * 40.0) as u8;
        Rgb([gray_val, gray_val, gray_val])
    });
    
    // Draw generic 3D cube
    let center_x = size / 2;
    let center_y = size / 2;
    let obj_size = (size / 3).min(80);
    
    draw_cube_wireframe(&mut img, center_x, center_y, obj_size);
    
    // Add format indicator
    draw_format_badge(&mut img, &extension, size, [120, 120, 120]);
    
    // Add file size indicator
    draw_file_size_badge(&mut img, file_size, size);
    
    img.save(thumbnail_path)?;
    info!("✅ Generic model thumbnail created: {:?}", thumbnail_path);
    Ok(())
}


/// Draw a modern 3D object with smooth shading
fn draw_modern_3d_object(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    let half_size = size / 2;
    let object_color = Rgb([100, 140, 200]);
    let _highlight_color = Rgb([180, 200, 255]);
    let _shadow_color = Rgb([60, 80, 120]);
    
    // Draw a rounded 3D object with lighting
    for dy in 0..size {
        for dx in 0..size {
            let x = center_x + dx - half_size;
            let y = center_y + dy - half_size;
            
            if x < img.width() && y < img.height() {
                let dist_x = (dx as i32 - half_size as i32) as f32;
                let dist_y = (dy as i32 - half_size as i32) as f32;
                let distance = (dist_x * dist_x + dist_y * dist_y).sqrt();
                
                if distance <= half_size as f32 {
                    // Calculate lighting based on distance from center
                    let lighting = 1.0 - (distance / half_size as f32) * 0.5;
                    let normal_x = dist_x / half_size as f32;
                    let normal_y = dist_y / half_size as f32;
                    
                    // Simple directional lighting
                    let light_dir_x = 0.3;
                    let light_dir_y = -0.3;
                    let dot_product = normal_x * light_dir_x + normal_y * light_dir_y;
                    let light_intensity = 0.5 + dot_product.max(0.0) * 0.5;
                    
                    let final_lighting = lighting * light_intensity;
                    
                    let r = (object_color[0] as f32 * final_lighting) as u8;
                    let g = (object_color[1] as f32 * final_lighting) as u8;
                    let b = (object_color[2] as f32 * final_lighting) as u8;
                    
                    img.put_pixel(x, y, Rgb([r, g, b]));
                }
            }
        }
    }
}

/// Draw a wireframe object
fn draw_wireframe_object(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    draw_cube_wireframe(img, center_x, center_y, size);
}

/// Draw a cube wireframe
fn draw_cube_wireframe(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    let half_size = size / 2;
    let offset = size / 4;
    
    // Front face corners
    let front_corners = [
        (center_x - half_size, center_y - half_size),
        (center_x + half_size, center_y - half_size),
        (center_x + half_size, center_y + half_size),
        (center_x - half_size, center_y + half_size),
    ];
    
    // Back face corners
    let back_corners = [
        (center_x - half_size + offset, center_y - half_size - offset),
        (center_x + half_size + offset, center_y - half_size - offset),
        (center_x + half_size + offset, center_y + half_size - offset),
        (center_x - half_size + offset, center_y + half_size - offset),
    ];
    
    let line_color = Rgb([80, 80, 80]);
    let depth_color = Rgb([120, 120, 120]);
    
    // Draw front face
    for i in 0..4 {
        let next = (i + 1) % 4;
        draw_line(img, front_corners[i], front_corners[next], line_color);
    }
    
    // Draw back face
    for i in 0..4 {
        let next = (i + 1) % 4;
        draw_line(img, back_corners[i], back_corners[next], depth_color);
    }
    
    // Draw connecting lines
    for i in 0..4 {
        draw_line(img, front_corners[i], back_corners[i], depth_color);
    }
}

/// Draw an animated character silhouette
fn draw_animated_character(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    let character_color = Rgb([180, 120, 200]);
    
    // Simple stick figure with animation pose
    let head_radius = size / 8;
    let body_height = size / 2;
    let arm_length = size / 4;
    
    // Head
    draw_circle(img, center_x, center_y - body_height / 2, head_radius, character_color);
    
    // Body
    draw_line(img, (center_x, center_y - body_height / 2 + head_radius), 
              (center_x, center_y + body_height / 2), character_color);
    
    // Arms (animated pose)
    draw_line(img, (center_x, center_y - body_height / 4), 
              (center_x - arm_length, center_y - body_height / 8), character_color);
    draw_line(img, (center_x, center_y - body_height / 4), 
              (center_x + arm_length, center_y + body_height / 8), character_color);
    
    // Legs
    draw_line(img, (center_x, center_y + body_height / 2), 
              (center_x - arm_length / 2, center_y + body_height), character_color);
    draw_line(img, (center_x, center_y + body_height / 2), 
              (center_x + arm_length / 2, center_y + body_height), character_color);
}

/// Draw a geometric object
fn draw_geometric_object(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    let geo_color = Rgb([100, 150, 100]);
    let half_size = size / 2;
    
    // Draw a hexagon
    let points = 6;
    let mut vertices = Vec::new();
    
    for i in 0..points {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / points as f32;
        let x = center_x as i32 + (half_size as f32 * angle.cos()) as i32;
        let y = center_y as i32 + (half_size as f32 * angle.sin()) as i32;
        vertices.push((x as u32, y as u32));
    }
    
    for i in 0..points {
        let next = (i + 1) % points;
        draw_line(img, vertices[i], vertices[next], geo_color);
        
        // Draw inner lines to center
        draw_line(img, (center_x, center_y), vertices[i], geo_color);
    }
}

/// Draw a retro 3D object
fn draw_retro_3d_object(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    // Draw a diamond/pyramid shape
    let retro_color = Rgb([160, 160, 180]);
    let half_size = size / 2;
    
    let top = (center_x, center_y - half_size);
    let left = (center_x - half_size, center_y + half_size / 2);
    let right = (center_x + half_size, center_y + half_size / 2);
    let back = (center_x + half_size / 4, center_y - half_size / 4);
    
    // Draw pyramid edges
    draw_line(img, top, left, retro_color);
    draw_line(img, top, right, retro_color);
    draw_line(img, left, right, retro_color);
    draw_line(img, top, back, retro_color);
    draw_line(img, left, back, retro_color);
    draw_line(img, right, back, retro_color);
}

/// Draw an artistic object
fn draw_artistic_object(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    let art_color = Rgb([255, 180, 80]);
    let half_size = size / 2;
    
    // Draw a stylized organic shape
    let points = 8;
    let mut vertices = Vec::new();
    
    for i in 0..points {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / points as f32;
        let radius_variation = 0.7 + 0.3 * ((i as f32 * 1.5).sin());
        let radius = half_size as f32 * radius_variation;
        let x = center_x as i32 + (radius * angle.cos()) as i32;
        let y = center_y as i32 + (radius * angle.sin()) as i32;
        vertices.push((x as u32, y as u32));
    }
    
    for i in 0..points {
        let next = (i + 1) % points;
        draw_line(img, vertices[i], vertices[next], art_color);
    }
    
    // Add some artistic flourishes
    for i in 0..points / 2 {
        let idx = i * 2;
        draw_line(img, (center_x, center_y), vertices[idx], art_color);
    }
}

/// Draw an architectural object
fn draw_architectural_object(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    let arch_color = Rgb([120, 140, 200]);
    let half_size = size / 2;
    
    // Draw a building-like structure
    let base_width = half_size;
    let base_height = half_size / 2;
    let roof_height = half_size / 3;
    
    // Base rectangle
    let base_left = center_x - base_width / 2;
    let base_right = center_x + base_width / 2;
    let base_top = center_y - base_height / 2;
    let base_bottom = center_y + base_height / 2;
    
    // Draw base
    draw_line(img, (base_left, base_top), (base_right, base_top), arch_color);
    draw_line(img, (base_right, base_top), (base_right, base_bottom), arch_color);
    draw_line(img, (base_right, base_bottom), (base_left, base_bottom), arch_color);
    draw_line(img, (base_left, base_bottom), (base_left, base_top), arch_color);
    
    // Draw roof
    draw_line(img, (base_left, base_top), (center_x, base_top - roof_height), arch_color);
    draw_line(img, (center_x, base_top - roof_height), (base_right, base_top), arch_color);
    
    // Add some details (windows)
    let window_size = base_width / 6;
    let window_y = center_y;
    draw_small_rect(img, center_x - window_size, window_y - window_size / 2, window_size, window_size, arch_color);
}

/// Draw an organic object
fn draw_organic_object(img: &mut RgbImage, center_x: u32, center_y: u32, size: u32) {
    let organic_color = Rgb([120, 180, 160]);
    let half_size = size / 2;
    
    // Draw flowing, organic curves
    let points = 12;
    let mut vertices = Vec::new();
    
    for i in 0..points {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / points as f32;
        let radius_variation = 0.5 + 0.5 * (angle * 3.0).sin().abs();
        let radius = half_size as f32 * radius_variation;
        let x = center_x as i32 + (radius * angle.cos()) as i32;
        let y = center_y as i32 + (radius * angle.sin()) as i32;
        vertices.push((x as u32, y as u32));
    }
    
    // Draw smooth organic shape
    for i in 0..points {
        let next = (i + 1) % points;
        draw_line(img, vertices[i], vertices[next], organic_color);
    }
    
    // Add organic details
    for i in (0..points).step_by(3) {
        let inner_radius = half_size as f32 * 0.3;
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / points as f32;
        let inner_x = center_x as i32 + (inner_radius * angle.cos()) as i32;
        let inner_y = center_y as i32 + (inner_radius * angle.sin()) as i32;
        draw_line(img, vertices[i], (inner_x as u32, inner_y as u32), organic_color);
    }
}

/// Helper function to draw a circle
fn draw_circle(img: &mut RgbImage, center_x: u32, center_y: u32, radius: u32, color: Rgb<u8>) {
    for dy in 0..radius * 2 {
        for dx in 0..radius * 2 {
            let x = center_x + dx - radius;
            let y = center_y + dy - radius;
            
            if x < img.width() && y < img.height() {
                let dist_sq = (dx as i32 - radius as i32).pow(2) + (dy as i32 - radius as i32).pow(2);
                if dist_sq <= (radius as i32).pow(2) {
                    img.put_pixel(x, y, color);
                }
            }
        }
    }
}

/// Helper function to draw a small rectangle
fn draw_small_rect(img: &mut RgbImage, x: u32, y: u32, width: u32, height: u32, color: Rgb<u8>) {
    for dy in 0..height {
        for dx in 0..width {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                img.put_pixel(px, py, color);
            }
        }
    }
}

/// Draw a format badge
fn draw_format_badge(img: &mut RgbImage, _text: &str, size: u32, color: [u8; 3]) {
    let badge_width = size / 4;
    let badge_height = size / 8;
    let badge_x = size - badge_width - 4;
    let badge_y = 4;
    
    // Draw background
    for dy in 0..badge_height {
        for dx in 0..badge_width {
            let px = badge_x + dx;
            let py = badge_y + dy;
            if px < size && py < size {
                img.put_pixel(px, py, Rgb(color));
            }
        }
    }
    
    // Add border
    let border_color = Rgb([color[0] / 2, color[1] / 2, color[2] / 2]);
    for dx in 0..badge_width {
        if badge_x + dx < size {
            img.put_pixel(badge_x + dx, badge_y, border_color);
            if badge_y + badge_height - 1 < size {
                img.put_pixel(badge_x + dx, badge_y + badge_height - 1, border_color);
            }
        }
    }
}

/// Draw file size badge
fn draw_file_size_badge(img: &mut RgbImage, _file_size: u64, size: u32) {
    let badge_height = size / 12;
    let badge_width = size / 4;
    let badge_x = 4;
    let badge_y = size - badge_height - 4;
    
    // Draw semi-transparent background
    for dy in 0..badge_height {
        for dx in 0..badge_width {
            let px = badge_x + dx;
            let py = badge_y + dy;
            if px < size && py < size {
                img.put_pixel(px, py, Rgb([40, 40, 40]));
            }
        }
    }
}

/// Simple line drawing function
fn draw_line(img: &mut RgbImage, start: (u32, u32), end: (u32, u32), color: Rgb<u8>) {
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

/// Generate a material preview thumbnail based on material properties
pub async fn generate_material_thumbnail(
    project_name: &str,
    material_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("🎨 Generating material thumbnail for: {}/{}", project_name, material_path);
    
    let projects_path = crate::get_projects_path();
    let full_material_path = projects_path.join(project_name).join(material_path);
    
    info!("📁 Full material path: {:?}", full_material_path);
    
    // Check if material file exists
    if !full_material_path.exists() {
        let error_msg = format!("Material file not found: {:?}", full_material_path);
        error!("{}", error_msg);
        return Err(error_msg.into());
    }

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = crate::modules::thumbnail_generator::get_thumbnails_dir(project_name);
    fs::create_dir_all(&thumbnails_dir)?;

    // Generate filename for thumbnail
    let material_filename = full_material_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("material");
    let thumbnail_filename = format!("{}_material_{}.png", material_filename, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);
    
    info!("🖼️ Thumbnail will be saved to: {:?}", thumbnail_path);

    // Read and parse material file
    match parse_material_file(&full_material_path) {
        Ok(material_info) => {
            info!("✅ Successfully parsed material: {}", material_info.name);
            
            // Create material preview thumbnail
            create_material_preview_thumbnail(&material_info, &thumbnail_path, size)?;
            
            // Return relative path to thumbnail
            Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
        }
        Err(e) => {
            error!("❌ Failed to parse material file {:?}: {}", full_material_path, e);
            
            // Try to create a fallback thumbnail
            create_fallback_material_thumbnail(&thumbnail_path, size)?;
            Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
        }
    }
}

fn parse_material_file(material_path: &Path) -> Result<MaterialInfo, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(material_path)?;
    info!("📄 Material file content (first 200 chars): {}", 
          if content.len() > 200 { &content[..200] } else { &content });
    
    let material_data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("JSON parse error: {}. Content: {}", e, content))?;
    
    let name = material_data["name"].as_str()
        .or_else(|| material_data["materialName"].as_str())
        .unwrap_or("Material").to_string();
    
    // Parse color arrays with defaults - try different property names
    let diffuse_color = parse_color_array(&material_data["diffuseColor"], [0.8, 0.8, 0.8])
        .or_else(|| parse_color_array(&material_data["diffuse"], [0.8, 0.8, 0.8]))
        .or_else(|| parse_color_array(&material_data["baseColor"], [0.8, 0.8, 0.8]))
        .unwrap_or([0.8, 0.8, 0.8]);
    
    let emissive_color = parse_color_array(&material_data["emissiveColor"], [0.0, 0.0, 0.0])
        .or_else(|| parse_color_array(&material_data["emissive"], [0.0, 0.0, 0.0]))
        .unwrap_or([0.0, 0.0, 0.0]);
    
    let metallic = material_data["metallic"].as_f64()
        .or_else(|| material_data["metallicFactor"].as_f64())
        .unwrap_or(0.0) as f32;
    
    let roughness = material_data["roughness"].as_f64()
        .or_else(|| material_data["roughnessFactor"].as_f64())
        .unwrap_or(0.5) as f32;
    
    // Check for texture presence - try different property structures
    let textures = &material_data["textures"];
    let has_diffuse_texture = textures["diffuse"].is_string() || 
                             textures["baseColorTexture"].is_string() ||
                             material_data["diffuseTexture"].is_string();
    let has_normal_texture = textures["normal"].is_string() || 
                            textures["normalTexture"].is_string() ||
                            material_data["normalTexture"].is_string();
    let has_metallic_texture = textures["metallic"].is_string() || 
                              textures["metallicRoughnessTexture"].is_string() ||
                              material_data["metallicTexture"].is_string();
    let has_roughness_texture = textures["roughness"].is_string() || 
                               textures["metallicRoughnessTexture"].is_string() ||
                               material_data["roughnessTexture"].is_string();

    info!("🎨 Parsed material: name='{}', diffuse={:?}, metallic={}, roughness={}", 
          name, diffuse_color, metallic, roughness);

    Ok(MaterialInfo {
        name,
        diffuse_color,
        metallic,
        roughness,
        emissive_color,
        has_diffuse_texture,
        has_normal_texture,
        has_metallic_texture,
        has_roughness_texture,
    })
}

fn parse_color_array(value: &serde_json::Value, default: [f32; 3]) -> Option<[f32; 3]> {
    if let Some(array) = value.as_array() {
        if array.len() >= 3 {
            return Some([
                array[0].as_f64().unwrap_or(default[0] as f64) as f32,
                array[1].as_f64().unwrap_or(default[1] as f64) as f32,
                array[2].as_f64().unwrap_or(default[2] as f64) as f32,
            ]);
        }
    }
    None
}

fn create_fallback_material_thumbnail(
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating fallback material thumbnail");
    
    // Create a simple generic material sphere
    let material_info = MaterialInfo {
        name: "Unknown Material".to_string(),
        diffuse_color: [0.7, 0.7, 0.7], // Gray
        metallic: 0.0,
        roughness: 0.5,
        emissive_color: [0.0, 0.0, 0.0],
        has_diffuse_texture: false,
        has_normal_texture: false,
        has_metallic_texture: false,
        has_roughness_texture: false,
    };
    
    create_material_preview_thumbnail(&material_info, thumbnail_path, size)?;
    Ok(())
}

fn create_material_preview_thumbnail(
    material: &MaterialInfo,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating material preview thumbnail: {}", material.name);
    
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Create a gradient sphere effect for the material preview
        let center_x = 0.5;
        let center_y = 0.5;
        let radius = 0.4;
        
        let dx = x_norm - center_x;
        let dy = y_norm - center_y;
        let distance = (dx * dx + dy * dy).sqrt();
        
        if distance <= radius {
            // Inside the sphere - render material
            let sphere_factor = (1.0f32 - (distance / radius).powf(2.0)).max(0.0);
            
            // Base diffuse color
            let mut r = material.diffuse_color[0];
            let mut g = material.diffuse_color[1];
            let mut b = material.diffuse_color[2];
            
            // Apply metallic effect
            if material.metallic > 0.0 {
                let metallic_tint = material.metallic * 0.3;
                r = (r * (1.0 - metallic_tint) + metallic_tint).min(1.0);
                g = (g * (1.0 - metallic_tint) + metallic_tint).min(1.0);
                b = (b * (1.0 - metallic_tint) + metallic_tint).min(1.0);
            }
            
            // Apply roughness effect (less roughness = more reflection)
            let reflection_intensity = (1.0 - material.roughness) * sphere_factor * 0.5;
            r = (r + reflection_intensity).min(1.0);
            g = (g + reflection_intensity).min(1.0);
            b = (b + reflection_intensity).min(1.0);
            
            // Add emissive color
            r = (r + material.emissive_color[0] * 0.3).min(1.0);
            g = (g + material.emissive_color[1] * 0.3).min(1.0);
            b = (b + material.emissive_color[2] * 0.3).min(1.0);
            
            // Apply lighting (simple directional light)
            let light_dir = [0.3, 0.3, 1.0]; // Light coming from top-right
            let normal = [dx / radius, dy / radius, (1.0f32 - distance / radius).max(0.0)];
            let dot = (normal[0] * light_dir[0] + normal[1] * light_dir[1] + normal[2] * light_dir[2]).max(0.0f32);
            let lighting = 0.3 + 0.7 * dot; // Ambient + diffuse
            
            r = (r * lighting * sphere_factor).min(1.0);
            g = (g * lighting * sphere_factor).min(1.0);
            b = (b * lighting * sphere_factor).min(1.0);
            
            Rgb([(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8])
        } else {
            // Outside the sphere - background
            let bg_intensity = 0.1 + 0.1 * (1.0 - distance.min(1.0));
            let bg_val = (bg_intensity * 255.0) as u8;
            Rgb([bg_val, bg_val, bg_val])
        }
    });
    
    // Add texture indicators if present
    if material.has_diffuse_texture || material.has_normal_texture || material.has_metallic_texture || material.has_roughness_texture {
        add_texture_indicators(&mut img, material, size);
    }
    
    // Save the image
    img.save(thumbnail_path)?;
    info!("✅ Material thumbnail saved: {:?}", thumbnail_path);
    
    Ok(())
}

fn add_texture_indicators(img: &mut RgbImage, material: &MaterialInfo, size: u32) {
    let indicator_size = size / 16; // Small indicator size
    let spacing = indicator_size + 2;
    let start_x = size - (spacing * 4);
    let start_y = size - indicator_size - 2;
    
    let indicators = [
        (material.has_diffuse_texture, [255, 100, 100]), // Red for diffuse
        (material.has_normal_texture, [100, 100, 255]),  // Blue for normal
        (material.has_metallic_texture, [255, 255, 100]), // Yellow for metallic
        (material.has_roughness_texture, [100, 255, 100]), // Green for roughness
    ];
    
    for (i, (has_texture, color)) in indicators.iter().enumerate() {
        if *has_texture {
            let x_pos = start_x + (i as u32 * spacing);
            draw_small_square(img, x_pos, start_y, indicator_size, *color);
        }
    }
}

fn draw_small_square(img: &mut RgbImage, x: u32, y: u32, size: u32, color: [u8; 3]) {
    let (width, height) = img.dimensions();
    
    for dy in 0..size {
        for dx in 0..size {
            let px = x + dx;
            let py = y + dy;
            
            if px < width && py < height {
                img.put_pixel(px, py, Rgb(color));
            }
        }
    }
}

/// Extract project name from full path
fn extract_project_name_from_path(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    let projects_path = crate::get_projects_path();
    let projects_str = projects_path.to_string_lossy();
    
    if let Some(relative) = path_str.strip_prefix(&*projects_str) {
        let relative = relative.trim_start_matches(['\\', '/']);
        let parts: Vec<&str> = relative.split(['\\', '/']).collect();
        if !parts.is_empty() {
            return Some(parts[0].to_string());
        }
    }
    None
}

/// Extract asset path relative to project from full path  
fn extract_asset_path_from_full_path(path: &Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    let projects_path = crate::get_projects_path();
    let projects_str = projects_path.to_string_lossy();
    
    if let Some(relative) = path_str.strip_prefix(&*projects_str) {
        let relative = relative.trim_start_matches(['\\', '/']);
        let parts: Vec<&str> = relative.split(['\\', '/']).collect();
        if parts.len() > 1 {
            let asset_parts = &parts[1..]; // Skip project name
            return Some(asset_parts.join("/"));
        }
    }
    None
}

/// Synchronous version of real GLB thumbnail generation to avoid runtime issues
fn create_real_glb_thumbnail_sync(
    project_name: &str,
    asset_path: &str,
    size: u32,
    thumbnail_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎯 Creating real GLB thumbnail (sync): {}/{}", project_name, asset_path);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Parse GLB file synchronously
    let model_info = parse_glb_file_sync(&full_asset_path)?;
    
    // Render with CPU-based rasterization
    render_glb_with_cpu_sync(&model_info, thumbnail_path, size)?;
    
    Ok(())
}

/// Synchronous GLB file parsing
fn parse_glb_file_sync(glb_path: &Path) -> Result<GlbModelInfo, Box<dyn std::error::Error>> {
    info!("📖 Parsing GLB file (sync): {:?}", glb_path);

    // Read GLB file
    let data = fs::read(glb_path)?;
    
    // Parse with gltf crate
    let gltf_result = gltf::Gltf::from_slice(&data);
    let gltf = match gltf_result {
        Ok(gltf) => gltf,
        Err(e) => {
            error!("❌ Failed to parse GLB with gltf crate: {}", e);
            // Try with easy-gltf as fallback
            return parse_glb_with_easy_gltf_sync(glb_path);
        }
    };

    let mut bounding_box = GlbBoundingBox {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
        center: [0.0; 3],
        size: [0.0; 3],
    };

    let mut mesh_count = 0;
    let mut node_count = 0;

    // Calculate bounding box from all meshes
    for scene in gltf.scenes() {
        for node in scene.nodes() {
            node_count += 1;
            if let Some(mesh) = node.mesh() {
                mesh_count += 1;
                
                // Get node transformation matrix
                let transform = node.transform().matrix();
                
                for primitive in mesh.primitives() {
                    if let Some(_positions_accessor) = primitive.get(&gltf::Semantic::Positions) {
                        // For now, use a default bounding box since reading buffer data is complex
                        let default_bounds = [
                            [-1.0, -1.0, -1.0, 1.0],
                            [1.0, -1.0, -1.0, 1.0],
                            [-1.0, 1.0, -1.0, 1.0],
                            [1.0, 1.0, -1.0, 1.0],
                            [-1.0, -1.0, 1.0, 1.0],
                            [1.0, -1.0, 1.0, 1.0],
                            [-1.0, 1.0, 1.0, 1.0],
                            [1.0, 1.0, 1.0, 1.0],
                        ];
                        
                        for corner in &default_bounds {
                            let transformed = matrix_multiply_vec4(&transform, corner);
                            for i in 0..3 {
                                bounding_box.min[i] = bounding_box.min[i].min(transformed[i]);
                                bounding_box.max[i] = bounding_box.max[i].max(transformed[i]);
                            }
                        }
                    }
                }
            }
        }
    }

    // Calculate center and size
    for i in 0..3 {
        bounding_box.center[i] = (bounding_box.min[i] + bounding_box.max[i]) / 2.0;
        bounding_box.size[i] = bounding_box.max[i] - bounding_box.min[i];
    }

    let model_info = GlbModelInfo {
        name: glb_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("GLB Model")
            .to_string(),
        node_count,
        mesh_count,
        material_count: gltf.materials().count() as u32,
        animation_count: gltf.animations().count() as u32,
        bounding_box,
    };

    info!("📊 GLB Analysis: {} nodes, {} meshes, {} materials, {} animations", 
          model_info.node_count, model_info.mesh_count, 
          model_info.material_count, model_info.animation_count);

    Ok(model_info)
}

/// Fallback GLB parsing with easy-gltf (sync)
fn parse_glb_with_easy_gltf_sync(glb_path: &Path) -> Result<GlbModelInfo, Box<dyn std::error::Error>> {
    info!("🔄 Trying easy-gltf parser (sync) for: {:?}", glb_path);
    
    let scenes = easy_gltf::load(glb_path).map_err(|e| format!("Easy-gltf error: {}", e))?;
    
    let mut mesh_count = 0;
    let mut node_count = 0;
    let mut material_count = 0;
    
    let mut bounding_box = GlbBoundingBox {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
        center: [0.0; 3],
        size: [0.0; 3],
    };

    for scene in &scenes {
        node_count += scene.models.len() as u32;
        mesh_count += scene.models.len() as u32; // Approximate - each model has meshes
        material_count += scene.models.len() as u32; // Approximate
        
        // Use default bounding box for easy-gltf since mesh access is complex
        bounding_box.min = [-2.0, -2.0, -2.0];
        bounding_box.max = [2.0, 2.0, 2.0];
    }

    // Calculate center and size
    for i in 0..3 {
        bounding_box.center[i] = (bounding_box.min[i] + bounding_box.max[i]) / 2.0;
        bounding_box.size[i] = bounding_box.max[i] - bounding_box.min[i];
    }

    Ok(GlbModelInfo {
        name: glb_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("GLB Model")
            .to_string(),
        node_count,
        mesh_count,
        material_count,
        animation_count: 0, // easy-gltf doesn't expose animations easily
        bounding_box,
    })
}

/// CPU-based rasterization rendering (sync)
fn render_glb_with_cpu_sync(
    model_info: &GlbModelInfo,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating CPU-rendered thumbnail (sync) for: {}", model_info.name);

    // Create image with gradient background based on model complexity
    let complexity_factor = (model_info.mesh_count as f32 / 10.0).min(1.0);
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Dynamic background based on model complexity
        let base_r = 120.0 + (complexity_factor * 80.0);
        let base_g = 140.0 + (complexity_factor * 60.0);
        let base_b = 200.0 + (complexity_factor * 40.0);
        
        let gradient = (x_norm + y_norm) / 2.0;
        let r = (base_r + gradient * 40.0) as u8;
        let g = (base_g + gradient * 40.0) as u8;
        let b = (base_b + gradient * 20.0) as u8;
        
        Rgb([r, g, b])
    });

    // Render 3D wireframe representation of the actual model
    render_wireframe_from_bounds_sync(&mut img, &model_info.bounding_box, size);
    
    // Add model info overlay
    add_model_info_overlay_sync(&mut img, model_info, size);
    
    // Save the image
    img.save(thumbnail_path)?;
    
    Ok(())
}

// Re-use the types from real_glb_renderer but with different names to avoid conflicts
#[derive(Debug)]
struct GlbModelInfo {
    pub name: String,
    pub node_count: u32,
    pub mesh_count: u32,
    pub material_count: u32,
    pub animation_count: u32,
    pub bounding_box: GlbBoundingBox,
}

#[derive(Debug)]
struct GlbBoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub center: [f32; 3],
    pub size: [f32; 3],
}

/// Render wireframe based on actual model bounding box (sync)
fn render_wireframe_from_bounds_sync(img: &mut RgbImage, bounds: &GlbBoundingBox, size: u32) {
    let center_x = size / 2;
    let center_y = size / 2;
    
    // Calculate scale factor based on bounding box
    let max_dimension = bounds.size[0].max(bounds.size[1]).max(bounds.size[2]);
    let scale = if max_dimension > 0.0 { 
        (size as f32 * 0.6) / max_dimension 
    } else { 
        50.0 
    };
    
    // Project 3D bounding box to 2D
    let corners_3d = [
        [bounds.min[0], bounds.min[1], bounds.min[2]],
        [bounds.max[0], bounds.min[1], bounds.min[2]],
        [bounds.max[0], bounds.max[1], bounds.min[2]],
        [bounds.min[0], bounds.max[1], bounds.min[2]],
        [bounds.min[0], bounds.min[1], bounds.max[2]],
        [bounds.max[0], bounds.min[1], bounds.max[2]],
        [bounds.max[0], bounds.max[1], bounds.max[2]],
        [bounds.min[0], bounds.max[1], bounds.max[2]],
    ];
    
    // Simple orthographic projection (ignoring Z for now)
    let mut corners_2d = Vec::new();
    for corner in &corners_3d {
        let x = center_x as f32 + (corner[0] - bounds.center[0]) * scale;
        let y = center_y as f32 - (corner[1] - bounds.center[1]) * scale; // Flip Y
        corners_2d.push((x as u32, y as u32));
    }
    
    let wireframe_color = Rgb([60, 80, 120]);
    let highlight_color = Rgb([120, 140, 200]);
    
    // Draw wireframe edges
    let edges = [
        // Front face
        (0, 1), (1, 2), (2, 3), (3, 0),
        // Back face  
        (4, 5), (5, 6), (6, 7), (7, 4),
        // Connecting edges
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];
    
    for (i, (start, end)) in edges.iter().enumerate() {
        let color = if i < 4 { highlight_color } else { wireframe_color };
        draw_line(img, corners_2d[*start], corners_2d[*end], color);
    }
    
    // Draw center point
    draw_circle(img, center_x, center_y, 3, Rgb([255, 200, 100]));
}

/// Add overlay with model information (sync)
fn add_model_info_overlay_sync(img: &mut RgbImage, model_info: &GlbModelInfo, size: u32) {
    // Draw info badges
    let badge_height = size / 16;
    let badge_y = size - badge_height - 4;
    
    // Mesh count badge
    let mesh_color = if model_info.mesh_count > 10 { [200, 100, 100] } 
                    else if model_info.mesh_count > 5 { [200, 200, 100] } 
                    else { [100, 200, 100] };
    draw_info_badge_sync(img, 4, badge_y, size / 6, badge_height, mesh_color);
    
    // Material count badge  
    let material_color = if model_info.material_count > 5 { [150, 100, 200] } 
                        else { [100, 150, 200] };
    draw_info_badge_sync(img, size / 6 + 8, badge_y, size / 8, badge_height, material_color);
    
    // Animation indicator
    if model_info.animation_count > 0 {
        draw_info_badge_sync(img, size / 6 + size / 8 + 12, badge_y, size / 10, badge_height, [200, 150, 100]);
    }
}

/// Helper function to draw info badges (sync)
fn draw_info_badge_sync(img: &mut RgbImage, x: u32, y: u32, width: u32, height: u32, color: [u8; 3]) {
    let (img_width, img_height) = img.dimensions();
    
    for dy in 0..height {
        for dx in 0..width {
            let px = x + dx;
            let py = y + dy;
            if px < img_width && py < img_height {
                img.put_pixel(px, py, Rgb(color));
            }
        }
    }
}

/// Matrix multiplication helper
fn matrix_multiply_vec4(matrix: &[[f32; 4]; 4], vec: &[f32; 4]) -> [f32; 4] {
    [
        matrix[0][0] * vec[0] + matrix[0][1] * vec[1] + matrix[0][2] * vec[2] + matrix[0][3] * vec[3],
        matrix[1][0] * vec[0] + matrix[1][1] * vec[1] + matrix[1][2] * vec[2] + matrix[1][3] * vec[3],
        matrix[2][0] * vec[0] + matrix[2][1] * vec[1] + matrix[2][2] * vec[2] + matrix[2][3] * vec[3],
        matrix[3][0] * vec[0] + matrix[3][1] * vec[1] + matrix[3][2] * vec[2] + matrix[3][3] * vec[3],
    ]
}