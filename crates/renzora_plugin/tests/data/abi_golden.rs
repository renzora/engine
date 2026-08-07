// GENERATED from `sys.rs` by the same parser the test uses — regenerate with
// `cargo test -p renzora_plugin --test abi_order -- --ignored dump_golden`.
//
// Editing this by hand to make a test pass is the one thing it exists to stop.
// If a diff here was not intentional, the ABI moved.

const GOLDEN: &[Golden] = &[
    Golden {
        name: "PluginScope",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "ScopeEntry",
        fields: &[
            "= unsafe extern \"C\" fn() -> PluginScope",
        ],
    },
    Golden {
        name: "Entity",
        fields: &[
            "0: u64",
        ],
    },
    Golden {
        name: "ComponentId",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "StrRef",
        fields: &[
            "ptr: *const u8",
            "len: usize",
        ],
    },
    Golden {
        name: "Schedule",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "Vec3",
        fields: &[
            "x: f32",
            "y: f32",
            "z: f32",
        ],
    },
    Golden {
        name: "Quat",
        fields: &[
            "x: f32",
            "y: f32",
            "z: f32",
            "w: f32",
        ],
    },
    Golden {
        name: "Transform",
        fields: &[
            "translation: Vec3",
            "rotation: Quat",
            "scale: Vec3",
        ],
    },
    Golden {
        name: "FieldKind",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "Str256",
        fields: &[
            "bytes: [u8; STR_CAP]",
            "len: u32",
        ],
    },
    Golden {
        name: "FieldDesc",
        fields: &[
            "name: StrRef",
            "kind: FieldKind",
            "offset: usize",
        ],
    },
    Golden {
        name: "ComponentDesc",
        fields: &[
            "name: StrRef",
            "size: usize",
            "align: usize",
            "drop: Option<unsafe extern \"C\" fn(*mut u8)>",
            "display_name: StrRef",
            "fields: *const FieldDesc",
            "field_count: usize",
            "default_init: Option<unsafe extern \"C\" fn(*mut u8)>",
        ],
    },
    Golden {
        name: "Access",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "Term",
        fields: &[
            "component: ComponentId",
            "access: Access",
        ],
    },
    Golden {
        name: "QueryDesc",
        fields: &[
            "terms: *const Term",
            "term_count: usize",
        ],
    },
    Golden {
        name: "QueryView",
        fields: &[
            "cells: *mut *mut u8",
            "entities: *const Entity",
            "entity_count: usize",
            "cell_count: usize",
        ],
    },
    Golden {
        name: "SystemDesc",
        fields: &[
            "entry: SystemEntry",
            "schedule: Schedule",
            "queries: *const QueryDesc",
            "query_count: usize",
            "resources: *const Term",
            "resource_count: usize",
            "user: *mut c_void",
            "flags: u32",
        ],
    },
    Golden {
        name: "RegisterStatus",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "FrameCtx",
        fields: &[
            "delta_secs: f32",
            "elapsed_secs: f32",
        ],
    },
    Golden {
        name: "SystemCall",
        fields: &[
            "views: *const QueryView",
            "view_count: usize",
            "frame: FrameCtx",
            "user: *mut c_void",
            "iface: *const Interface",
            "host: *mut Host",
            "commands: *mut CommandSink",
            "resources: *const ResourceSlot",
            "resource_count: usize",
            "input: *const InputState",
            "meshes: *mut MeshSource",
            "images: *mut ImageSource",
            "http: *mut HttpSource",
            "removed: *mut RemovedSource",
            // Appended in MINOR 4.5 — the generic reply channel.
            "replies: *mut ReplySource",
            // Appended in MINOR 4.8 — the host's measurement store.
            "diagnostics: *mut DiagnosticSource",
        ],
    },
    // Crosses the boundary: the host writes these into a plugin-allocated
    // buffer. `path` borrows the host's string for the call only.
    Golden {
        name: "DiagnosticEntry",
        fields: &[
            "path: StrRef",
            "value: f64",
            "smoothed: f64",
        ],
    },
    // Host-allocated, reached through `SystemCall::diagnostics`.
    Golden {
        name: "DiagnosticSource",
        fields: &[
            "read: unsafe extern \"C\" fn( src: *mut DiagnosticSource, out: *mut DiagnosticEntry, cap: u32, ) -> u32",
        ],
    },
    Golden {
        name: "HttpRead",
        fields: &[
            "body_capacity: usize",
            "body: *mut u8",
            "body_len: usize",
            "status: u16",
            "_pad: [u8; 6]",
        ],
    },
    // Crosses the boundary, plugin-allocated. The generic counterpart to a
    // service call's payload: the host writes bytes whose meaning is the
    // domain's, not this crate's.
    Golden {
        name: "ReplyRead",
        fields: &[
            "data_capacity: usize",
            "data: *mut u8",
            "data_len: usize",
            "op: u32",
            "_pad: [u8; 4]",
        ],
    },
    // Host-allocated, reached through `SystemCall::replies`.
    Golden {
        name: "ReplySource",
        fields: &[
            "poll: unsafe extern \"C\" fn( src: *mut ReplySource, service: u64, tag: u64, out: *mut ReplyRead, ) -> bool",
        ],
    },
    // Crosses the boundary: written by the host into a plugin-allocated
    // `HttpChunkRead`. An append-only integer, like `Access` and `SystemStatus`.
    Golden {
        name: "HttpChunkKind",
        fields: &[
            "0: u32",
        ],
    },
    // Crosses the boundary, plugin-allocated. Deliberately a NEW struct rather
    // than fields appended to `HttpRead`: the host writes into memory the plugin
    // owns, so growing that one would write past an older plugin's allocation.
    Golden {
        name: "HttpChunkRead",
        fields: &[
            "body_capacity: usize",
            "body: *mut u8",
            "body_len: usize",
            "kind: HttpChunkKind",
            "status: u16",
            "_pad: [u8; 2]",
        ],
    },
    Golden {
        name: "HttpSource",
        fields: &[
            "poll: unsafe extern \"C\" fn( src: *mut HttpSource, tag: u64, out: *mut HttpRead, ) -> bool",
            // Appended in MINOR 4.4. Host-allocated, so an older plugin never
            // reads it and a newer one is refused by the version check.
            "poll_stream: unsafe extern \"C\" fn( src: *mut HttpSource, tag: u64, out: *mut HttpChunkRead, ) -> bool",
        ],
    },
    Golden {
        name: "MeshRead",
        fields: &[
            "position_capacity: usize",
            "positions: *mut Vec3",
            "normal_capacity: usize",
            "normals: *mut Vec3",
            "uv_capacity: usize",
            "uvs: *mut [f32; 2]",
            "index_capacity: usize",
            "indices: *mut u32",
            "position_count: usize",
            "normal_count: usize",
            "uv_count: usize",
            "index_count: usize",
        ],
    },
    Golden {
        name: "MeshSource",
        fields: &[
            "read: unsafe extern \"C\" fn( src: *mut MeshSource, entity: Entity, out: *mut MeshRead, ) -> bool",
            "write: unsafe extern \"C\" fn( src: *mut MeshSource, handle: AssetHandle, data: *const MeshDataDesc, colors: *const MeshColors, ) -> bool",
        ],
    },
    Golden {
        name: "MeshColors",
        fields: &[
            "colors: *const [f32; 4]",
            "color_count: usize",
        ],
    },
    Golden {
        name: "InputState",
        fields: &[
            "keys_down: [u64; 4]",
            "keys_just_pressed: [u64; 4]",
            "keys_just_released: [u64; 4]",
            "mouse_down: u32",
            "mouse_just_pressed: u32",
            "mouse_just_released: u32",
            "cursor_x: f32",
            "cursor_y: f32",
            "cursor_delta_x: f32",
            "cursor_delta_y: f32",
            "scroll_x: f32",
            "scroll_y: f32",
        ],
    },
    Golden {
        name: "Key",
        fields: &[
            "0: u16",
        ],
    },
    Golden {
        name: "MouseButton",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "ResourceSlot",
        fields: &[
            "id: ComponentId",
            "ptr: *mut u8",
        ],
    },
    Golden {
        name: "SystemStatus",
        fields: &[
            "0: i32",
        ],
    },
    Golden {
        name: "SystemEntry",
        fields: &[
            "= unsafe extern \"C\" fn(call: *const SystemCall) -> SystemStatus",
        ],
    },
    Golden {
        name: "PipelineId",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "RenderCtx",
        fields: &[
            "0: *mut c_void",
        ],
    },
    Golden {
        name: "RenderPhase",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "RenderPassDesc",
        fields: &[
            "id: StrRef",
            "fragment_wgsl: StrRef",
            "phase: RenderPhase",
            "order: f32",
            "callback: RenderCallback",
        ],
    },
    Golden {
        name: "PostProcessDesc",
        fields: &[
            "id: StrRef",
            "fragment_wgsl: StrRef",
            "settings: ComponentId",
            "settings_size: u64",
            "phase: RenderPhase",
            "order: f32",
        ],
    },
    Golden {
        name: "AssetHandle",
        fields: &[
            "0: u64",
        ],
    },
    Golden {
        name: "Primitive",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "MeshDesc",
        fields: &[
            "primitive: Primitive",
            "size: Vec3",
        ],
    },
    Golden {
        name: "MeshDataDesc",
        fields: &[
            "positions: *const Vec3",
            "position_count: usize",
            "normals: *const Vec3",
            "normal_count: usize",
            "uvs: *const [f32; 2]",
            "uv_count: usize",
            "indices: *const u32",
            "index_count: usize",
        ],
    },
    Golden {
        name: "ImageFormat",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "ImageDesc",
        fields: &[
            "width: u32",
            "height: u32",
            "format: ImageFormat",
            "data: *const u8",
            "data_len: usize",
        ],
    },
    Golden {
        name: "ImageSource",
        fields: &[
            "write: unsafe extern \"C\" fn( src: *mut ImageSource, handle: AssetHandle, data: *const u8, len: usize, ) -> bool",
        ],
    },
    Golden {
        name: "AlphaMode",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "MaterialShaderDesc",
        fields: &[
            "id: StrRef",
            "wgsl: StrRef",
            "settings: ComponentId",
            "settings_size: u64",
            "alpha_mode: AlphaMode",
            "textures: *const AssetHandle",
            "texture_count: usize",
        ],
    },
    Golden {
        name: "MaterialDesc",
        fields: &[
            "color: [f32; 4]",
            "metallic: f32",
            "roughness: f32",
            "emissive: [f32; 4]",
        ],
    },
    Golden {
        name: "CommandKind",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "ServiceCall",
        fields: &[
            "service: u64",
            "op: u32",
            "_pad: u32",
        ],
    },
    Golden {
        name: "SpawnMeshDesc",
        fields: &[
            "mesh: AssetHandle",
            "material: AssetHandle",
            "transform: Transform",
        ],
    },
    Golden {
        name: "Command",
        fields: &[
            "kind: CommandKind",
            "entity: Entity",
            "component: ComponentId",
            "data: *const u8",
            "data_len: usize",
        ],
    },
    Golden {
        name: "CommandSink",
        fields: &[
            "reserve_entity: unsafe extern \"C\" fn(sink: *mut CommandSink) -> Entity",
            "push: unsafe extern \"C\" fn(sink: *mut CommandSink, cmd: *const Command)",
        ],
    },
    Golden {
        name: "LogLevel",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "Host",
        fields: &[
            "_private: [u8; 0]",
        ],
    },
    Golden {
        name: "Interface",
        fields: &[
            "version_major: u32",
            "version_minor: u32",
            "register_component: unsafe extern \"C\" fn(host: *mut Host, desc: *const ComponentDesc) -> ComponentId",
            "component_id_by_name: unsafe extern \"C\" fn(host: *mut Host, name: StrRef) -> ComponentId",
            "add_system: unsafe extern \"C\" fn(host: *mut Host, desc: *const SystemDesc) -> RegisterStatus",
            "log: unsafe extern \"C\" fn(host: *mut Host, level: LogLevel, msg: StrRef)",
            "add_render_pass: unsafe extern \"C\" fn(host: *mut Host, desc: *const RenderPassDesc)",
            "render_set_pipeline: unsafe extern \"C\" fn(ctx: RenderCtx, pipeline: PipelineId)",
            "render_draw: unsafe extern \"C\" fn(ctx: RenderCtx, vertices: u32, instances: u32)",
            "add_post_process: unsafe extern \"C\" fn(host: *mut Host, desc: *const PostProcessDesc)",
            "add_mesh: unsafe extern \"C\" fn(host: *mut Host, desc: *const MeshDesc) -> AssetHandle",
            "add_material: unsafe extern \"C\" fn(host: *mut Host, desc: *const MaterialDesc) -> AssetHandle",
            "register_resource: unsafe extern \"C\" fn(host: *mut Host, desc: *const ComponentDesc) -> ComponentId",
            "insert_resource: unsafe extern \"C\" fn( host: *mut Host, id: ComponentId, value: *const u8, len: usize, )",
            "add_panel: unsafe extern \"C\" fn(host: *mut Host, desc: *const PanelDesc) -> RegisterStatus",
            "set_field_range: unsafe extern \"C\" fn( host: *mut Host, component: ComponentId, field: usize, range: *const FieldRange, ) -> RegisterStatus",
            "add_mesh_data: unsafe extern \"C\" fn( host: *mut Host, desc: *const MeshDataDesc, ) -> AssetHandle",
            "add_material_shader: unsafe extern \"C\" fn( host: *mut Host, desc: *const MaterialShaderDesc, ) -> AssetHandle",
            "add_image: unsafe extern \"C\" fn( host: *mut Host, desc: *const ImageDesc, ) -> AssetHandle",
            "prefix_hashes: *const u64",
            "prefix_count: usize",
            "add_script_backend: unsafe extern \"C\" fn(host: *mut Host, desc: *const ScriptBackendDesc) -> RegisterStatus",
            // Appended in MINOR 4.7 — a settings section is a panel that
            // renders in the Settings overlay, hence the shared PanelDesc.
            "add_settings_section: unsafe extern \"C\" fn(host: *mut Host, desc: *const PanelDesc) -> RegisterStatus",
        ],
    },
    Golden {
        name: "FieldRange",
        fields: &[
            "min: f32",
            "max: f32",
            "speed: f32",
        ],
    },
    Golden {
        name: "PanelDesc",
        fields: &[
            "id: StrRef",
            "title: StrRef",
            "icon: StrRef",
            "category: StrRef",
            "markup: StrRef",
            "on_action: Option<PanelActionEntry>",
            "user: *mut c_void",
        ],
    },
    Golden {
        name: "PanelAction",
        fields: &[
            "name: StrRef",
            "value: f32",
            "user: *mut c_void",
            "iface: *const Interface",
            "commands: *mut CommandSink",
            // Appended in MINOR 4.6. Host-allocated and passed by pointer to the
            // plugin's thunk, so an older plugin never reads it.
            "text: StrRef",
        ],
    },
    Golden {
        name: "PanelActionEntry",
        fields: &[
            "= unsafe extern \"C\" fn(action: *const PanelAction) -> SystemStatus",
        ],
    },
    Golden {
        name: "RemovedRead",
        fields: &[
            "entity_capacity: usize",
            "entities: *mut Entity",
            "entity_count: usize",
        ],
    },
    Golden {
        name: "RemovedSource",
        fields: &[
            "read: unsafe extern \"C\" fn( src: *mut RemovedSource, component: ComponentId, out: *mut RemovedRead, ) -> bool",
        ],
    },
    Golden {
        name: "InitResult",
        fields: &[
            "0: i32",
        ],
    },
    Golden {
        name: "ScriptOp",
        fields: &[
            "0: u32",
        ],
    },
    Golden {
        name: "ScriptStatus",
        fields: &[
            "0: i32",
        ],
    },
    Golden {
        name: "BlobRef",
        fields: &[
            "ptr: *const u8",
            "len: usize",
        ],
    },
    Golden {
        name: "ByteSink",
        fields: &[
            "ctx: *mut c_void",
            "write: unsafe extern \"C\" fn(ctx: *mut c_void, bytes: *const u8, len: usize)",
        ],
    },
    Golden {
        name: "ScriptHostCalls",
        fields: &[
            "ctx: *mut c_void",
            "get: unsafe extern \"C\" fn( ctx: *mut c_void, entity: StrRef, component: StrRef, field: StrRef, reply: *const ByteSink, )",
            "get_component: unsafe extern \"C\" fn( ctx: *mut c_void, entity: StrRef, component: StrRef, reply: *const ByteSink, )",
            "get_components: unsafe extern \"C\" fn(ctx: *mut c_void, entity: StrRef, reply: *const ByteSink)",
            "asset_progress: unsafe extern \"C\" fn(ctx: *mut c_void, reply: *const ByteSink)",
            "translate: unsafe extern \"C\" fn(ctx: *mut c_void, key: StrRef, reply: *const ByteSink)",
        ],
    },
    Golden {
        name: "ScriptCall",
        fields: &[
            "op: ScriptOp",
            "_pad: u32",
            "path: StrRef",
            "source: StrRef",
            "version: u64",
            "entity: u64",
            "frame_seq: u64",
            "frame: BlobRef",
            "entity_ctx: BlobRef",
            "args: BlobRef",
            "vars: BlobRef",
            "out: *const ByteSink",
            "host: *const ScriptHostCalls",
        ],
    },
    Golden {
        name: "ScriptEntry",
        fields: &[
            "= unsafe extern \"C\" fn(call: *const ScriptCall) -> ScriptStatus",
        ],
    },
    Golden {
        name: "ScriptBackendDesc",
        fields: &[
            "name: Str256",
            "extensions: *const Str256",
            "extension_count: usize",
            "entry: ScriptEntry",
        ],
    },
];
