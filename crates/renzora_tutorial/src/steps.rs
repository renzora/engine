//! Tutorial step definitions — each step describes what to highlight and what to tell the user.

/// Where to point the tutorial highlight.
#[derive(Clone, Debug)]
pub enum TutorialTarget {
    /// Highlight an entire panel by its dock ID.
    Panel(&'static str),
    /// Highlight the top-right corner of a panel (e.g. a "+" button area).
    PanelTopRight(&'static str),
    /// Highlight the top area of a panel (e.g. toolbar).
    PanelTop(&'static str),
    /// Highlight the bottom area of a panel.
    PanelBottom(&'static str),
    /// The title bar / workspace tabs area.
    TitleBar,
    /// Center of the screen (no specific target, just info).
    Center,
}

/// Direction an arrow should point from the tooltip card toward the target.
#[derive(Clone, Copy, Debug)]
pub enum ArrowDirection {
    Left,
    Right,
    Up,
    Down,
    None,
}

/// How to advance to the next step.
#[derive(Clone, Debug)]
pub enum AdvanceCondition {
    /// User clicks "Next" button.
    Manual,
    /// Automatically advance after N seconds.
    Timer(f64),
}

/// A single step in the tutorial sequence.
#[derive(Clone, Debug)]
pub struct TutorialStep {
    pub title: &'static str,
    pub description: &'static str,
    pub target: TutorialTarget,
    pub arrow: ArrowDirection,
    pub advance: AdvanceCondition,
}

/// Build the complete list of tutorial steps.
pub fn build_steps() -> Vec<TutorialStep> {
    vec![
        // 0 — Welcome
        TutorialStep {
            title: "Welcome to Renzora Engine",
            description: "This guided tour will walk you through the editor basics.\n\nYou'll learn how to create entities, move them around, add components, set up your world, and more.",
            target: TutorialTarget::Center,
            arrow: ArrowDirection::None,
            advance: AdvanceCondition::Manual,
        },
        // 1 — Hierarchy overview
        TutorialStep {
            title: "The Hierarchy Panel",
            description: "This is the Hierarchy panel. It shows every entity in your scene as a tree.\n\nEntities are the building blocks of your game world.",
            target: TutorialTarget::Panel("hierarchy"),
            arrow: ArrowDirection::Left,
            advance: AdvanceCondition::Manual,
        },
        // 2 — Add an entity
        TutorialStep {
            title: "Add an Entity",
            description: "Click the  +  button at the top of the Hierarchy panel to add a new entity.\n\nA search overlay will appear — try adding a \"Cube\" or \"Empty Entity\".",
            target: TutorialTarget::PanelTopRight("hierarchy"),
            arrow: ArrowDirection::Left,
            advance: AdvanceCondition::Manual,
        },
        // 3 — Viewport overview
        TutorialStep {
            title: "The Viewport",
            description: "This is the 3D Viewport where you see your scene rendered in real time.\n\nUse the mouse to orbit, scroll to zoom, and middle-click to pan.",
            target: TutorialTarget::Panel("viewport"),
            arrow: ArrowDirection::Right,
            advance: AdvanceCondition::Manual,
        },
        // 4 — Select and move
        TutorialStep {
            title: "Select & Move Entities",
            description: "Click an entity in the Hierarchy or directly in the Viewport to select it.\n\nA transform gizmo will appear — drag the colored arrows to move the entity along each axis.",
            target: TutorialTarget::Panel("viewport"),
            arrow: ArrowDirection::Right,
            advance: AdvanceCondition::Manual,
        },
        // 5 — Inspector overview
        TutorialStep {
            title: "The Inspector Panel",
            description: "When an entity is selected, the Inspector shows all of its components.\n\nYou can edit transforms, materials, physics, and any other properties here.",
            target: TutorialTarget::Panel("inspector"),
            arrow: ArrowDirection::Right,
            advance: AdvanceCondition::Manual,
        },
        // 6 — Add a component
        TutorialStep {
            title: "Add a Component",
            description: "Click \"Add Component\" at the bottom of the Inspector to attach new behaviors.\n\nComponents define what an entity is — a light, a mesh, a script, physics, and more.",
            target: TutorialTarget::PanelBottom("inspector"),
            arrow: ArrowDirection::Right,
            advance: AdvanceCondition::Manual,
        },
        // 7 — World environment
        TutorialStep {
            title: "World Environment",
            description: "Add environment entities to bring your scene to life:\n\n\u{2022} Directional Light — the sun\n\u{2022} Skybox — sky and horizon\n\u{2022} Atmosphere — volumetric scattering\n\u{2022} Fog — depth-based fog\n\nUse the + button in the Hierarchy to add these.",
            target: TutorialTarget::PanelTopRight("hierarchy"),
            arrow: ArrowDirection::Left,
            advance: AdvanceCondition::Manual,
        },
        // 8 — Post-processing
        TutorialStep {
            title: "Post-Processing Effects",
            description: "Post-processing effects are components added to the camera entity.\n\nSelect the camera in the Hierarchy, then use Add Component to attach effects like Bloom, Vignette, Color Grading, and more.",
            target: TutorialTarget::Panel("inspector"),
            arrow: ArrowDirection::Right,
            advance: AdvanceCondition::Manual,
        },
        // 9 — Assets panel
        TutorialStep {
            title: "The Asset Browser",
            description: "The Asset Browser shows files in your project folder.\n\nDrag models, textures, and scripts from here into the Viewport or onto entities in the Hierarchy.",
            target: TutorialTarget::PanelTop("assets"),
            arrow: ArrowDirection::Down,
            advance: AdvanceCondition::Manual,
        },
        // 10 — Workspaces
        TutorialStep {
            title: "Workspace Layouts",
            description: "The top bar has workspace tabs — Scene, Materials, Blueprints, and more.\n\nEach workspace rearranges the panels for a specific workflow. Click a tab to switch.",
            target: TutorialTarget::TitleBar,
            arrow: ArrowDirection::Up,
            advance: AdvanceCondition::Manual,
        },
        // 11 — Blueprints intro
        TutorialStep {
            title: "Visual Scripting with Blueprints",
            description: "Switch to the Blueprints workspace to create visual scripts.\n\nBlueprints let you define game logic by connecting nodes — no code required.\n\nConnect event nodes to actions to make things happen when the game runs.",
            target: TutorialTarget::TitleBar,
            arrow: ArrowDirection::Up,
            advance: AdvanceCondition::Manual,
        },
        // 12 — Materials
        TutorialStep {
            title: "Material Editor",
            description: "Switch to the Materials workspace to create custom shaders.\n\nThe node graph editor lets you build materials visually by connecting math, texture, and color nodes.",
            target: TutorialTarget::TitleBar,
            arrow: ArrowDirection::Up,
            advance: AdvanceCondition::Manual,
        },
        // 13 — Play mode
        TutorialStep {
            title: "Play Mode",
            description: "Press the Play button in the title bar to test your game.\n\nThe viewport will go fullscreen. Press Escape to return to the editor.\n\nYou can also use Scripts Only mode to test scripts without entering play mode.",
            target: TutorialTarget::TitleBar,
            arrow: ArrowDirection::Up,
            advance: AdvanceCondition::Manual,
        },
        // 14 — Console
        TutorialStep {
            title: "The Console",
            description: "The Console panel shows log messages, warnings, and errors.\n\nScript print() calls also appear here. Use filters to narrow down what you see.",
            target: TutorialTarget::PanelTop("console"),
            arrow: ArrowDirection::Down,
            advance: AdvanceCondition::Manual,
        },
        // 15 — Done
        TutorialStep {
            title: "You're Ready!",
            description: "That covers the basics of the Renzora Editor.\n\nExplore the panels, experiment with entities and components, and build something amazing.\n\nYou can restart this tutorial anytime from the Help menu.",
            target: TutorialTarget::Center,
            arrow: ArrowDirection::None,
            advance: AdvanceCondition::Manual,
        },
    ]
}
