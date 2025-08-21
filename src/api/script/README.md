# RenScript - Game Object Scripting Language

RenScript is a simple, intuitive scripting language designed for game object behavior in Babylon.js. It provides a clean syntax for common game development tasks like movement, rotation, animation, and property management.

## Table of Contents

- [Basic Syntax](#basic-syntax)
- [Script Structure](#script-structure)
- [Properties System](#properties-system)
- [API Reference](#api-reference)
- [Examples](#examples)
- [Best Practices](#best-practices)

## Basic Syntax

RenScript uses a clean, readable syntax similar to other scripting languages:

```renscript
script MyScript {
  # Variables
  speed = 2.0
  counter = 0
  
  # Lifecycle methods
  start {
    log("Script started!")
  }
  
  update(dt) {
    rotate_by(0, dt * speed, 0)
  }
  
  destroy {
    log("Script destroyed!")
  }
}
```

## Script Structure

Every RenScript file contains a single script with the following structure:

### Script Declaration
```renscript
script ScriptName {
  # Script content
}
```

### Variables
Declare variables at the script level:
```renscript
speed = 1.0
health = 100
player_name = "Hero"
is_active = true
```

### Lifecycle Methods

#### `start` - Called once when script is attached
```renscript
start {
  log("Initializing script...")
  set_color(1.0, 0.0, 0.0)  # Set to red
  add_tag("player")
}
```

#### `update(dt)` - Called every frame
```renscript
update(dt) {
  # dt is delta time in seconds
  rotate_by(0, dt * speed, 0)
  move_by(0, 0, dt * 2.0)
}
```

#### `destroy` - Called when script is removed
```renscript
destroy {
  log("Cleanup complete")
}
```

## Properties System

RenScript supports configurable properties that appear in the editor UI, organized into named sections:

### Basic Properties
```renscript
script ConfigurableScript {
  props {
    speed: number {
      default: 1.0,
      min: 0.0,
      max: 5.0,
      description: "Movement speed"
    }
    
    color: range {
      default: 0.5,
      min: 0.0,
      max: 1.0,
      description: "Red color component"
    }
    
    enabled: boolean {
      default: true,
      description: "Enable this feature"
    }
    
    mode: select {
      default: "normal",
      options: ["normal", "fast", "slow"],
      description: "Speed mode"
    }
  }
}
```

### Named Property Sections
Organize properties into logical groups:

```renscript
script AdvancedScript {
  props movement {
    speed: float {
      default: 2.0,
      min: 0.1,
      max: 10.0,
      description: "Movement speed"
    }
    
    jump_height: number {
      default: 5,
      min: 1,
      max: 20,
      description: "Jump height"
    }
  }
  
  props appearance {
    red: range {
      default: 1.0,
      min: 0.0,
      max: 1.0,
      description: "Red component"
    }
    
    green: range {
      default: 0.5,
      min: 0.0,
      max: 1.0,
      description: "Green component"
    }
    
    glow_enabled: boolean {
      default: false,
      description: "Enable glow effect"
    }
  }
  
  props physics {
    gravity: float {
      default: -9.81,
      min: -20.0,
      max: 20.0,
      description: "Gravity strength"
    }
    
    bounce_factor: range {
      default: 0.8,
      min: 0.0,
      max: 1.0,
      description: "Bounciness"
    }
  }
}
```

### Property Types

| Type | UI Control | Description |
|------|------------|-------------|
| `number` | Number input | Integer values |
| `float` | Number input | Decimal values |
| `boolean` | Toggle switch | True/false values |
| `string` | Text input | Text values |
| `range` | Slider | Numeric values with visual slider |
| `select` | Dropdown | Choice from predefined options |

## API Reference

### Transform & Movement
```renscript
# Position
get_position()                    # Returns current position
set_position(x, y, z)            # Set absolute position
move_by(x, y, z)                 # Move relative to current position
move_to(x, y, z)                 # Move to absolute position
get_world_position()             # Get world space position

# Rotation
get_rotation()                   # Get current rotation
set_rotation(x, y, z)           # Set absolute rotation
rotate_by(x, y, z)              # Rotate relative to current rotation

# Scale
get_scale()                     # Get current scale
set_scale(x, y, z)             # Set scale

# Advanced
look_at(target, up)            # Look at a target position
```

### Appearance & Materials
```renscript
# Visibility
is_visible()                    # Check if object is visible
set_visible(true/false)         # Show/hide object

# Colors
set_color(r, g, b)             # Set diffuse color (0-1 range)
get_color()                    # Get current color
set_emissive_color(r, g, b)    # Set glow/emissive color
get_emissive_color()           # Get emissive color

# Material properties
set_material_property(property, value)  # Set any material property
get_material_property(property)         # Get material property value
```

### Animation & Time
```renscript
# Time
get_time()                     # Get current time in milliseconds

# Animation
animate(property, targetValue, duration, easing)  # Animate property
stop_animation()               # Stop current animation
pause_animation()              # Pause animation
resume_animation()             # Resume animation
```

### Math Functions
```renscript
# Trigonometry
sin(angle)                     # Sine
cos(angle)                     # Cosine
tan(angle)                     # Tangent

# Utility
abs(value)                     # Absolute value
min(a, b, ...)                # Minimum value
max(a, b, ...)                # Maximum value
sqrt(value)                    # Square root
pow(base, exponent)           # Power

# Vector operations
dot(vec1, vec2)               # Dot product
cross(vec1, vec2)             # Cross product
normalize(vector)             # Normalize vector
```

### Utilities
```renscript
# Logging
log(message, ...)             # Log to console

# Tags
add_tag(tag)                  # Add tag to object
remove_tag(tag)               # Remove tag from object
has_tag(tag)                  # Check if object has tag

# Scene queries
find_object(name)             # Find object by name
find_objects_by_tag(tag)      # Find objects with tag
```

## Examples

### Simple Rotator
```renscript
script SimpleRotator {
  props {
    speed: float {
      default: 1.0,
      min: 0.0,
      max: 5.0,
      description: "Rotation speed"
    }
  }
  
  start {
    log("Starting rotation")
  }
  
  update(dt) {
    rotate_by(0, dt * speed, 0)
  }
}
```

### Color Animator
```renscript
script ColorAnimator {
  props appearance {
    cycle_speed: range {
      default: 1.0,
      min: 0.1,
      max: 3.0,
      description: "Color cycle speed"
    }
    
    brightness: range {
      default: 1.0,
      min: 0.0,
      max: 2.0,
      description: "Color brightness"
    }
  }
  
  update(dt) {
    time = get_time() * 0.001
    
    red = sin(time * cycle_speed) * 0.5 + 0.5
    green = sin(time * cycle_speed + 2.0) * 0.5 + 0.5  
    blue = sin(time * cycle_speed + 4.0) * 0.5 + 0.5
    
    set_color(red * brightness, green * brightness, blue * brightness)
  }
}
```

### Physics Bouncer
```renscript
script PhysicsBouncer {
  props physics {
    gravity: float {
      default: -9.81,
      min: -20.0,
      max: 0.0,
      description: "Gravity strength"
    }
    
    bounce_factor: range {
      default: 0.8,
      min: 0.0,
      max: 1.0,
      description: "Bounce strength"
    }
    
    ground_level: float {
      default: 0.0,
      min: -10.0,
      max: 10.0,
      description: "Ground Y position"
    }
  }
  
  velocity_y = 0.0
  
  start {
    log("Physics bouncer initialized")
  }
  
  update(dt) {
    pos = get_position()
    current_y = pos[1] # Note: array indexing not supported yet
    
    # Apply gravity
    velocity_y = velocity_y + gravity * dt
    new_y = current_y + velocity_y * dt
    
    # Ground collision
    if new_y <= ground_level {
      new_y = ground_level
      velocity_y = velocity_y * -bounce_factor
    }
    
    set_position(pos[0], new_y, pos[2])
  }
}
```

### Multi-Section Demo
```renscript
script MultiSectionDemo {
  props rotation {
    spin_speed: number {
      default: 1.0,
      min: 0.0,
      max: 5.0,
      description: "Rotation speed"
    }
    
    auto_rotate: boolean {
      default: true,
      description: "Enable automatic rotation"
    }
  }
  
  props appearance {
    red: range {
      default: 0.8,
      min: 0.0,
      max: 1.0,
      description: "Red color component"
    }
    
    glow_intensity: float {
      default: 0.0,
      min: 0.0,
      max: 2.0,
      description: "Emissive glow strength"
    }
  }
  
  props animation {
    bounce_enabled: boolean {
      default: false,
      description: "Enable bouncing"
    }
    
    bounce_speed: float {
      default: 2.0,
      min: 0.5,
      max: 8.0,
      description: "Bounce frequency"
    }
  }
  
  start {
    log("Multi-section demo started!")
    set_color(red, 0.5, 1.0)
    add_tag("demo")
  }
  
  update(dt) {
    # Rotation with boolean check
    if auto_rotate {
      rotate_by(0, dt * spin_speed, 0)
    }
    
    # Dynamic color
    set_color(red, 0.5, 1.0)
    set_emissive_color(red * glow_intensity, 0.5 * glow_intensity, glow_intensity)
    
    # Bouncing animation
    if bounce_enabled {
      time = get_time() * 0.001
      bounce_offset = sin(time * bounce_speed) * 0.5
      move_to(0, bounce_offset, 0)
    }
  }
}
```

## Best Practices

### Performance
- **Minimize expensive operations in `update()`** - Avoid complex calculations every frame
- **Cache frequently used values** - Store results instead of recalculating
- **Use properties for tweakable values** - Makes scripts more flexible and performant

### Code Organization
- **Use descriptive property names** - `movement_speed` instead of `speed`
- **Group related properties** - Use named sections like `props movement`, `props appearance`
- **Add meaningful descriptions** - Help users understand what each property does
- **Set reasonable min/max values** - Prevent invalid configurations

### Debugging
- **Use logging strategically** - Log important state changes, not every frame
- **Test with different property values** - Ensure scripts work across the full range
- **Handle edge cases** - Check for zero values, negative numbers, etc.

### Property Design
```renscript
# Good: Organized, well-described properties
props movement {
  walk_speed: float {
    default: 2.0,
    min: 0.1,
    max: 10.0,
    description: "Walking speed in units per second"
  }
  
  run_multiplier: range {
    default: 2.0,
    min: 1.0,
    max: 5.0,
    description: "Running speed multiplier"
  }
}

props combat {
  attack_damage: number {
    default: 10,
    min: 1,
    max: 100,
    description: "Base attack damage"
  }
  
  attack_type: select {
    default: "melee",
    options: ["melee", "ranged", "magic"],
    description: "Type of attack"
  }
}
```

## File Structure

Place your RenScript files in the `assets/` directory with a `.ren` extension:

```
assets/
├── player_controller.ren
├── enemy_ai.ren
├── pickup_item.ren
└── environmental/
    ├── rotating_platform.ren
    └── moving_obstacle.ren
```

## Loading Scripts

Scripts are automatically loaded when dragged onto objects in the editor. They can also be attached programmatically through the script runtime API.

## Limitations

Current limitations of RenScript:
- **No array indexing** - Cannot use `array[index]` syntax
- **No custom functions** - Only predefined API functions available
- **No loops** - For loops and while loops not supported
- **No complex conditionals** - Limited if/else support
- **No object creation** - Cannot instantiate new objects

## Future Features

Planned enhancements:
- Array indexing and manipulation
- Custom function definitions
- Loop constructs (for, while)
- Enhanced conditional statements
- Object instantiation and management
- Event system integration
- Network synchronization support

---

For more examples and detailed API documentation, see the individual script files in the `assets/` directory.