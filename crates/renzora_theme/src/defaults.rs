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
            blueprint: BlueprintColors::default(),
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
            semantic: SemanticColors {
                accent: ThemeColor::new(45, 120, 210),
                success: ThemeColor::new(60, 160, 85),
                warning: ThemeColor::new(210, 140, 40),
                error: ThemeColor::new(200, 60, 60),
                selection: ThemeColor::new(45, 120, 210),
                selection_stroke: ThemeColor::new(70, 150, 230),
            },
            surfaces: SurfaceColors {
                window: ThemeColor::new(245, 245, 248),
                window_stroke: ThemeColor::new(200, 200, 210),
                panel: ThemeColor::new(250, 250, 252),
                popup: ThemeColor::new(255, 255, 255),
                overlay: ThemeColor::with_alpha(0, 0, 0, 120),
                faint: ThemeColor::new(240, 240, 245),
                extreme: ThemeColor::new(255, 255, 255),
            },
            text: TextColors {
                primary: ThemeColor::new(30, 30, 35),
                secondary: ThemeColor::new(60, 60, 70),
                muted: ThemeColor::new(120, 120, 135),
                heading: ThemeColor::new(50, 50, 60),
                disabled: ThemeColor::new(160, 160, 175),
                hyperlink: ThemeColor::new(45, 120, 210),
            },
            widgets: WidgetColors {
                noninteractive_bg: ThemeColor::new(235, 235, 240),
                noninteractive_fg: ThemeColor::new(80, 80, 90),
                inactive_bg: ThemeColor::new(225, 225, 235),
                inactive_fg: ThemeColor::new(60, 60, 70),
                hovered_bg: ThemeColor::new(215, 215, 230),
                hovered_fg: ThemeColor::new(40, 40, 50),
                active_bg: ThemeColor::new(45, 120, 210),
                active_fg: ThemeColor::new(255, 255, 255),
                border: ThemeColor::new(200, 200, 210),
                border_light: ThemeColor::new(220, 220, 230),
            },
            panels: PanelColors {
                tree_line: ThemeColor::new(190, 190, 200),
                drop_line: ThemeColor::new(45, 120, 210),
                drop_child_highlight: ThemeColor::with_alpha(45, 120, 210, 50),
                row_odd_bg: ThemeColor::with_alpha(0, 0, 0, 10),
                inspector_row_even: ThemeColor::new(250, 250, 252),
                inspector_row_odd: ThemeColor::new(245, 245, 248),
                category_frame_bg: ThemeColor::new(250, 250, 252),
                item_bg: ThemeColor::new(240, 240, 245),
                item_hover: ThemeColor::new(230, 235, 245),
                tab_active: ThemeColor::new(255, 255, 255),
                tab_inactive: ThemeColor::new(245, 245, 248),
                tab_hover: ThemeColor::new(235, 235, 245),
                close_hover: ThemeColor::new(220, 80, 80),
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
            blueprint: BlueprintColors {
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
