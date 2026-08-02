//! Every `#[repr(C)]` table the plugin reads by offset, pinned field by field.
//!
//! This exists because the append-only rule has been broken three times out of
//! five and nothing noticed until an audit went looking:
//!
//! - `SystemEntry` gained a return value under a MINOR. The process died with no
//!   diagnostic (recorded at `sys.rs:44`).
//! - `add_material_shader` (MINOR 9) and `add_image` (MINOR 11) were each
//!   *inserted* into the middle of [`Interface`] and recorded as appended.
//!
//! A plugin reads these tables by offset, so their field order **is** the ABI.
//! Get it wrong and a plugin calls the slot it compiled against and lands in a
//! different function — a `MeshDataDesc*` read as an `ImageDesc*`, or an
//! unchecked UTF-8 conversion over vertex positions. That is a segfault, and the
//! panic guard around plugin calls catches panics rather than those.
//!
//! ## Why the golden text includes types
//!
//! A first version of this test compared field *names* only. It was useless
//! against the failure that has actually happened here: a function pointer is
//! one `usize` whatever its arity, so changing a signature in place — adding a
//! parameter, retyping one, reordering two, changing the return — leaves both
//! the name list and `size_of` byte-identical. Verified by doing it: adding a
//! `flags: u32` parameter to `add_image` and updating the call site passed both
//! tests. So the written type is pinned too.
//!
//! ## Why it covers more than `Interface`
//!
//! `Interface` was the table that broke, but it is the *least* exposed of the
//! six — read once at init. [`SystemCall`] is read every frame and has already
//! been appended to four times. It also embeds [`FrameCtx`] **by value**, which
//! is the nastiest shape here: appending one field to an innocuous two-float
//! struct silently repoints `user`, `iface`, `host` and `commands`, an ABI break
//! authored in a different struct entirely.
//!
//! ## What this does NOT cover
//!
//! The layout of anything a field *points to*. `MeshDataDesc`, `ImageDesc`,
//! `ComponentDesc` and the domain payloads are reached through pointers, so
//! reordering their fields is invisible here and equally fatal. The rule for
//! those is different and cannot be tested this way: **never edit a `#[repr(C)]`
//! struct that crosses the boundary** — mint a new one beside it and a new
//! function that takes it.
//!
//! ## When you add a function or a field
//!
//! Append it to the end of the struct, append its `"name: type"` to the end of
//! the matching golden list, and bump `VERSION_MINOR`. Any other diff here means
//! you moved something, and every already-built plugin now reads the wrong
//! bytes.

use renzora_plugin::sys::Interface;

/// `(struct name, ordered "field: type" declarations)`.
///
/// Types are written exactly as they appear in the source with whitespace
/// collapsed. That makes the golden text diff-legible — a failure names the
/// field and shows the signature that changed, rather than reporting that two
/// hashes differ.
struct Table {
    name: &'static str,
    fields: &'static [&'static str],
}

const TABLES: &[Table] = &[
    // The frozen function table. Read once at init, and the one that broke.
    Table {
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
        ],
    },
    // Read every frame, and the most-appended table in the ABI. An insertion
    // here repoints `iface`, and a garbage `iface` corrupts every later table
    // read — so this is the one that fails worst. Embeds `FrameCtx` BY VALUE.
    Table {
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
        ],
    },
    // Pinned because `SystemCall` embeds it by value, not for its own sake:
    // one appended float here silently repoints nine fields over there.
    Table {
        name: "FrameCtx",
        fields: &[
            "delta_secs: f32",
            "elapsed_secs: f32",
        ],
    },
    Table {
        name: "CommandSink",
        fields: &[
            "reserve_entity: unsafe extern \"C\" fn(sink: *mut CommandSink) -> Entity",
            "push: unsafe extern \"C\" fn(sink: *mut CommandSink, cmd: *const Command)",
        ],
    },
    // Already grew once (`write`, MINOR 10).
    Table {
        name: "MeshSource",
        fields: &[
            "read: unsafe extern \"C\" fn( src: *mut MeshSource, entity: Entity, out: *mut MeshRead, ) -> bool",
            "write: unsafe extern \"C\" fn( src: *mut MeshSource, handle: AssetHandle, data: *const MeshDataDesc, colors: *const MeshColors, ) -> bool",
        ],
    },
    Table {
        name: "ImageSource",
        fields: &[
            "write: unsafe extern \"C\" fn( src: *mut ImageSource, handle: AssetHandle, data: *const u8, len: usize, ) -> bool",
        ],
    },
    Table {
        name: "HttpSource",
        fields: &[
            "poll: unsafe extern \"C\" fn( src: *mut HttpSource, tag: u64, out: *mut HttpRead, ) -> bool",
        ],
    },
];

/// Field declarations of `struct_name`, in order, as `"name: type"`.
///
/// Parsed out of the source rather than reflected: these are `#[repr(C)]`
/// structs of raw pointers with no reflection and no `Debug`, so the declaration
/// is the only source of truth. Reading it directly also means the test cannot
/// pass by checking something other than what ships.
fn fields_of(src: &str, struct_name: &str) -> Vec<String> {
    // `Interface` is emitted by the `interface!` macro rather than written out,
    // because its field list also has to produce the prefix hashes the load-time
    // check compares — one list, two consumers. Its fields therefore carry no
    // `pub`, which the field parser below allows for.
    let needle = if struct_name == "Interface" {
        "interface! {".to_string()
    } else {
        format!("pub struct {struct_name} {{")
    };
    let start = src
        .find(&needle)
        .unwrap_or_else(|| panic!("`{struct_name}` not found — was it renamed?"));
    let body = &src[start + needle.len()..];
    let end = body
        .find("\n}")
        .unwrap_or_else(|| panic!("unterminated `{struct_name}`"));

    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;

    for line in body[..end].lines() {
        let trimmed = line.trim();
        // Doc comments, section headers and blank lines carry no layout.
        if current.is_empty()
            && (trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#["))
        {
            continue;
        }
        // A field starts at `pub name:` in a written struct, or plain `name:` in
        // the macro form. Requiring a `:` is what keeps a stray line from being
        // mistaken for one — and the depth guard below means an argument line
        // inside a multi-line `fn(..)` is already being accumulated, not started.
        if current.is_empty() {
            let looks_like_field = trimmed
                .strip_prefix("pub ")
                .unwrap_or(trimmed)
                .split(':')
                .next()
                .is_some_and(|n| {
                    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                })
                && trimmed.contains(':');
            if !looks_like_field {
                continue;
            }
        }

        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);

        // A field ends at a comma outside parentheses. Tracking depth is what
        // lets a multi-line `fn(a, b) -> T` declaration be joined into one
        // entry instead of being split at every argument.
        depth += trimmed.matches('(').count();
        depth -= trimmed.matches(')').count().min(depth);
        if depth == 0 && current.ends_with(',') {
            current.pop();
            out.push(current.trim_start_matches("pub ").to_owned());
            current.clear();
        }
    }
    out
}

#[test]
fn offset_keyed_tables_are_append_only() {
    let src = include_str!("../src/sys.rs");

    for table in TABLES {
        let actual = fields_of(src, table.name);
        assert_eq!(
            actual,
            table.fields,
            "\n\n`{}` changed shape.\n\
             \n\
             This struct is read by OFFSET by every already-built plugin. Moving, \
             retyping or removing a field means those plugins now read different \
             bytes than they were compiled for — a wrong-type pointer \
             dereference, not a clean failure.\n\
             \n\
             ADDED something? Put it at the END of the struct and the END of this \
             table's golden list, then bump VERSION_MINOR.\n\
             MOVED, RETYPED or REMOVED something? That is a VERSION_MAJOR change. \
             No MINOR makes it safe.\n",
            table.name
        );
    }
}

/// A second guard for the one mistake the golden text cannot catch on its own.
///
/// The lists above can be edited to match a bad struct in a single pass. The
/// compiler's own answer cannot, so a size that stops matching its field list is
/// evidence something was added or removed without the list being updated.
#[test]
fn interface_size_matches_its_field_list() {
    let fns = TABLES[0].fields.len() - 2; // minus the two u32 version fields
    assert_eq!(
        core::mem::size_of::<Interface>(),
        2 * core::mem::size_of::<u32>() + fns * core::mem::size_of::<usize>(),
        "Interface size does not match its golden field list — a field was added \
         or removed without updating TABLES"
    );
}



