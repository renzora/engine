//! One builder per element: the ember widget for its tag, from its attributes,
//! tied to its value where it has one. See the module docs for the tag list.

use bevy::prelude::*;

use super::source::{bind_bool, bind_choice, bind_number, bind_text, NumberMap, Source};
use super::{Attrs, ElementKind, MarkupElement};
use crate::font::EmberFonts;
use crate::theme::text_muted;
use crate::widgets::*;

/// What [`build`] made.
pub(super) struct Built {
    pub widget: Entity,
    /// Stretch the widget to the element's width (it was sized for an
    /// inspector row).
    pub fill_width: bool,
    /// Also fill the element's height, when the template sets one.
    pub fill_height: bool,
}

impl Built {
    fn fixed(widget: Entity) -> Self {
        Self { widget, fill_width: false, fill_height: false }
    }
    fn filling(widget: Entity) -> Self {
        Self { widget, fill_width: true, fill_height: false }
    }
    /// Fills the element both ways: a code editor, a scroll area.
    fn tall(widget: Entity) -> Self {
        Self { widget, fill_width: true, fill_height: true }
    }
}

fn tone(attrs: &Attrs) -> Tone {
    match attrs.text("tone") {
        "info" => Tone::Info,
        "success" => Tone::Success,
        "warn" | "warning" => Tone::Warn,
        "error" => Tone::Error,
        _ => Tone::Neutral,
    }
}

/// A plain column holding `children`, for a container's content.
fn column(commands: &mut Commands, children: &[Entity]) -> Entity {
    let content = commands
        .spawn((Node { flex_direction: FlexDirection::Column, width: Val::Percent(100.0), ..default() }, Name::new("content")))
        .id();
    commands.entity(content).add_children(children);
    content
}

/// A choice's labels and the value each stands for. `values` defaults to the
/// labels; a `language` element with no `options` lists the loaded packs.
pub(super) fn choice_options(el: &MarkupElement) -> (Vec<String>, Vec<String>) {
    if el.source == Source::Setting("language".into()) && !el.attrs.has("options") {
        let packs = renzora_lang_api::available();
        return (
            packs.iter().map(|p| p.name.clone()).collect(),
            packs.iter().map(|p| p.code.clone()).collect(),
        );
    }
    let labels = el.attrs.list("options");
    let values = if el.attrs.has("values") { el.attrs.list("values") } else { labels.clone() };
    (labels, values)
}

fn refs(items: &[String]) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

pub(super) fn build(
    commands: &mut Commands,
    fonts: &EmberFonts,
    el: &MarkupElement,
    children: Vec<Entity>,
    depth: usize,
) -> Built {
    let a = &el.attrs;
    let ui = &fonts.ui;
    let source = el.source.clone();
    match el.kind {
        // ── Values ──────────────────────────────────────────────────────────
        ElementKind::Dropdown | ElementKind::Segmented | ElementKind::Radio => {
            let (labels, values) = choice_options(el);
            let selected = a.count("selected", 0);
            let w = match el.kind {
                ElementKind::Dropdown => dropdown(commands, fonts, &refs(&labels), selected),
                ElementKind::Segmented => segmented(commands, ui, &refs(&labels), selected),
                _ => radio_group(commands, ui, &refs(&labels), selected),
            };
            bind_choice(commands, w, source, values);
            Built::fixed(w)
        }
        ElementKind::Switch => {
            let w = toggle_switch(commands, a.flag("on"));
            bind_bool(commands, w, source);
            Built::fixed(w)
        }
        ElementKind::Checkbox => {
            let w = checkbox(commands, a.flag("on"));
            bind_bool(commands, w, source);
            Built::fixed(w)
        }
        ElementKind::Slider => {
            let (min, max) = (a.number("min", 0.0), a.number("max", 1.0));
            let w = slider_ranged(commands, a.number("value", min), min, max);
            bind_number(commands, w, source, NumberMap { unit_range: None, step: a.number("step", 0.0) });
            Built::filling(w)
        }
        ElementKind::Fader | ElementKind::Knob => {
            // Both work in 0..1; `min`/`max` give the range of what they drive.
            let (min, max) = (a.number("min", 0.0), a.number("max", 1.0));
            let start = a.number("value", min);
            let unit = if (max - min).abs() > f32::EPSILON { (start - min) / (max - min) } else { 0.0 };
            let w = if el.kind == ElementKind::Fader {
                if a.text("direction") == "horizontal" {
                    fader_horizontal(commands, unit)
                } else {
                    fader(commands, unit)
                }
            } else {
                knob(commands, unit)
            };
            bind_number(commands, w, source, NumberMap { unit_range: Some((min, max)), step: a.number("step", 0.0) });
            Built::fixed(w)
        }
        ElementKind::SpinSlider => {
            let (min, max) = (a.number("min", 0.0), a.number("max", 1.0));
            let w = spin_slider(commands, ui, a.text("label"), a.number("value", min), min, max);
            bind_number(commands, w, source, NumberMap { unit_range: None, step: a.number("step", 0.0) });
            Built::filling(w)
        }
        ElementKind::DragValue => {
            let step = a.number("step", 0.1);
            let w = drag_value(commands, ui, a.text("label"), text_muted(), a.number("value", 0.0), step);
            bind_number(commands, w, source, NumberMap { unit_range: None, step: 0.0 });
            Built::fixed(w)
        }
        ElementKind::Field | ElementKind::Password | ElementKind::Search => {
            let (placeholder, value) = (a.text("placeholder"), a.text("value"));
            let w = match el.kind {
                ElementKind::Field => text_input(commands, ui, placeholder, value),
                ElementKind::Password => password_input(commands, ui, placeholder, value),
                _ => search_input(commands, fonts, placeholder, value),
            };
            bind_text(commands, w, source);
            Built::filling(w)
        }

        // ── Interactive, with their own state ───────────────────────────────
        ElementKind::Toggle => Built::fixed(toggle(commands, a.flag("on"))),
        ElementKind::Stepper => {
            Built::fixed(number_stepper(commands, ui, a.number("value", 0.0), a.number("step", 1.0)))
        }
        ElementKind::Range => Built::filling(range(commands, a.number("low", 0.25), a.number("high", 0.75))),
        ElementKind::XyPad => Built::fixed(xy_pad(commands, a.number("x", 0.5), a.number("y", 0.5))),
        ElementKind::Vector => {
            // Tag names cannot hold digits, so one `<vector>` sized by its values.
            let v = a.numbers("value");
            let at = |i: usize| v.get(i).copied().unwrap_or(0.0);
            Built::filling(match v.len() {
                0..=2 => vec2_edit(commands, ui, at(0), at(1)),
                3 => vec3_edit(commands, ui, at(0), at(1), at(2)),
                _ => vec4_edit(commands, ui, at(0), at(1), at(2), at(3)),
            })
        }
        ElementKind::ColorPicker => {
            let c = a.numbers("color");
            let at = |i: usize| c.get(i).copied().unwrap_or(128.0).clamp(0.0, 255.0) as u8;
            Built::fixed(color_picker(commands, (at(0), at(1), at(2))))
        }
        ElementKind::HsvPicker => Built::fixed(hsv_picker(
            commands,
            a.number("hue", 0.6),
            a.number("saturation", 0.7),
            a.number("value", 0.9),
        )),
        ElementKind::Curve => {
            let p = a.numbers("points");
            let at = |i: usize, d: f32| p.get(i).copied().unwrap_or(d);
            Built::fixed(curve_editor(commands, Vec2::new(at(0, 0.25), at(1, 0.1)), Vec2::new(at(2, 0.75), at(3, 0.9))))
        }
        ElementKind::Multiline => Built::tall(textarea(commands, ui, a.text("placeholder"), &a.prose("value"))),
        ElementKind::TagsInput => {
            let tags = a.list("tags");
            Built::filling(tags_input(commands, fonts, &refs(&tags)))
        }
        ElementKind::MultiSelect => {
            let items = a.list("items");
            let on: Vec<bool> = {
                let picked = a.list("selected");
                items.iter().map(|i| picked.contains(i)).collect()
            };
            Built::fixed(multi_select(commands, ui, &refs(&items), &on))
        }
        ElementKind::ListGroup => {
            let items = a.list("items");
            Built::fixed(list_group(commands, ui, &refs(&items), a.count("active", 0)))
        }
        ElementKind::Pagination => {
            Built::fixed(pagination(commands, ui, a.count("pages", 5), a.count("active", 0)))
        }
        ElementKind::Hamburger => {
            let items = a.list("items");
            Built::fixed(hamburger(commands, fonts, &refs(&items)))
        }
        ElementKind::IconButton => Built::fixed(icon_button(commands, fonts, a.text("icon"))),
        ElementKind::IconLabelButton => {
            Built::fixed(icon_label_button(commands, fonts, a.text("icon"), a.text("label")))
        }

        // ── Display ─────────────────────────────────────────────────────────
        ElementKind::Badge => Built::fixed(badge(commands, ui, a.text("text"), tone(a))),
        ElementKind::Chip => Built::fixed(chip(commands, fonts, a.text("text"))),
        ElementKind::Alert => Built::filling(alert(commands, fonts, tone(a), a.text("title"), a.text("text"))),
        ElementKind::Toast => Built::fixed(toast(commands, fonts, tone(a), a.text("text"))),
        ElementKind::Validation => {
            Built::fixed(validation(commands, fonts, tone(a), a.text("value"), a.text("text")))
        }
        ElementKind::Card => Built::fixed(card(commands, ui, a.text("title"), a.text("text"))),
        ElementKind::Divider => Built::filling(divider(commands)),
        ElementKind::Spinner => Built::fixed(spinner(commands)),
        ElementKind::Skeleton => {
            // `length`/`thickness`, since `width`/`height` style the element.
            Built::fixed(skeleton(commands, a.number("length", 160.0), a.number("thickness", 12.0)))
        }
        ElementKind::Progress => Built::filling(if a.flag("indeterminate") {
            progress_indeterminate(commands, a.number("length", 160.0), a.number("thickness", 6.0))
        } else {
            progress(commands, a.number("value", 0.0))
        }),
        ElementKind::Gauge => Built::fixed(gauge(commands, fonts, a.number("value", 0.0))),
        ElementKind::FloatingLabel => {
            Built::filling(floating_label(commands, ui, a.text("label"), a.text("value")))
        }
        ElementKind::InputGroup => {
            Built::filling(input_group(commands, ui, a.text("addon"), a.text("placeholder")))
        }
        ElementKind::Breadcrumb => {
            let items = a.list("items");
            Built::fixed(breadcrumb(commands, fonts, &refs(&items)))
        }
        ElementKind::Navbar => {
            let links = a.list("links");
            Built::filling(navbar(commands, fonts, a.text("brand"), &refs(&links)))
        }
        ElementKind::Table => {
            // `rows="a,b;c,d"`: rows split by `;`, cells by `,`.
            let headers = a.list("headers");
            let rows: Vec<Vec<String>> = a
                .text("rows")
                .split(';')
                .filter(|r| !r.trim().is_empty())
                .map(|r| r.split(',').map(|c| c.trim().to_string()).collect())
                .collect();
            let row_refs: Vec<Vec<&str>> = rows.iter().map(|r| refs(r)).collect();
            let row_slices: Vec<&[&str]> = row_refs.iter().map(Vec::as_slice).collect();
            Built::fixed(table(commands, ui, &refs(&headers), &row_slices))
        }
        // Flex tracks share out the element's width, so it fills.
        ElementKind::Grid => Built::filling(grid(commands, ui, a.count("count", 6), a.count("columns", 3))),
        ElementKind::Markdown => Built::filling(markdown_view(commands, fonts, &a.prose("text"))),
        ElementKind::CodeBlock => Built::filling(code_block(commands, fonts, &a.prose("text"))),
        ElementKind::CodeEditor => Built::tall(code_editor(commands, &a.prose("text"))),
        ElementKind::LineChart => Built::fixed(line_chart(commands, fonts, &a.numbers("values"))),
        ElementKind::BarChart => Built::fixed(bar_chart(commands, &a.numbers("values"))),
        ElementKind::Sparkline => Built::fixed(sparkline(commands, &a.numbers("values"))),
        ElementKind::Waveform => Built::fixed(waveform(commands, &a.numbers("values"))),
        ElementKind::VuMeter => Built::fixed(vu_meter(commands)),
        ElementKind::MixerStrip => {
            Built::fixed(mixer_strip(commands, fonts, a.text("label"), a.number("value", 0.8)))
        }

        // ── Containers ──────────────────────────────────────────────────────
        ElementKind::Accordion => {
            let content = column(commands, &children);
            Built::filling(accordion_section(commands, fonts, a.text("title"), content, a.flag("open")))
        }
        ElementKind::Scroll => {
            let content = column(commands, &children);
            Built::tall(scroll_view(commands, content))
        }
        ElementKind::Tabs => {
            // One tab per markup child, labelled from `labels` in order.
            let labels = a.list("labels");
            Built::tall(tabs(commands, ui, &refs(&labels), children))
        }
        ElementKind::Popover => {
            let content = column(commands, &children);
            Built::fixed(popover(commands, fonts, a.text("label"), content))
        }
        ElementKind::Tree => {
            Built::filling(tree_node(commands, fonts, a.text("label"), depth, children, a.flag("open")))
        }
    }
}
