//! Pixel Art Editor - Core data model
//!
//! Provides the state, project model, tools, layers, animation frames,
//! undo/redo, and drawing primitives for the pixel art editor.

use bevy::prelude::*;
use std::collections::VecDeque;
use std::path::PathBuf;

/// Maximum undo history depth
const MAX_UNDO_DEPTH: usize = 50;

/// Active pixel tool
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelTool {
    Pencil,
    Eraser,
    Fill,
    Line,
    Rect,
    Circle,
    Select,
    Eyedropper,
    Move,
}

impl Default for PixelTool {
    fn default() -> Self {
        Self::Pencil
    }
}

impl PixelTool {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::Eraser => "Eraser",
            Self::Fill => "Fill",
            Self::Line => "Line",
            Self::Rect => "Rectangle",
            Self::Circle => "Circle",
            Self::Select => "Select",
            Self::Eyedropper => "Eyedropper",
            Self::Move => "Move",
        }
    }

    pub fn shortcut(&self) -> &'static str {
        match self {
            Self::Pencil => "P",
            Self::Eraser => "E",
            Self::Fill => "G",
            Self::Line => "L",
            Self::Rect => "U",
            Self::Circle => "O",
            Self::Select => "S",
            Self::Eyedropper => "I",
            Self::Move => "M",
        }
    }
}

/// Brush shape for drawing tools
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushShape {
    Square,
    Circle,
}

impl Default for BrushShape {
    fn default() -> Self {
        Self::Square
    }
}

/// Blend mode for layers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

impl BlendMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
        }
    }

    pub const ALL: &'static [BlendMode] = &[
        BlendMode::Normal,
        BlendMode::Multiply,
        BlendMode::Screen,
        BlendMode::Overlay,
    ];
}

/// A single layer in the pixel project
#[derive(Debug, Clone)]
pub struct PixelLayer {
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    /// Pixel data per frame: Vec of frames, each frame is Vec<u8> RGBA
    pub frames: Vec<Vec<u8>>,
}

impl PixelLayer {
    pub fn new(name: String, width: u32, height: u32, num_frames: usize) -> Self {
        let frame_size = (width * height * 4) as usize;
        let frames = (0..num_frames.max(1))
            .map(|_| vec![0u8; frame_size])
            .collect();
        Self {
            name,
            visible: true,
            locked: false,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            frames,
        }
    }

    /// Add a new blank frame to this layer
    pub fn add_frame(&mut self, width: u32, height: u32) {
        let frame_size = (width * height * 4) as usize;
        self.frames.push(vec![0u8; frame_size]);
    }
}

/// Animation frame metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PixelFrame {
    pub duration_ms: u32,
    pub tag: String,
}

impl Default for PixelFrame {
    fn default() -> Self {
        Self {
            duration_ms: 100,
            tag: String::new(),
        }
    }
}

/// An undo entry capturing layer pixel data before an operation
#[derive(Debug, Clone)]
pub struct PixelUndoEntry {
    pub description: String,
    pub layer_index: usize,
    pub frame_index: usize,
    pub pixels: Vec<u8>,
}

/// A pixel art project with layers, frames, and metadata
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PixelProject {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<PixelLayer>,
    pub frames: Vec<PixelFrame>,
    pub active_layer: usize,
    pub active_frame: usize,
    pub is_modified: bool,
    pub file_path: Option<PathBuf>,
    /// Undo stack
    pub undo_stack: VecDeque<PixelUndoEntry>,
    /// Redo stack
    pub redo_stack: Vec<PixelUndoEntry>,
    /// Whether the flattened texture needs to be rebuilt
    pub texture_dirty: bool,
}

impl PixelProject {
    pub fn new(name: String, width: u32, height: u32) -> Self {
        let layer = PixelLayer::new("Layer 1".to_string(), width, height, 1);
        Self {
            name,
            width,
            height,
            layers: vec![layer],
            frames: vec![PixelFrame::default()],
            active_layer: 0,
            active_frame: 0,
            is_modified: false,
            file_path: None,
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            texture_dirty: true,
        }
    }

    /// Get pixel color at (x, y) on the given layer and frame
    pub fn get_pixel(&self, layer: usize, frame: usize, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        if layer >= self.layers.len() || frame >= self.layers[layer].frames.len() {
            return [0, 0, 0, 0];
        }
        let idx = ((y * self.width + x) * 4) as usize;
        let data = &self.layers[layer].frames[frame];
        if idx + 3 < data.len() {
            [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
        } else {
            [0, 0, 0, 0]
        }
    }

    /// Set pixel color at (x, y) on the given layer and frame
    pub fn set_pixel(&mut self, layer: usize, frame: usize, x: u32, y: u32, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        if layer >= self.layers.len() || frame >= self.layers[layer].frames.len() {
            return;
        }
        let idx = ((y * self.width + x) * 4) as usize;
        let data = &mut self.layers[layer].frames[frame];
        if idx + 3 < data.len() {
            data[idx] = color[0];
            data[idx + 1] = color[1];
            data[idx + 2] = color[2];
            data[idx + 3] = color[3];
            self.texture_dirty = true;
            self.is_modified = true;
        }
    }

    /// Save undo snapshot for the active layer/frame before an operation
    pub fn push_undo(&mut self, description: &str) {
        let layer = self.active_layer;
        let frame = self.active_frame;
        if layer < self.layers.len() && frame < self.layers[layer].frames.len() {
            let pixels = self.layers[layer].frames[frame].clone();
            self.undo_stack.push_back(PixelUndoEntry {
                description: description.to_string(),
                layer_index: layer,
                frame_index: frame,
                pixels,
            });
            if self.undo_stack.len() > MAX_UNDO_DEPTH {
                self.undo_stack.pop_front();
            }
            self.redo_stack.clear();
        }
    }

    /// Undo the last operation
    pub fn undo(&mut self) {
        if let Some(entry) = self.undo_stack.pop_back() {
            // Save current state to redo
            if entry.layer_index < self.layers.len()
                && entry.frame_index < self.layers[entry.layer_index].frames.len()
            {
                let current = self.layers[entry.layer_index].frames[entry.frame_index].clone();
                self.redo_stack.push(PixelUndoEntry {
                    description: entry.description.clone(),
                    layer_index: entry.layer_index,
                    frame_index: entry.frame_index,
                    pixels: current,
                });
                self.layers[entry.layer_index].frames[entry.frame_index] = entry.pixels;
                self.texture_dirty = true;
            }
        }
    }

    /// Redo the last undone operation
    pub fn redo(&mut self) {
        if let Some(entry) = self.redo_stack.pop() {
            if entry.layer_index < self.layers.len()
                && entry.frame_index < self.layers[entry.layer_index].frames.len()
            {
                let current = self.layers[entry.layer_index].frames[entry.frame_index].clone();
                self.undo_stack.push_back(PixelUndoEntry {
                    description: entry.description.clone(),
                    layer_index: entry.layer_index,
                    frame_index: entry.frame_index,
                    pixels: current,
                });
                self.layers[entry.layer_index].frames[entry.frame_index] = entry.pixels;
                self.texture_dirty = true;
            }
        }
    }

    /// Add a new layer
    pub fn add_layer(&mut self) {
        let num = self.layers.len() + 1;
        let num_frames = self.frames.len();
        self.layers.push(PixelLayer::new(
            format!("Layer {}", num),
            self.width,
            self.height,
            num_frames,
        ));
        self.active_layer = self.layers.len() - 1;
        self.texture_dirty = true;
        self.is_modified = true;
    }

    /// Remove a layer by index
    pub fn remove_layer(&mut self, index: usize) {
        if self.layers.len() <= 1 || index >= self.layers.len() {
            return;
        }
        self.layers.remove(index);
        if self.active_layer >= self.layers.len() {
            self.active_layer = self.layers.len() - 1;
        }
        self.texture_dirty = true;
        self.is_modified = true;
    }

    /// Duplicate a layer
    pub fn duplicate_layer(&mut self, index: usize) {
        if index >= self.layers.len() {
            return;
        }
        let mut new_layer = self.layers[index].clone();
        new_layer.name = format!("{} copy", new_layer.name);
        self.layers.insert(index + 1, new_layer);
        self.active_layer = index + 1;
        self.texture_dirty = true;
        self.is_modified = true;
    }

    /// Move layer from one index to another
    pub fn move_layer(&mut self, from: usize, to: usize) {
        if from >= self.layers.len() || to >= self.layers.len() || from == to {
            return;
        }
        let layer = self.layers.remove(from);
        self.layers.insert(to, layer);
        self.active_layer = to;
        self.texture_dirty = true;
        self.is_modified = true;
    }

    /// Add a new animation frame
    pub fn add_frame(&mut self) {
        self.frames.push(PixelFrame::default());
        for layer in &mut self.layers {
            layer.add_frame(self.width, self.height);
        }
        self.active_frame = self.frames.len() - 1;
        self.texture_dirty = true;
        self.is_modified = true;
    }

    /// Remove an animation frame
    pub fn remove_frame(&mut self, index: usize) {
        if self.frames.len() <= 1 || index >= self.frames.len() {
            return;
        }
        self.frames.remove(index);
        for layer in &mut self.layers {
            if index < layer.frames.len() {
                layer.frames.remove(index);
            }
        }
        if self.active_frame >= self.frames.len() {
            self.active_frame = self.frames.len() - 1;
        }
        self.texture_dirty = true;
        self.is_modified = true;
    }

    /// Duplicate an animation frame
    pub fn duplicate_frame(&mut self, index: usize) {
        if index >= self.frames.len() {
            return;
        }
        self.frames.insert(index + 1, self.frames[index].clone());
        for layer in &mut self.layers {
            if index < layer.frames.len() {
                let frame_data = layer.frames[index].clone();
                layer.frames.insert(index + 1, frame_data);
            }
        }
        self.active_frame = index + 1;
        self.texture_dirty = true;
        self.is_modified = true;
    }

    /// Flatten all visible layers for the given frame into a single RGBA buffer
    pub fn flatten_layers(&self, frame: usize) -> Vec<u8> {
        let size = (self.width * self.height * 4) as usize;
        let mut result = vec![0u8; size];

        for layer in &self.layers {
            if !layer.visible || frame >= layer.frames.len() {
                continue;
            }
            let src = &layer.frames[frame];
            let opacity = layer.opacity;

            for i in (0..size).step_by(4) {
                let sa = (src[i + 3] as f32 / 255.0) * opacity;
                if sa <= 0.0 {
                    continue;
                }
                let da = result[i + 3] as f32 / 255.0;

                let (sr, sg, sb) = (src[i] as f32, src[i + 1] as f32, src[i + 2] as f32);
                let (dr, dg, db) = (result[i] as f32, result[i + 1] as f32, result[i + 2] as f32);

                // Apply blend mode
                let (br, bg, bb) = match layer.blend_mode {
                    BlendMode::Normal => (sr, sg, sb),
                    BlendMode::Multiply => (sr * dr / 255.0, sg * dg / 255.0, sb * db / 255.0),
                    BlendMode::Screen => {
                        (255.0 - (255.0 - sr) * (255.0 - dr) / 255.0,
                         255.0 - (255.0 - sg) * (255.0 - dg) / 255.0,
                         255.0 - (255.0 - sb) * (255.0 - db) / 255.0)
                    }
                    BlendMode::Overlay => {
                        let ov = |s: f32, d: f32| -> f32 {
                            if d < 128.0 {
                                2.0 * s * d / 255.0
                            } else {
                                255.0 - 2.0 * (255.0 - s) * (255.0 - d) / 255.0
                            }
                        };
                        (ov(sr, dr), ov(sg, dg), ov(sb, db))
                    }
                };

                // Alpha compositing (source over)
                let out_a = sa + da * (1.0 - sa);
                if out_a > 0.0 {
                    result[i] = ((br * sa + dr * da * (1.0 - sa)) / out_a) as u8;
                    result[i + 1] = ((bg * sa + dg * da * (1.0 - sa)) / out_a) as u8;
                    result[i + 2] = ((bb * sa + db * da * (1.0 - sa)) / out_a) as u8;
                    result[i + 3] = (out_a * 255.0) as u8;
                }
            }
        }
        result
    }

    /// Flood fill starting at (x, y) on the active layer/frame
    pub fn flood_fill(&mut self, x: u32, y: u32, fill_color: [u8; 4]) {
        let layer = self.active_layer;
        let frame = self.active_frame;
        let target = self.get_pixel(layer, frame, x, y);
        if target == fill_color {
            return;
        }

        let mut stack = vec![(x, y)];
        while let Some((px, py)) = stack.pop() {
            if px >= self.width || py >= self.height {
                continue;
            }
            if self.get_pixel(layer, frame, px, py) != target {
                continue;
            }
            self.set_pixel(layer, frame, px, py, fill_color);

            if px > 0 { stack.push((px - 1, py)); }
            if px + 1 < self.width { stack.push((px + 1, py)); }
            if py > 0 { stack.push((px, py - 1)); }
            if py + 1 < self.height { stack.push((px, py + 1)); }
        }
    }

    /// Draw a line from (x0, y0) to (x1, y1) using Bresenham's algorithm
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4], brush_size: u32, brush_shape: BrushShape) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut cx = x0;
        let mut cy = y0;

        loop {
            self.draw_brush(cx, cy, color, brush_size, brush_shape);
            if cx == x1 && cy == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; cx += sx; }
            if e2 <= dx { err += dx; cy += sy; }
        }
    }

    /// Draw a rectangle outline from (x0, y0) to (x1, y1)
    pub fn draw_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4], brush_size: u32, brush_shape: BrushShape) {
        self.draw_line(x0, y0, x1, y0, color, brush_size, brush_shape);
        self.draw_line(x1, y0, x1, y1, color, brush_size, brush_shape);
        self.draw_line(x1, y1, x0, y1, color, brush_size, brush_shape);
        self.draw_line(x0, y1, x0, y0, color, brush_size, brush_shape);
    }

    /// Draw a circle outline centered at (cx, cy) with given radius
    pub fn draw_circle(&mut self, cx: i32, cy: i32, radius: i32, color: [u8; 4], brush_size: u32, brush_shape: BrushShape) {
        let mut x = radius;
        let mut y = 0i32;
        let mut err = 1 - radius;

        while x >= y {
            self.draw_brush(cx + x, cy + y, color, brush_size, brush_shape);
            self.draw_brush(cx - x, cy + y, color, brush_size, brush_shape);
            self.draw_brush(cx + x, cy - y, color, brush_size, brush_shape);
            self.draw_brush(cx - x, cy - y, color, brush_size, brush_shape);
            self.draw_brush(cx + y, cy + x, color, brush_size, brush_shape);
            self.draw_brush(cx - y, cy + x, color, brush_size, brush_shape);
            self.draw_brush(cx + y, cy - x, color, brush_size, brush_shape);
            self.draw_brush(cx - y, cy - x, color, brush_size, brush_shape);
            y += 1;
            if err < 0 {
                err += 2 * y + 1;
            } else {
                x -= 1;
                err += 2 * (y - x) + 1;
            }
        }
    }

    /// Draw a brush stamp at the given position
    pub fn draw_brush(&mut self, cx: i32, cy: i32, color: [u8; 4], brush_size: u32, brush_shape: BrushShape) {
        let layer = self.active_layer;
        let frame = self.active_frame;
        let half = brush_size as i32 / 2;
        let radius_sq = (brush_size as f32 / 2.0).powi(2);

        for dy in -half..=(half.max(0)) {
            for dx in -half..=(half.max(0)) {
                if brush_shape == BrushShape::Circle {
                    let dist_sq = (dx as f32 + 0.5).powi(2) + (dy as f32 + 0.5).powi(2);
                    if dist_sq > radius_sq {
                        continue;
                    }
                }
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 {
                    self.set_pixel(layer, frame, px as u32, py as u32, color);
                }
            }
        }
    }
}

/// Default color palettes
pub struct DefaultPalette;

impl DefaultPalette {
    /// Pico-8 palette (16 colors)
    pub fn pico8() -> Vec<[u8; 4]> {
        vec![
            [0, 0, 0, 255],        // black
            [29, 43, 83, 255],     // dark blue
            [126, 37, 83, 255],    // dark purple
            [0, 135, 81, 255],     // dark green
            [171, 82, 54, 255],    // brown
            [95, 87, 79, 255],     // dark grey
            [194, 195, 199, 255],  // light grey
            [255, 241, 232, 255],  // white
            [255, 0, 77, 255],     // red
            [255, 163, 0, 255],    // orange
            [255, 236, 39, 255],   // yellow
            [0, 228, 54, 255],     // green
            [41, 173, 255, 255],   // blue
            [131, 118, 156, 255],  // lavender
            [255, 119, 168, 255],  // pink
            [255, 204, 170, 255],  // peach
        ]
    }

    /// DB32 palette (32 colors)
    pub fn db32() -> Vec<[u8; 4]> {
        vec![
            [0, 0, 0, 255], [34, 32, 52, 255], [69, 40, 60, 255], [102, 57, 49, 255],
            [143, 86, 59, 255], [223, 113, 38, 255], [217, 160, 102, 255], [238, 195, 154, 255],
            [251, 242, 54, 255], [153, 229, 80, 255], [106, 190, 48, 255], [55, 148, 110, 255],
            [75, 105, 47, 255], [82, 75, 36, 255], [50, 60, 57, 255], [63, 63, 116, 255],
            [48, 96, 130, 255], [91, 110, 225, 255], [99, 155, 255, 255], [95, 205, 228, 255],
            [203, 219, 252, 255], [255, 255, 255, 255], [155, 173, 183, 255], [132, 126, 135, 255],
            [105, 106, 106, 255], [89, 86, 82, 255], [118, 66, 138, 255], [172, 50, 50, 255],
            [217, 87, 99, 255], [215, 123, 186, 255], [143, 151, 74, 255], [138, 111, 48, 255],
        ]
    }
}

/// Pixel editor state (Bevy Resource)
#[derive(Resource)]
#[allow(dead_code)]
pub struct PixelEditorState {
    /// The active project, if any
    pub project: Option<PixelProject>,
    /// Current active tool
    pub tool: PixelTool,
    /// Primary drawing color
    pub primary_color: [u8; 4],
    /// Secondary drawing color (right-click)
    pub secondary_color: [u8; 4],
    /// Brush size in pixels
    pub brush_size: u32,
    /// Brush shape
    pub brush_shape: BrushShape,
    /// Brush opacity (0.0 to 1.0)
    pub brush_opacity: f32,
    /// Pixel-perfect drawing mode
    pub pixel_perfect: bool,
    /// Show pixel grid at high zoom
    pub grid_visible: bool,
    /// Onion skinning enabled
    pub onion_skin: bool,
    /// Onion skin opacity
    pub onion_skin_opacity: f32,
    /// Canvas zoom level
    pub zoom: f32,
    /// Canvas pan offset
    pub pan_offset: [f32; 2],
    /// Currently drawing (mouse held)
    pub is_drawing: bool,
    /// Start position for shape tools (line, rect, circle)
    pub shape_start: Option<(i32, i32)>,
    /// Color palette
    pub palette: Vec<[u8; 4]>,
    /// Recent colors
    pub recent_colors: Vec<[u8; 4]>,
    /// Active palette name
    pub palette_name: String,
    /// Whether the new project dialog is open
    pub new_project_dialog: bool,
    /// New project width input
    pub new_project_width: u32,
    /// New project height input
    pub new_project_height: u32,
    /// Animation playback state
    pub playing: bool,
    /// Playback timer accumulator
    pub playback_timer_ms: f32,
    /// egui texture handle for the canvas
    pub canvas_texture: Option<bevy_egui::egui::TextureHandle>,
    /// Hex color input string
    pub hex_input: String,
}

impl Default for PixelEditorState {
    fn default() -> Self {
        Self {
            project: None,
            tool: PixelTool::Pencil,
            primary_color: [255, 255, 255, 255],
            secondary_color: [0, 0, 0, 255],
            brush_size: 1,
            brush_shape: BrushShape::Square,
            brush_opacity: 1.0,
            pixel_perfect: false,
            grid_visible: true,
            onion_skin: false,
            onion_skin_opacity: 0.3,
            zoom: 8.0,
            pan_offset: [0.0, 0.0],
            is_drawing: false,
            shape_start: None,
            palette: DefaultPalette::pico8(),
            recent_colors: Vec::new(),
            palette_name: "Pico-8".to_string(),
            new_project_dialog: false,
            new_project_width: 32,
            new_project_height: 32,
            playing: false,
            playback_timer_ms: 0.0,
            canvas_texture: None,
            hex_input: String::new(),
        }
    }
}

impl PixelEditorState {
    /// Create a new project with the given dimensions
    pub fn new_project(&mut self, width: u32, height: u32) {
        let name = format!("Untitled {}x{}", width, height);
        self.project = Some(PixelProject::new(name, width, height));
        self.zoom = (512.0 / width.max(height) as f32).max(1.0);
        self.pan_offset = [0.0, 0.0];
        self.canvas_texture = None;
    }

    /// Add current primary color to recent colors
    pub fn push_recent_color(&mut self) {
        let color = self.primary_color;
        self.recent_colors.retain(|c| *c != color);
        self.recent_colors.insert(0, color);
        if self.recent_colors.len() > 16 {
            self.recent_colors.truncate(16);
        }
    }

    /// Swap primary and secondary colors
    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.primary_color, &mut self.secondary_color);
    }

    /// Get the drawing color (factoring in eraser and opacity)
    pub fn draw_color(&self) -> [u8; 4] {
        match self.tool {
            PixelTool::Eraser => [0, 0, 0, 0],
            _ => {
                let mut c = self.primary_color;
                c[3] = (c[3] as f32 * self.brush_opacity) as u8;
                c
            }
        }
    }

    /// Export the current project as PNG to the given path
    pub fn export_as_png(&self, path: &std::path::Path) -> Result<(), String> {
        let project = self.project.as_ref().ok_or("No project open")?;
        let data = project.flatten_layers(project.active_frame);

        let img = image::RgbaImage::from_raw(project.width, project.height, data)
            .ok_or("Failed to create image buffer")?;

        img.save(path).map_err(|e| format!("Failed to save: {}", e))
    }
}
