//! Default theme implementations

use super::*;

impl Theme {
    /// Create the default dark theme (slate neutrals, one blue accent). Its
    /// colors are the `Default` impls in `lib.rs`, mirrored by ember's fallback
    /// palette constants.
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
            syntax: SyntaxColors::default(),
            effects: ThemeEffects::default(),
            fonts: ThemeFonts::default(),
            images: ThemeImages::default(),
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
            // Slate light, the dark theme's counterpart. The previous version
            // kept every surface between 226 and 254, so panels, headers and
            // inputs had no edges and the whole editor read as one white sheet.
            // Here the chrome is a clearly darker slate framing near-white
            // panels, inputs are white with a border you can see, and text is
            // dark enough that muted labels still pass contrast.
            semantic: SemanticColors {
                accent: ThemeColor::new(44, 104, 230),
                success: ThemeColor::new(26, 127, 60),
                warning: ThemeColor::new(180, 110, 0),
                error: ThemeColor::new(200, 50, 50),
                selection: ThemeColor::new(212, 225, 252),
                selection_stroke: ThemeColor::new(44, 104, 230),
            },
            surfaces: SurfaceColors {
                window: ThemeColor::new(221, 225, 232),
                window_stroke: ThemeColor::new(190, 197, 209),
                panel: ThemeColor::new(246, 247, 249),
                popup: ThemeColor::new(255, 255, 255),
                overlay: ThemeColor::with_alpha(16, 20, 28, 90),
                faint: ThemeColor::new(234, 237, 241),
                // ember maps the panel tab headers to `extreme`: between the
                // chrome and the panel, so a header reads as part of its panel.
                extreme: ThemeColor::new(232, 235, 240),
            },
            text: TextColors {
                primary: ThemeColor::new(22, 26, 33),
                secondary: ThemeColor::new(52, 58, 70),
                muted: ThemeColor::new(88, 96, 112),
                heading: ThemeColor::new(16, 19, 25),
                disabled: ThemeColor::new(148, 155, 168),
                hyperlink: ThemeColor::new(44, 104, 230),
            },
            widgets: WidgetColors {
                noninteractive_bg: ThemeColor::new(238, 240, 244),
                noninteractive_fg: ThemeColor::new(60, 66, 78),
                inactive_bg: ThemeColor::new(255, 255, 255),
                inactive_fg: ThemeColor::new(40, 46, 58),
                hovered_bg: ThemeColor::new(226, 231, 240),
                hovered_fg: ThemeColor::new(22, 26, 33),
                active_bg: ThemeColor::new(44, 104, 230),
                active_fg: ThemeColor::new(255, 255, 255),
                border: ThemeColor::new(186, 192, 204),
                border_light: ThemeColor::new(204, 210, 220),
            },
            panels: PanelColors {
                tree_line: ThemeColor::new(196, 202, 212),
                drop_line: ThemeColor::new(44, 104, 230),
                drop_child_highlight: ThemeColor::with_alpha(44, 104, 230, 46),
                row_odd_bg: ThemeColor::with_alpha(0, 0, 0, 8),
                inspector_row_even: ThemeColor::new(246, 247, 249),
                inspector_row_odd: ThemeColor::new(240, 242, 246),
                category_frame_bg: ThemeColor::new(236, 239, 244),
                item_bg: ThemeColor::new(255, 255, 255),
                item_hover: ThemeColor::new(226, 232, 242),
                tab_active: ThemeColor::new(255, 255, 255),
                tab_inactive: ThemeColor::new(232, 235, 240),
                tab_hover: ThemeColor::new(220, 225, 233),
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
                node_selected_border: ThemeColor::new(44, 104, 230),
                connection: ThemeColor::new(80, 86, 100),
                connection_preview: ThemeColor::new(200, 180, 60),
                selection_rect_fill: ThemeColor::with_alpha(44, 104, 230, 30),
                selection_rect_stroke: ThemeColor::new(44, 104, 230),
            },
            viewport: ViewportColors {
                grid_line: ThemeColor::new(190, 190, 200),
                gizmo_x: ThemeColor::new(220, 60, 60),
                gizmo_y: ThemeColor::new(60, 200, 60),
                gizmo_z: ThemeColor::new(60, 60, 220),
                gizmo_selected: ThemeColor::new(220, 200, 0),
            },
            // A readable light code theme: near-black text on the light panel,
            // saturated-but-dark token hues so they stay legible on a pale
            // background (the dark theme's pastel tokens would wash out).
            syntax: SyntaxColors {
                normal: ThemeColor::new(36, 41, 51),
                keyword: ThemeColor::new(167, 41, 130),
                r#type: ThemeColor::new(141, 98, 26),
                function: ThemeColor::new(54, 96, 191),
                number: ThemeColor::new(155, 66, 46),
                string: ThemeColor::new(48, 132, 70),
                comment: ThemeColor::new(140, 146, 156),
                operator: ThemeColor::new(72, 78, 88),
                constant: ThemeColor::new(155, 66, 46),
                punctuation: ThemeColor::new(92, 98, 108),
                line_number: ThemeColor::new(168, 172, 180),
                line_number_active: ThemeColor::new(60, 64, 72),
                current_line: ThemeColor::with_alpha(0, 0, 0, 12),
                selection: ThemeColor::with_alpha(44, 104, 230, 56),
                cursor: ThemeColor::new(44, 104, 230),
                indent_guide: ThemeColor::with_alpha(0, 0, 0, 18),
                bracket_match: ThemeColor::with_alpha(44, 104, 230, 72),
                find_match: ThemeColor::with_alpha(220, 170, 40, 110),
            },
            effects: ThemeEffects::default(),
            fonts: ThemeFonts::default(),
            images: ThemeImages::default(),
        }
    }
}
