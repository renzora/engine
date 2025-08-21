# RenScript Language Support for VS Code

This extension provides syntax highlighting and language support for RenScript (.ren) files in Visual Studio Code.

## Features

- **Syntax Highlighting**: Full syntax highlighting for RenScript language constructs
- **Language Support**: Auto-closing brackets, comment toggling, and code folding
- **Script Types**: Highlights different script types (camera, light, mesh, scene, transform)
- **Property Definitions**: Syntax support for props blocks with type annotations
- **Built-in Functions**: Highlights all RenScript API functions

## Supported Constructs

### Script Declarations
```renscript
camera MyCamera { }
light MyLight { }
mesh MyMesh { }
scene MyScene { }
transform MyTransform { }
```

### Properties
```renscript
props movement {
  speed: float {
    default: 5.0,
    min: 0.1,
    max: 20.0,
    description: "Movement speed"
  }
  
  enabled: boolean {
    default: true,
    description: "Enable movement"
  }
}
```

### Lifecycle Functions
- `start { }` - Initialization
- `update(dt) { }` - Frame update
- `destroy { }` - Cleanup
- `on_collision(other) { }` - Collision events
- `on_trigger(other) { }` - Trigger events

### Built-in API Functions
- Transform: `get_position()`, `set_position()`, `get_rotation()`, `set_rotation()`, etc.
- Camera: `detach_camera_controls()`, `attach_camera_controls()`, `set_camera_target()`
- Light: `set_light_intensity()`, `set_light_color()`, `get_light_intensity()`
- Gamepad: `get_left_stick_x()`, `get_right_stick_y()`, `is_gamepad_button_pressed()`
- Math: `sin()`, `cos()`, `lerp()`, `clamp()`, `random()`
- And many more...

## Installation

### From VSIX file
1. Download the .vsix file
2. In VS Code, open Command Palette (Ctrl+Shift+P / Cmd+Shift+P)
3. Run "Extensions: Install from VSIX..."
4. Select the downloaded .vsix file

### From source
1. Copy the `vscode-renscript` folder to your VS Code extensions folder:
   - Windows: `%USERPROFILE%\.vscode\extensions`
   - macOS: `~/.vscode/extensions`
   - Linux: `~/.vscode/extensions`
2. Restart VS Code

## Usage

Once installed, the extension will automatically activate for any file with a `.ren` extension.

## File Association

The extension automatically associates with `.ren` files. If needed, you can manually set the language mode:
1. Open a .ren file
2. Click on the language indicator in the status bar (bottom right)
3. Select "RenScript" from the language list

## License

MIT