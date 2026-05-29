//! Source-of-truth bookkeeping for markup-built entities.
//!
//! When the loader spawns an entity from an `<XNode>`, it stamps a
//! [`MarkupSource`] component pointing back to the `.html` asset and the path
//! of child indices needed to walk from `template.root[i]` down to that
//! specific node. The editor's inspector reads this back when an attribute is
//! edited so it can locate the byte range in `HtmlTemplate::source` to patch.

use bevy::prelude::*;
use bevy_hui::prelude::HtmlTemplate;

/// Attached to every entity spawned by the renzora_hui loader.
///
/// `template_handle` is the asset handle the markup came from (used to fetch
/// `HtmlTemplate::source` for byte-range edits). `node_path` is the chain of
/// child indices from the template's root: `[]` for `template.root[0]`,
/// `[2]` for `root[0].children[2]`, `[2, 1]` for `root[0].children[2].children[1]`,
/// and so on. The very first index into `template.root` is fixed at 0 — the
/// loader only ever spawns from `template.root[0]` per
/// [`build_template_onto`](crate::loader::build_template_onto).
///
/// Custom component tags (`<menu_button .../>`) flatten into the host entity;
/// the loader stamps the *host* entity's `MarkupSource` since the host is the
/// one the inspector sees and edits. Children spawned by the inner component
/// template carry `MarkupSource`s rooted at that inner template's handle.
#[derive(Component, Debug, Clone)]
pub struct MarkupSource {
    pub template_handle: Handle<HtmlTemplate>,
    pub node_path: Vec<u32>,
}

/// Marker for entities the loader spawned from an `<image>` markup node,
/// regardless of whether they currently carry an `ImageNode` (which is only
/// inserted once `src` resolves to a non-empty path). The editor inspector
/// keys its "UI Image" card off this marker so the drag-drop `Source` slot
/// appears even on a freshly-spawned `<cursor>` that has no texture yet.
#[derive(Component, Default, Debug, Clone)]
pub struct MarkupImage;
