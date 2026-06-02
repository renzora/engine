//! The gallery panels — a live catalog showcasing every ember component.

use bevy::prelude::*;

use crate::font::{ui_font, EmberFonts};
use crate::theme::{
    rgb, ACCENT_BLUE, CLOSE_RED, HEADER_BG, PLAY_GREEN, TAB_HOVER_BG, TEXT_MUTED, TEXT_PRIMARY,
};

// Bring every widget builder + `Tone` into scope.
use super::*;

/// A titled panel column — the shell of each gallery category panel.
fn panel_column(
    commands: &mut Commands,
    font: &Handle<Font>,
    title: &str,
    rows: Vec<Entity>,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                // Size to content (not 100% height) so it can overflow — and
                // therefore scroll within — its host pane.
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            Name::new("gallery-panel"),
        ))
        .id();
    let heading = commands
        .spawn((
            Text::new(title),
            ui_font(font, 15.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(root).add_child(heading);
    commands.entity(root).add_children(&rows);
    root
}

/// Gallery panel: buttons & toggles.
pub fn gallery_buttons(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let btns = [
        button(commands, font, "Primary"),
        button(commands, font, "Secondary"),
        button(commands, font, "Cancel"),
    ];
    let buttons = hstack(commands, 8.0, &btns);
    let f_buttons = field(commands, font, "Buttons", buttons);

    let togs = [toggle(commands, true), toggle(commands, false)];
    let toggles = hstack(commands, 10.0, &togs);
    let f_toggle = field(commands, font, "Toggles", toggles);

    panel_column(commands, font, "Buttons & Toggles", vec![f_buttons, f_toggle])
}

/// Gallery panel: text / numeric / list inputs.
pub fn gallery_inputs(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let ti = text_input(commands, font, "Type here…", "");
    let f_text = field(commands, font, "Text", ti);

    let dd = dropdown(commands, fonts, &["Forward", "Deferred", "Mobile"], 0);
    let f_dropdown = field(commands, font, "Dropdown", dd);

    let sld = slider(commands, 0.6);
    let f_slider = field(commands, font, "Slider", sld);

    let step = number_stepper(commands, font, 12.0, 1.0);
    let f_step = field(commands, font, "Stepper", step);

    panel_column(
        commands,
        font,
        "Inputs",
        vec![f_text, f_dropdown, f_slider, f_step],
    )
}

/// Gallery panel: selection controls.
pub fn gallery_selection(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let cbs = [checkbox(commands, true), checkbox(commands, false)];
    let checks = hstack(commands, 10.0, &cbs);
    let f_check = field(commands, font, "Checkbox", checks);

    let radios = radio_group(commands, font, &["A", "B", "C"], 0);
    let f_radio = field(commands, font, "Radio", radios);

    let seg = segmented(commands, font, &["One", "Two", "Three"], 1);
    let f_seg = field(commands, font, "Segmented", seg);

    panel_column(commands, font, "Selection", vec![f_check, f_radio, f_seg])
}

/// Gallery panel: typography scale.
pub fn gallery_typography(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let rows = vec![
        h1(commands, font, "Heading 1"),
        h2(commands, font, "Heading 2"),
        h3(commands, font, "Heading 3"),
        h4(commands, font, "Heading 4"),
        paragraph(commands, font, "Body paragraph in the UI font."),
        caption(commands, font, "Caption — small and muted."),
        link(commands, font, "A hyperlink"),
        code(commands, font, "inline_code()"),
    ];
    panel_column(commands, font, "Typography", rows)
}

/// Gallery panel: feedback components.
pub fn gallery_feedback(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let badges = [
        badge(commands, font, "Info", Tone::Info),
        badge(commands, font, "OK", Tone::Success),
        badge(commands, font, "Warn", Tone::Warn),
        badge(commands, font, "Error", Tone::Error),
    ];
    let badge_row = hstack(commands, 6.0, &badges);
    let f_badge = field(commands, font, "Badge", badge_row);

    let al = alert(
        commands,
        fonts,
        Tone::Info,
        "Heads up",
        "This is an inline alert message.",
    );
    let to = toast(commands, fonts, Tone::Success, "Saved successfully");

    let pr = progress(commands, 0.7);
    let f_prog = field(commands, font, "Progress", pr);

    let sk = skeleton(commands, 180.0, 12.0);
    let f_skel = field(commands, font, "Skeleton", sk);

    panel_column(
        commands,
        font,
        "Feedback",
        vec![f_badge, al, to, f_prog, f_skel],
    )
}

/// Gallery panel: inspector value editors.
pub fn gallery_inspector(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;

    let pos = vec3_edit(commands, font, 0.0, 1.0, 0.0);
    let r_pos = property_row(commands, font, "Position", pos);

    let dv = drag_value(commands, font, "", TEXT_MUTED, 1.0, 0.05);
    let r_scale = property_row(commands, font, "Scale", dv);

    let cp = color_picker(commands, (80, 140, 255));
    let r_color = property_row(commands, font, "Color", cp);

    let knobs = [knob(commands, 0.3), knob(commands, 0.7)];
    let knob_row = hstack(commands, 12.0, &knobs);
    let r_knob = property_row(commands, font, "Knobs", knob_row);

    let gauges = [gauge(commands, fonts, 0.65), gauge(commands, fonts, 0.3)];
    let gauge_row = hstack(commands, 12.0, &gauges);
    let r_gauge = property_row(commands, font, "Gauges", gauge_row);

    let pads = [fader(commands, 0.6), xy_pad(commands, 0.5, 0.5)];
    let pad_row = hstack(commands, 16.0, &pads);
    let r_pads = property_row(commands, font, "Fader / XY", pad_row);

    panel_column(
        commands,
        font,
        "Inspector",
        vec![r_pos, r_scale, r_color, r_knob, r_gauge, r_pads],
    )
}

/// Gallery panel: containers.
pub fn gallery_containers(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let c = card(commands, font, "Card title", "A themed card container.");
    let d = divider(commands);
    let acc1_body = paragraph(commands, font, "Section one content.");
    let acc1 = accordion_section(commands, fonts, "Section one", acc1_body, true);
    let acc2_body = paragraph(commands, font, "Section two content.");
    let acc2 = accordion_section(commands, fonts, "Section two", acc2_body, false);
    let p1 = paragraph(commands, font, "Tab one panel.");
    let p2 = paragraph(commands, font, "Tab two panel.");
    let p3 = paragraph(commands, font, "Tab three panel.");
    let tb = tabs(commands, font, &["One", "Two", "Three"], vec![p1, p2, p3]);
    panel_column(commands, font, "Containers", vec![c, d, acc1, acc2, tb])
}

/// Gallery panel: navigation.
pub fn gallery_nav(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let bc = breadcrumb(commands, fonts, &["Home", "Scene", "Mesh"]);
    let pg = pagination(commands, font, 5, 0);
    let nb = navbar(commands, fonts, "Renzora", &["File", "Edit", "View"]);
    let lg = list_group(commands, font, &["Item A", "Item B", "Item C"], 0);
    panel_column(commands, font, "Navigation", vec![bc, pg, nb, lg])
}

/// Gallery panel: data display.
pub fn gallery_data(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let rows: [&[&str]; 3] = [
        &["Mesh.001", "Mesh"],
        &["Light", "Point"],
        &["Camera", "Camera"],
    ];
    let tbl = table(commands, font, &["Name", "Type"], &rows);
    let gr = grid(commands, font, 6, 3);
    let leaf1 = tree_node(commands, fonts, "child.png", 1, vec![], false);
    let leaf2 = tree_node(commands, fonts, "data.json", 1, vec![], false);
    let tree = tree_node(commands, fonts, "assets", 0, vec![leaf1, leaf2], true);
    let chips = [
        chip(commands, fonts, "tag1"),
        chip(commands, fonts, "tag2"),
        chip(commands, fonts, "tag3"),
    ];
    let chip_row = hstack(commands, 6.0, &chips);
    let av = avatar(commands, font, "RZ", ACCENT_BLUE);
    let f_av = field(commands, font, "Avatar", av);
    panel_column(commands, font, "Data", vec![tbl, gr, tree, chip_row, f_av])
}

/// Gallery panel: extended form controls.
pub fn gallery_forms(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let ta = textarea(commands, font, "Multi-line text…", "");
    let f_ta = field(commands, font, "Textarea", ta);
    let ig = input_group(commands, font, "https://", "example.com");
    let f_ig = field(commands, font, "Input group", ig);
    let fl = floating_label(commands, font, "Email", "user@host");
    let val = validation(commands, fonts, Tone::Error, "bad value", "This field is required.");
    let rg = range(commands, 0.3, 0.7);
    let f_rg = field(commands, font, "Range", rg);
    panel_column(commands, font, "Forms", vec![f_ta, f_ig, fl, val, f_rg])
}

/// Gallery panel: overlays.
pub fn gallery_overlays(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let tip_target = button(commands, font, "Hover me");
    let tip = tooltip(commands, font, "A tooltip!", tip_target);
    let f_tip = field(commands, font, "Tooltip", tip);
    let pop_content = paragraph(commands, font, "Popover content.");
    let pop = popover(commands, fonts, "Open popover", pop_content);
    let f_pop = field(commands, font, "Popover", pop);
    let md = modal(commands, fonts, "Dialog title", "Modal body text goes here.");
    panel_column(commands, font, "Overlays", vec![f_tip, f_pop, md])
}

/// Gallery panel: menus (hamburger + context menu with submenu).
pub fn gallery_menus(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let hb = hamburger(commands, fonts, &["New", "Open", "Save", "Quit"]);
    let f_hb = field(commands, font, "Hamburger", hb);

    let target = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((30, 30, 38))),
            BorderColor::all(rgb((60, 60, 74))),
            Name::new("ctx-target"),
        ))
        .id();
    let hint = paragraph(commands, font, "Right-click me");
    commands.entity(target).add_child(hint);
    let cm = context_menu(
        commands,
        fonts,
        target,
        &[
            ("Cut", &[][..]),
            ("Copy", &[][..]),
            ("Paste", &[][..]),
            ("More", &["Duplicate", "Rename", "Delete"][..]),
        ],
    );
    let f_cm = field(commands, font, "Context", cm);

    panel_column(commands, font, "Menus", vec![f_hb, f_cm])
}

/// Gallery panel: rich text, spinner, multi-select, sortable, scroll area.
pub fn gallery_extras(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let rt = rich_text(
        commands,
        font,
        &[
            ("Colored ", TEXT_PRIMARY),
            ("rich ", ACCENT_BLUE),
            ("text", PLAY_GREEN),
        ],
    );
    let f_rt = field(commands, font, "Rich text", rt);

    let sp = spinner(commands);
    let f_sp = field(commands, font, "Spinner", sp);

    let ms = multi_select(commands, font, &["Alpha", "Beta", "Gamma"], &[true, false, true]);
    let f_ms = field(commands, font, "Multi-select", ms);

    let sl = sortable_list(commands, fonts, &["Drag me", "Reorder", "Sortable", "Rows"]);
    let f_sl = field(commands, font, "Sortable", sl);

    let lines: Vec<Entity> = (0..12)
        .map(|i| paragraph(commands, font, &format!("Scrollable row {}", i + 1)))
        .collect();
    let tall = vstack(commands, 4.0, &lines);
    let sa = scroll_area(commands, tall, 90.0);
    let f_sa = field(commands, font, "Scroll area", sa);

    panel_column(commands, font, "More", vec![f_rt, f_sp, f_ms, f_sl, f_sa])
}

/// Gallery panel: the node graph editor.
pub fn gallery_node_graph(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let ng = node_graph(commands, fonts);
    panel_column(commands, &fonts.ui, "Node Graph", vec![ng])
}

/// Gallery panel: the code editor.
pub fn gallery_code(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let sample = r#"// ember code editor — click to focus, then type
fn main() {
    let mut count = 0;
    for i in 0..10 {
        count += i * 2;
    }
    let name: String = "renzora".into();
}

struct Widget {
    id: u32,
    label: String,
}"#;
    let ed = code_editor(commands, sample);
    panel_column(commands, font, "Code Editor", vec![ed])
}

/// Gallery panel: the timeline.
pub fn gallery_timeline(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let video = [(0.5f32, 3.0, "Intro"), (4.0, 5.0, "Scene A"), (9.5, 2.0, "Outro")];
    let audio = [(0.0f32, 8.0, "Music"), (8.5, 3.2, "VO")];
    let keys = [1.0f32, 2.5, 4.0, 6.0, 8.0, 10.5];
    let tracks = [
        Track {
            name: "Video",
            color: (90, 140, 230),
            lane: Lane::Clips(&video),
        },
        Track {
            name: "Audio",
            color: (90, 191, 115),
            lane: Lane::Clips(&audio),
        },
        Track {
            name: "Anim",
            color: (224, 170, 72),
            lane: Lane::Keys(&keys),
        },
    ];
    let tl = timeline(commands, fonts, 12.0, &tracks);
    panel_column(commands, font, "Timeline", vec![tl])
}

/// Gallery panel: charts.
pub fn gallery_charts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let series = [
        0.2, 0.5, 0.35, 0.8, 0.55, 0.9, 0.45, 0.7, 0.5, 0.85, 0.6, 0.95, 0.4, 0.75,
    ];
    let lc = line_chart(commands, fonts, &series);
    let f_lc = field(commands, font, "Line", lc);

    let bc = bar_chart(commands, &[0.3, 0.6, 0.45, 0.8, 0.5, 0.7, 0.9, 0.4, 0.65]);
    let f_bc = field(commands, font, "Bars", bc);

    let sp = sparkline(commands, &series);
    let f_sp = field(commands, font, "Sparkline", sp);

    panel_column(commands, font, "Charts", vec![f_lc, f_bc, f_sp])
}

/// Gallery panel: pickers & drag-and-drop inspector controls.
pub fn gallery_pickers(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;

    let hsv = hsv_picker(commands, 0.58, 0.7, 0.9);
    let r_hsv = property_row(commands, font, "HSV", hsv);

    let spin = spin_slider(commands, font, "Mass", 1.0, 0.0, 10.0);
    let r_spin = property_row(commands, font, "Spin slider", spin);

    let tags = tags_input(commands, fonts, &["player", "solid", "spawn"]);
    let r_tags = property_row(commands, font, "Tags", tags);

    let files = [
        draggable_file(commands, fonts, "albedo.png"),
        draggable_file(commands, fonts, "mesh.glb"),
    ];
    let file_row = hstack(commands, 8.0, &files);
    let r_files = property_row(commands, font, "Drag from", file_row);

    let slot = asset_slot(commands, fonts, "");
    let r_slot = property_row(commands, font, "Drop onto", slot);

    let v2 = vec2_edit(commands, font, 0.5, 0.5);
    let r_v2 = property_row(commands, font, "Vec2", v2);
    let v4 = vec4_edit(commands, font, 0.0, 0.0, 0.0, 1.0);
    let r_v4 = property_row(commands, font, "Quat", v4);

    panel_column(
        commands,
        font,
        "Pickers & DnD",
        vec![r_hsv, r_spin, r_tags, r_files, r_slot, r_v2, r_v4],
    )
}

/// Gallery panel: animation editors (easing curve + gradient).
pub fn gallery_animation(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;

    let curve = curve_editor(commands, Vec2::new(0.25, 0.1), Vec2::new(0.75, 0.9));
    let f_curve = field(commands, font, "Easing curve", curve);

    let grad = gradient_editor(
        commands,
        &[
            (0.0, (40, 60, 200)),
            (0.45, (200, 60, 160)),
            (1.0, (250, 220, 80)),
        ],
    );
    let f_grad = field(commands, font, "Gradient", grad);

    panel_column(commands, font, "Animation", vec![f_curve, f_grad])
}

/// Gallery panel: audio (waveform, VU meter, mixer strips).
pub fn gallery_audio(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;

    let amps = [
        0.1, 0.35, 0.6, 0.45, 0.8, 0.55, 0.9, 0.4, 0.7, 0.3, 0.65, 0.5, 0.85, 0.45, 0.6, 0.25,
    ];
    let wf = waveform(commands, &amps);
    let f_wf = field(commands, font, "Waveform", wf);

    let meters = [
        vu_meter(commands),
        vu_meter(commands),
        vu_meter(commands),
    ];
    let meter_row = hstack(commands, 8.0, &meters);
    let f_vu = field(commands, font, "VU meters", meter_row);

    let strips = [
        mixer_strip(commands, fonts, "Master", 0.8),
        mixer_strip(commands, fonts, "Music", 0.6),
        mixer_strip(commands, fonts, "SFX", 0.7),
    ];
    let mix_row = hstack(commands, 10.0, &strips);
    let f_mix = field(commands, font, "Mixer", mix_row);

    panel_column(commands, font, "Audio", vec![f_wf, f_vu, f_mix])
}

/// Gallery panel: color swatches.
pub fn gallery_colors(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let chips = [
        swatch(commands, ACCENT_BLUE),
        swatch(commands, PLAY_GREEN),
        swatch(commands, CLOSE_RED),
        swatch(commands, TAB_HOVER_BG),
        swatch(commands, HEADER_BG),
    ];
    let swatches = hstack(commands, 8.0, &chips);
    let f_swatch = field(commands, font, "Swatches", swatches);
    panel_column(commands, font, "Colors", vec![f_swatch])
}
