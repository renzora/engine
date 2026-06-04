//! Default theme implementations

use super::*;

impl Theme {
    /// Create the default dark theme
    /// All colors here match the original hardcoded values in the editor
    pub fn dark() -> Self {
        Self {
            meta: ThemeMeta {
                name: "Dark".to_string(),
                author: "Renzora".to_string(),
                version: "1.0".to_string(),
            },
            semantic: SemanticColors::default(),
            surfaces: SurfaceColors::default(),
            text: TextColors::default(),
            widgets: WidgetColors::default(),
            panels: PanelColors::default(),
            categories: CategoryColors::default(),
            material: MaterialColors::default(),
            viewport: ViewportColors::default(),
        }
    }

    /// Create a light theme variant
    pub fn light() -> Self {
        Self {
            meta: ThemeMeta {
                name: "Light".to_string(),
                author: "Renzora".to_string(),
                version: "1.0".to_string(),
            },
            // A clean, professional light theme. Surfaces layer in soft cool
            // grays (app < panels < popups) instead of flat white, so panels,
            // headers and cards read as distinct planes; text keeps a strong
            // near-black contrast for readability.
            semantic: SemanticColors {
                accent: ThemeColor::new(38, 108, 200),
                success: ThemeColor::new(48, 148, 78),
                warning: ThemeColor::new(196, 132, 28),
                error: ThemeColor::new(200, 58, 58),
                selection: ThemeColor::new(208, 223, 245),
                selection_stroke: ThemeColor::new(38, 108, 200),
            },
            surfaces: SurfaceColors {
                window: ThemeColor::new(234, 236, 240),
                window_stroke: ThemeColor::new(206, 209, 215),
                panel: ThemeColor::new(244, 245, 248),
                popup: ThemeColor::new(252, 253, 254),
                overlay: ThemeColor::with_alpha(0, 0, 0, 90),
                faint: ThemeColor::new(228, 230, 234),
                // ember maps the chrome header bar to `extreme`; keep it a subtle
                // gray (not pure white) so toolbars read as their own surface.
                extreme: ThemeColor::new(226, 229, 234),
            },
            text: TextColors {
                primary: ThemeColor::new(26, 28, 32),
                secondary: ThemeColor::new(62, 66, 74),
                muted: ThemeColor::new(108, 114, 124),
                heading: ThemeColor::new(16, 18, 22),
                disabled: ThemeColor::new(158, 162, 170),
                hyperlink: ThemeColor::new(38, 108, 200),
            },
            widgets: WidgetColors {
                noninteractive_bg: ThemeColor::new(238, 240, 243),
                noninteractive_fg: ThemeColor::new(70, 74, 82),
                inactive_bg: ThemeColor::new(226, 229, 234),
                inactive_fg: ThemeColor::new(52, 56, 64),
                hovered_bg: ThemeColor::new(214, 219, 227),
                hovered_fg: ThemeColor::new(26, 28, 32),
                active_bg: ThemeColor::new(38, 108, 200),
                active_fg: ThemeColor::new(255, 255, 255),
                border: ThemeColor::new(200, 204, 210),
                border_light: ThemeColor::new(216, 219, 225),
            },
            panels: PanelColors {
                tree_line: ThemeColor::new(196, 200, 207),
                drop_line: ThemeColor::new(38, 108, 200),
                drop_child_highlight: ThemeColor::with_alpha(38, 108, 200, 46),
                row_odd_bg: ThemeColor::with_alpha(0, 0, 0, 8),
                inspector_row_even: ThemeColor::new(244, 245, 248),
                inspector_row_odd: ThemeColor::new(238, 240, 243),
                category_frame_bg: ThemeColor::new(240, 242, 245),
                item_bg: ThemeColor::new(236, 238, 242),
                item_hover: ThemeColor::new(224, 230, 240),
                tab_active: ThemeColor::new(252, 253, 254),
                tab_inactive: ThemeColor::new(232, 234, 238),
                tab_hover: ThemeColor::new(224, 227, 233),
                close_hover: ThemeColor::new(208, 66, 66),
            },
            categories: CategoryColors {
                transform: CategoryStyle {
                    accent: ThemeColor::new(60, 140, 200),
                    header_bg: ThemeColor::new(230, 240, 250),
                },
                environment: CategoryStyle {
                    accent: ThemeColor::new(70, 150, 90),
                    header_bg: ThemeColor::new(230, 245, 235),
                },
                lighting: CategoryStyle {
                    accent: ThemeColor::new(200, 165, 60),
                    header_bg: ThemeColor::new(250, 245, 230),
                },
                camera: CategoryStyle {
                    accent: ThemeColor::new(140, 100, 180),
                    header_bg: ThemeColor::new(242, 235, 250),
                },
                scripting: CategoryStyle {
                    accent: ThemeColor::new(200, 120, 80),
                    header_bg: ThemeColor::new(250, 240, 235),
                },
                physics: CategoryStyle {
                    accent: ThemeColor::new(80, 160, 160),
                    header_bg: ThemeColor::new(230, 245, 245),
                },
                rendering: CategoryStyle {
                    accent: ThemeColor::new(60, 140, 200),
                    header_bg: ThemeColor::new(230, 240, 250),
                },
                audio: CategoryStyle {
                    accent: ThemeColor::new(70, 145, 70),
                    header_bg: ThemeColor::new(230, 245, 235),
                },
                ui: CategoryStyle {
                    accent: ThemeColor::new(150, 130, 200),
                    header_bg: ThemeColor::new(242, 238, 250),
                },
                effects: CategoryStyle {
                    accent: ThemeColor::new(210, 140, 180),
                    header_bg: ThemeColor::new(250, 235, 242),
                },
                post_process: CategoryStyle {
                    accent: ThemeColor::new(90, 170, 130),
                    header_bg: ThemeColor::new(230, 248, 240),
                },
                gameplay: CategoryStyle {
                    accent: ThemeColor::new(210, 110, 110),
                    header_bg: ThemeColor::new(250, 235, 235),
                },
                nodes_2d: CategoryStyle {
                    accent: ThemeColor::new(200, 100, 150),
                    header_bg: ThemeColor::new(250, 235, 242),
                },
                plugin: CategoryStyle {
                    accent: ThemeColor::new(140, 110, 140),
                    header_bg: ThemeColor::new(245, 238, 245),
                },
            },
            material: MaterialColors {
                canvas_bg: ThemeColor::new(240, 240, 245),
                grid_dot: ThemeColor::new(200, 200, 210),
                node_bg: ThemeColor::new(255, 255, 255),
                node_border: ThemeColor::new(200, 200, 210),
                node_selected_border: ThemeColor::new(45, 120, 210),
                connection: ThemeColor::new(80, 80, 90),
                connection_preview: ThemeColor::new(200, 180, 60),
                selection_rect_fill: ThemeColor::with_alpha(45, 120, 210, 30),
                selection_rect_stroke: ThemeColor::new(45, 120, 210),
            },
            viewport: ViewportColors {
                grid_line: ThemeColor::new(190, 190, 200),
                gizmo_x: ThemeColor::new(220, 60, 60),
                gizmo_y: ThemeColor::new(60, 200, 60),
                gizmo_z: ThemeColor::new(60, 60, 220),
                gizmo_selected: ThemeColor::new(220, 200, 0),
            },
        }
    }
}
