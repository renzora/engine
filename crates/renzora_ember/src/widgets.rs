//! `renzora_ember` UI components — a reusable bevy_ui widget set used by the
//! editor and games. Each widget lives in its own submodule (a builder fn +
//! the systems that animate its interaction state). [`WidgetsPlugin`] registers
//! every interaction system; the `gallery_*` panels showcase the whole set.

use bevy::prelude::*;

// Shared helpers.
mod common;
mod tone;

// Form controls.
mod button;
mod checkbox;
mod dropdown;
mod font_picker;
mod radio;
mod segmented;
mod slider;
mod stepper;
mod popup;
mod text_input;
mod toggle;
mod toggle_switch;

// Inspector value editors.
mod asset_slot;
mod asset_tile;
mod color_picker;
mod colorpicker;
mod drag_value;
mod fader;
mod gauge;
mod knob;
mod property_row;
mod spin_slider;
mod tags_input;
mod vec3_edit;
mod xy_pad;

// Animation editors.
mod curve;
mod gradient;

// Audio.
mod mixer;
mod vu_meter;
mod waveform;

// Typography.
mod typography;

// Feedback.
mod alert;
mod badge;
mod progress;
mod skeleton;
mod toast;

// Containers.
mod accordion;
mod card;
mod divider;
mod tabs;

// Navigation.
mod breadcrumb;
mod list_group;
mod navbar;
mod pagination;

// Data display.
mod avatar;
mod chip;
mod grid;
mod image;
mod table;
mod tree;

// Forms (extra).
mod floating_label;
mod input_group;
mod range;
mod textarea;
mod validation;

// Overlays.
mod modal;
mod popover;
mod tooltip;

// Menus / interaction / utilities.
mod chart;
mod code_editor;
mod collapsible;
mod context_menu;
mod hamburger;
mod menu;
mod multi_select;
mod node_graph;
mod overlay;
mod markdown;
mod rich_text;
mod scroll_area;
mod section;
mod search;
mod sortable;
mod spinner;
mod timeline;
mod timeline_view;

// The gallery showcase panels.
mod gallery;

pub use common::*;
pub use tone::*;

pub use button::*;
pub use checkbox::*;
pub use dropdown::*;
pub use font_picker::*;
pub use radio::*;
pub use segmented::*;
pub use slider::*;
pub use stepper::*;
pub use text_input::*;
pub use popup::*;
pub use toggle::*;
pub use toggle_switch::*;

pub use asset_slot::*;
pub use asset_tile::asset_tile;
pub use color_picker::*;
pub use colorpicker::*;
pub use drag_value::*;
pub use fader::*;
pub use gauge::*;
pub use knob::*;
pub use property_row::*;
pub use spin_slider::*;
pub use tags_input::*;
pub use vec3_edit::*;
pub use xy_pad::*;

pub use curve::*;
pub use gradient::*;

pub use mixer::*;
pub use vu_meter::*;
pub use waveform::*;

pub use typography::*;

pub use alert::*;
pub use badge::*;
pub use progress::*;
pub use skeleton::*;
pub use toast::*;

pub use accordion::*;
pub use card::*;
pub use divider::*;
pub use tabs::*;

pub use breadcrumb::*;
pub use list_group::*;
pub use navbar::*;
pub use pagination::*;

pub use avatar::*;
pub use chip::*;
pub use grid::*;
pub use image::*;
pub use table::*;
pub use tree::*;

pub use floating_label::*;
pub use input_group::*;
pub use range::*;
pub use textarea::*;
pub use validation::*;

pub use modal::*;
pub use popover::*;
pub use tooltip::*;

pub use chart::*;
pub use collapsible::collapsible;
pub use code_editor::*;
pub use context_menu::*;
pub use hamburger::*;
pub use multi_select::*;
pub use node_graph::*;
pub use markdown::*;
pub use rich_text::*;
pub use overlay::*;
pub use scroll_area::*;
pub use section::{
    section, section_with_header, section_with_header_open, set_section_open, Section,
};
pub use search::*;
pub use sortable::*;
pub use spinner::*;
pub use timeline::*;
pub use timeline_view::{timeline_view, TimelineHandle, TimelineView, LANE_INSET};

pub use gallery::*;

/// Registers every widget interaction system.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<popup::PointerOverOverlay>();
        app.init_resource::<drag_value::WheelOverDragValue>();
        app.init_resource::<drag_value::WheelGesture>();
        app.init_resource::<scroll_area::ScrollMemory>();
        app.add_systems(
            Update,
            (
                (
                    button::button_interact,
                    toggle::toggle_interact,
                    slider::slider_drag,
                    slider::slider_apply,
                    checkbox::checkbox_interact,
                    checkbox::checkbox_apply,
                    radio::radio_interact,
                    segmented::segmented_interact,
                    stepper::stepper_interact,
                    dropdown::dropdown_toggle,
                    dropdown::dropdown_select,
                    dropdown::dropdown_apply,
                    dropdown::dropdown_dismiss,
                    dropdown::dropdown_option_hover,
                    text_input::text_input_focus,
                    text_input::text_input_type,
                ),
                (
                    drag_value::drag_value_drag,
                    drag_value::drag_value_edit,
                    drag_value::drag_value_scroll.before(scroll_area::scroll_wheel),
                    drag_value::drag_value_apply,
                    toggle_switch::switch_interact,
                    toggle_switch::switch_apply,
                    popup::popup_toggle,
                    popup::popup_dismiss,
                    popup::popup_position,
                    color_picker::color_picker_sync,
                    knob::knob_drag,
                    fader::fader_drag,
                    fader::fader_apply,
                    xy_pad::xy_pad_drag,
                    accordion::accordion_toggle,
                    tabs::tab_select,
                    pagination::pagination_select,
                    list_group::list_select,
                    chip::chip_close,
                    tree::tree_toggle,
                ),
                (
                    range::range_drag,
                    tooltip::tooltip_hover,
                    popover::popover_toggle,
                    modal::modal_toggle,
                    popup::screen_menu_clamp,
                    popup::menu_action_run,
                    popup::screen_menu_dismiss,
                    popup::update_pointer_over_overlay,
                    popup::tag_popup_panels,
                    overlay::overlay_dismiss,
                    search::search_list_focus,
                    search::search_list_visibility,
                    search::search_list_select,
                    search::search_cat_toggle,
                    search::search_sidebar_jump,
                    search::search_list_enter,
                ),
                (
                    spinner::spinner_anim,
                    scroll_area::scroll_wheel,
                    scroll_area::scroll_restore.before(scroll_area::scroll_update),
                    scroll_area::scroll_update,
                    scroll_area::scroll_persist.after(scroll_area::scroll_update),
                    scroll_area::scroll_thumb_drag,
                    multi_select::multi_select_toggle,
                    menu::menu_hover,
                    menu::submenu_hover,
                    menu::menu_item_close,
                    hamburger::hamburger_toggle,
                    context_menu::context_menu_open,
                    sortable::sortable_drag,
                    text_input::caret_blink,
                    text_input::text_input_highlight,
                ),
                (
                    spin_slider::spin_drag,
                    tags_input::tags_commit,
                    vu_meter::vu_animate,
                    mixer::mixer_toggle,
                    mixer::mixer_button_apply,
                    knob::knob_apply,
                ),
            ),
        );
        app.add_plugins(node_graph::NodeGraphPlugin);
        app.add_plugins(collapsible::CollapsiblePlugin);
        app.add_plugins(section::SectionPlugin);
        app.add_plugins(timeline::TimelinePlugin);
        app.add_plugins(timeline_view::TimelineViewPlugin);
        app.add_plugins(code_editor::CodeEditorPlugin);
        app.add_plugins(asset_slot::DndPlugin);
        app.add_plugins(colorpicker::ColorPickerPlugin);
        app.add_plugins(curve::CurveEditorPlugin);
        app.add_plugins(gradient::GradientEditorPlugin);
        // gauge / chart / waveform are ALSO added by the markup runtime
        // (`markup::vector::plugin`, via `MarkupPlugin`), which runs first.
        // bevy panics on a duplicate plugin TYPE at the `add_plugins` call
        // (before `build()`), so guard each here — whichever path runs first
        // installs it, the other skips.
        if !app.is_plugin_added::<gauge::GaugePlugin>() {
            app.add_plugins(gauge::GaugePlugin);
        }
        if !app.is_plugin_added::<chart::ChartPlugin>() {
            app.add_plugins(chart::ChartPlugin);
        }
        if !app.is_plugin_added::<waveform::WaveformPlugin>() {
            app.add_plugins(waveform::WaveformPlugin);
        }
    }
}
