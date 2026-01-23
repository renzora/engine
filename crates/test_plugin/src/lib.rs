//! Test Plugin for the Bevy Editor
//!
//! This plugin demonstrates the plugin API by creating panels, menus,
//! and responding to user interactions.

use editor_plugin_api::prelude::*;

/// A test plugin that demonstrates the plugin API
pub struct TestPlugin {
    /// Counter for button clicks
    click_count: u32,
    /// Current slider value
    slider_value: f32,
    /// Checkbox state
    checkbox_checked: bool,
    /// Selected entity name
    selected_entity_name: Option<String>,
    /// Selected entity ID for manipulation
    selected_entity_id: Option<EntityId>,
    /// Number of entities spawned by this plugin
    spawned_count: u32,
}

impl TestPlugin {
    pub fn new() -> Self {
        Self {
            click_count: 0,
            slider_value: 0.5,
            checkbox_checked: false,
            selected_entity_name: None,
            selected_entity_id: None,
            spawned_count: 0,
        }
    }
}

impl Default for TestPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorPlugin for TestPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("com.bevy-editor.test-plugin", "Test Plugin", "0.1.0")
            .author("Bevy Editor Team")
            .description("A test plugin demonstrating the editor plugin API")
            .capability(PluginCapability::Panel)
            .capability(PluginCapability::MenuItem)
            .capability(PluginCapability::Inspector)
    }

    fn on_load(&mut self, api: &mut dyn EditorApi) -> Result<(), PluginError> {
        api.log_info("Test Plugin loaded!");

        // Register a floating panel
        api.register_panel(
            PanelDefinition::new("test_panel", "Test Plugin")
                .icon("🧪")
                .location(PanelLocation::Floating)
                .min_size(280.0, 350.0)
        );

        // Register menu items in Tools menu
        api.register_menu_item(
            MenuLocation::Tools,
            MenuItem::new("Spawn Test Entity", UiId::new(1001))
                .shortcut("Ctrl+Shift+T")
                .icon("➕")
        );

        api.register_menu_item(
            MenuLocation::Tools,
            MenuItem::new("Plugin Settings...", UiId::new(1002))
                .icon("⚙")
        );

        // Register a submenu
        api.register_menu_item(
            MenuLocation::Tools,
            MenuItem::new("Test Submenu", UiId::new(1003))
                .submenu(vec![
                    MenuItem::new("Action 1", UiId::new(1004)),
                    MenuItem::new("Action 2", UiId::new(1005)),
                    MenuItem::new("Action 3", UiId::new(1006)).disabled(),
                ])
        );

        // Register menu items in File menu
        api.register_menu_item(
            MenuLocation::File,
            MenuItem::new("Export Test Data", UiId::new(1010))
        );

        // Register context menu for hierarchy
        api.register_context_menu(
            ContextMenuLocation::Hierarchy,
            MenuItem::new("Test Plugin: Duplicate", UiId::new(2001))
        );

        api.register_context_menu(
            ContextMenuLocation::Hierarchy,
            MenuItem::new("Test Plugin: Reset Transform", UiId::new(2002))
        );

        // Register context menu for viewport
        api.register_context_menu(
            ContextMenuLocation::Viewport,
            MenuItem::new("Spawn Entity Here", UiId::new(2010))
        );

        // Register inspector section for custom component
        api.register_inspector(
            "TestComponent",
            InspectorDefinition {
                type_id: "TestComponent".to_string(),
                label: "Test Plugin Data".to_string(),
                priority: 100,
            }
        );

        // Register toolbar button
        api.register_toolbar_item(
            ToolbarItem::new(UiId::new(3001), "🧪", "Test Plugin Quick Action")
                .group("plugins")
        );

        // Subscribe to events
        api.subscribe(EditorEventType::EntitySelected);
        api.subscribe(EditorEventType::EntityDeselected);
        api.subscribe(EditorEventType::UiEvent);
        api.subscribe(EditorEventType::All);

        Ok(())
    }

    fn on_unload(&mut self, api: &mut dyn EditorApi) {
        api.log_info("Test Plugin unloaded!");
    }

    fn on_update(&mut self, api: &mut dyn EditorApi, _dt: f32) {
        // Build panel content
        let mut content = vec![
            Widget::heading("Test Plugin Panel"),
            Widget::separator(),

            // Button section
            Widget::panel("Interaction Test", vec![
                Widget::label(format!("Button clicks: {}", self.click_count)),
                Widget::row(vec![
                    Widget::button("Click Me!", UiId::new(1)),
                    Widget::button("Reset", UiId::new(2)),
                ]),
            ]),

            Widget::spacer(8.0),

            // Slider section
            Widget::panel("Slider Test", vec![
                Widget::Slider {
                    value: self.slider_value,
                    min: 0.0,
                    max: 1.0,
                    id: UiId::new(10),
                    label: Some("Value".to_string()),
                },
                Widget::label(format!("Current: {:.2}", self.slider_value)),
            ]),

            Widget::spacer(4.0),

            // Checkbox section
            Widget::checkbox("Enable feature", self.checkbox_checked, UiId::new(20)),

            Widget::separator(),

            // Entity spawn section
            Widget::panel("Entity Spawning", vec![
                Widget::label(format!("Spawned by plugin: {}", self.spawned_count)),
                Widget::row(vec![
                    Widget::button("Spawn Empty", UiId::new(100)),
                    Widget::button("Spawn at Origin", UiId::new(101)),
                ]),
            ]),

            Widget::separator(),
        ];

        // Selected entity section
        content.push(Widget::panel("Selection Info", vec![
            if let Some(ref name) = self.selected_entity_name {
                Widget::column(vec![
                    Widget::label(format!("Selected: {}", name)),
                    Widget::row(vec![
                        Widget::button("Rename", UiId::new(200)),
                        Widget::button("Delete", UiId::new(201)),
                    ]),
                ])
            } else {
                Widget::Label {
                    text: "No entity selected".to_string(),
                    style: TextStyle::Caption,
                }
            },
        ]));

        // Scene stats
        let entities = api.query_entities(&EntityQuery::default());
        content.push(Widget::Label {
            text: format!("Total entities: {}", entities.len()),
            style: TextStyle::Caption,
        });

        api.set_panel_content("test_panel", content);

        // Inspector content - only show when entity is selected
        if self.selected_entity_id.is_some() {
            let inspector_content = vec![
                Widget::Label {
                    text: "Plugin Data".to_string(),
                    style: TextStyle::Heading3,
                },
                Widget::Slider {
                    value: self.slider_value,
                    min: 0.0,
                    max: 1.0,
                    id: UiId::new(500),
                    label: Some("Custom Value".to_string()),
                },
                Widget::Label {
                    text: format!("Value: {:.2}", self.slider_value),
                    style: TextStyle::Caption,
                },
                Widget::checkbox("Plugin Feature", self.checkbox_checked, UiId::new(501)),
                Widget::row(vec![
                    Widget::button("Apply", UiId::new(502)),
                    Widget::button("Reset", UiId::new(503)),
                ]),
            ];
            api.set_inspector_content("TestComponent", inspector_content);
        }
    }

    fn on_event(&mut self, api: &mut dyn EditorApi, event: &EditorEvent) {
        match event {
            EditorEvent::EntitySelected(entity_id) => {
                if let Some(name) = api.get_entity_name(*entity_id) {
                    api.log_info(&format!("Entity selected: {}", name));
                    self.selected_entity_name = Some(name);
                    self.selected_entity_id = Some(*entity_id);
                }
            }
            EditorEvent::EntityDeselected(_) => {
                self.selected_entity_name = None;
                self.selected_entity_id = None;
            }
            EditorEvent::UiEvent(ui_event) => {
                self.handle_ui_event(api, ui_event);
            }
            EditorEvent::SceneLoaded { path } => {
                api.log_info(&format!("Scene loaded: {}", path));
            }
            EditorEvent::SceneSaved { path } => {
                api.log_info(&format!("Scene saved: {}", path));
            }
            _ => {}
        }
    }
}

impl TestPlugin {
    fn handle_ui_event(&mut self, api: &mut dyn EditorApi, event: &UiEvent) {
        match event {
            UiEvent::ButtonClicked(id) => {
                match id.0 {
                    // Panel buttons
                    1 => {
                        self.click_count += 1;
                        api.log_info(&format!("Button clicked! Count: {}", self.click_count));
                    }
                    2 => {
                        self.click_count = 0;
                        api.log_info("Counter reset!");
                    }

                    // Entity spawn buttons
                    100 => {
                        self.spawn_entity(api, "Plugin Entity", None);
                    }
                    101 => {
                        self.spawn_entity(api, "Origin Entity", Some(PluginTransform::default()));
                    }

                    // Selection action buttons
                    200 => {
                        if let Some(entity_id) = self.selected_entity_id {
                            let new_name = format!("Renamed_{}", self.click_count);
                            api.set_entity_name(entity_id, &new_name);
                            self.selected_entity_name = Some(new_name.clone());
                            api.log_info(&format!("Entity renamed to: {}", new_name));
                        }
                    }
                    201 => {
                        if let Some(entity_id) = self.selected_entity_id {
                            api.despawn_entity(entity_id);
                            api.log_info("Entity deleted!");
                            self.selected_entity_id = None;
                            self.selected_entity_name = None;
                        }
                    }

                    // Menu items
                    1001 => {
                        self.spawn_entity(api, "Menu Spawned", None);
                        api.log_info("Entity spawned from menu!");
                    }
                    1002 => {
                        api.log_info("Plugin settings requested (not implemented)");
                    }
                    1004 => api.log_info("Submenu Action 1 triggered"),
                    1005 => api.log_info("Submenu Action 2 triggered"),
                    1010 => api.log_info("Export Test Data requested"),

                    // Context menu items
                    2001 => {
                        if let (Some(entity_id), Some(name)) = (self.selected_entity_id, &self.selected_entity_name) {
                            if let Some(transform) = api.get_transform(entity_id) {
                                let mut new_transform = transform;
                                new_transform.translation[0] += 1.0;
                                let new_name = format!("{}_copy", name);
                                api.spawn_entity(&EntityDefinition::new(&new_name).transform(new_transform));
                                api.log_info(&format!("Duplicated entity: {}", new_name));
                            }
                        }
                    }
                    2002 => {
                        if let Some(entity_id) = self.selected_entity_id {
                            api.set_transform(entity_id, &PluginTransform::default());
                            api.log_info("Transform reset to origin!");
                        }
                    }
                    2010 => {
                        self.spawn_entity(api, "Viewport Spawned", None);
                        api.log_info("Entity spawned from viewport context menu!");
                    }

                    // Toolbar
                    3001 => {
                        api.log_info("Toolbar quick action triggered!");
                        self.click_count += 10;
                    }

                    // Inspector buttons
                    502 => {
                        api.log_info(&format!("Inspector Apply clicked! Value: {:.2}, Feature: {}", self.slider_value, self.checkbox_checked));
                    }
                    503 => {
                        self.slider_value = 0.5;
                        self.checkbox_checked = false;
                        api.log_info("Inspector values reset!");
                    }

                    _ => {
                        api.log_info(&format!("Unknown button clicked: {}", id.0));
                    }
                }
            }
            UiEvent::SliderChanged { id, value } => {
                if id.0 == 10 || id.0 == 500 {
                    self.slider_value = *value;
                }
            }
            UiEvent::CheckboxToggled { id, checked } => {
                if id.0 == 20 || id.0 == 501 {
                    self.checkbox_checked = *checked;
                    api.log_info(&format!("Feature enabled: {}", checked));
                }
            }
            _ => {}
        }
    }

    fn spawn_entity(&mut self, api: &mut dyn EditorApi, base_name: &str, transform: Option<PluginTransform>) {
        self.spawned_count += 1;
        let name = format!("{}_{}", base_name, self.spawned_count);

        let mut def = EntityDefinition::new(&name);
        if let Some(t) = transform {
            def = def.transform(t);
        }

        api.spawn_entity(&def);
        api.log_info(&format!("Spawned entity: {}", name));
    }
}

// Export the plugin entry point
declare_plugin!(TestPlugin, TestPlugin::new());
