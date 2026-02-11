//! Render pipeline graph data for visualization
//!
//! Extracts Bevy's actual render graph at startup and displays it as an
//! auto-laid-out node graph with live timing overlays.

use bevy::prelude::*;
use std::collections::HashMap;

/// Category of a render graph node, each with a display color
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum RenderNodeCategory {
    CameraSetup,
    Shadow,
    Geometry,
    Transparency,
    PostProcess,
    Upscale,
    Custom,
    Other,
}

impl RenderNodeCategory {
    /// Display color as [r, g, b]
    pub fn color(&self) -> [u8; 3] {
        match self {
            RenderNodeCategory::CameraSetup => [80, 140, 220],   // Blue
            RenderNodeCategory::Shadow => [160, 90, 200],         // Purple
            RenderNodeCategory::Geometry => [80, 190, 120],       // Green
            RenderNodeCategory::Transparency => [80, 200, 200],   // Teal
            RenderNodeCategory::PostProcess => [220, 160, 60],    // Orange
            RenderNodeCategory::Upscale => [200, 80, 180],        // Magenta
            RenderNodeCategory::Custom => [200, 70, 70],          // Red
            RenderNodeCategory::Other => [140, 140, 150],         // Gray
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RenderNodeCategory::CameraSetup => "Camera Setup",
            RenderNodeCategory::Shadow => "Shadow",
            RenderNodeCategory::Geometry => "Geometry",
            RenderNodeCategory::Transparency => "Transparency",
            RenderNodeCategory::PostProcess => "Post Process",
            RenderNodeCategory::Upscale => "Upscale",
            RenderNodeCategory::Custom => "Custom",
            RenderNodeCategory::Other => "Other",
        }
    }
}

/// A node in the render graph visualization
#[derive(Clone, Debug)]
pub struct RenderGraphNode {
    /// Unique identifier
    pub id: usize,
    /// Display name (cleaned from type_name)
    pub display_name: String,
    /// The raw Rust type name of the node
    pub type_name: String,
    /// Which sub-graph this belongs to (e.g. "Core3d", "Core2d", "Main")
    pub sub_graph: String,
    /// Node category for coloring
    pub category: RenderNodeCategory,
    /// Canvas position (set by auto_layout, or dragged by user)
    pub position: [f32; 2],
    /// Layer index (set by auto_layout)
    pub layer: usize,
    /// Live GPU time in ms (updated each frame from RenderStats)
    pub gpu_time_ms: f32,
}

/// An edge in the render graph
#[derive(Clone, Debug)]
pub struct RenderGraphEdge {
    /// Source node id
    pub from: usize,
    /// Destination node id
    pub to: usize,
}

/// Canvas state for the render pipeline viewer (pan/zoom)
#[derive(Clone, Debug)]
pub struct RenderPipelineCanvasState {
    /// Pan offset
    pub offset: [f32; 2],
    /// Zoom level
    pub zoom: f32,
}

impl Default for RenderPipelineCanvasState {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            zoom: 1.0,
        }
    }
}

impl RenderPipelineCanvasState {
    /// Apply zoom at a specific screen position (gradual)
    pub fn zoom_at(&mut self, screen_pos: [f32; 2], canvas_center: [f32; 2], delta: f32) {
        let old_zoom = self.zoom;
        // Gradual zoom: small multiplier for smooth feel
        self.zoom = (self.zoom * (1.0 + delta * 0.003)).clamp(0.15, 5.0);

        if (self.zoom - old_zoom).abs() > 0.0001 {
            let rel = [
                screen_pos[0] - canvas_center[0],
                screen_pos[1] - canvas_center[1],
            ];
            let canvas_before = [
                rel[0] / old_zoom - self.offset[0],
                rel[1] / old_zoom - self.offset[1],
            ];
            let canvas_after = [
                rel[0] / self.zoom - self.offset[0],
                rel[1] / self.zoom - self.offset[1],
            ];
            self.offset[0] += canvas_after[0] - canvas_before[0];
            self.offset[1] += canvas_after[1] - canvas_before[1];
        }
    }
}

/// Resource holding the full render pipeline graph data
#[derive(Resource)]
pub struct RenderPipelineGraphData {
    /// All nodes in the graph
    pub nodes: Vec<RenderGraphNode>,
    /// All edges
    pub edges: Vec<RenderGraphEdge>,
    /// Sub-graph names
    pub sub_graphs: Vec<String>,
    /// Canvas state (pan/zoom)
    pub canvas: RenderPipelineCanvasState,
    /// Whether to show timing overlay
    pub show_timing: bool,
    /// Whether to show sub-graph group rectangles
    pub show_sub_graphs: bool,
    /// Quick lookup from node id -> index in nodes vec
    pub node_index: HashMap<usize, usize>,
    /// Currently hovered node (if any)
    pub hovered_node: Option<usize>,
    /// Node being dragged (if any)
    pub dragged_node: Option<usize>,
    /// Offset from the drag start to the node's top-left corner (in canvas units)
    pub drag_offset: [f32; 2],
    /// Whether to lay out top-to-bottom (vertical) instead of left-to-right
    pub vertical: bool,
    /// Whether the graph has been initialized
    pub initialized: bool,
}

impl Default for RenderPipelineGraphData {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            sub_graphs: Vec::new(),
            canvas: RenderPipelineCanvasState::default(),
            show_timing: true,
            show_sub_graphs: true,
            node_index: HashMap::new(),
            hovered_node: None,
            dragged_node: None,
            drag_offset: [0.0, 0.0],
            vertical: false,
            initialized: false,
        }
    }
}

impl RenderPipelineGraphData {
    /// Look up a node by id
    pub fn get_node(&self, id: usize) -> Option<&RenderGraphNode> {
        self.node_index.get(&id).and_then(|&idx| self.nodes.get(idx))
    }

    /// Rebuild the node_index map
    pub fn rebuild_index(&mut self) {
        self.node_index.clear();
        for (idx, node) in self.nodes.iter().enumerate() {
            self.node_index.insert(node.id, idx);
        }
    }
}

// =============================================================================
// Render graph extraction from Bevy's actual RenderApp
// =============================================================================

/// Extract the render graph from Bevy's render sub-app.
/// Called from `DiagnosticsPlugin::finish()` after all render plugins are built.
pub fn extract_render_graph(app: &mut App) {
    use bevy::render::RenderApp;
    use bevy::render::render_graph::RenderGraph;

    let mut data = app.world_mut().resource_mut::<RenderPipelineGraphData>();
    data.nodes.clear();
    data.edges.clear();
    data.sub_graphs.clear();

    // We need to drop the borrow on the main world before accessing sub-app
    drop(data);

    let mut all_nodes: Vec<RenderGraphNode> = Vec::new();
    let mut all_edges: Vec<RenderGraphEdge> = Vec::new();
    let mut sub_graph_names: Vec<String> = Vec::new();
    let mut next_id: usize = 0;

    // Try to read the render sub-app's RenderGraph
    if let Some(render_app) = app.get_sub_app(RenderApp) {
        let render_graph = render_app.world().resource::<RenderGraph>();

        // Extract main graph nodes
        let label_to_id = extract_graph_nodes(
            render_graph,
            "Main",
            &mut all_nodes,
            &mut all_edges,
            &mut next_id,
        );
        if !label_to_id.is_empty() {
            sub_graph_names.push("Main".to_string());
        }

        // Extract sub-graphs (Core3d, Core2d, etc.)
        for (sub_label, sub_graph) in render_graph.iter_sub_graphs() {
            let sg_name = format!("{:?}", sub_label);
            let _sg_label_to_id = extract_graph_nodes(
                sub_graph,
                &sg_name,
                &mut all_nodes,
                &mut all_edges,
                &mut next_id,
            );
            sub_graph_names.push(sg_name);
        }
    }

    // Store into resource
    let mut data = app.world_mut().resource_mut::<RenderPipelineGraphData>();
    data.nodes = all_nodes;
    data.edges = all_edges;
    data.sub_graphs = sub_graph_names;
    data.rebuild_index();
    auto_layout(&mut data);
    data.initialized = true;
}

/// Extract nodes and edges from a single RenderGraph into our flat structures.
/// Returns a map of (label debug string -> node id) for edge resolution.
fn extract_graph_nodes(
    graph: &bevy::render::render_graph::RenderGraph,
    sub_graph_name: &str,
    all_nodes: &mut Vec<RenderGraphNode>,
    all_edges: &mut Vec<RenderGraphEdge>,
    next_id: &mut usize,
) -> HashMap<String, usize> {
    let mut label_to_id: HashMap<String, usize> = HashMap::new();

    // First pass: create nodes
    for node_state in graph.iter_nodes() {
        let label_str = format!("{:?}", node_state.label);
        let type_name = node_state.type_name.to_string();
        let display_name = clean_type_name(&type_name, &label_str);
        let category = categorize_node(&type_name, &label_str);

        let id = *next_id;
        *next_id += 1;

        label_to_id.insert(label_str, id);

        all_nodes.push(RenderGraphNode {
            id,
            display_name,
            type_name,
            sub_graph: sub_graph_name.to_string(),
            category,
            position: [0.0, 0.0],
            layer: 0,
            gpu_time_ms: 0.0,
        });
    }

    // Second pass: extract edges from each node's output_edges
    for node_state in graph.iter_nodes() {
        let from_label = format!("{:?}", node_state.label);
        let from_id = match label_to_id.get(&from_label) {
            Some(&id) => id,
            None => continue,
        };

        for edge in node_state.edges.output_edges() {
            let to_label = format!("{:?}", edge.get_input_node());
            if let Some(&to_id) = label_to_id.get(&to_label) {
                // Avoid duplicate edges
                let already_exists = all_edges.iter().any(|e| e.from == from_id && e.to == to_id);
                if !already_exists {
                    all_edges.push(RenderGraphEdge {
                        from: from_id,
                        to: to_id,
                    });
                }
            }
        }
    }

    label_to_id
}

/// Clean a Rust type name into a human-readable display name.
/// e.g. "bevy_core_pipeline::core_3d::main_opaque_pass_3d_node::MainOpaquePass3dNode"
/// becomes "Main Opaque Pass 3d"
fn clean_type_name(type_name: &str, label_str: &str) -> String {
    // Use the label if it's informative (not just a number/hash)
    let label_clean = label_str.trim_matches(|c: char| !c.is_alphanumeric() && c != ' ' && c != '_');

    // Extract the struct name from the full path
    let struct_name = type_name.rsplit("::").next().unwrap_or(type_name);

    // Remove common suffixes
    let name = struct_name
        .trim_end_matches("Node")
        .trim_end_matches("Pass")
        .trim_end_matches("Driver");

    if name.is_empty() {
        // Fall back to the label
        return label_clean.replace('_', " ");
    }

    // Insert spaces before uppercase letters (CamelCase -> Camel Case)
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            let prev = name.chars().nth(i - 1).unwrap_or(' ');
            if prev.is_lowercase() || (prev.is_uppercase() && name.chars().nth(i + 1).map_or(false, |n| n.is_lowercase())) {
                result.push(' ');
            }
        }
        result.push(ch);
    }

    if result.trim().is_empty() {
        label_clean.replace('_', " ")
    } else {
        result
    }
}

/// Categorize a render node based on its type name and label
fn categorize_node(type_name: &str, label_str: &str) -> RenderNodeCategory {
    let lower = type_name.to_lowercase();
    let label_lower = label_str.to_lowercase();

    // Camera/driver
    if lower.contains("camera") || lower.contains("driver") {
        return RenderNodeCategory::CameraSetup;
    }
    // Shadow
    if lower.contains("shadow") {
        return RenderNodeCategory::Shadow;
    }
    // Geometry / opaque passes
    if lower.contains("opaque") || lower.contains("prepass") || lower.contains("deferred")
        || lower.contains("main_pass") || label_lower.contains("opaque")
        || label_lower.contains("prepass") || label_lower.contains("deferred")
    {
        return RenderNodeCategory::Geometry;
    }
    // Transparency
    if lower.contains("transparent") || lower.contains("transmissive") || lower.contains("alpha")
        || label_lower.contains("transparent") || label_lower.contains("transmissive")
    {
        return RenderNodeCategory::Transparency;
    }
    // Post-process
    if lower.contains("bloom") || lower.contains("tonemap") || lower.contains("fxaa")
        || lower.contains("smaa") || lower.contains("taa") || lower.contains("sharpen")
        || lower.contains("auto_exposure") || lower.contains("contrast_adaptive")
        || lower.contains("msaa") || lower.contains("skybox") || lower.contains("dof")
        || lower.contains("motion_blur") || lower.contains("chromatic")
        || lower.contains("post_process") || lower.contains("copy_deferred")
        || label_lower.contains("bloom") || label_lower.contains("tonemap")
        || label_lower.contains("fxaa") || label_lower.contains("smaa")
        || label_lower.contains("taa") || label_lower.contains("dof")
    {
        return RenderNodeCategory::PostProcess;
    }
    // Upscale
    if lower.contains("upscal") || label_lower.contains("upscal") {
        return RenderNodeCategory::Upscale;
    }
    // Custom project nodes (outline, compute preview, etc.)
    if lower.contains("outline") || lower.contains("compute_preview")
        || lower.contains("renzora") || lower.contains("custom")
    {
        return RenderNodeCategory::Custom;
    }

    RenderNodeCategory::Other
}

// =============================================================================
// Auto-layout (Sugiyama-style layered graph)
// =============================================================================

/// Automatic graph layout using a Sugiyama-style layered approach.
pub fn auto_layout(data: &mut RenderPipelineGraphData) {
    if data.nodes.is_empty() {
        return;
    }

    let node_width: f32 = 180.0;
    let node_height: f32 = 60.0;
    let h_gap: f32 = 100.0;
    let v_gap: f32 = 30.0;

    // Build adjacency
    let mut predecessors: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut successors: HashMap<usize, Vec<usize>> = HashMap::new();
    for node in &data.nodes {
        predecessors.insert(node.id, Vec::new());
        successors.insert(node.id, Vec::new());
    }
    for edge in &data.edges {
        predecessors.entry(edge.to).or_default().push(edge.from);
        successors.entry(edge.from).or_default().push(edge.to);
    }

    // Layer assignment via topological sort (Kahn's algorithm, longest path)
    let mut layer_of: HashMap<usize, usize> = HashMap::new();
    let mut in_degree: HashMap<usize, usize> = HashMap::new();
    for node in &data.nodes {
        in_degree.insert(node.id, 0);
    }
    for edge in &data.edges {
        *in_degree.entry(edge.to).or_default() += 1;
    }

    let mut queue: Vec<usize> = Vec::new();
    for node in &data.nodes {
        if in_degree[&node.id] == 0 {
            queue.push(node.id);
            layer_of.insert(node.id, 0);
        }
    }

    while let Some(nid) = queue.first().copied() {
        queue.remove(0);
        if let Some(succs) = successors.get(&nid) {
            for &s in succs {
                let current_layer = layer_of[&nid];
                let entry = layer_of.entry(s).or_insert(0);
                *entry = (*entry).max(current_layer + 1);
                let deg = in_degree.get_mut(&s).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(s);
                }
            }
        }
    }

    // Handle disconnected/cyclic nodes
    for node in &data.nodes {
        layer_of.entry(node.id).or_insert(0);
    }

    let max_layer = layer_of.values().copied().max().unwrap_or(0);

    // Group by layer
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for node in &data.nodes {
        layers[layer_of[&node.id]].push(node.id);
    }

    // Barycenter ordering
    for layer_idx in 1..=max_layer {
        let mut barycenters: Vec<(usize, f32)> = Vec::new();
        for &nid in &layers[layer_idx] {
            let preds = &predecessors[&nid];
            if preds.is_empty() {
                barycenters.push((nid, 0.0));
            } else {
                let mut sum = 0.0f32;
                let mut count = 0;
                for &p in preds {
                    let p_layer = layer_of[&p];
                    if let Some(pos) = layers[p_layer].iter().position(|&x| x == p) {
                        sum += pos as f32;
                        count += 1;
                    }
                }
                let bc = if count > 0 { sum / count as f32 } else { 0.0 };
                barycenters.push((nid, bc));
            }
        }
        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        layers[layer_idx] = barycenters.into_iter().map(|(nid, _)| nid).collect();
    }

    // Position nodes — horizontal (left-to-right) or vertical (top-to-bottom)
    let vertical = data.vertical;
    let max_nodes_in_layer = layers.iter().map(|l| l.len()).max().unwrap_or(1);

    if vertical {
        // Vertical: layers go top-to-bottom, siblings spread left-to-right
        let total_width = max_nodes_in_layer as f32 * (node_width + h_gap) - h_gap;

        for (layer_idx, layer_nodes) in layers.iter().enumerate() {
            let layer_width = layer_nodes.len() as f32 * (node_width + h_gap) - h_gap;
            let x_offset = (total_width - layer_width) / 2.0;

            for (node_idx, &nid) in layer_nodes.iter().enumerate() {
                let x = x_offset + node_idx as f32 * (node_width + h_gap);
                let y = layer_idx as f32 * (node_height + v_gap + 40.0);

                if let Some(&idx) = data.node_index.get(&nid) {
                    data.nodes[idx].position = [x, y];
                    data.nodes[idx].layer = layer_idx;
                }
            }
        }
    } else {
        // Horizontal: layers go left-to-right, siblings spread top-to-bottom
        let total_height = max_nodes_in_layer as f32 * (node_height + v_gap) - v_gap;

        for (layer_idx, layer_nodes) in layers.iter().enumerate() {
            let layer_height = layer_nodes.len() as f32 * (node_height + v_gap) - v_gap;
            let y_offset = (total_height - layer_height) / 2.0;

            for (node_idx, &nid) in layer_nodes.iter().enumerate() {
                let x = layer_idx as f32 * (node_width + h_gap);
                let y = y_offset + node_idx as f32 * (node_height + v_gap);

                if let Some(&idx) = data.node_index.get(&nid) {
                    data.nodes[idx].position = [x, y];
                    data.nodes[idx].layer = layer_idx;
                }
            }
        }
    }
}

// =============================================================================
// Timing update system
// =============================================================================

/// System to copy timing data from RenderStats into the graph nodes
pub fn update_render_pipeline_timing(
    mut graph_data: ResMut<RenderPipelineGraphData>,
    render_stats: Res<crate::core::resources::diagnostics::RenderStats>,
) {
    if !graph_data.initialized {
        return;
    }

    // Match render pass names to graph nodes
    for pass in &render_stats.render_passes {
        for node in &mut graph_data.nodes {
            if pass_name_matches(&pass.name, &node.display_name) {
                node.gpu_time_ms = pass.gpu_time_ms;
            }
        }
    }

    // If we have total GPU time but no per-pass data, distribute estimate
    if render_stats.render_passes.is_empty() && render_stats.gpu_time_ms > 0.0 {
        let total_weight: f32 = graph_data.nodes.iter().map(|n| category_weight(n.category)).sum();
        if total_weight > 0.0 {
            for node in &mut graph_data.nodes {
                let weight = category_weight(node.category);
                node.gpu_time_ms = render_stats.gpu_time_ms * weight / total_weight;
            }
        }
    }
}

fn category_weight(cat: RenderNodeCategory) -> f32 {
    match cat {
        RenderNodeCategory::Geometry => 3.0,
        RenderNodeCategory::Shadow => 2.0,
        RenderNodeCategory::Transparency => 2.0,
        RenderNodeCategory::PostProcess => 1.5,
        RenderNodeCategory::Upscale => 1.0,
        RenderNodeCategory::CameraSetup => 0.5,
        RenderNodeCategory::Custom => 1.0,
        RenderNodeCategory::Other => 0.5,
    }
}

fn pass_name_matches(pass_name: &str, node_name: &str) -> bool {
    let pass_lower = pass_name.to_lowercase();
    let node_lower = node_name.to_lowercase().replace(' ', "_");
    pass_lower.contains(&node_lower) || node_lower.contains(&pass_lower.replace(' ', "_"))
}
