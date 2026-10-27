//! `RENZORA_GPU_REPORT=1`: log where GPU memory goes, every ten seconds.
//!
//! The editor was measured at 8 GB of dedicated GPU memory on the Bistro scene
//! with Solari on Ultra, of which the scene's own meshes and textures are about
//! one. Nothing else in the editor says where the rest goes, and Windows only
//! reports a total per process. wgpu can list every live allocation by the
//! label it was created with, so this groups them by label and prints the
//! largest, which names the passes and caches responsible.
//!
//! Off unless asked for: building the report walks every allocation the
//! device holds, which is not free on a big scene.

use std::time::Duration;

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;

/// How often the report prints.
const EVERY: Duration = Duration::from_secs(10);
/// How many label groups each report lists.
const TOP: usize = 30;

pub(crate) struct GpuMemoryReportPlugin;

impl Plugin for GpuMemoryReportPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("RENZORA_GPU_REPORT").is_none() {
            return;
        }
        app.add_systems(Update, report_gpu_memory);
    }
}

fn report_gpu_memory(
    time: Res<Time<Real>>,
    device: Option<Res<RenderDevice>>,
    mut next: Local<Duration>,
) {
    if time.elapsed() < *next {
        return;
    }
    *next = time.elapsed() + EVERY;
    let Some(device) = device else { return };
    let Some(report) = device.wgpu_device().generate_allocator_report() else {
        info!("[gpu-memory] this backend gives no allocator report (DX12 does; try --renderer dx12)");
        return;
    };

    // Group by label with trailing digits trimmed, so "image_12" and
    // "image_340" count as one kind of thing.
    let mut groups: HashMap<String, (u64, usize)> = HashMap::default();
    for allocation in &report.allocations {
        let label = allocation.name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_' || c == ' ');
        let label = if label.is_empty() { "(unlabelled)" } else { label };
        let entry = groups.entry(label.to_string()).or_default();
        entry.0 += allocation.size;
        entry.1 += 1;
    }
    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_unstable_by_key(|(_, (bytes, _))| std::cmp::Reverse(*bytes));

    let mb = |bytes: u64| bytes as f64 / (1024.0 * 1024.0);
    let mut out = format!(
        "[gpu-memory] {:.0} MB allocated in {} allocations, {:.0} MB reserved in {} blocks. Largest:",
        mb(report.total_allocated_bytes),
        report.allocations.len(),
        mb(report.total_reserved_bytes),
        report.blocks.len(),
    );
    for (label, (bytes, count)) in groups.into_iter().take(TOP) {
        out.push_str(&format!("\n  {:>8.1} MB  x{:<5} {label}", mb(bytes), count));
    }
    info!("{out}");
}
