# RenScript Programming Guide

RenScript is a custom scripting language designed for the Renzora game engine. It provides an intuitive way to script game object behaviors, animations, and interactions with a simplified syntax optimized for game development.

## Table of Contents

- [Language Syntax](#language-syntax)
- [Script Structure](#script-structure)  
- [Data Types](#data-types)
- [Variables](#variables)
- [Control Flow](#control-flow)
- [Lifecycle Functions](#lifecycle-functions)
- [Built-in Functions](#built-in-functions)
- [Examples](#examples)
- [Best Practices](#best-practices)

## Language Syntax

### Basic Syntax Rules

- **Comments**: Use `#` for single-line comments, or `//` 
- **Variables**: Declared by direct assignment (no `var` keyword)
- **Blocks**: Use `{}` for code blocks
- **File Extension**: `.ren`
- **Script Declaration**: Must start with `script ScriptName {}`

```renscript
# This is a comment
// This is also a comment
speed = 10
name = "PlayerController"
```

## Script Structure

### Script Declaration

Every RenScript file must start with a script declaration:

```renscript
script MyScriptName {
    # Script content goes here
}
```

Alternative script types for specific objects:
```renscript
camera MyCameraScript { }
light MyLightScript { }
mesh MyMeshScript { }
scene MySceneScript { }
transform MyTransformScript { }
```

### Properties Block

Define configurable properties that appear in the editor:

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

**Property Types:**
- `boolean` - true/false values
- `float` - Decimal numbers  
- `int` - Whole numbers
- `string` - Text values
- `vector3` - 3D vectors
- `color` - Color values
- `range` - Number with slider
- `dropdown` - Selection list
- `file` - File picker

## Data Types

### Numbers
```renscript
integer = 42
decimal = 3.14159
negative = -10
```

### Booleans  
```renscript
isEnabled = true
isDisabled = false
```

### Strings
```renscript
message = "Hello World"
singleQuotes = 'Also valid'
```

### Arrays (Limited Support)
```renscript
# Arrays have limited support in RenScript
# Mostly used for position/color data
position = getPosition()  # Returns array [x, y, z]
```

## Variables

Variables are declared by simple assignment without keywords:

```renscript
script PlayerController {
    # Global script variables
    currentHealth = 100
    maxHealth = 100  
    speed = 5.0
    playerName = "Player1"
    
    start {
        # Can access and modify global variables
        currentHealth = maxHealth
    }
    
    update {
        # Variables can be reassigned
        speed = 15
        currentHealth = currentHealth - 1
    }
}
```

## Control Flow

### Conditional Statements
```renscript
update {
    if (health <= 0) {
        log("Player is dead")
    } elif (health < 20) {
        log("Player health is low") 
    } else {
        log("Player is healthy")
    }
}

# Comparison operators: ==, !=, <, <=, >, >=
# Logical operators: &&, ||, !
```

### Loops
```renscript
# While loop
counter = 0
while (counter < 5) {
    log("Count: " + counter)
    counter = counter + 1
}

# For loops may have limited support
```

## Lifecycle Functions

RenScript provides predefined lifecycle functions:

```renscript
start {
    # Called when script starts/object is created
    log("Script started")
    setPosition(0, 0, 0)
}

update {
    # Called every frame
    currentTime = time()
    x = sin(currentTime)
    setPosition(x, 0, 0)
}

destroy {
    # Called when script/object is destroyed  
    log("Script destroyed")
}

once {
    # Called once (special lifecycle function)
    log("This runs once")
}
```

**Note**: Custom functions are not supported. All logic must be organized within these lifecycle functions.

## Built-in Functions

RenScript provides extensive built-in functions organized by category:

### Core Functions
```renscript
# Logging and timing
log("Debug message")
currentTime = time()

# Object tagging
addTag("enemy")
removeTag("player")  
hasTagResult = hasTag("collectible")
allTags = getTags()
```

### Transform Functions
```renscript
# Position
currentPos = position()        # Same as getPosition()
setPosition(x, y, z)
move(x, y, z)                 # Same as moveBy()

# Rotation
currentRot = rotation()        # Same as getRotation() 
setRotation(x, y, z)          # In radians
rotate(x, y, z)               # Same as rotateBy()

# Scale
setScale(x, y, z)
```

### Math Functions
```renscript
# Trigonometry
angle = sin(time())
cosValue = cos(angle)

# Utility math
distance = sqrt(x*x + y*y)
rounded = floor(3.7)          # Returns 3
absolute = abs(-5)            # Returns 5
clamped = clamp(value, 0, 10) # Clamp between 0 and 10
interpolated = lerp(a, b, t)  # Linear interpolation

# Random
randomValue = random()         # 0.0 to 1.0
randomInRange = randomRange(1, 10)  # Between 1 and 10
```

### Input Functions
```renscript
# Keyboard
if (isKeyPressed("W")) {
    # Move forward
}

ctrlPressed = isCtrlPressed()
shiftPressed = isShiftPressed()

# Mouse
if (isLeftMouse()) {
    # Left click
}
if (isRightMouse()) {
    # Right click  
}
if (isMiddleMouse()) {
    # Middle click
}

mousePos = mousePosition()
x = mouseX()
y = mouseY()
```

### Gamepad Functions
```renscript
# Check if gamepad is connected
if (isGamepadConnected(0)) {
    # Analog sticks
    leftStickX = leftX(0)     # Left stick X axis, gamepad 0
    leftStickY = leftY(0)     # Left stick Y axis  
    rightStickX = rightX(0)   # Right stick X
    rightStickY = rightY(0)   # Right stick Y
    
    # Buttons
    if (button(0, 0)) {         # Button A on gamepad 0
        # Jump
    }
    
    # Specific button checks
    if (isButtonA(0)) { }
    if (isButtonB(0)) { }
    if (isButtonX(0)) { } 
    if (isButtonY(0)) { }
    
    # Triggers
    leftTriggerValue = leftTrigger(0)
    rightTriggerValue = rightTrigger(0)
}
```

### Animation Functions
```renscript
# Basic animation
animate(targetPosition, duration)
animatePosition([5, 0, 0], 2.0)
animateRotation([0, 90, 0], 1.5)
animateColor([1, 0, 0], 1.0)

# Animation control
stopAnimation()
animationSpeed(2.0)          # Double speed
isCurrentlyAnimating = isAnimating()
```

### Object Management
```renscript
# Find objects
player = findByName("Player")
enemies = findByTag("enemy")
allMeshes = getAllMeshes()

# Object queries
objectsNearby = getInRadius(position(), 10)
closestEnemy = getClosest("enemy")

# Raycasting
hit = raycast(startPos, direction, distance)

# Metadata
setMetadata("health", 100)
health = getMetadata("health")

# Object lifecycle
clone("MyObject")
dispose()                    # Destroy this object
```

### Lighting Functions
```renscript
# Create lights
createDirectionalLight("sun", [0, -1, 0])
createPointLight("lamp", [0, 5, 0])

# Light properties  
setLightIntensity("sun", 1.5)
setLightDiffuse("lamp", [1, 0.8, 0.6])
lightPos = getLightPosition("lamp")

# Enable/disable
enableLight("sun")
disableLight("lamp")
```

### Physics Functions
```renscript
# Enable physics
physics()                    # Enable physics for this object

# Apply forces
impulse([0, 10, 0])         # Instant force (jump)
force([5, 0, 0])            # Continuous force

# Velocity
linearVelocity([2, 0, 0])   # Set velocity directly
velocity = getLinearVelocity()
```

## Examples

### Simple Movement Script

```renscript
script SimpleMovement {
    speed = 5.0
    
    update {
        currentPos = position()
        newX = currentPos[0]
        newZ = currentPos[2]
        
        if (isKeyPressed("W")) {
            newZ = newZ - speed * time()
        }
        if (isKeyPressed("S")) {
            newZ = newZ + speed * time()
        }
        if (isKeyPressed("A")) {
            newX = newX - speed * time()
        }
        if (isKeyPressed("D")) {
            newX = newX + speed * time()
        }
        
        setPosition(newX, currentPos[1], newZ)
    }
}
```

### Animated Rotating Platform

```renscript
script RotatingPlatform {
    rotationSpeed = 45.0
    initialY = 0
    bobAmount = 2.0
    bobSpeed = 2.0
    
    start {
        pos = position()
        initialY = pos[1]
    }
    
    update {
        currentTime = time()
        
        # Rotate around Y axis
        rotY = currentTime * rotationSpeed
        setRotation(0, rotY, 0)
        
        # Bob up and down
        bobOffset = sin(currentTime * bobSpeed) * bobAmount
        pos = position()
        setPosition(pos[0], initialY + bobOffset, pos[2])
    }
}
```

### Health System

```renscript
script HealthSystem {
    maxHealth = 100
    currentHealth = 100
    regenerationRate = 5.0
    lastDamageTime = 0
    regenerationDelay = 3.0
    
    start {
        currentHealth = maxHealth
        addTag("damageable")
        log("Health system initialized")
    }
    
    update {
        currentTime = time()
        
        # Regenerate health after delay
        if (currentHealth < maxHealth && currentTime - lastDamageTime > regenerationDelay) {
            currentHealth = currentHealth + regenerationRate * time()
            if (currentHealth > maxHealth) {
                currentHealth = maxHealth
            }
        }
        
        # Check for death
        if (currentHealth <= 0) {
            log("Object destroyed due to no health")
            dispose()
        }
    }
}
```

### Gamepad Controller

```renscript
script GamepadController {
    speed = 3.0
    rotationSpeed = 2.0
    gamepadIndex = 0
    
    start {
        addTag("gamepad_controlled")
        log("Gamepad controller started")
    }
    
    update {
        if (isGamepadConnected(gamepadIndex)) {
            # Get stick input
            leftStickX = leftX(gamepadIndex)
            leftStickY = leftY(gamepadIndex)
            rightStickX = rightX(gamepadIndex)
            
            # Movement
            currentPos = position()
            moveX = leftStickX * speed * time()
            moveZ = leftStickY * speed * time() * -1  # Invert Y
            
            newX = currentPos[0] + moveX  
            newZ = currentPos[2] + moveZ
            setPosition(newX, currentPos[1], newZ)
            
            # Rotation
            currentRot = rotation()
            newRotY = currentRot[1] + rightStickX * rotationSpeed * time()
            setRotation(currentRot[0], newRotY, currentRot[2])
            
            # Jump button
            if (button(0, gamepadIndex)) {  # A button
                currentPos = position()
                setPosition(currentPos[0], currentPos[1] + 0.1, currentPos[2])
            }
        }
    }
}
```

## Best Practices

1. **Keep it Simple**: RenScript is designed for simple, clear logic
2. **Use Descriptive Names**: `playerSpeed` instead of `s`
3. **Add Comments**: Explain complex calculations
4. **Use Properties**: Make scripts configurable from the editor
5. **Handle Edge Cases**: Check for valid objects and values
6. **Optimize Update Logic**: Avoid expensive operations in `update {}`
7. **Use Lifecycle Functions**: Initialize in `start {}`, cleanup in `destroy {}`
8. **Test Thoroughly**: Verify scripts work with different object types

## Common Patterns

### State Management
```renscript
script StateMachine {
    currentState = "idle"
    
    update {
        if (currentState == "idle") {
            # Idle behavior
            if (isKeyPressed("SPACE")) {
                currentState = "jumping"
            }
        } elif (currentState == "jumping") {
            # Jump behavior
            # ...
            currentState = "idle"  # Return to idle
        }
    }
}
```

### Timer System
```renscript
script Timer {
    timer = 0.0
    interval = 5.0  # 5 seconds
    
    update {
        timer = timer + time()
        
        if (timer >= interval) {
            log("Timer expired!")
            # Do something every 5 seconds
            timer = 0.0  # Reset
        }
    }
}
```

### Distance-Based Behavior
```renscript
script ProximityTrigger {
    triggerDistance = 5.0
    playerTag = "player"
    
    update {
        players = findByTag(playerTag)
        if (players) {
            player = players[0]  # Get first player
            playerPos = player.position()
            myPos = position()
            
            dist = distance(myPos, playerPos)
            if (dist < triggerDistance) {
                log("Player is close!")
                # Trigger behavior
            }
        }
    }
}
```

This guide covers the essential aspects of RenScript programming based on the actual compiler implementation and supported features.