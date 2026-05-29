//! Renzora's markup loader.
//!
//! Walks a parsed `HtmlTemplate` and spawns a `bevy_ui` entity tree. Each
//! `<node>` / `<text>` / `<image>` / `<button>` becomes a real entity with
//! standard bevy_ui components (`Node`, `BackgroundColor`, `Text`, `TextFont`,
//! `TextColor`, `BorderColor`, etc.) attached directly. **No `HtmlNode`, no
//! `HtmlStyle`, no per-frame style re-assertion.** Components hold the truth;
//! bevy_ui handles layout and rendering as it does for any other UI.
//!
//! Features covered:
//! - All `<node>` / `<text>` / `<image>` / `<button>` styling: layout (flex +
//!   grid), box model, colors, borders, font size/color, text content.
//! - `{prop}` substitution in both attribute values (via `AttrTokens::compile`)
//!   and text content.
//! - Custom component tags (`<stat_bar>`, `<menu_button>`, …) — looked up in
//!   the [`ComponentRegistry`], built recursively with merged property
//!   overrides.
//! - `<slot/>` — caller's children get reparented to slots in the component
//!   template.
//!
//! Out of scope for now (later phases):
//! - Hover/pressed transitions.
//! - Atlas/flipbook animations.
//! - Custom font assets.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy_hui::prelude::{
    Action, AssetServerAdaptor, AttrTokens, Attribute, HtmlTemplate, NodeType, OnUiChange,
    OnUiEnter, OnUiExit, OnUiPress, OnUiSpawn, StyleAttr, TemplateProperties, XNode,
};

use crate::drag::Draggable;
use crate::provenance::{MarkupImage, MarkupSource};

/// Spawn the template's root node onto `entity` (and its subtree as children).
///
/// `template_handle` is the asset handle the markup was loaded from; it gets
/// stamped on every spawned entity via [`MarkupSource`] so the editor's
/// inspector can locate this entity's attribute byte ranges in
/// `template.source` when writing back to disk. Passing
/// `Handle<HtmlTemplate>::default()` is valid for non-editor callers that
/// don't need provenance.
#[allow(clippy::too_many_arguments)]
pub fn build_template_onto(
    commands: &mut Commands,
    server: &AssetServer,
    host: Entity,
    entity: Entity,
    template: &HtmlTemplate,
    template_handle: Handle<HtmlTemplate>,
    templates: &Assets<HtmlTemplate>,
    overrides: &HashMap<String, String>,
    slot_children: Option<&[XNode]>,
) {
    let mut props = TemplateProperties::default();
    // Defaults first, then overrides win.
    for (k, v) in &template.properties {
        props.insert(k.clone(), v.clone());
    }
    for (k, v) in overrides {
        props.insert(k.clone(), v.clone());
    }

    let ctx = BuildCtx {
        server,
        templates,
        props: &props,
        slot_children,
        template_handle: &template_handle,
        host,
    };

    let Some(root) = template.root.first() else {
        return;
    };
    // Root entity lives at the empty path — `template.root[0]` itself.
    let root_path: Vec<u32> = Vec::new();
    apply_xnode_to(commands, &ctx, entity, root, template, &root_path);
    for (i, child) in root.children.iter().enumerate() {
        let mut child_path = root_path.clone();
        child_path.push(i as u32);
        spawn_subtree(commands, &ctx, entity, child, template, child_path);
    }
}

#[derive(Copy, Clone)]
struct BuildCtx<'a> {
    server: &'a AssetServer,
    templates: &'a Assets<HtmlTemplate>,
    props: &'a TemplateProperties,
    slot_children: Option<&'a [XNode]>,
    /// Asset handle the *current* template was loaded from. `template="..."`
    /// expansions swap this with the inner template's handle in their child
    /// context so provenance keeps pointing at the right `.html`.
    template_handle: &'a Handle<HtmlTemplate>,
    /// The entity that holds the `HtmlTemplatePath` this whole tree was built
    /// from — the data source for `{{ Component.field }}` bindings. Stays
    /// constant through nested `template="..."` expansions so a reused
    /// component (e.g. a healthbar) reads the host entity's components, not
    /// the intermediate UI node it expanded onto.
    host: Entity,
}

fn spawn_subtree(
    commands: &mut Commands,
    ctx: &BuildCtx,
    parent: Entity,
    node: &XNode,
    template: &HtmlTemplate,
    node_path: Vec<u32>,
) {
    // `<slot/>` — drop the caller's children here instead of spawning ourselves.
    if matches!(node.node_type, NodeType::Slot) {
        if let Some(slot_kids) = ctx.slot_children {
            // Use the OUTER context for the slot children's properties (their
            // scope is the caller, not the component template). Slot children
            // *originate* from the caller's template, so they don't carry a
            // provenance path under *this* node — the loader that invoked us
            // already paid the cost to attribute them.
            let outer_ctx = BuildCtx {
                slot_children: None,
                ..*ctx
            };
            for slot_child in slot_kids {
                spawn_subtree(commands, &outer_ctx, parent, slot_child, template, node_path.clone());
            }
        }
        return;
    }

    let child = commands.spawn_empty().id();
    commands.entity(parent).add_child(child);
    apply_xnode_to(commands, ctx, child, node, template, &node_path);

    // `<for>` spawns children per entity at runtime; `<input>` and `<icon>`
    // own their own text content. None recurse their markup children here.
    if is_for_node(node) || is_input_node(node) || is_icon_node(node) {
        return;
    }
    for (i, grand) in node.children.iter().enumerate() {
        let mut grand_path = node_path.clone();
        grand_path.push(i as u32);
        spawn_subtree(commands, ctx, child, grand, template, grand_path);
    }
}

/// True for a `<for>` element.
fn is_for_node(node: &XNode) -> bool {
    matches!(&node.node_type, NodeType::Custom(n) if n == "for")
}

/// True for an `<input>` element.
fn is_input_node(node: &XNode) -> bool {
    matches!(&node.node_type, NodeType::Custom(n) if n == "input")
}

/// True for an `<icon>` element.
fn is_icon_node(node: &XNode) -> bool {
    matches!(&node.node_type, NodeType::Custom(n) if n == "icon")
}

/// Navigate `template` from its root down `path` (chain of child indices).
fn node_at<'a>(template: &'a HtmlTemplate, path: &[u32]) -> Option<&'a XNode> {
    let mut node = template.root.first()?;
    for &idx in path {
        node = node.children.get(idx as usize)?;
    }
    Some(node)
}

/// Build the children of a `<for>` node once for a single matched `host`
/// entity, parented under `parent`. Called by `foreach::update_foreach` per
/// matched entity; each spawned subtree binds `{{ Component.field }}` against
/// `host`, so the loop body reads the entity it was spawned for.
pub fn build_for_children(
    commands: &mut Commands,
    server: &AssetServer,
    templates: &Assets<HtmlTemplate>,
    template_handle: &Handle<HtmlTemplate>,
    for_node_path: &[u32],
    host: Entity,
    parent: Entity,
) {
    let Some(template) = templates.get(template_handle) else {
        return;
    };
    let Some(for_node) = node_at(template, for_node_path) else {
        return;
    };
    let mut props = TemplateProperties::default();
    for (k, v) in &template.properties {
        props.insert(k.clone(), v.clone());
    }
    let ctx = BuildCtx {
        server,
        templates,
        props: &props,
        slot_children: None,
        template_handle,
        host,
    };
    for (i, child) in for_node.children.iter().enumerate() {
        spawn_subtree(commands, &ctx, parent, child, template, vec![i as u32]);
    }
}

/// Persistent cache of every `template="..."` handle the loader has seen.
///
/// Without this resource, strong handles produced inside `template_deps_ready`
/// would drop at the end of the function call. Bevy's asset GC then unloads
/// the template, the next frame's call re-triggers the load, drops it again,
/// and we ping-pong forever — never reaching the "loaded" state. Stashing the
/// handle here keeps the asset alive while the template that needs it is
/// still being built (and beyond — they act as a hot-reload cache).
#[derive(Resource, Default)]
pub struct TemplateHandles {
    handles: bevy::platform::collections::HashMap<String, Handle<HtmlTemplate>>,
}

/// Walk a template's AST (and recursively, every template it references via
/// `template="path"`) to confirm all dependent assets are loaded. Used by
/// `finalize_pending_templates` before kicking off a build so the loader never
/// has to bail half-way through with "template not loaded yet."
pub fn template_deps_ready(
    template: &HtmlTemplate,
    server: &AssetServer,
    templates: &Assets<HtmlTemplate>,
    keeper: &mut TemplateHandles,
) -> bool {
    fn walk(
        nodes: &[XNode],
        server: &AssetServer,
        templates: &Assets<HtmlTemplate>,
        keeper: &mut TemplateHandles,
        seen: &mut bevy::platform::collections::HashSet<String>,
    ) -> bool {
        for node in nodes {
            if let Some(path) = &node.template {
                // Cache the handle so the asset doesn't get dropped between
                // our load and the next frame's check. Idempotent: subsequent
                // visits reuse the same handle.
                let handle = keeper
                    .handles
                    .entry(path.clone())
                    .or_insert_with(|| server.load(path))
                    .clone();
                if !seen.insert(path.clone()) {
                    continue;
                }
                let Some(sub) = templates.get(&handle) else {
                    return false;
                };
                if !walk(&sub.root, server, templates, keeper, seen) {
                    return false;
                }
            }
            if !walk(&node.children, server, templates, keeper, seen) {
                return false;
            }
        }
        true
    }
    let mut seen = bevy::platform::collections::HashSet::default();
    walk(&template.root, server, templates, keeper, &mut seen)
}

/// Build one `XNode` onto `entity`: insert the appropriate bevy_ui components
/// based on node type, parsed `StyleAttr`s, and compiled `{prop}` references.
fn apply_xnode_to(
    commands: &mut Commands,
    ctx: &BuildCtx,
    entity: Entity,
    node: &XNode,
    template: &HtmlTemplate,
    node_path: &[u32],
) {
    // `template="path/to/foo.html"` — explicit-path expansion. Takes priority
    // over `<custom_tag>` resolution so an author can use `<node template="x"
    // .../>` on any built-in element and have the inner template's root
    // collapse onto this entity. Property overrides come from `node.defs`
    // (every unknown attribute), slot children come from `node.children` —
    // identical semantics to the registry path, just resolved differently.
    if let Some(template_path) = &node.template {
        let handle: Handle<HtmlTemplate> = ctx.server.load(template_path);
        // `template_deps_ready` already gated us — the asset *should* be in
        // `Assets<HtmlTemplate>` by the time we get here. Bail gracefully if
        // the gate missed an edge case (e.g. hot-reload races); the parent
        // build will be retried next frame and pick it up.
        let Some(component_template) = ctx.templates.get(&handle) else {
            warn!(
                "renzora_hui: template `{}` not loaded yet — re-try next frame",
                template_path
            );
            return;
        };
        // `<node template="x" label="HP" fill="72%">` → `node.defs = { label,
        // fill }`. Substitute any `{prop}` refs in those override values from
        // the OUTER scope, so chaining works.
        let mut overrides: HashMap<String, String> = HashMap::default();
        for (k, v) in &node.defs {
            overrides.insert(k.clone(), substitute_text(v, ctx.props));
        }
        // `src="..."` is parsed as `Attribute::Path` → `node.src`, not into
        // `defs`. Forward it manually so a template can use `{src}` for its
        // own image source (e.g. the cursor template's `<image src="{src}">`).
        if let Some(s) = &node.src {
            overrides
                .entry("src".to_string())
                .or_insert_with(|| substitute_text(s, ctx.props));
        }
        // The host entity's provenance still points at the *outer* template
        // (it's the entity the user sees and clicks), so stamp it before
        // recursing into the inner template. Children spawned by
        // `build_template_onto` will get their own MarkupSource entries
        // anchored at the inner template's handle.
        commands.entity(entity).insert(MarkupSource {
            template_handle: ctx.template_handle.clone(),
            node_path: node_path.to_vec(),
        });
        build_template_onto(
            commands,
            ctx.server,
            ctx.host,
            entity,
            component_template,
            handle,
            ctx.templates,
            &overrides,
            Some(&node.children),
        );
        return;
    }

    // `<for tag="...">` — a runtime-reactive list. Stamp a `ForEach` and fall
    // through to normal node styling so the `<for>` is itself a styled flex
    // container (its `flex_direction`, `row_gap`, etc. apply); the foreach
    // system fills it with one copy of its children per matching entity.
    if is_for_node(node) {
        let tag = node.defs.get("tag").cloned().unwrap_or_default();
        commands.entity(entity).insert(crate::foreach::ForEach::new(
            ctx.template_handle.clone(),
            node_path.to_vec(),
            tag,
        ));
    } else if is_input_node(node) {
        // `<input bind="Entity.var" placeholder=".." password="true">` — a
        // focusable text field. Stamp TextInput + Button (for click focus);
        // the `Text` is added in the node-type match below. Falls through to
        // node styling so its box/border/padding apply.
        let bind = node.defs.get("bind").cloned().unwrap_or_default();
        let placeholder = node.defs.get("placeholder").cloned().unwrap_or_default();
        let password = node
            .defs
            .get("password")
            .map(|v| v != "false")
            .unwrap_or(false);
        commands
            .entity(entity)
            .insert(crate::input_field::TextInput::new(
                bind,
                placeholder,
                password,
                ctx.host,
            ));
        commands.entity(entity).insert(Button);
    } else if is_icon_node(node) {
        // `<icon>` — styled like a node here; the glyph + Phosphor font are
        // applied later by `icons::apply_icons` (stamped below where the font
        // size/color are known). Fall through to node styling.
    } else if let NodeType::Custom(name) = &node.node_type {
        // Bare `<custom_tag>` with no `template=` attribute. The file-stem
        // registry that used to resolve these is gone — components must be
        // referenced by explicit path now. Warn so any un-migrated demo
        // template surfaces clearly instead of silently rendering an empty box.
        warn!(
            "renzora_hui: <{name}> is not a built-in element — use \
             `<node template=\"path/to/{name}.html\" ...>` instead"
        );
        return;
    }

    let mut layout = Node::default();
    let mut background = None;
    let mut border_color = None;
    let mut border_radius_rect: Option<UiRect> = None;
    let mut font_size = None;
    let mut font_color = None;
    let mut image_src: Option<String> = node.src.clone();

    // Statically-parsed styles (no `{prop}` in attribute value).
    for style in &node.styles {
        apply_style(
            style,
            &mut layout,
            &mut background,
            &mut border_color,
            &mut border_radius_rect,
            &mut font_size,
            &mut font_color,
        );
    }

    // Attributes with `{prop}` refs — compile them now with the current scope.
    let mut adapter = AssetServerAdaptor { server: ctx.server };
    for tokens in &node.uncompiled {
        if let Some(attr) = compile_attr(tokens, ctx.props, &mut adapter) {
            match attr {
                Attribute::Style(s) => apply_style(
                    &s,
                    &mut layout,
                    &mut background,
                    &mut border_color,
                    &mut border_radius_rect,
                    &mut font_size,
                    &mut font_color,
                ),
                Attribute::Path(p) => image_src = Some(p),
                // Other compiled attribute kinds (Action, Tag, PropertyDefinition, Name,
                // Id, Target, Watch, Uncompiled) aren't applied per-node here — they're
                // attribute-class concerns we'll wire as we add interaction (Phase D).
                _ => {}
            }
        }
    }

    // `border_radius` lives on `Node`; map `UiRect` (top,right,bottom,left) to
    // the adjacent corners (top→top_left, right→top_right, …) — same mapping
    // bevy_hui's apply_computed uses.
    if let Some(r) = border_radius_rect {
        layout.border_radius.top_left = r.top;
        layout.border_radius.top_right = r.right;
        layout.border_radius.bottom_right = r.bottom;
        layout.border_radius.bottom_left = r.left;
    }

    // Capture the authored display before `layout` is moved, so a `show=`
    // conditional can restore it when the condition is true.
    let display_when_shown = layout.display;

    let mut ec = commands.entity(entity);
    ec.insert(layout);

    // `show="{{ cond }}"` — conditional visibility. Stamp a `ShowBinding` the
    // binding system evaluates each frame (Display::None when falsy).
    if let Some(expr) = node.tags.get("show") {
        ec.insert(crate::binding::ShowBinding::new(
            expr.clone(),
            ctx.host,
            display_when_shown,
        ));
    }

    // Interactive widget behaviors (see `widgets.rs`). Each needs `Button` so
    // bevy_ui delivers `Interaction`. Targets are plain paths (not `{{ }}`).
    if let Some(target) = node.defs.get("toggle") {
        ec.insert(crate::widgets::Toggle {
            target: target.clone(),
            host: ctx.host,
        });
        ec.insert(Button);
    }
    if let Some(target) = node.defs.get("drag_value") {
        let min = node.defs.get("drag_min").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let max = node.defs.get("drag_max").and_then(|s| s.parse().ok()).unwrap_or(1.0);
        ec.insert(crate::widgets::DragValue {
            target: target.clone(),
            min,
            max,
            host: ctx.host,
        });
        ec.insert(Button);
        // `RelativeCursorPosition` is auto-updated by bevy_ui's focus system and
        // gives the cursor's 0..1 position within the node — what the drag reads.
        ec.insert(bevy::ui::RelativeCursorPosition::default());
    }
    if let Some(target) = node.defs.get("fill") {
        let min = node.defs.get("fill_min").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let max = node.defs.get("fill_max").and_then(|s| s.parse().ok()).unwrap_or(1.0);
        ec.insert(crate::widgets::ValueFill {
            target: target.clone(),
            min,
            max,
            host: ctx.host,
        });
        // A fill is decorative — let clicks pass through to the slider track
        // beneath it so dragging works across the filled portion.
        ec.insert(bevy::ui::FocusPolicy::Pass);
    }
    if let Some(target) = node.defs.get("toggles") {
        ec.insert(crate::widgets::Disclose {
            target: target.clone(),
        });
        ec.insert(Button);
    }

    // `<icon name="...">` — stamp the Icon; the icons system renders the glyph
    // in the Phosphor font. Size from `font_size` (or `size`), color from
    // `font_color`.
    if is_icon_node(node) {
        let raw_size = font_size
            .or_else(|| node.defs.get("size").and_then(|s| s.parse().ok()))
            .unwrap_or(16.0);
        let size = if raw_size.is_finite() {
            raw_size.clamp(1.0, 512.0)
        } else {
            16.0
        };
        // `name="check"` parses to the entity `Name` (node.name), not defs —
        // that's the icon name. Fall back to defs["icon"] if someone used that.
        let icon_name = node
            .name
            .clone()
            .or_else(|| node.defs.get("icon").cloned())
            .unwrap_or_default();
        ec.insert(crate::icons::Icon::new(icon_name, size, font_color));
    }
    // Every markup node is its own selectable widget — the canvas editor
    // hit-tests `UiWidget` entities, so clicking a `<text>` inside a `<panel>`
    // lands on the text (deepest match wins). Combined with `MarkupSource`
    // below, that makes per-element edits round-trip to the `.html` file.
    ec.insert(renzora_game_ui::UiWidget::default());

    // `hover:` / `pressed:` color overrides + tween timing → `Interactive`.
    // bevy_hui parses these into `StyleAttr::Hover/Pressed(inner)`; collect the
    // background/border ones so `transitions::apply_interactive` can swap/ease
    // them on interaction. (Base colors are the already-computed `background` /
    // `border_color`.)
    {
        let mut hover_bg = None;
        let mut hover_border = None;
        let mut pressed_bg = None;
        let mut pressed_border = None;
        let mut tween = 0.0_f32;
        for style in &node.styles {
            match style {
                StyleAttr::Hover(inner) => match inner.as_ref() {
                    StyleAttr::Background(c) => hover_bg = Some(*c),
                    StyleAttr::BorderColor(c) => hover_border = Some(*c),
                    _ => {}
                },
                StyleAttr::Pressed(inner) => match inner.as_ref() {
                    StyleAttr::Background(c) => pressed_bg = Some(*c),
                    StyleAttr::BorderColor(c) => pressed_border = Some(*c),
                    _ => {}
                },
                StyleAttr::Delay(d) | StyleAttr::Duration(d) => tween = *d,
                _ => {}
            }
        }

        if hover_bg.is_some()
            || hover_border.is_some()
            || pressed_bg.is_some()
            || pressed_border.is_some()
        {
            // The transition system mutates BackgroundColor/BorderColor, so make
            // sure they exist even when only a hover/pressed value was given.
            if background.is_none() && (hover_bg.is_some() || pressed_bg.is_some()) {
                ec.insert(BackgroundColor(Color::NONE));
            }
            // Interaction must be present to detect hover/press. Buttons already
            // require it; insert for plain hover nodes too (harmless on buttons).
            ec.insert(Interaction::default());
            ec.insert(crate::transitions::Interactive {
                base_bg: background,
                hover_bg,
                pressed_bg,
                base_border: border_color,
                hover_border,
                pressed_border,
                duration: tween,
            });
        }
    }

    // `draggable="true"` (parsed into `node.tags`) opts the entity into the
    // drag system. Any value except `"false"` counts as truthy — same loose
    // semantics HTML's own `draggable` attr uses.
    if let Some(v) = node.tags.get("draggable") {
        if v != "false" {
            ec.insert(Draggable);
        }
    }

    // `drag_item` — a drag-and-drop source. Payload is the binding host (the
    // item entity inside a `<for>`). Needs Button for Interaction.
    if node.defs.contains_key("drag_item") || node.tags.contains_key("drag_item") {
        ec.insert(crate::dnd::DragItem { payload: ctx.host });
        ec.insert(Button);
        // Pass so the item still receives Interaction (for pickup) but doesn't
        // block the drop zone beneath it from detecting the cursor.
        ec.insert(bevy::ui::FocusPolicy::Pass);
    }
    // `cursor="grab"` — OS cursor icon shown while this node is hovered.
    if let Some(name) = node.defs.get("cursor") {
        if let Some(icon) = crate::cursor_icon::parse_cursor(name) {
            ec.insert(crate::cursor_icon::HoverCursor(icon));
            ec.insert(Interaction::default());
        }
    }
    // `dropzone drop_tag="basket" on_drop="..."` — a drop target.
    if node.defs.contains_key("dropzone") || node.tags.contains_key("dropzone") {
        ec.insert(crate::dnd::DropZone {
            drop_tag: node.defs.get("drop_tag").cloned(),
            on_drop: node.defs.get("on_drop").cloned(),
        });
        ec.insert(bevy::ui::RelativeCursorPosition::default());
        ec.insert(Interaction::default());
    }

    // Event listeners (`on_press="..."`, `on_enter="..."`, `on_exit="..."`,
    // `on_spawn`, `on_change`) get attached as bevy_hui's prelude components.
    // `interactions.rs` then watches `Changed<Interaction>` on these entities
    // and forwards the names into `renzora::ScriptUiInbox` so every script's
    // `on_ui(name, args, entity)` Lua hook fires.
    for action in &node.event_listener {
        match action {
            Action::OnPress(fns) => {
                ec.insert(OnUiPress(fns.clone()));
            }
            Action::OnEnter(fns) => {
                ec.insert(OnUiEnter(fns.clone()));
            }
            Action::OnExit(fns) => {
                ec.insert(OnUiExit(fns.clone()));
            }
            Action::OnSpawn(fns) => {
                ec.insert(OnUiSpawn(fns.clone()));
            }
            Action::OnChange(fns) => {
                ec.insert(OnUiChange(fns.clone()));
            }
        }
    }

    // Provenance: the editor's inspector reads this back to locate the byte
    // range to patch when a user edits an attribute. Skipped for entities
    // whose handle is `default()` (non-editor callers that don't need it).
    if ctx.template_handle != &Handle::<HtmlTemplate>::default() {
        ec.insert(MarkupSource {
            template_handle: ctx.template_handle.clone(),
            node_path: node_path.to_vec(),
        });
    }

    if let Some(c) = background {
        ec.insert(BackgroundColor(c));
    }
    if let Some(c) = border_color {
        ec.insert(BorderColor::all(c));
    }
    // Always set a `Name` so the entity is visible in the hierarchy panel and
    // identifiable. Markup `id="..."` and `name="..."` win over the node-type
    // fallback.
    let display_name = node
        .id
        .as_ref()
        .map(|id| format!("#{}", id))
        .or_else(|| node.name.clone())
        .unwrap_or_else(|| match &node.node_type {
            NodeType::Node => "node".to_string(),
            NodeType::Button => "button".to_string(),
            NodeType::Text => "text".to_string(),
            NodeType::Image => "image".to_string(),
            NodeType::Custom(s) => s.clone(),
            NodeType::Slot => "slot".to_string(),
            NodeType::Template => "template".to_string(),
            NodeType::Property => "property".to_string(),
        });
    ec.insert(Name::new(display_name));

    match &node.node_type {
        NodeType::Button => {
            ec.insert(Button);
        }
        NodeType::Text => {
            // `<text>HERE</text>`: text content lives in the template's slot
            // map, indexed by the node's `content_id` (captured at parse). May
            // contain `{prop}` references.
            let raw = template
                .content
                .get(node.content_id)
                .cloned()
                .unwrap_or_default();
            let content = substitute_text(raw.trim(), ctx.props);
            if !content.is_empty() {
                // Runtime binding (`{{ Component.field }}`): stamp a
                // `TextBinding` so `binding::update_text_bindings` re-resolves
                // it against `ctx.host`'s live components each frame. The
                // initial `Text` is the raw token; the system overwrites it
                // on the first tick.
                if has_binding(&content) {
                    ec.insert(crate::binding::TextBinding::new(content.clone(), ctx.host));
                }
                ec.insert(Text::new(content));
                if let Some(s) = font_size {
                    ec.insert(TextFont::from_font_size(s));
                }
                if let Some(c) = font_color {
                    ec.insert(TextColor(c));
                }
            }
        }
        NodeType::Image => {
            // Every `<image>` gets a `MarkupImage` marker so the inspector can
            // surface the "UI Image" drag-drop slot even before `src` is set.
            ec.insert(MarkupImage);
            // `ImageNode` only gets inserted when `src` actually resolves to a
            // non-empty path — otherwise `server.load("")` errors out and we'd
            // be left with a broken image handle on what's meant to render as
            // a styled `<node>` fallback (e.g. the default `<cursor>` dot).
            if let Some(src) = image_src {
                if !src.is_empty() {
                    ec.insert(ImageNode {
                        image: ctx.server.load(src),
                        image_mode: NodeImageMode::Auto,
                        ..default()
                    });
                }
            }
        }
        NodeType::Node
        | NodeType::Custom(_)
        | NodeType::Slot
        | NodeType::Template
        | NodeType::Property => {}
    }

    // `<input>` renders its edited value as text on the entity itself; the
    // input system (`input_field::sync_inputs`) keeps it updated.
    if is_input_node(node) {
        ec.insert(Text::new(String::new()));
        if let Some(s) = font_size {
            ec.insert(TextFont::from_font_size(s));
        }
        if let Some(c) = font_color {
            ec.insert(TextColor(c));
        }
    }
}

/// Resolve one `AttrTokens` (a `key="{prop}"` style attribute) using the
/// current property scope. Wraps bevy_hui's `AttrTokens::compile`.
fn compile_attr(
    tokens: &AttrTokens,
    props: &TemplateProperties,
    adapter: &mut AssetServerAdaptor,
) -> Option<Attribute> {
    tokens.compile(props, adapter)
}

/// Replace build-time `{key}` occurrences in `s` with `props[key]`.
///
/// **Runtime bindings `{{ ... }}` are passed through verbatim** — they're a
/// different mechanism (resolved every frame against live ECS components by
/// `binding::update_text_bindings`), so the build-time pass must not touch
/// them. A `{{` always wins over a single `{`.
fn substitute_text(s: &str, props: &TemplateProperties) -> String {
    if !s.contains('{') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        // Runtime binding `{{ ... }}` — copy through (both braces + body +
        // closing `}}`) untouched and leave it for the binding system.
        if chars.peek() == Some(&'{') {
            out.push('{');
            out.push(chars.next().unwrap()); // second '{'
            while let Some(pc) = chars.next() {
                out.push(pc);
                if pc == '}' && chars.peek() == Some(&'}') {
                    out.push(chars.next().unwrap()); // closing second '}'
                    break;
                }
            }
            continue;
        }
        let mut key = String::new();
        let mut closed = false;
        for pc in chars.by_ref() {
            if pc == '}' {
                closed = true;
                break;
            }
            key.push(pc);
        }
        if !closed {
            // Unterminated `{...` — emit literally.
            out.push('{');
            out.push_str(&key);
            continue;
        }
        let key = key.trim();
        if let Some(v) = props.get(key) {
            out.push_str(v);
        }
        // If unknown, drop silently (matches bevy_hui's behaviour).
    }
    out
}

/// True if `s` contains a runtime binding token `{{ ... }}`.
fn has_binding(s: &str) -> bool {
    s.contains("{{")
}

/// Update a partial `Node` (and the color/text "side" slots) from one parsed
/// style attribute.
fn apply_style(
    style: &StyleAttr,
    n: &mut Node,
    bg: &mut Option<Color>,
    border: &mut Option<Color>,
    border_radius_rect: &mut Option<UiRect>,
    font_size: &mut Option<f32>,
    font_color: &mut Option<Color>,
) {
    use StyleAttr as S;
    match style {
        S::Display(d) => n.display = *d,
        S::Position(p) => n.position_type = *p,
        S::Left(v) => n.left = *v,
        S::Right(v) => n.right = *v,
        S::Top(v) => n.top = *v,
        S::Bottom(v) => n.bottom = *v,
        S::Width(v) => n.width = *v,
        S::Height(v) => n.height = *v,
        S::MinWidth(v) => n.min_width = *v,
        S::MinHeight(v) => n.min_height = *v,
        S::MaxWidth(v) => n.max_width = *v,
        S::MaxHeight(v) => n.max_height = *v,
        S::AspectRatio(f) => n.aspect_ratio = Some(*f),
        S::AlignItems(a) => n.align_items = *a,
        S::JustifyItems(a) => n.justify_items = *a,
        S::AlignSelf(a) => n.align_self = *a,
        S::JustifySelf(a) => n.justify_self = *a,
        S::AlignContent(a) => n.align_content = *a,
        S::JustifyContent(a) => n.justify_content = *a,
        S::Margin(r) => n.margin = *r,
        S::Padding(r) => n.padding = *r,
        S::FlexDirection(d) => n.flex_direction = *d,
        S::FlexWrap(w) => n.flex_wrap = *w,
        S::FlexGrow(f) => n.flex_grow = *f,
        S::FlexShrink(f) => n.flex_shrink = *f,
        S::FlexBasis(v) => n.flex_basis = *v,
        S::RowGap(v) => n.row_gap = *v,
        S::ColumnGap(v) => n.column_gap = *v,
        S::Border(r) => n.border = *r,
        S::BorderColor(c) => *border = Some(*c),
        S::BorderRadius(r) => *border_radius_rect = Some(*r),
        S::Background(c) => *bg = Some(*c),
        // Clamp to a finite, sane range. A malformed markup value (e.g. a
        // huge number that overflows to inf) would otherwise produce
        // infinite/NaN text dimensions and crash bevy_ui's layout
        // (`BorderRadius::resolve` panics clamping against NaN).
        S::FontSize(f) => {
            let v = if f.is_finite() { f.clamp(1.0, 512.0) } else { 16.0 };
            *font_size = Some(v);
        }
        S::FontColor(c) => *font_color = Some(*c),
        S::GridAutoFlow(g) => n.grid_auto_flow = *g,
        S::GridTemplateRows(v) => n.grid_template_rows = v.clone(),
        S::GridTemplateColumns(v) => n.grid_template_columns = v.clone(),
        S::GridAutoRows(v) => n.grid_auto_rows = v.clone(),
        S::GridAutoColumns(v) => n.grid_auto_columns = v.clone(),
        S::GridRow(g) => n.grid_row = *g,
        S::GridColumn(g) => n.grid_column = *g,
        // Skipped for now: Hover/Pressed/Active (transitions → Phase D),
        // animation knobs, image styles, shadows, outline, overflow, zindex,
        // custom fonts.
        _ => {}
    }
}
