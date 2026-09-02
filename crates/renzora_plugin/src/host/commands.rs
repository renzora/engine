//! The per-call data channels: what a plugin can queue, poll and read while a
//! system of its is running.
//!
//! Two patterns run through everything here.
//!
//! **`#[repr(C)]` with the ABI struct first is load-bearing.** Each `*Impl`
//! below is handed to the plugin as a pointer to its first field and cast back
//! here, which is only sound if that field sits at offset 0. A missing
//! `#[repr(C)]` once let rustc reorder `SinkImpl` — nothing reads `self.sink`
//! from Rust, so it looked dead — and every `spawn_mesh` then wrote through a
//! pointer to whatever landed at offset zero. The `const` block asserts all
//! seven rather than trusting them.
//!
//! **A probe pass must not consume.** `http_poll`, `http_poll_stream` and
//! `removed_read` are called twice: once with no buffer to learn the size, once
//! with one to take the bytes. Removing on the first call would drop the data if
//! the caller then failed to allocate. `diagnostics_read` is the deliberate
//! exception — reading a diagnostic takes nothing away, so there is nothing to
//! lose.

use bevy::diagnostic::DiagnosticsStore;
use bevy::ecs::component::ComponentId;
use bevy::ecs::lifecycle::{RemovedComponentEntity, RemovedComponentMessages};
use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use std::collections::HashMap;

use crate::sys;

use super::assets::{attach_material, build_mesh_from_desc, PluginAssets};
use super::query::from_mirror;
use super::reload::guard_host;
use super::render::BsnSpawner;

/// Completed HTTP responses waiting for the plugin that asked for them.
///
/// Held here rather than acted on for the same reason [`PluginServiceCalls`] is:
/// this crate has no HTTP client and cannot depend on one. Whichever engine
/// crate owns networking drains the *requests* and pushes results back here.
///
/// Keyed by the plugin's own tag. Nothing ages entries out: a plugin that fires
/// a request and never polls for it leaks one response, which is bounded by how
/// many requests it makes and is the plugin's own bug to fix. Dropping them on a
/// timer would instead make a slow frame look like a network failure.
#[derive(Resource, Default)]
pub struct PluginHttpInbox(pub Vec<PluginHttpResponse>);

/// One completed response, or one piece of a streaming one.
pub struct PluginHttpResponse {
    /// The tag the plugin supplied with the request.
    pub tag: u64,
    /// HTTP status, or 0 if the request never completed — `body` then holds the
    /// error text.
    pub status: u16,
    pub body: String,
    /// `None` for a whole-body response, collected through `HttpSource::poll`.
    /// `Some(..)` for one piece of a stream, collected through
    /// `HttpSource::poll_stream`.
    ///
    /// The two populations share this queue but not their consumers, and the
    /// distinction has to be explicit: both pollers match on `tag` alone, so a
    /// stream chunk reaching `poll` would be handed over as if it were the
    /// entire body, and the plugin would act on a third of a JSON document.
    pub chunk: Option<sys::HttpChunkKind>,
}

/// Answers to service calls, waiting for the plugin that asked.
///
/// The mirror of [`PluginServiceCalls`]: that queue is filled by plugins and
/// drained by whichever engine crate claims the service; this one is filled by
/// that crate and drained by the plugin. Neither is interpreted here — this
/// crate cannot depend on an engine crate, so it does not know what any service
/// means and must not guess.
///
/// Nothing ages entries out, for the same reason: a plugin that asks and never
/// collects leaks one reply, bounded by how many it asked for, and dropping them
/// on a timer would make a slow frame look like a failure.
#[derive(Resource, Default)]
pub struct PluginServiceReplies(pub Vec<ServiceReply>);

/// One answer, addressed to the plugin's own `(service, tag)`.
pub struct ServiceReply {
    /// Which service produced it — the same id the plugin called.
    pub service: u64,
    /// The tag the plugin supplied with the request.
    pub tag: u64,
    /// Domain-defined discriminator, handed back untouched.
    pub op: u32,
    pub payload: Vec<u8>,
}

/// Backs [`sys::ReplySource`] for one system call.
#[repr(C)]
pub(crate) struct ReplySourceImpl<'a> {
    pub(crate) src: sys::ReplySource,
    pub(crate) replies: Option<&'a mut PluginServiceReplies>,
}

/// Hand the plugin the next reply for `(service, tag)`.
///
/// Matched on **both**, not just the tag: tags are chosen by the plugin, and
/// nothing stops it using `1` for a dialog and `1` for some future domain. The
/// service id is what keeps two domains from eating each other's answers.
pub(crate) unsafe extern "C" fn reply_poll(
    src: *mut sys::ReplySource,
    service: u64,
    tag: u64,
    out: *mut sys::ReplyRead,
) -> bool {
    let me = &mut *(src as *mut ReplySourceImpl);
    let out = &mut *out;
    out.data_len = 0;
    out.op = 0;

    let Some(replies) = me.replies.as_deref_mut() else {
        return false;
    };
    let Some(at) = replies
        .0
        .iter()
        .position(|r| r.service == service && r.tag == tag)
    else {
        return false;
    };

    let consuming = !out.data.is_null() && out.data_capacity > 0;
    {
        let r = &replies.0[at];
        out.op = r.op;
        out.data_len = r.payload.len();
        if consuming {
            let n = out.data_capacity.min(r.payload.len());
            std::ptr::copy_nonoverlapping(r.payload.as_ptr(), out.data, n);
            out.data_len = n;
        }
    }
    if consuming {
        replies.0.remove(at);
    }
    true
}

/// Backs [`sys::HttpSource`] for one system call.
#[repr(C)]
pub(crate) struct HttpSourceImpl<'a> {
    pub(crate) src: sys::HttpSource,
    pub(crate) inbox: Option<&'a mut PluginHttpInbox>,
}

/// Hand the plugin the next response for `tag`.
///
/// **The probe pass does not consume.** A caller that learns the length and then
/// fails to allocate must be able to try again; removing on the first call would
/// drop the response on the floor. The filling pass — the one that actually
/// takes the bytes — is what removes it.
pub(crate) unsafe extern "C" fn http_poll(
    src: *mut sys::HttpSource,
    tag: u64,
    out: *mut sys::HttpRead,
) -> bool {
    let me = &mut *(src as *mut HttpSourceImpl);
    let out = &mut *out;
    out.body_len = 0;
    out.status = 0;

    let Some(inbox) = me.inbox.as_deref_mut() else {
        return false;
    };
    // `chunk.is_none()` matters as much as the tag: stream pieces sit in the
    // same queue, and handing one to a caller expecting a whole body would look
    // like a complete response that happens to be truncated.
    let Some(at) = inbox
        .0
        .iter()
        .position(|r| r.tag == tag && r.chunk.is_none())
    else {
        return false;
    };

    let consuming = !out.body.is_null() && out.body_capacity > 0;
    {
        let r = &inbox.0[at];
        out.status = r.status;
        out.body_len = r.body.len();
        if consuming {
            let n = out.body_capacity.min(r.body.len());
            std::ptr::copy_nonoverlapping(r.body.as_ptr(), out.body, n);
            out.body_len = n;
        }
    }
    if consuming {
        inbox.0.remove(at);
    }
    true
}

/// Hand the plugin the next *chunk* for `tag`. Backs
/// [`sys::HttpSource::poll_stream`].
///
/// Same two-pass contract as [`http_poll`], with one difference that matters: a
/// terminal chunk carries no body, so `body_len` is 0 and the guest's "is this
/// the consuming pass" test — a non-null buffer with capacity — is the only
/// thing that can distinguish the two passes. The guest allocates a one-byte
/// scratch buffer for exactly this reason; without it an end marker would be
/// re-delivered every frame forever, and the plugin would never see the stream
/// finish.
pub(crate) unsafe extern "C" fn http_poll_stream(
    src: *mut sys::HttpSource,
    tag: u64,
    out: *mut sys::HttpChunkRead,
) -> bool {
    let me = &mut *(src as *mut HttpSourceImpl);
    let out = &mut *out;
    out.body_len = 0;
    out.status = 0;
    out.kind = sys::HttpChunkKind::Data;

    let Some(inbox) = me.inbox.as_deref_mut() else {
        return false;
    };
    // Chunks only, and the FIRST one — `position` is what keeps a stream in
    // order. Delivering out of order would silently scramble a reply, which is
    // far worse than dropping it.
    let Some(at) = inbox
        .0
        .iter()
        .position(|r| r.tag == tag && r.chunk.is_some())
    else {
        return false;
    };

    let consuming = !out.body.is_null() && out.body_capacity > 0;
    {
        let r = &inbox.0[at];
        out.status = r.status;
        out.kind = r.chunk.unwrap_or(sys::HttpChunkKind::Data);
        out.body_len = r.body.len();
        if consuming {
            let n = out.body_capacity.min(r.body.len());
            std::ptr::copy_nonoverlapping(r.body.as_ptr(), out.body, n);
            out.body_len = n;
        }
    }
    if consuming {
        inbox.0.remove(at);
    }
    true
}

/// Backs [`sys::ImageSource`] for one system call.
#[repr(C)]
pub(crate) struct ImageSourceImpl<'a> {
    pub(crate) src: sys::ImageSource,
    pub(crate) assets: Option<&'a mut Assets<Image>>,
    /// Slot table, so `write` can resolve a handle the plugin got at init.
    pub(crate) store: Option<&'a PluginAssets>,
}

/// Replace a plugin image's pixels from inside a system.
///
/// Dimensions and format are fixed at creation, so only the byte count is
/// re-checked — a wrong length here would be the same heap over-read
/// `add_image` refuses, just arriving a frame later.
pub(crate) unsafe extern "C" fn image_write(
    src: *mut sys::ImageSource,
    handle: sys::AssetHandle,
    data: *const u8,
    len: usize,
) -> bool {
    let me = &mut *(src as *mut ImageSourceImpl);
    let Some(store) = me.store else {
        return false;
    };
    let Some((_, target)) = store.images.get(handle.0 as usize).cloned() else {
        error!("[plugin] image write named slot {}, which was never created", handle.0);
        return false;
    };
    let Some(assets) = me.assets.as_deref_mut() else {
        return false;
    };
    let Some(mut image) = assets.get_mut(&target) else {
        return false;
    };
    let Some(existing) = image.data.as_mut() else {
        return false;
    };
    if data.is_null() || len != existing.len() {
        error!(
            "[plugin] image write is {len} bytes; this image is {}",
            existing.len()
        );
        return false;
    }
    // Written in place rather than by replacing the `Image`: the asset keeps its
    // descriptor, and only the pixel upload is redone.
    existing.copy_from_slice(std::slice::from_raw_parts(data, len));
    true
}

/// Backs [`sys::MeshSource`] for one system call.
///
/// `src` is first so a `*mut MeshSourceImpl` can be handed over as a
/// `*mut sys::MeshSource` — the same layout trick [`SinkImpl`] uses, which is
/// what lets the plugin call a plain function pointer and get back here.
#[repr(C)]
pub(crate) struct MeshSourceImpl<'a, 'w, 's> {
    pub(crate) src: sys::MeshSource,
    pub(crate) assets: Option<&'a mut Assets<Mesh>>,
    pub(crate) handles: &'a Query<'w, 's, &'static Mesh3d>,
    /// Slot table, so `write` can resolve the handle the plugin was given at
    /// init. Read-only — a write replaces the asset's contents, never the slot.
    pub(crate) store: Option<&'a PluginAssets>,
}

/// Replace a plugin mesh's geometry from inside a system.
///
/// The counterpart to `add_mesh_data`, which is init-only. Validation is shared
/// with it, so a mesh that would be refused at registration is refused here too
/// — and the existing geometry is left alone rather than half-replaced.
pub(crate) unsafe extern "C" fn mesh_write(
    src: *mut sys::MeshSource,
    handle: sys::AssetHandle,
    data: *const sys::MeshDataDesc,
    colors: *const sys::MeshColors,
) -> bool {
    let me = &mut *(src as *mut MeshSourceImpl);
    let Some(store) = me.store else {
        return false;
    };
    let Some((_, target)) = store.meshes.get(handle.0 as usize).cloned() else {
        error!("[plugin] mesh write named slot {}, which was never created", handle.0);
        return false;
    };
    let colors = if colors.is_null() { None } else { Some(&*colors) };
    let Some(mesh) = build_mesh_from_desc(&*data, colors) else {
        return false;
    };
    let Some(assets) = me.assets.as_deref_mut() else {
        return false;
    };
    // Replace the contents at the existing handle rather than adding a new
    // asset: everything already rendering this mesh holds that handle, and a
    // fresh one would leave them drawing the old geometry forever.
    let Some(mut slot) = assets.get_mut(&target) else {
        return false;
    };
    *slot = mesh;
    true
}

/// Copy one mesh's geometry into the plugin's buffers.
///
/// Counts are always reported in full, whatever the capacity, so the two-pass
/// probe works: the first call passes zero capacity and reads the sizes back.
pub(crate) unsafe extern "C" fn mesh_read(
    src: *mut sys::MeshSource,
    entity: sys::Entity,
    out: *mut sys::MeshRead,
) -> bool {
    let me = &*(src as *mut MeshSourceImpl);
    let out = &mut *out;
    out.position_count = 0;
    out.normal_count = 0;
    out.uv_count = 0;
    out.index_count = 0;

    let Some(assets) = me.assets.as_deref() else {
        return false;
    };
    let Some(entity) = Entity::try_from_bits(entity.0) else {
        return false;
    };
    let Ok(handle) = me.handles.get(entity) else {
        return false;
    };
    // A miss here is the normal early-frame state, not an error: mesh assets
    // load asynchronously, so a plugin polls until this succeeds.
    let Some(mesh) = assets.get(&handle.0) else {
        return false;
    };

    /// Copy up to `cap` items into `dst`, and report how many exist.
    unsafe fn fill<T: Copy>(dst: *mut T, cap: usize, src: &[T]) -> usize {
        if !dst.is_null() && cap > 0 {
            let n = cap.min(src.len());
            std::ptr::copy_nonoverlapping(src.as_ptr(), dst, n);
        }
        src.len()
    }

    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(p)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        // `sys::Vec3` is three `f32`s in order, so `[f32; 3]` is the same bytes.
        out.position_count = fill(out.positions.cast::<[f32; 3]>(), out.position_capacity, p);
    }
    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x3(n)) =
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    {
        out.normal_count = fill(out.normals.cast::<[f32; 3]>(), out.normal_capacity, n);
    }
    if let Some(bevy::render::mesh::VertexAttributeValues::Float32x2(u)) =
        mesh.attribute(Mesh::ATTRIBUTE_UV_0)
    {
        out.uv_count = fill(out.uvs, out.uv_capacity, u);
    }
    // Widened to u32 rather than refused: a 16-bit index buffer is the common
    // case for small meshes, and a plugin should not have to handle both.
    match mesh.indices() {
        Some(bevy::render::mesh::Indices::U32(i)) => {
            out.index_count = fill(out.indices, out.index_capacity, i);
        }
        Some(bevy::render::mesh::Indices::U16(i)) => {
            let widened: Vec<u32> = i.iter().map(|&x| x as u32).collect();
            out.index_count = fill(out.indices, out.index_capacity, &widened);
        }
        None => {}
    }
    true
}

/// Backs [`sys::RemovedSource`] for one system invocation.
///
/// `src` must be the FIRST field — see the offset assertions below.
///
/// The cursors are borrowed from a `Local` on the dispatcher, which is what
/// makes the per-system semantics work: each plugin system has its own
/// dispatcher, so each has its own cursor per component and sees every removal
/// exactly once, matching Bevy's `RemovedComponents<T>`.
#[repr(C)]
pub(crate) struct RemovedSourceImpl<'a> {
    pub(crate) src: sys::RemovedSource,
    pub(crate) messages: Option<&'a RemovedComponentMessages>,
    pub(crate) cursors: &'a mut HashMap<ComponentId, MessageCursor<RemovedComponentEntity>>,
}

/// Copy out the removals this system has not seen yet.
///
/// The probe pass must NOT consume, or a caller that learns the count and then
/// fails to allocate would silently lose them — the same rule `http_poll`
/// follows, and for the same reason.
pub(crate) unsafe extern "C" fn removed_read(
    src: *mut sys::RemovedSource,
    component: sys::ComponentId,
    out: *mut sys::RemovedRead,
) -> bool {
    guard_host("removed_read", false, || {
        let this = &mut *(src as *mut RemovedSourceImpl);
        let Some(messages) = this.messages else {
            return false;
        };
        if !component.is_valid() {
            return false;
        }
        let id = ComponentId::new(component.0 as usize);
        // `None` means nothing has ever removed this component. Not an error, and
        // the normal state for most components on most frames.
        let Some(queue) = messages.get(id) else {
            return false;
        };
        let out = &mut *out;
        let cursor = this.cursors.entry(id).or_default();

        if out.entities.is_null() || out.entity_capacity == 0 {
            // Probe. `len` reads the cursor's outstanding count without advancing
            // it, which is what keeps this pass non-consuming.
            out.entity_count = cursor.len(queue);
            return true;
        }

        let mut n = 0usize;
        for message in cursor.read(queue) {
            if n == out.entity_capacity {
                break;
            }
            let entity: Entity = message.clone().into();
            out.entities.add(n).write(sys::Entity(entity.to_bits()));
            n += 1;
        }
        out.entity_count = n;
        true
    })
}

/// Backs [`sys::CommandSink`] for one system call.
///
/// **`#[repr(C)]` is load-bearing, not tidiness.** The plugin is handed a
/// `*mut sys::CommandSink`, and [`sink_reserve`] / [`sink_push`] cast it back to
/// `*mut SinkImpl` — which is only sound if `sink` is at offset 0. Under
/// `repr(Rust)` the compiler may order fields however it likes, and it has every
/// reason to move this one: nothing ever *reads* `self.sink`, so it looks dead.
/// The result is `me.commands` resolving to whatever happens to sit at that
/// offset — a wild `&mut Commands` — and the first `spawn_empty` through it
/// faults with no Rust-level error to report.
#[repr(C)]
pub(crate) struct SinkImpl<'a, 'w, 's> {
    /// Never read in Rust — the plugin reaches it through the pointer above.
    /// It exists for its address, which is why the field order matters.
    #[allow(dead_code)]
    pub(crate) sink: sys::CommandSink,
    pub(crate) commands: &'a mut Commands<'w, 's>,
    pub(crate) queued: Vec<(sys::Command, Vec<u8>)>,
}

/// The four `*Impl` structs are handed to a plugin as a pointer to their **first
/// field**, and recovered here by casting that pointer back to the whole struct.
/// The entire pattern rests on the first field sitting at offset zero.
///
/// That is guaranteed by `#[repr(C)]` and by nothing else. A missing `#[repr(C)]`
/// on `SinkImpl` once let rustc reorder it — the first field is never read from
/// Rust, so the compiler had every reason to move it, and it warned only that the
/// field was unused. Every `spawn_mesh` then wrote through a pointer to whatever
/// landed at offset zero: a hard crash with no panic, no log and no crash report,
/// which took a day of file-based tracing to find.
///
/// So the invariant is asserted rather than trusted. These fail to compile if
/// anyone reorders a field or drops the attribute.
const _: () = {
    assert!(core::mem::offset_of!(SinkImpl, sink) == 0);
    assert!(core::mem::offset_of!(MeshSourceImpl, src) == 0);
    assert!(core::mem::offset_of!(ImageSourceImpl, src) == 0);
    assert!(core::mem::offset_of!(HttpSourceImpl, src) == 0);
    assert!(core::mem::offset_of!(RemovedSourceImpl, src) == 0);
    assert!(core::mem::offset_of!(ReplySourceImpl, src) == 0);
    assert!(core::mem::offset_of!(DiagnosticSourceImpl, src) == 0);
};

/// Backs [`sys::DiagnosticSource`]. `src` must be the FIRST field — see the
/// assertions above.
#[repr(C)]
pub(crate) struct DiagnosticSourceImpl<'a> {
    pub(crate) src: sys::DiagnosticSource,
    /// `None` when the host keeps no diagnostics, which is the normal state for
    /// a shipped game — the plugin sees an empty store rather than a failure.
    pub(crate) store: Option<&'a DiagnosticsStore>,
}

/// Copy this frame's measurements into a plugin-owned buffer.
///
/// One pass, unlike `http_poll` and `removed_read`, and the difference is worth
/// stating: those two *consume*, so a probe that also took the data would lose it
/// if the caller then failed to allocate. Reading a diagnostic takes nothing away,
/// so the count pass is just a count and there is nothing to lose.
///
/// The `StrRef`s point into the store, which is borrowed for the whole system
/// call — valid until this returns and not one instruction longer. The guest side
/// copies them before the borrow ends; see `diagnostics::Diagnostics::iter`.
pub(crate) unsafe extern "C" fn diagnostics_read(
    src: *mut sys::DiagnosticSource,
    out: *mut sys::DiagnosticEntry,
    cap: u32,
) -> u32 {
    guard_host("diagnostics_read", 0, || {
        let this = &mut *(src as *mut DiagnosticSourceImpl);
        let Some(store) = this.store else {
            return 0;
        };

        let mut total: u32 = 0;
        for diag in store.iter() {
            // Count everything, but only write while there is room. Returning the
            // full total rather than what was written is what lets a caller
            // detect a short buffer and grow — reporting the truncated count
            // would make truncation invisible, which is the failure mode where a
            // profiler silently stops plotting whatever sorts last.
            if out.is_null() || total >= cap {
                total = total.saturating_add(1);
                continue;
            }
            let path = diag.path().as_str();
            // `value()` is `None` before the first sample — a real state for the
            // first frames, not an error. `NaN` carries that across the boundary
            // without needing a second field to say "no value", and the guest's
            // `Diagnostic::is_valid` is the documented check.
            let value = diag.value().unwrap_or(f64::NAN);
            out.add(total as usize).write(sys::DiagnosticEntry {
                path: sys::StrRef {
                    ptr: path.as_ptr(),
                    len: path.len(),
                },
                value,
                smoothed: diag.smoothed().unwrap_or(value),
            });
            total += 1;
        }
        total
    })
}

/// A command sink for a call that is not a system dispatch.
///
/// A plugin invoked from the editor's UI still needs to be able to spawn and
/// despawn, and structural changes must go through Bevy's deferred queue there
/// for the same reason they do in a system.
pub struct HostCommandSink<'a, 'w, 's>(SinkImpl<'a, 'w, 's>);

impl<'a, 'w, 's> HostCommandSink<'a, 'w, 's> {
    pub fn new(commands: &'a mut Commands<'w, 's>) -> Self {
        Self(SinkImpl {
            sink: sys::CommandSink {
                reserve_entity: sink_reserve,
                push: sink_push,
            },
            commands,
            queued: Vec::new(),
        })
    }

    /// The pointer to hand across the ABI. Valid until this is dropped.
    pub fn as_ptr(&mut self) -> *mut sys::CommandSink {
        (&mut self.0 as *mut SinkImpl).cast()
    }

    /// Apply whatever the plugin queued.
    ///
    /// Takes `self` and applies through the borrow it already holds — asking the
    /// caller for `&mut Commands` again would be a second mutable borrow of the
    /// one this sink is built on.
    pub fn drain(mut self) {
        let queued = std::mem::take(&mut self.0.queued);
        apply_queued(self.0.commands, queued);
    }
}

pub(crate) unsafe extern "C" fn sink_reserve(sink: *mut sys::CommandSink) -> sys::Entity {
    let me = &mut *(sink as *mut SinkImpl);
    // `spawn_empty` reserves an id that is valid immediately and materialises
    // when commands are applied — which is what lets a plugin use the id in the
    // same frame it asked for it.
    sys::Entity(me.commands.spawn_empty().id().to_bits())
}

pub(crate) unsafe extern "C" fn sink_push(sink: *mut sys::CommandSink, cmd: *const sys::Command) {
    let me = &mut *(sink as *mut SinkImpl);
    let cmd = &*cmd;
    // Copy the payload NOW. `data` may point at a plugin stack local that is gone
    // by the time commands are applied.
    let data = if cmd.data.is_null() || cmd.data_len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(cmd.data, cmd.data_len).to_vec()
    };
    me.queued.push((
        sys::Command {
            kind: cmd.kind,
            entity: cmd.entity,
            component: cmd.component,
            data: std::ptr::null(),
            data_len: 0,
        },
        data,
    ));
}

/// Apply what a system queued. Runs after the system body, never during it.
pub(crate) fn apply_queued(commands: &mut Commands, queued: Vec<(sys::Command, Vec<u8>)>) {
    for (cmd, data) in queued {
        let Some(entity) = Entity::try_from_bits(cmd.entity.0) else {
            continue;
        };
        match cmd.kind {
            sys::CommandKind::Despawn => {
                commands.entity(entity).try_despawn();
            }
            sys::CommandKind::Remove => {
                if cmd.component.is_valid() {
                    let id = ComponentId::new(cmd.component.0 as usize);
                    commands.queue(move |world: &mut World| {
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.remove_by_id(id);
                        }
                    });
                }
            }
            sys::CommandKind::SpawnMesh => {
                if data.len() < size_of::<sys::SpawnMeshDesc>() {
                    continue;
                }
                // SAFETY: pushed by `make_renderable`, which writes exactly one.
                let d = unsafe { *data.as_ptr().cast::<sys::SpawnMeshDesc>() };
                commands.queue(move |world: &mut World| {
                    let (mesh, material) = {
                        let Some(store) = world.get_resource::<PluginAssets>() else {
                            return;
                        };
                        // `.1` — the store keys each handle by owning slot so a
                        // reload can free its own; a spawn only wants the handle.
                        let m = store.meshes.get(d.mesh.0 as usize).map(|(_, h)| h.clone());
                        let mat = store
                            .materials
                            .get(d.material.0 as usize)
                            .map(|(_, h)| h.clone());
                        match (m, mat) {
                            (Some(m), Some(mat)) => (m, mat),
                            _ => {
                                error!("[plugin] spawn_mesh used an unknown asset handle");
                                return;
                            }
                        }
                    };
                    if let Ok(mut e) = world.get_entity_mut(entity) {
                        e.insert((Mesh3d(mesh), from_mirror(&d.transform)));
                    }
                    attach_material(world, entity, material, d.material.0 as usize, "spawn_mesh");
                });
            }
            sys::CommandKind::SetMaterial => {
                if data.len() < size_of::<sys::SpawnMeshDesc>() {
                    continue;
                }
                // SAFETY: pushed by `set_material`, which writes exactly one.
                // Only `material` is read; the struct is shared with SpawnMesh.
                let d = unsafe { *data.as_ptr().cast::<sys::SpawnMeshDesc>() };
                commands.queue(move |world: &mut World| {
                    let index = d.material.0 as usize;
                    let Some(slot) = world
                        .get_resource::<PluginAssets>()
                        .and_then(|store| store.materials.get(index))
                        .map(|(_, slot)| slot.clone())
                    else {
                        error!("[plugin] set_material used an unknown material handle");
                        return;
                    };
                    attach_material(world, entity, slot, index, "set_material");
                });
            }
            sys::CommandKind::Insert => {
                // An invalid id here means the plugin inserted a component it
                // never registered or queried. `component_id_of` reads the id
                // the host assigned at init, and nothing assigns one for a type
                // the plugin only ever inserts — so this was a silent no-op, the
                // worst possible outcome for "my component never appears".
                if !cmd.component.is_valid() {
                    error!(
                        "plugin queued an insert for a component it never registered —                          call `app.register_component::<T>()` in `build()` for every type                          you insert, including host types like `Transform`"
                    );
                    continue;
                }
                if data.is_empty() {
                    continue;
                }
                let id = ComponentId::new(cmd.component.0 as usize);
                commands.queue(move |world: &mut World| {
                    // The plugin sent bytes in ITS representation. For a
                    // plugin-owned component that is also the host's, so the
                    // bytes go in verbatim. For a host component it is the frozen
                    // mirror, which is a different size AND a different field
                    // layout — `sys::Transform` is 40 bytes with rotation at
                    // offset 12, `bevy::Transform` is 48 with rotation at 16 —
                    // so it must be marshalled exactly as the query write-back
                    // marshals it.
                    if Some(id) == world.component_id::<Transform>() {
                        if data.len() != size_of::<sys::Transform>() {
                            error!(
                                "plugin sent {} bytes for a Transform; expected {}",
                                data.len(),
                                size_of::<sys::Transform>()
                            );
                            return;
                        }
                        // SAFETY: length checked, and `sys::Transform` is
                        // `#[repr(C)]` plain-old-data.
                        let mirror =
                            unsafe { data.as_ptr().cast::<sys::Transform>().read_unaligned() };
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.insert(from_mirror(&mirror));
                        }
                        return;
                    }

                    // Size is checked against the LIVE layout, not against what
                    // the plugin claimed. `insert_by_id` copies `layout.size()`
                    // bytes from the pointer regardless of how many the plugin
                    // actually sent, so a short buffer is a heap over-read that
                    // lands in component storage and surfaces later as garbage in
                    // an unrelated field.
                    let Some(size) = world
                        .components()
                        .get_info(id)
                        .map(|i| i.layout().size())
                    else {
                        error!("plugin inserted an unregistered component id {}", id.index());
                        return;
                    };
                    if data.len() != size {
                        error!(
                            "plugin sent {} bytes for component id {}; it is {size} bytes here",
                            data.len(),
                            id.index()
                        );
                        return;
                    }

                    let mut bytes = data;
                    // SAFETY: `bytes` is one instance of this component, copied
                    // from the plugin at push time, and its length now matches
                    // the registered layout.
                    unsafe {
                        let ptr = bevy::ptr::OwningPtr::new(
                            std::ptr::NonNull::new_unchecked(bytes.as_mut_ptr().cast()),
                        );
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.insert_by_id(id, ptr);
                        }
                    }
                    // `bytes` drops here — see `write_resource_bytes` for why
                    // that is correct rather than a double free.
                });
            }
            sys::CommandKind::SpawnBsn => {
                let Ok(source) = String::from_utf8(data) else {
                    error!("plugin sent BSN that is not valid UTF-8");
                    continue;
                };
                commands.queue(move |world: &mut World| {
                    let Some(spawner) = world.get_resource::<BsnSpawner>().copied() else {
                        error!(
                            "a plugin spawned BSN but nothing installed a `BsnSpawner` — \
                             the tree was dropped"
                        );
                        return;
                    };
                    (spawner.0)(world, entity, &source);
                });
            }
            sys::CommandKind::Service => {
                let hdr_len = size_of::<sys::ServiceCall>();
                if data.len() < hdr_len {
                    error!(
                        "plugin sent {} bytes for a service call; the header alone is {hdr_len}",
                        data.len()
                    );
                    continue;
                }
                // SAFETY: length checked, and `sys::ServiceCall` is `#[repr(C)]`
                // plain-old-data.
                let hdr = unsafe { data.as_ptr().cast::<sys::ServiceCall>().read_unaligned() };
                let payload = data[hdr_len..].to_vec();
                // Parked, not applied, and deliberately not inspected: what these
                // bytes mean is the consumer's business. This crate cannot depend
                // on any engine crate — see the module doc — so it does not know
                // and must not guess.
                commands.queue(move |world: &mut World| {
                    world
                        .get_resource_or_insert_with(PluginServiceCalls::default)
                        .0
                        .push(ServiceCall {
                            entity,
                            service: hdr.service,
                            op: hdr.op,
                            payload,
                        });
                });
            }
            // A command kind from a newer ABI. Dropping it is the only
            // option: what the payload means is exactly the thing this build
            // does not know.
            other => {
                warn!("plugin queued command kind {} which this build does not have", other.0);
            }
        }
    }
}

// ── Services ─────────────────────────────────────────────────────────────────

/// One [`sys::CommandKind::Service`] call, as parked for its consumer.
pub struct ServiceCall {
    pub entity: Entity,
    /// From `sys::service_id`. Which crate this is for.
    pub service: u64,
    /// The operation, in that service's own numbering.
    pub op: u32,
    /// The payload, exactly as the plugin wrote it. **Not** interpreted here —
    /// this crate has no idea what any of it means, which is the point.
    pub payload: Vec<u8>,
}

/// Service calls plugins queued this frame, waiting for whoever claims them.
///
/// Held rather than acted on, for the same reason [`super::render::PluginPanel`]
/// is: this crate must stay publishable to crates.io, so it cannot depend on
/// `renzora_animation` — or on anything else in the engine. It carries bytes it
/// does not read.
///
/// **Nothing draining a service is a valid configuration.** A dedicated server or
/// a lean export that dropped the crate in question simply discards those calls;
/// see [`discard_unhandled_service_calls`].
#[derive(Resource, Default)]
pub struct PluginServiceCalls(pub Vec<ServiceCall>);

impl PluginServiceCalls {
    /// Take every call for one service, leaving the rest for other consumers.
    ///
    /// Per-service rather than "drain everything", because more than one bridge
    /// reads this queue and a consumer that took the lot would silently eat
    /// another domain's calls — a failure with no symptom except a feature that
    /// quietly stops working when an unrelated crate is present.
    pub fn take(&mut self, service: u64) -> Vec<ServiceCall> {
        let mut taken = Vec::new();
        let mut i = 0;
        while i < self.0.len() {
            if self.0[i].service == service {
                taken.push(self.0.remove(i));
            } else {
                i += 1;
            }
        }
        taken
    }
}

/// Discards service calls nothing claimed, at the end of the frame.
///
/// Registered by the host, and it has to be: without a consumer the queue is
/// append-only, and a plugin calling into a service every frame in a build that
/// lacks its bridge would grow it until the process died. Real consumers drain
/// their own service earlier in the frame and this sees only what is left.
pub fn discard_unhandled_service_calls(queue: Option<ResMut<PluginServiceCalls>>) {
    if let Some(mut queue) = queue {
        if !queue.0.is_empty() {
            queue.0.clear();
        }
    }
}
