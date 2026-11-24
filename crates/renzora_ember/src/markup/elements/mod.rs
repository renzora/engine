//! Ember widgets as markup elements: `<dropdown>`, `<slider>`, `<card>`,
//! `<tabs>` and the rest of the toolkit, by name.
//!
//! ```html
//! <dropdown setting="window.mode" options="Windowed,Fullscreen,Borderless" />
//! <switch setting="window.vsync" />
//! <slider setting="audio.buses.Music.volume" min="0" max="1" step="0.01" />
//! <badge text="New" tone="success" />
//! <tabs labels="General,Audio">
//!     <node>...first panel...</node>
//!     <node>...second panel...</node>
//! </tabs>
//! ```
//!
//! Each element is the same ember widget the editor's panels are built from,
//! so a template shows real widgets wherever it is drawn: in the game, in the
//! editor's viewport and in the UI editor. Built by a script instead, they
//! left the editor with empty slots, because scripts do not run while editing.
//!
//! Three kinds of element:
//!
//! - **Values** (`<dropdown>`, `<switch>`, `<slider>`, `<field>`, ...) tie to
//!   `setting="<path>"`, a game setting by its `set_setting` path (plus
//!   `ui.scale` and `language`), or `bind="<var>"`, a variable on the host's
//!   script. See [`source`]. In the editor they show the value and never
//!   write it.
//! - **Display** (`<badge>`, `<alert>`, `<table>`, `<line_chart>`, ...) take
//!   their content from attributes.
//! - **Containers** (`<tabs>`, `<accordion>`, `<scroll>`, `<popover>`,
//!   `<tree>`) hold their markup children as their content.
//!
//! **Names.** Markup tag names are letters and underscores, and the parser
//! reads any tag that *starts with* `node`, `image`, `button`, `text`, `slot`
//! or `template` as that built-in, so no element here begins with one: the
//! text inputs are `<field>`, `<password>`, `<search>` and `<multiline>`.
//!
//! The loader styles the element like any node (`width`, `margin`, ...) and
//! stamps a [`MarkupElement`]; [`build_elements`] puts the widget inside it
//! once the UI fonts exist, a few frames after startup.

mod build;
pub(crate) mod source;

use bevy::ecs::world::CommandQueue;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use renzora_project_api::CurrentProject;

use crate::font::EmberFonts;

/// Every markup element this module builds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ElementKind {
    // Values a setting or script variable can drive.
    Dropdown,
    Segmented,
    Radio,
    Switch,
    Checkbox,
    Slider,
    Fader,
    Knob,
    SpinSlider,
    DragValue,
    Field,
    Password,
    Search,
    // Interactive, with their own state.
    Toggle,
    Stepper,
    Range,
    XyPad,
    /// Two, three or four number fields, by how many `value`s it has.
    Vector,
    ColorPicker,
    HsvPicker,
    Curve,
    Multiline,
    TagsInput,
    MultiSelect,
    ListGroup,
    Pagination,
    Hamburger,
    IconButton,
    IconLabelButton,
    // Display.
    Badge,
    Chip,
    Alert,
    Toast,
    Validation,
    Card,
    Divider,
    Spinner,
    Skeleton,
    Progress,
    Gauge,
    FloatingLabel,
    InputGroup,
    Breadcrumb,
    Navbar,
    Table,
    Grid,
    Markdown,
    CodeBlock,
    CodeEditor,
    LineChart,
    BarChart,
    Sparkline,
    Waveform,
    VuMeter,
    MixerStrip,
    // Containers: their markup children are their content.
    Accordion,
    Scroll,
    Tabs,
    Popover,
    Tree,
}

/// Tag name to element, in one table so the docs and the parser agree.
const TAGS: &[(&str, ElementKind)] = &[
    ("dropdown", ElementKind::Dropdown),
    ("segmented", ElementKind::Segmented),
    ("radio", ElementKind::Radio),
    ("switch", ElementKind::Switch),
    ("checkbox", ElementKind::Checkbox),
    ("slider", ElementKind::Slider),
    ("fader", ElementKind::Fader),
    ("knob", ElementKind::Knob),
    ("spin_slider", ElementKind::SpinSlider),
    ("drag_value", ElementKind::DragValue),
    ("field", ElementKind::Field),
    ("password", ElementKind::Password),
    ("search", ElementKind::Search),
    ("toggle", ElementKind::Toggle),
    ("stepper", ElementKind::Stepper),
    ("range", ElementKind::Range),
    ("xy_pad", ElementKind::XyPad),
    ("vector", ElementKind::Vector),
    ("color_picker", ElementKind::ColorPicker),
    ("hsv_picker", ElementKind::HsvPicker),
    ("curve", ElementKind::Curve),
    ("multiline", ElementKind::Multiline),
    ("tags_input", ElementKind::TagsInput),
    ("multi_select", ElementKind::MultiSelect),
    ("list_group", ElementKind::ListGroup),
    ("pagination", ElementKind::Pagination),
    ("hamburger", ElementKind::Hamburger),
    ("icon_button", ElementKind::IconButton),
    ("icon_label_button", ElementKind::IconLabelButton),
    ("badge", ElementKind::Badge),
    ("chip", ElementKind::Chip),
    ("alert", ElementKind::Alert),
    ("toast", ElementKind::Toast),
    ("validation", ElementKind::Validation),
    ("card", ElementKind::Card),
    ("divider", ElementKind::Divider),
    ("spinner", ElementKind::Spinner),
    ("skeleton", ElementKind::Skeleton),
    ("progress", ElementKind::Progress),
    ("gauge", ElementKind::Gauge),
    ("floating_label", ElementKind::FloatingLabel),
    ("input_group", ElementKind::InputGroup),
    ("breadcrumb", ElementKind::Breadcrumb),
    ("navbar", ElementKind::Navbar),
    ("table", ElementKind::Table),
    ("grid", ElementKind::Grid),
    ("markdown", ElementKind::Markdown),
    ("code_block", ElementKind::CodeBlock),
    ("code_editor", ElementKind::CodeEditor),
    ("line_chart", ElementKind::LineChart),
    ("bar_chart", ElementKind::BarChart),
    ("sparkline", ElementKind::Sparkline),
    ("waveform", ElementKind::Waveform),
    ("vu_meter", ElementKind::VuMeter),
    ("mixer_strip", ElementKind::MixerStrip),
    ("accordion", ElementKind::Accordion),
    ("scroll", ElementKind::Scroll),
    ("tabs", ElementKind::Tabs),
    ("popover", ElementKind::Popover),
    ("tree", ElementKind::Tree),
];

impl ElementKind {
    /// The element a tag name stands for.
    pub(crate) fn from_tag(tag: &str) -> Option<Self> {
        TAGS.iter().find(|(name, _)| *name == tag).map(|(_, kind)| *kind)
    }

    /// Whether its markup children are its content (built into it) rather than
    /// ignored.
    pub(crate) fn takes_children(self) -> bool {
        matches!(self, Self::Accordion | Self::Scroll | Self::Tabs | Self::Popover | Self::Tree)
    }
}

/// A markup element waiting for its ember widget. Stamped by the loader.
#[derive(Component, Clone, Debug)]
pub(crate) struct MarkupElement {
    kind: ElementKind,
    source: source::Source,
    attrs: Attrs,
}

impl MarkupElement {
    pub(crate) fn new(kind: ElementKind, defs: &HashMap<String, String>, host: Entity) -> Self {
        Self { kind, source: source::Source::from_attrs(defs, host), attrs: Attrs(defs.clone()) }
    }
}

/// An element's attributes, with the parsing every builder needs.
#[derive(Clone, Debug, Default)]
pub(crate) struct Attrs(HashMap<String, String>);

impl Attrs {
    fn text(&self, key: &str) -> &str {
        self.0.get(key).map(String::as_str).unwrap_or("")
    }
    /// Text that may span lines. An attribute value cannot hold a line break,
    /// so `\n` (and `\t`) are written as escapes.
    fn prose(&self, key: &str) -> String {
        self.text(key).replace("\\n", "\n").replace("\\t", "\t")
    }
    fn number(&self, key: &str, default: f32) -> f32 {
        self.0.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(default)
    }
    fn count(&self, key: &str, default: usize) -> usize {
        self.0.get(key).and_then(|v| v.trim().parse().ok()).unwrap_or(default)
    }
    fn flag(&self, key: &str) -> bool {
        self.0.get(key).is_some_and(|v| !matches!(v.trim(), "" | "false" | "0"))
    }
    fn has(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
    /// A comma-separated list.
    fn list(&self, key: &str) -> Vec<String> {
        self.0
            .get(key)
            .map(|v| v.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect())
            .unwrap_or_default()
    }
    /// A comma-separated list of numbers.
    fn numbers(&self, key: &str) -> Vec<f32> {
        self.list(key).iter().filter_map(|v| v.parse().ok()).collect()
    }
}

/// The widget has been built into this element.
#[derive(Component)]
struct ElementBuilt;

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<source::ElementSettings>().add_systems(
        Update,
        (
            source::refresh_settings.run_if(resource_exists_and_changed::<CurrentProject>),
            build_elements,
        )
            .chain(),
    );
}

/// Build the ember widget into every new element.
fn build_elements(world: &mut World) {
    let pending: Vec<(Entity, MarkupElement, Vec<Entity>)> = {
        let mut q = world.query_filtered::<(Entity, &MarkupElement, Option<&Children>), Without<ElementBuilt>>();
        q.iter(world)
            .map(|(e, el, kids)| (e, el.clone(), kids.map(|k| k.to_vec()).unwrap_or_default()))
            .collect()
    };
    if pending.is_empty() {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // A tree's depth is how many trees it sits inside.
    let depths: Vec<usize> = pending.iter().map(|(e, el, _)| tree_depth(world, *e, el.kind)).collect();

    let mut queue = CommandQueue::default();
    let mut sizing = Vec::new();
    {
        let mut commands = Commands::new(&mut queue, world);
        for ((element, el, children), depth) in pending.into_iter().zip(depths) {
            let content = if el.kind.takes_children() { children } else { Vec::new() };
            let built = build::build(&mut commands, &fonts, &el, content, depth);
            sizing.push((element, built.widget, built.fill_width, built.fill_height));
            commands.entity(element).add_child(built.widget).insert(ElementBuilt);
        }
    }
    queue.apply(world);

    for (element, widget, fill_width, fill_height) in sizing {
        fit(world, element, widget, fill_width, fill_height);
    }
}

/// Size a widget to its element.
///
/// A filling widget (a slider sized for an inspector row, a grid of flex
/// tracks) is a percentage of the element, so an element the template gave no
/// `width` would shrink to nothing inside a row: it takes its parent's width
/// instead. A tall widget (a code editor, a scroll area) fills the element's
/// `height` when the template sets one, in place of its own fixed height.
fn fit(world: &mut World, element: Entity, widget: Entity, fill_width: bool, fill_height: bool) {
    let element_height = {
        let Some(mut node) = world.get_mut::<Node>(element) else { return };
        if fill_width && node.width == Val::Auto {
            node.width = Val::Percent(100.0);
        }
        node.height
    };
    let Some(mut node) = world.get_mut::<Node>(widget) else { return };
    if fill_width {
        node.width = Val::Percent(100.0);
    }
    if fill_height && element_height != Val::Auto {
        node.height = Val::Percent(100.0);
    }
}

fn tree_depth(world: &World, entity: Entity, kind: ElementKind) -> usize {
    if kind != ElementKind::Tree {
        return 0;
    }
    let mut depth = 0;
    let mut at = entity;
    while let Some(parent) = world.get::<ChildOf>(at).map(|c| c.parent()) {
        if world.get::<MarkupElement>(parent).is_some_and(|el| el.kind == ElementKind::Tree) {
            depth += 1;
        }
        at = parent;
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tag_resolves_and_none_collide_with_a_built_in() {
        for (name, kind) in TAGS {
            assert_eq!(ElementKind::from_tag(name), Some(*kind), "{name}");
            assert!(
                name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{name}: tag names are letters and underscores"
            );
            for built_in in ["node", "image", "button", "text", "slot", "template"] {
                assert!(!name.starts_with(built_in), "{name} would parse as <{built_in}>");
            }
        }
        assert_eq!(ElementKind::from_tag("node"), None);
    }

    #[test]
    fn only_containers_take_children() {
        assert!(ElementKind::Tabs.takes_children());
        assert!(ElementKind::Tree.takes_children());
        assert!(!ElementKind::Dropdown.takes_children());
        assert!(!ElementKind::Card.takes_children());
    }

    #[test]
    fn attribute_lists_trim_and_skip_empties() {
        let attrs = Attrs([("v".to_string(), " a, b ,,c ".to_string())].into_iter().collect());
        assert_eq!(attrs.list("v"), ["a", "b", "c"]);
        let nums = Attrs([("n".to_string(), "1, 2.5, x".to_string())].into_iter().collect());
        assert_eq!(nums.numbers("n"), [1.0, 2.5]);
        let prose = Attrs([("t".to_string(), "# A\\nb\\tc".to_string())].into_iter().collect());
        assert_eq!(prose.prose("t"), "# A\nb\tc");
    }
}
