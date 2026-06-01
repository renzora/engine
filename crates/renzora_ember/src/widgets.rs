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
mod radio;
mod segmented;
mod slider;
mod stepper;
mod text_input;
mod toggle;

// Inspector value editors.
mod color_picker;
mod drag_value;
mod fader;
mod gauge;
mod knob;
mod property_row;
mod vec3_edit;
mod xy_pad;

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
mod context_menu;
mod hamburger;
mod menu;
mod multi_select;
mod node_graph;
mod rich_text;
mod scroll_area;
mod sortable;
mod spinner;
mod timeline;

// The gallery showcase panels.
mod gallery;

pub use common::*;
pub use tone::*;

pub use button::*;
pub use checkbox::*;
pub use dropdown::*;
pub use radio::*;
pub use segmented::*;
pub use slider::*;
pub use stepper::*;
pub use text_input::*;
pub use toggle::*;

pub use color_picker::*;
pub use drag_value::*;
pub use fader::*;
pub use gauge::*;
pub use knob::*;
pub use property_row::*;
pub use vec3_edit::*;
pub use xy_pad::*;

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
pub use context_menu::*;
pub use hamburger::*;
pub use multi_select::*;
pub use node_graph::*;
pub use rich_text::*;
pub use scroll_area::*;
pub use sortable::*;
pub use spinner::*;
pub use timeline::*;

pub use gallery::*;

/// Registers every widget interaction system.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                (
                    button::button_interact,
                    toggle::toggle_interact,
                    slider::slider_drag,
                    checkbox::checkbox_interact,
                    radio::radio_interact,
                    segmented::segmented_interact,
                    stepper::stepper_interact,
                    dropdown::dropdown_toggle,
                    dropdown::dropdown_select,
                    dropdown::dropdown_option_hover,
                    text_input::text_input_focus,
                    text_input::text_input_type,
                ),
                (
                    drag_value::drag_value_drag,
                    color_picker::color_picker_sync,
                    knob::knob_drag,
                    fader::fader_drag,
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
                ),
                (
                    spinner::spinner_anim,
                    scroll_area::scroll_drive,
                    multi_select::multi_select_toggle,
                    menu::menu_hover,
                    menu::submenu_hover,
                    menu::menu_item_close,
                    hamburger::hamburger_toggle,
                    context_menu::context_menu_open,
                    sortable::sortable_drag,
                    text_input::caret_blink,
                ),
            ),
        );
        app.add_plugins(node_graph::NodeGraphPlugin);
        app.add_plugins(chart::ChartPlugin);
        app.add_plugins(timeline::TimelinePlugin);
        app.add_plugins(gauge::GaugePlugin);
    }
}
