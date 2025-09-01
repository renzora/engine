# RenScript - Complete 3D Game Scripting API

RenScript is a comprehensive, modular scripting system built on **Babylon.js** that provides a complete API for 3D game development. It features 20 specialized modules covering everything from basic transforms to advanced physics, VR/AR, and visual effects.

## Table of Contents

- [Overview](#overview)
- [Basic Syntax](#basic-syntax)
- [Complete API Reference](#complete-api-reference)
- [Examples](#examples)
- [Best Practices](#best-practices)

## Overview

RenScript provides **777+ methods** across **20 specialized modules**, offering production-ready capabilities for:

- **3D Graphics** - Complete mesh creation, materials, textures, lighting
- **Animation** - Keyframe, skeletal, morph targets, procedural animation
- **Physics** - Full physics simulation with Havok/Cannon.js/Ammo.js
- **Audio** - 3D spatial audio with effects and synthesis
- **Input** - Keyboard, mouse, touch, gamepad support
- **UI/GUI** - Complete interface system with controls and layouts
- **Visual Effects** - Particle systems and post-processing
- **XR Support** - VR/AR with hand tracking and spatial anchors
- **Asset Management** - Loading and optimization of 3D assets
- **Developer Tools** - Debugging and performance monitoring

## Basic Syntax

```renscript
script MyGameObject {
  # Properties (appear in editor UI)
  props movement {
    speed: float {
      default: 2.0,
      min: 0.1,
      max: 10.0,
      description: "Movement speed"
    }
  }
  
  # Variables
  health = 100
  
  # Lifecycle methods
  start {
    log("Object initialized!")
    set_color(1.0, 0.0, 0.0)
  }
  
  update(dt) {
    rotate_by(0, dt * speed, 0)
  }
  
  destroy {
    log("Cleanup complete")
  }
}
```

## Complete API Reference

### 🔧 Core API - Essential Functions (40+ methods)
*Priority: HIGHEST - Used in almost every script*

#### Transform & Movement
```renscript
# Position Control
position(x, y, z)                # Set absolute position
getPosition()                    # Get current position [x, y, z]
worldPosition()                  # Get world space position
move(x, y, z)                   # Move relative to current
moveTo(x, y, z)                 # Move to target position

# Rotation Control  
rotation(x, y, z)               # Set absolute rotation
getRotation()                   # Get current rotation [x, y, z]
worldRotation()                 # Get world rotation quaternion
rotate(x, y, z)                 # Rotate relative to current
lookAt(x, y, z)                # Orient towards target

# Scale Control
scale(x, y, z)                  # Set scale factors
getScale()                      # Get current scale [x, y, z]
```

#### Visibility & State
```renscript
visible(state)                  # Show/hide object
isVisible()                     # Check visibility state
enabled(state)                  # Enable/disable object
isEnabled()                     # Check enabled state
name(newName)                   # Set object name
getName()                       # Get object name
id()                           # Get unique object ID
```

#### Tags & Metadata
```renscript
addTag(tag)                     # Add tag to object
removeTag(tag)                  # Remove specific tag
hasTag(tag)                     # Check if object has tag
getTags()                       # Get all tags array
metadata(key, value)            # Set metadata key-value
getMetadata(key)                # Get metadata value
hasMetadata(key)                # Check if metadata exists
removeMetadata(key)             # Remove metadata key
```

#### Time & Math Utilities
```renscript
# Time Functions
getTime()                       # Current time (milliseconds)
getDeltaTime()                  # Frame time delta

# Basic Math
random()                        # Random 0-1
randomRange(min, max)           # Random in range
clamp(value, min, max)          # Constrain value
lerp(start, end, t)            # Linear interpolation

# Vector Math
distance(x1,y1,z1, x2,y2,z2)   # Distance between points
normalize(x, y, z)              # Normalize vector
dot(x1,y1,z1, x2,y2,z2)        # Dot product
cross(x1,y1,z1, x2,y2,z2)      # Cross product
toRadians(degrees)              # Convert degrees to radians
toDegrees(radians)              # Convert radians to degrees

# Logging
log(message, ...)               # Console logging
```

---

### 🎨 Material API - Materials & Appearance (65+ methods)
*Priority: HIGH - Essential for visual appearance*

#### Material Creation
```renscript
# Basic Materials
standardMaterial(name, opts)            # Basic standard material
pbrMaterial(name, opts)                 # Physically-based rendering
pbrMetallicRoughnessMaterial(name, opts) # PBR metallic/roughness workflow
pbrSpecularGlossinessMaterial(name, opts) # PBR specular/glossiness workflow
unlitMaterial(name, opts)               # Performance unlit material
backgroundMaterial(name, opts)          # Skybox background material
nodeMaterial(name, opts)                # Node-based material editor
shaderMaterial(name, opts)              # Custom shader material
multiMaterial(name, opts)               # Multi-material for submeshes

# Specialized Materials
cellMaterial(name, opts)                # Toon/cel shading
customMaterial(name, opts)              # Custom material
pbrCustomMaterial(name, opts)           # Custom PBR material
simpleMaterial(name, opts)              # Lightweight simple material
shadowOnlyMaterial(name, opts)          # Shadow-only rendering
skyMaterial(name, opts)                 # Sky dome material
waterMaterial(name, opts)               # Water simulation material
terrainMaterial(name, opts)             # Multi-texture terrain blending
gridMaterial(name, opts)                # Grid overlay material
triplanarMaterial(name, opts)           # Triplanar projection material
mixMaterial(name, opts)                 # Texture mixing material
lavaMaterial(name, opts)                # Animated lava material
fireMaterial(name, opts)                # Fire effect material
furMaterial(name, opts)                 # Fur/hair rendering
gradientMaterial(name, opts)            # Color gradient material
```

#### Color & Properties
```renscript
# Color Control
color(r, g, b, a)                      # Set diffuse color
getColor()                             # Get current color
alpha(value)                           # Set transparency
getAlpha()                             # Get transparency value
diffuseColor(r, g, b)                  # Set diffuse color
specularColor(r, g, b)                 # Set specular color
emissiveColor(r, g, b)                 # Set emissive/glow color
ambientColor(r, g, b)                  # Set ambient color
getEmissiveColor()                     # Get emissive color
specularPower(power)                   # Set specular sharpness

# Material Properties
materialProperty(prop, value)          # Set any material property
getMaterialProperty(prop)              # Get property value
backFaceCulling(enabled)               # Enable/disable backface culling
disableLighting(disabled)              # Disable lighting calculations
wireframe(enabled)                     # Enable wireframe mode
pointsCloud(enabled)                   # Enable points cloud mode
fillMode(mode)                         # Set fill mode (solid/wireframe/points)
```

#### Advanced Rendering
```renscript
# Normal & Bump Mapping
invertNormalMapX(invert)               # Invert normal map X channel
invertNormalMapY(invert)               # Invert normal map Y channel
bumpLevel(level)                       # Set normal map intensity
parallaxScaleBias(scale, bias)         # Parallax occlusion mapping

# Refraction & Reflection
indexOfRefraction(ior)                 # Set index of refraction
fresnelParameters(bias, scale, power)  # Set Fresnel reflection parameters

# Dynamic Textures
dynamicTexture(name, options)          # Create dynamic texture
renderTargetTexture(name, options)     # Create render target texture
```

#### Texture System
```renscript
# Basic Texture Assignment
texture(url)                           # Set main texture
diffuseTexture(url)                    # Set diffuse map
normalTexture(url)                     # Set normal map
emissiveTexture(url)                   # Set emissive map
specularTexture(url)                   # Set specular map
ambientTexture(url)                    # Set ambient occlusion map
opacityTexture(url)                    # Set transparency map
reflectionTexture(url)                 # Set reflection map
refractionTexture(url)                 # Set refraction map
lightmapTexture(url)                   # Set lightmap texture

# PBR Texture Maps
metallicTexture(url)                   # Set metallic map
roughnessTexture(url)                  # Set roughness map
microRoughnessTexture(url)             # Set micro-roughness map
displacementTexture(url)               # Set displacement map
detailTexture(url)                     # Set detail texture
```

---

### 🖼️ Texture API - Texture Management (50+ methods)
*Priority: HIGH - Visual enhancement*

#### Basic Textures
```renscript
texture(url, options)                  # Load basic texture
cubeTexture(url, options)              # Skybox cube texture
hdrCubeTexture(url, options)           # HDR cube texture
videoTexture(url, options)             # Video as texture
mirrorTexture(options)                 # Real-time mirror texture
refractionTexture(options)             # Refraction texture
depthTexture(options)                  # Depth texture
dynamicTexture(name, options)          # Programmable texture
renderTargetTexture(name, options)     # Render-to-texture
```

#### Procedural Textures
```renscript
proceduralTexture(name, options)       # Base procedural texture
noiseTexture(name, options)            # Noise texture
woodTexture(name, options)             # Wood grain pattern
marbleTexture(name, options)           # Marble pattern
fireTexture(name, options)             # Animated fire texture
cloudTexture(name, options)            # Cloud pattern
grassTexture(name, options)            # Grass pattern
roadTexture(name, options)             # Road surface texture
brickTexture(name, options)            # Brick pattern
perlinNoiseTexture(name, options)      # Perlin noise texture
normalMapTexture(name, options)        # Generate normal map
```

---

### 🎭 Mesh API - 3D Geometry Creation (35+ methods)
*Priority: HIGH - Core 3D object creation*

#### Basic Geometry
```renscript
box(name, options)                      # Create box/cube mesh
sphere(name, options)                   # Create sphere mesh
cylinder(name, options)                 # Create cylinder mesh
plane(name, options)                    # Create flat plane
ground(name, options)                   # Create ground plane
torus(name, options)                    # Create torus/donut shape
tube(name, path, options)               # Create tube along path
ribbon(name, paths, options)            # Create ribbon mesh
lathe(name, shape, options)             # Revolve shape around axis
extrusion(name, shape, path, options)   # Extrude shape along path
polygon(name, shape, options)           # Create polygon mesh
icosphere(name, options)                # Create icosphere
capsule(name, options)                  # Create capsule shape
text(name, text, options)               # Create 3D text mesh
decal(name, options)                    # Create decal mesh
```

#### Advanced Mesh Types
```renscript
lineSystem(name, lines, options)        # Create line system
dashedLines(name, points, options)      # Create dashed lines
trail(name, generator, options)         # Create trail mesh
```

#### CSG Operations
```renscript
csg(mesh)                              # Convert mesh to CSG
csgUnion(mesh1, mesh2)                 # CSG union operation
csgSubtract(mesh1, mesh2)              # CSG subtraction
csgIntersect(mesh1, mesh2)             # CSG intersection
csgToMesh(csgObject, name)             # Convert CSG to mesh
```

#### Instancing & Performance
```renscript
instances(source, matrices)            # Create instances array
thinInstances(source, matrices)        # Create thin instances
updateInstanceData(instances, data)    # Update instance data
disposeInstances(instances)            # Dispose instances
getInstanceCount(instances)            # Get instance count

# Performance Optimization
freezeWorldMatrix()                    # Freeze world matrix updates
unfreezeWorldMatrix()                  # Unfreeze world matrix
renderingGroup(groupId)                # Set rendering group
getRenderingGroup()                    # Get rendering group ID
layerMask(mask)                        # Set layer mask
getLayerMask()                         # Get layer mask
```

#### Visual Enhancement
```renscript
edges(enabled)                         # Enable edge rendering
disableEdges()                         # Disable edge rendering
outline(enabled)                       # Enable outline rendering
disableOutline()                       # Disable outline
outlineColor(r, g, b)                  # Set outline color
outlineWidth(width)                    # Set outline width
```

---

### 🎬 Animation API - Animation System (85+ methods)  
*Priority: HIGH - Animation system*

#### Basic Animation Control
```renscript
# Animation Playback
animate(property, to, duration, ease)     # Basic property animation
stopAnimation(target)                     # Stop all animations
pauseAnimation(target)                    # Pause animations
resumeAnimation(target)                   # Resume animations
animateTo(target, properties, duration)   # Animate multiple properties
animatePosition(target, position, duration) # Animate position
animateRotation(target, rotation, duration) # Animate rotation
animateScale(target, scale, duration)     # Animate scale
animateColor(target, color, duration)     # Animate color
animateAlpha(target, alpha, duration)     # Animate transparency

# Animation State
isAnimating(target)                       # Check if animating
getActiveAnimations(target)               # Get active animations
getAnimationProgress(target)              # Get animation progress
animationSpeed(speed)                     # Set animation speed
getAnimationSpeed()                       # Get animation speed
```

#### Keyframe Animation System
```renscript
# Animation Creation
createAnimation(name, property, frameRate) # Create keyframe animation
createVectorAnimation(name, property)      # Vector3 keyframe animation
createColorAnimation(name, property)       # Color keyframe animation
createQuaternionAnimation(name, property)  # Quaternion keyframe animation

# Keyframe Management
addAnimationKeys(animation, keyframes)     # Add keyframes to animation
parseAnimationValue(value, type)          # Parse animation value

# Animation Playback
playAnimation(target, animations)          # Play animation set
stopAnimation(target)                      # Stop animations
pauseAnimation(target)                     # Pause animations
restartAnimation(target)                   # Restart animations
```

#### Animation Groups
```renscript
animationGroup(name)                       # Create animation group
addToAnimationGroup(group, animation)     # Add animation to group
addAnimationToGroup(group, animation)     # Add animation to group
playAnimationGroup(group)                 # Play animation group
stopAnimationGroup(group)                 # Stop animation group  
pauseAnimationGroup(group)                # Pause animation group
resetAnimationGroup(group)                # Reset animation group
```

#### Easing Functions
```renscript
bezierEase(x1, y1, x2, y2)               # Bezier curve easing
circleEase(mode)                          # Circle easing
backEase(amplitude)                       # Back easing
bounceEase(bounces, bounciness)          # Bounce easing
elasticEase(oscillations, springiness)   # Elastic easing
exponentialEase(exponent)                # Exponential easing
powerEase(power)                         # Power easing
quadraticEase()                          # Quadratic easing
quarticEase()                            # Quartic easing
quinticEase()                            # Quintic easing
sineEase()                               # Sine easing
bezierCurveEase(x1, y1, x2, y2)         # Bezier curve easing
easingMode(mode)                         # Set easing mode (in/out/inout)
```

#### Skeleton & Bone Animation
```renscript
# Skeleton Management
hasSkeleton()                            # Check if has skeleton
getSkeleton()                            # Get mesh skeleton
getBoneCount()                           # Get bone count
getBone(index)                           # Get bone by index
getBoneByName(name)                      # Get bone by name
bonePosition(bone, position)             # Set bone position
boneRotation(bone, rotation)             # Set bone rotation
getBonePosition(bone)                    # Get bone position
getBoneRotation(bone)                    # Get bone rotation

# Skeleton Animation
skeleton(name, bones)                    # Create skeleton
playSkeletonAnimation(skeleton, name)    # Play skeleton animation
stopSkeletonAnimation(skeleton)          # Stop skeleton animation

# Animation Ranges
animationRange(name, start, end)         # Create animation range
deleteAnimationRange(name)              # Delete animation range
getSkeletonAnimationRanges()             # Get all animation ranges
playAnimationRange(name)                 # Play animation by name
stopAnimationRange(name)                # Stop animation range

# Bone Manipulation
boneTransform(bone, matrix)              # Set bone transform matrix
getBoneWorldMatrix(bone)                 # Get bone world matrix
attachMeshToBone(mesh, bone)             # Attach mesh to bone
```

#### Character Animation Presets
```renscript
walkAnimation()                          # Play walk animation
runAnimation()                           # Play run animation
idleAnimation()                          # Play idle animation
jumpAnimation()                          # Play jump animation
crouchAnimation()                        # Play crouch animation
customAnimation(name)                    # Play custom animation
```

#### Animation Blending & Mixing
```renscript
blendAnimations(anim1, anim2, factor)    # Blend two animations
animationWeight(animation, weight)       # Set animation weight
blendToAnimation(animation, duration)    # Blend to animation
crossfadeAnimation(from, to, duration)   # Crossfade animations

# Animation State Queries
isAnimationPlaying(name)                 # Check if animation playing
getCurrentAnimation()                    # Get current animation name
getAnimationTime()                       # Get animation time
animationTime(time)                      # Set animation time
getAnimationInfo(name)                   # Get animation information
```

#### Morph Target Animation
```renscript
morphTargetManager(mesh)                 # Create morph target manager
addMorphTarget(mesh, name, positions)    # Add morph target
morphTargetInfluence(mesh, index, value) # Set morph influence
animateMorphTarget(mesh, index, influence) # Animate morph target
```

#### Advanced Animation Features
```renscript
# Procedural Animation
animateAlongPath(mesh, path, duration)   # Animate along path
animateRotationAroundAxis(mesh, axis, speed) # Rotate around axis
animateOpacity(mesh, from, to, duration)  # Animate opacity

# Animation Events
addAnimationEvent(animation, frame, callback) # Add animation event
removeAnimationEvents(animation)          # Remove animation events

# Animation Curves
animationCurve(points)                    # Create animation curve
getCurvePoint(curve, t)                   # Get curve point at t
getCurveTangent(curve, t)                 # Get curve tangent

# Advanced Features
animateWithPhysics(mesh, force)          # Physics-based animation
onAnimationComplete(callback)            # Animation complete event
onAnimationLoop(callback)                # Animation loop event
getAllAnimations()                       # Get all animations
playAnimationByName(name)                # Play animation by name
```

---

### ⚡ Physics API - Havok Physics V2 Simulation (45+ methods)
*Priority: HIGH - Realistic object behavior using Babylon.js Physics V2 with Havok engine*

#### Physics Engine Control
```renscript
physics(engine, gravity)                 # Enable physics engine
disablePhysics()                         # Disable physics engine
isPhysicsEnabled()                       # Check if physics active
gravity(x, y, z)                         # Set world gravity
getGravity()                             # Get gravity vector
pausePhysics()                           # Pause physics simulation
resumePhysics()                          # Resume physics simulation
physicsTimeStep(timeStep)                # Set physics time step
physicsDebug(enabled)                    # Enable physics debug visualization
disablePhysicsDebug()                    # Disable physics debug
disposePhysics()                         # Dispose physics engine
```

#### Physics Bodies & Aggregates (V2 API)
```renscript
physicsAggregate(type, options)          # Create physics aggregate (V2 API)
removePhysicsAggregate()                 # Remove physics aggregate
hasPhysicsAggregate()                    # Check if has physics body

# Mass & Material Properties
mass(value)                              # Set object mass
getMass()                                # Get object mass
friction(value)                          # Set surface friction
getFriction()                            # Get friction value
restitution(value)                       # Set bounciness/elasticity
getRestitution()                         # Get restitution value
physicsMaterial(material)                # Create physics material
setPhysicsMaterial(material)             # Apply physics material
```

#### Forces & Velocities
```renscript
impulse(force, contactPoint)             # Apply instant impulse force
force(force, contactPoint)               # Apply continuous force
linearVelocity(x, y, z)                 # Set linear velocity
getLinearVelocity()                      # Get linear velocity
angularVelocity(x, y, z)                # Set angular velocity
getAngularVelocity()                     # Get angular velocity
```

#### Collision Detection & Events
```renscript
onCollisionEnter(callback)               # Collision start event
onCollisionExit(callback)                # Collision end event
physicsRaycast(origin, direction, max)   # Physics raycast test
```

#### Constraints & Joints
```renscript
physicsJoint(type, options)             # Create physics joint
removePhysicsJoint(joint)               # Remove physics joint
```

#### Character Physics
```renscript
characterController(options)            # Create character controller
moveCharacter(movement)                  # Move character controller
jumpCharacter(force)                     # Make character jump
```

#### Advanced Physics Features
```renscript
ragdoll(enabled)                         # Enable ragdoll physics
disableRagdoll()                         # Disable ragdoll
softBody(enabled)                        # Enable soft body physics
softBodyProperties(properties)           # Set soft body properties
```

---

### 🎮 Input API - Input Handling (50+ methods)
*Priority: HIGH - User interaction*

#### Keyboard Input
```renscript
# Basic Key Detection
isKeyPressed(key)                        # Check if key pressed this frame
isKeyDown(key)                          # Check if key held down
isAnyKeyPressed()                       # Check if any key pressed
getPressedKeys()                        # Get array of pressed keys
isKeyComboPressed(keys)                 # Check key combination

# Modifier Keys
isCtrlPressed()                         # Check Ctrl modifier key
isShiftPressed()                        # Check Shift modifier key
isAltPressed()                          # Check Alt modifier key

# Keyboard Events
onKeyDown(callback)                     # Key press event handler
onKeyUp(callback)                       # Key release event handler
```

#### Mouse Input
```renscript
# Mouse Button Detection
isMousePressed(button)                  # Check mouse button pressed
isLeftMouse()                           # Check left mouse button
isRightMouse()                          # Check right mouse button
isMiddleMouse()                         # Check middle mouse button

# Mouse Position & Movement
mousePosition()                         # Get mouse screen coordinates
mouseX()                               # Get mouse X coordinate
mouseY()                               # Get mouse Y coordinate
mouseNormalized()                      # Get normalized coordinates (0-1)

# Mouse Events & Control
onMouseDown(callback)                  # Mouse button press event
onMouseUp(callback)                    # Mouse button release event
pointerLock()                          # Request pointer lock
exitPointerLock()                      # Exit pointer lock
isPointerLocked()                      # Check pointer lock state
```

#### Touch Input
```renscript
touchCount()                           # Get number of active touches
getTouches()                           # Get all touch data
getTouch(index)                        # Get specific touch
isTouching()                           # Check if any touch active
pinchDistance()                        # Get pinch gesture distance
touchCenter()                          # Get center point of touches
```

#### Gamepad Input
```renscript
# Gamepad Connection
gamepads()                             # Get all connected gamepads
gamepad(index)                         # Get specific gamepad
isGamepadConnected(index)              # Check if gamepad connected

# Button Input
button(index, buttonId)                # Check gamepad button
buttonValue(index, buttonId)           # Get button pressure value
isButtonA(index)                       # Check A button (Xbox)
isButtonB(index)                       # Check B button (Xbox)
isButtonX(index)                       # Check X button (Xbox)
isButtonY(index)                       # Check Y button (Xbox)

# Analog Sticks
leftStick(index)                       # Get left stick values [x, y]
rightStick(index)                      # Get right stick values [x, y]
leftX(index)                           # Get left stick X axis
leftY(index)                           # Get left stick Y axis
rightX(index)                          # Get right stick X axis
rightY(index)                          # Get right stick Y axis

# Triggers
leftTrigger(index)                     # Get left trigger value
rightTrigger(index)                    # Get right trigger value
trigger(index, triggerId)              # Get specific trigger value

# Deadzone & Calibration
deadzone(value, deadzone)              # Apply deadzone to analog value
leftStickDeadzone(index, deadzone)     # Get left stick with deadzone
rightStickDeadzone(index, deadzone)    # Get right stick with deadzone

# Haptic Feedback
vibrate(index, weak, strong, duration) # Vibrate gamepad motors

# Advanced Input
virtualJoystick(options)               # Create virtual joystick for mobile
inputSnapshot()                        # Capture current input state
```

---

### 🎯 Scene API - Scene Management & Queries (30+ methods)
*Priority: HIGH - Object interaction and scene management*

#### Object Finding & Queries
```renscript
findByName(name)                        # Find object by name
findObjectsByName(name)                 # Find all objects by name pattern  
findByTag(tag)                          # Find objects by tag
findWithTag(tag)                        # Find objects with tag
getAllMeshes()                          # Get all scene meshes
getAllLights()                          # Get all scene lights
getAllCameras()                         # Get all scene cameras
```

#### Spatial Queries & Raycasting
```renscript
raycast(origin, direction, maxDistance) # Cast ray and get intersection
cameraRaycast(x, y)                     # Raycast from camera through screen point
multiRaycast(rays)                      # Cast multiple rays
pick(x, y)                             # Pick object at screen coordinates
pickObjects(x, y)                       # Pick multiple objects
```

#### Spatial Proximity
```renscript
getInRadius(position, radius)           # Get objects in radius
getInBox(min, max)                      # Get objects in bounding box
getClosest(position, tag)               # Get closest object to position
intersectsMesh(mesh1, mesh2)           # Test mesh intersection
intersectsPoint(point)                  # Test point intersection
getBoundingInfo()                       # Get object bounding information
```

#### Object Management
```renscript
dispose()                              # Remove object from scene
clone(newName)                         # Clone object with new name
isInCameraView()                       # Check if object visible to camera
occlusionQuery(enabled)                # Enable occlusion culling
addLodLevel(distance, mesh)            # Add level-of-detail
removeLodLevel(distance)               # Remove LOD level
```

#### Scene Information
```renscript
sceneInfo()                            # Get complete scene statistics
performanceMonitor(enabled)            # Enable performance monitoring
disablePerformanceMonitor()            # Disable performance monitoring
performanceData()                      # Get performance metrics
```

---

### 📷 Camera API - Camera Control (25+ methods)
*Priority: HIGH - Camera control and viewport management*

#### Camera Management
```renscript
isCamera()                             # Check if object is camera
getActiveCamera()                      # Get current active camera
cameraPosition(x, y, z)                # Set camera position
getCameraPosition()                    # Get camera position
cameraTarget(x, y, z)                  # Set camera target
getCameraTarget()                      # Get camera target
cameraRotation(x, y, z)                # Set camera rotation
getCameraRotation()                    # Get camera rotation
```

#### Camera Properties
```renscript
cameraFov(fov)                         # Set field of view
getCameraFov()                         # Get field of view
cameraType(type)                       # Set camera type
cameraRadius(radius)                   # Set orbit camera radius
getCameraRadius()                      # Get orbit camera radius
```

#### Camera Types
```renscript
arcRotateCamera(name, alpha, beta, radius, target) # Create arc rotate camera
freeCamera(name, position)             # Create free camera
universalCamera(name, position)        # Create universal camera
flyCamera(name, position)              # Create fly camera
followCamera(name, target)             # Create follow camera
deviceOrientationCamera(name, position) # Create device orientation camera
virtualJoysticksCamera(name, position) # Create virtual joystick camera
webvrFreeCamera(name, position)        # Create WebVR camera
vrDeviceOrientationCamera(name, pos)   # Create VR device camera
```

#### Camera Control
```renscript
orbitCamera(alpha, beta, radius)       # Orbit camera around target
detachCameraControls()                 # Detach camera controls
attachCameraControls()                 # Attach camera controls
```

---

### 💡 Lighting API - Light Management (30+ methods)
*Priority: HIGH - Scene lighting and shadows*

#### Light Management
```renscript
isLight()                              # Check if object is light
lightIntensity(intensity)              # Set light intensity
getLightIntensity()                    # Get light intensity
lightColor(r, g, b)                    # Set light color
getLightColor()                        # Get light color
lightRange(range)                      # Set light range
getLightRange()                        # Get light range
ensureLight()                          # Ensure object is a light
lightPosition(x, y, z)                 # Set light position
lightDirection(x, y, z)                # Set light direction
lightSpecular(r, g, b)                 # Set specular light color
```

#### Light Types
```renscript
directionalLight(name, direction)      # Create directional light
hemisphericLight(name, direction)      # Create hemispheric light
pointLight(name, position)             # Create point light
spotLight(name, position, direction)   # Create spot light
hemisphericGroundColor(r, g, b)        # Set hemispheric ground color
```

#### Scene Lighting
```renscript
sceneExposure(exposure)                # Set scene exposure
```

#### Shadow System
```renscript
shadowEnabled(enabled)                 # Enable/disable shadows
shadowDarkness(darkness)               # Set shadow darkness
shadowBias(bias)                       # Set shadow bias
shadowQuality(quality)                 # Set shadow quality
shadowSoftness(softness)               # Set shadow softness
```

#### Skybox System
```renscript
ensureSkybox()                         # Ensure skybox exists
skyboxColors(top, horizon, ground)     # Set skybox gradient colors
skyboxTexture(url)                     # Set skybox texture
skyboxSize(size)                       # Set skybox size
skyboxEnabled(enabled)                 # Enable/disable skybox
skyboxInfinite(infinite)               # Set skybox infinite distance
```

---

### ✨ Particle API - Visual Effects (20+ methods)
*Priority: MEDIUM - Visual effects and particle systems*

#### Particle System Creation
```renscript
particleSystem(name, capacity)            # Create basic particle system
gpuParticleSystem(name, capacity)         # Create GPU particle system
solidParticleSystem(name, mesh, nb)       # Create solid particle system
pointsCloudSystem(name, nb)              # Create points cloud system
```

#### Particle Control
```renscript
startParticles(system)                    # Start particle emission
stopParticles(system)                     # Stop particle emission
particleEmissionRate(system, rate)        # Set emission rate
particleLifeTime(system, min, max)        # Set particle lifetime
particleSize(system, min, max)            # Set particle size range
particleColor(system, color1, color2)     # Set particle color range
particleVelocity(system, min, max)        # Set velocity range
particleGravity(system, gravity)          # Set gravity effect
particleTexture(system, texture)          # Set particle texture
```

---

### 🔊 Audio API - 3D Audio System (20+ methods)
*Priority: MEDIUM - Audio and sound effects*

#### Sound Creation & Playback
```renscript
sound(name, url, options)                 # Create basic sound
spatialSound(name, url, options)          # Create 3D spatial sound
soundTrack(name, sounds)                  # Create sound track
playSound(name)                           # Play sound
stopSound(name)                           # Stop sound
soundVolume(name, volume)                 # Set sound volume
```

#### 3D Audio Positioning
```renscript
soundPosition(name, x, y, z)              # Set sound position in 3D space
soundMaxDistance(name, distance)          # Set maximum hearing distance
soundRolloffFactor(name, factor)          # Set distance rolloff factor
```

#### Audio Analysis
```renscript
audioAnalyser(sound)                      # Create audio analyser
audioFrequencyData(analyser)              # Get frequency data
audioTimeData(analyser)                   # Get time domain data
```

---

### 🖥️ GUI API - User Interface (50+ methods)
*Priority: MEDIUM - User interface creation*

#### 2D GUI System
```renscript
guiTexture(name, size)                    # Create GUI texture/layer
guiButton(name, text)                     # Create button control
guiTextBlock(name, text)                  # Create text display
guiStackPanel(name, vertical)             # Create stack layout panel
guiRectangle(name)                        # Create rectangle shape
guiEllipse(name)                          # Create ellipse shape
guiLine(name)                             # Create line shape
guiSlider(name, min, max, value)          # Create slider control
guiCheckbox(name, checked)                # Create checkbox
guiRadioButton(name, group)               # Create radio button
guiInputText(name, placeholder)           # Create text input
guiPassword(name, placeholder)            # Create password input
guiScrollViewer(name)                     # Create scroll viewer
guiVirtualKeyboard(name)                  # Create virtual keyboard
guiImage(name, url)                       # Create image control
```

#### 3D GUI System
```renscript
gui3dManager()                            # Create 3D GUI manager
cylinderPanel(name, radius)               # Create cylinder panel
planePanel(name, width, height)           # Create plane panel
spherePanel(name, radius)                 # Create sphere panel
stackPanel3d(name, vertical)              # Create 3D stack panel
button3d(name, text)                      # Create 3D button
holographicButton(name, text)             # Create holographic button
meshButton3d(name, mesh)                  # Create mesh button
```

---

### 🎬 Post-Processing API - Visual Effects (25+ methods)
*Priority: MEDIUM - Visual enhancement and effects*

#### Rendering Pipelines
```renscript
defaultRenderingPipeline(name)            # Create default rendering pipeline
ssaoRenderingPipeline(name)               # Create SSAO pipeline
ssao2RenderingPipeline(name)              # Create SSAO2 pipeline
standardRenderingPipeline(name)           # Create standard pipeline
lensRenderingPipeline(name)               # Create lens effects pipeline
```

#### Post-Processing Effects
```renscript
addPostProcess(effect)                    # Add post-process effect
removePostProcess(effect)                 # Remove post-process effect
blurPostProcess(name, direction, kernel)  # Create blur effect
blackAndWhitePostProcess(name)            # Create B&W effect
convolutionPostProcess(name, kernel)      # Create convolution effect
filterPostProcess(name, matrix)           # Create filter effect
fxaaPostProcess(name)                     # Create FXAA anti-aliasing
highlightsPostProcess(name)               # Create highlights effect
refractionPostProcess(name, texture)      # Create refraction effect
volumetricLightPostProcess(name, mesh)    # Create volumetric light
colorCorrectionPostProcess(name, table)   # Create color correction
tonemapPostProcess(name, exposure)        # Create tone mapping
imageProcessingPostProcess(name)          # Create image processing
```

---

### 🥽 XR/VR/AR API - Extended Reality (15+ methods)
*Priority: LOW - VR/AR support*

#### WebXR System
```renscript
webxrDefaultExperience(options)           # Create WebXR default experience
webxrExperienceHelper(options)            # Create WebXR helper
webxr(options)                            # Enable WebXR
disableWebxr()                            # Disable WebXR
isWebxrAvailable()                        # Check WebXR availability
isWebxrSessionActive()                    # Check if XR session active
webxrControllers()                        # Get XR controllers
webxrInputSources()                       # Get XR input sources
teleportInXr(position)                    # Teleport in XR
```

#### Hand Tracking
```renscript
handTracking(enabled)                     # Enable hand tracking
disableHandTracking()                     # Disable hand tracking
```

### 🎭 Behavior API - Object Behaviors (15+ methods)
*Priority: MEDIUM - Advanced object behaviors*

#### Behavior System
```renscript
autoRotationBehavior(options)             # Add auto-rotation behavior
bouncingBehavior(options)                 # Add bouncing behavior
framingBehavior(options)                  # Add camera framing behavior
attachToBoxBehavior(faceVector)           # Add attach-to-box behavior
fadeInOutBehavior(options)                # Add fade in/out behavior
multiPointerScaleBehavior()               # Add multi-pointer scaling
pointerDragBehavior(options)              # Add pointer drag behavior
sixDofDragBehavior()                      # Add 6DOF drag behavior
removeBehavior(behavior)                  # Remove specific behavior
getBehaviors()                            # Get all behaviors
```

---

### 🎛️ Gizmo API - Manipulation Tools (10+ methods)
*Priority: LOW - Development tools*

#### Gizmo System
```renscript
gizmoManager()                            # Create gizmo manager
positionGizmo(options)                    # Create position gizmo
rotationGizmo(options)                    # Create rotation gizmo
scaleGizmo(options)                       # Create scale gizmo
boundingBoxGizmo(options)                 # Create bounding box gizmo
gizmos(enabled)                           # Enable/disable all gizmos
disableGizmos()                           # Disable all gizmos
```

---

### 🎨 Layer API - Rendering Layers (15+ methods)
*Priority: MEDIUM - Visual effects layers*

#### Effect Layers
```renscript
layer(name, scene)                        # Create basic layer
highlightLayer(name, scene)               # Create highlight layer
glowLayer(name, scene)                    # Create glow layer
effectLayer(name, scene)                  # Create effect layer
addToHighlightLayer(mesh, color)          # Add to highlight layer
removeFromHighlightLayer(mesh)            # Remove from highlight
addToGlowLayer(mesh, color)              # Add to glow layer
removeFromGlowLayer(mesh)                # Remove from glow
```

---

### 🖼️ Sprite API - 2D Sprite System (10+ methods)
*Priority: LOW - 2D sprite management*

#### Sprite System
```renscript
sprite(name, manager)                     # Create sprite
spriteManager(name, imgUrl, capacity)     # Create sprite manager
spriteMap(name, options)                  # Create sprite map
spriteTexture(sprite, texture)            # Set sprite texture
spriteFrame(sprite, frame)                # Set sprite frame
animateSprite(sprite, from, to, loop)     # Animate sprite frames
disposeSprite(sprite)                     # Dispose sprite
```

---

### 🦴 Morph Target API - Vertex Animation (10+ methods)
*Priority: MEDIUM - Advanced animation*

#### Morph Target System
```renscript
morphTarget(name, mesh)                   # Create morph target
morphTargetManager(mesh)                  # Create morph manager
addMorphTarget(manager, target)           # Add morph target
removeMorphTarget(manager, target)        # Remove morph target
morphTargetInfluence(target, influence)   # Set influence value
getMorphTargetInfluence(target)           # Get influence value
```

---

### 🧭 Navigation API - Pathfinding (10+ methods)
*Priority: LOW - AI navigation*

#### Navigation & Crowd
```renscript
navigationMesh(mesh)                      # Create navigation mesh
findPath(start, end)                      # Find path between points
crowd(maxAgents, maxRadius, scene)        # Create crowd simulation
addAgentToCrowd(agent, position)          # Add agent to crowd
removeAgentFromCrowd(agent)               # Remove agent from crowd
agentDestination(agent, destination)      # Set agent destination
getAgentPosition(agent)                   # Get agent position
getAgentVelocity(agent)                   # Get agent velocity
```

---

### 📦 Asset Loading API - Asset Management (20+ methods)
*Priority: HIGH - Asset management*

#### Asset Loading
```renscript
loadMesh(name, rootUrl, sceneFile)        # Load mesh file
loadGltf(name, rootUrl, sceneFile)        # Load GLTF/GLB file
loadAssetContainer(rootUrl, sceneFile)    # Load as container
importMesh(names, rootUrl, sceneFile)     # Import specific meshes
appendScene(rootUrl, sceneFile)           # Append to current scene
```

#### Asset Manager
```renscript
assetsManager()                           # Create assets manager
meshTask(manager, name, rootUrl, file)    # Add mesh task
textureTask(manager, name, url)           # Add texture task
loadAllAssets(manager)                    # Load all queued assets
```

#### Asset Operations
```renscript
mergeModelWithSkeleton(model, skeleton)   # Merge model with skeleton
loadAndMergeAssets(assets)                # Load and merge assets
getLoadedAsset(name)                      # Get loaded asset
getLoadedMesh(name)                       # Get loaded mesh
getLoadedAnimations(name)                 # Get loaded animations
getLoadedSkeleton(name)                   # Get loaded skeleton
```

---

### 💾 Serialization API - Export & Import (10+ methods)
*Priority: LOW - Data export*

#### Export Functions
```renscript
serializeScene()                          # Serialize scene to JSON
exportGltf(filename, scene)               # Export to GLTF format
exportObj(filename, meshes)               # Export to OBJ format
exportStl(filename, mesh)                 # Export to STL format
exportUsdz(filename, scene)               # Export to USDZ format
exportSplat(filename, pointCloud)         # Export to Splat format
```

---

### 🔧 Advanced APIs - Specialized Systems (40+ methods)
*Priority: LOW to MEDIUM - Advanced features*

#### Compute Shaders
```renscript
computeShader(name, code)                 # Create compute shader
computeEffect(name, defines)              # Create compute effect
dispatchCompute(x, y, z)                  # Dispatch compute shader
computeUniform(name, value)               # Set compute uniform
getComputeBuffer(name)                    # Get compute buffer
```

#### Flow Graph System
```renscript
flowGraph(name)                           # Create flow graph
flowGraphBlock(type, inputs, outputs)     # Add flow graph block
connectFlowGraphNodes(out, in)            # Connect flow nodes
executeFlowGraph(graph)                   # Execute flow graph
```

#### Frame Graph System
```renscript
frameGraph(name)                          # Create frame graph
frameGraphTask(task, dependencies)        # Add frame graph task
executeFrameGraph(graph)                  # Execute frame graph
```

#### Advanced Rendering
```renscript
depthRenderer(enabled)                    # Enable depth renderer
geometryBufferRenderer(enabled)           # Enable G-buffer renderer
outlineRenderer(enabled)                  # Enable outline renderer
edgesRenderer(enabled)                    # Enable edges renderer
boundingBoxRenderer(enabled)              # Enable bbox renderer
utilityLayerRenderer(scene)               # Create utility layer
```

#### Environment Helpers
```renscript
environmentHelper(options)                # Create environment helper
photoDome(name, url, options)             # Create photo dome
videoDome(name, url, options)             # Create video dome
textureDome(name, texture, options)       # Create texture dome
```

#### Debug & Visualization
```renscript
axesViewer(size, scene)                   # Create axes viewer
boneAxesViewer(mesh, bone)                # Create bone axes viewer
skeletonViewer(skeleton, mesh)            # Create skeleton viewer
physicsViewer(scene)                      # Create physics viewer
rayHelper(ray)                           # Create ray helper
debugLayer(enabled)                       # Enable debug layer
disableDebugLayer()                       # Disable debug layer
showWorldAxes(size)                       # Show world axes
hideWorldAxes()                           # Hide world axes
```

#### Dynamic Properties
```renscript
dynamicProperty(name, type, options)      # Add dynamic property
updatePropertyOptions(name, options)      # Update property options
removeDynamicProperty(name)               # Remove dynamic property
getPropertyValue(name)                    # Get property value
propertyValue(name, value)                # Set property value
```

---


---

## Complete Method Count by Module

| Module | Methods | Priority | Description |
|--------|---------|----------|-------------|
| **Core API** | 40+ | HIGHEST | Transform, visibility, math, time, logging |
| **Material API** | 65+ | HIGH | Materials, colors, shaders, textures |
| **Texture API** | 50+ | HIGH | Texture creation, procedural, dynamic |
| **Mesh API** | 35+ | HIGH | 3D geometry creation and manipulation |
| **Animation API** | 85+ | HIGH | Keyframe, skeleton, morph, procedural animation |
| **Physics API** | 45+ | HIGH | Physics simulation, forces, collision |
| **Input API** | 50+ | HIGH | Keyboard, mouse, touch, gamepad input |
| **Scene API** | 30+ | HIGH | Scene queries, raycasting, object finding |
| **Camera API** | 25+ | HIGH | Camera control and viewport management |
| **Lighting API** | 30+ | HIGH | Scene lighting, shadows, skybox |
| **Particle API** | 20+ | MEDIUM | Particle effects, emitters, forces |
| **Audio API** | 20+ | MEDIUM | 3D audio, spatial sound, synthesis |
| **GUI API** | 50+ | MEDIUM | UI controls, layouts, events |
| **Post-Processing API** | 25+ | MEDIUM | Visual effects, post-processing |
| **Asset Loading API** | 20+ | HIGH | Asset loading, management, optimization |
| **XR/VR/AR API** | 15+ | LOW | VR/AR support, hand tracking |
| **Behavior API** | 15+ | MEDIUM | Object behaviors and interactions |
| **Gizmo API** | 10+ | LOW | Development manipulation tools |
| **Layer API** | 15+ | MEDIUM | Visual effects layers |
| **Sprite API** | 10+ | LOW | 2D sprite management |
| **Morph Target API** | 10+ | MEDIUM | Vertex animation |
| **Navigation API** | 10+ | LOW | AI pathfinding and crowd simulation |
| **Serialization API** | 10+ | LOW | Data export and import |
| **Advanced APIs** | 40+ | LOW-MEDIUM | Compute shaders, flow graphs, debug tools |

**Total: 777+ methods across 24 specialized modules**

## Examples

### Complete Feature Demo Script
```renscript
script CompleteDemo {
  props movement {
    speed: float {
      default: 2.0,
      min: 0.1,
      max: 10.0,
      description: "Movement speed"
    }
    
    auto_rotate: boolean {
      default: true,
      description: "Enable rotation"
    }
  }
  
  props appearance {
    color_cycle: range {
      default: 1.0,
      min: 0.1,
      max: 5.0,
      description: "Color cycle speed"
    }
    
    particle_intensity: range {
      default: 0.5,
      min: 0.0,
      max: 1.0,
      description: "Particle effect intensity"
    }
  }
  
  props physics {
    enable_physics: boolean {
      default: false,
      description: "Enable physics simulation"
    }
    
    bounce_factor: range {
      default: 0.8,
      min: 0.0,
      max: 1.0,
      description: "Physics bounciness"
    }
  }
  
  # Variables
  time_offset = 0
  particle_system = null
  
  start {
    log("Complete demo script started!")
    
    # Setup materials
    set_color(1.0, 0.5, 0.2)
    set_emissive_color(0.1, 0.05, 0.02)
    
    # Setup physics if enabled
    if enable_physics {
      physicsAggregate("box", {mass: 1, restitution: bounce_factor})
    }
    
    # Create particle effect
    particle_system = create_fire_particles("demo_fire", particle_intensity)
    attach_particles_to_mesh(particle_system, this)
    
    # Setup 3D audio
    create_3d_sound("ambient", "assets/ambient.wav", {loop: true, volume: 0.3})
    attach_sound_to_mesh("ambient", this)
    play_sound("ambient")
    
    # Add tags
    add_tag("demo")
    add_tag("interactive")
    
    # Enable debug info for this object
    show_mesh_info(this)
  }
  
  update(dt) {
    time_offset = time_offset + dt
    
    # Animated rotation
    if auto_rotate {
      rotate_by(0, dt * speed, dt * speed * 0.5)
    }
    
    # Dynamic color cycling
    time_factor = get_time() * 0.001 * color_cycle
    red = sin(time_factor) * 0.5 + 0.5
    green = sin(time_factor + 2.0) * 0.5 + 0.5
    blue = sin(time_factor + 4.0) * 0.5 + 0.5
    set_color(red, green, blue)
    
    # Floating animation
    y_offset = sin(time_factor * 2.0) * 0.5
    current_pos = get_position()
    set_position(current_pos[0], current_pos[1] + y_offset * dt, current_pos[2])
    
    # Input handling
    if is_key_pressed("Space") {
      apply_impulse([0, 10, 0], get_position())
      log("Jump!")
    }
    
    if is_key_pressed("R") {
      # Reset position
      set_position(0, 5, 0)
      log("Reset position")
    }
  }
  
  destroy {
    # Cleanup
    stop_sound("ambient")
    stop_particle_system(particle_system)
    log("Demo script destroyed")
  }
}
```

### Animation Controller
```renscript
script AnimationController {
  props character {
    walk_speed: float {
      default: 2.0,
      min: 0.5,
      max: 8.0,
      description: "Walking speed"
    }
    
    run_multiplier: range {
      default: 2.5,
      min: 1.0,
      max: 5.0,
      description: "Running speed multiplier"
    }
    
    jump_height: float {
      default: 5.0,
      min: 1.0,
      max: 15.0,
      description: "Jump force"
    }
  }
  
  # State variables
  current_animation = "idle"
  is_grounded = true
  
  start {
    # Setup physics for character
    physicsAggregate("capsule", {mass: 1})
    
    # Load animation presets
    play_idle_animation(this)
    
    # Setup sound effects
    create_3d_sound("footsteps", "assets/footsteps.wav", {loop: true})
    create_sound("jump_sound", "assets/jump.wav")
    
    add_tag("character")
    log("Character controller initialized")
  }
  
  update(dt) {
    # Input-driven movement
    move_x = 0
    move_z = 0
    
    if is_key_down("W") { move_z = move_z + 1 }
    if is_key_down("S") { move_z = move_z - 1 }
    if is_key_down("A") { move_x = move_x - 1 }
    if is_key_down("D") { move_x = move_x + 1 }
    
    # Calculate movement
    speed_multiplier = is_key_down("Shift") ? run_multiplier : 1.0
    actual_speed = walk_speed * speed_multiplier
    
    # Apply movement
    if move_x != 0 || move_z != 0 {
      move_by(move_x * actual_speed * dt, 0, move_z * actual_speed * dt)
      
      # Animation state
      new_anim = speed_multiplier > 1.5 ? "run" : "walk"
      if current_animation != new_anim {
        if new_anim == "run" {
          play_run_animation(this)
        } else {
          play_walk_animation(this)
        }
        current_animation = new_anim
      }
      
      # Sound effects
      if !is_sound_playing("footsteps") {
        play_sound("footsteps")
      }
    } else {
      # Idle state
      if current_animation != "idle" {
        play_idle_animation(this)
        current_animation = "idle"
        stop_sound("footsteps")
      }
    }
    
    # Jump
    if is_key_pressed("Space") && is_grounded {
      apply_impulse([0, jump_height, 0], get_position())
      play_jump_animation(this)
      play_sound("jump_sound")
      is_grounded = false
    }
    
    # Ground detection (simplified)
    pos = get_position()
    if pos[1] <= 1.0 && !is_grounded {
      is_grounded = true
    }
  }
}
```

## Architecture Benefits

1. **Comprehensive Coverage** - 777+ methods across 24 specialized modules covering every aspect of 3D development
2. **Modular Design** - Each API module is self-contained and focused on specific functionality
3. **Babylon.js Foundation** - Built on industry-standard 3D engine with full WebGL 2.0 support
4. **Performance Optimized** - GPU acceleration, instancing, thin instances, LOD systems, compute shaders
5. **Developer Friendly** - Extensive debugging tools, gizmos, and visualization helpers
6. **Future Ready** - WebXR support, compute shaders, flow graphs, advanced rendering pipelines
7. **Production Ready** - Asset loading, serialization, optimization, and performance monitoring
8. **Cross-Platform** - Web, mobile, VR/AR support with unified API

## System Requirements

- **Babylon.js** 6.0+ (core 3D engine)
- **@babylonjs/gui** (UI components)
- **@babylonjs/materials** (advanced materials)
- **@babylonjs/inspector** (debugging tools)
- **@babylonjs/post-processes** (visual effects)
- Modern browser with **WebGL 2.0** support
- Optional: **WebXR** for VR/AR features

---

*This documentation covers the complete RenScript API with all 777+ available methods across 24 specialized modules. The system provides professional-grade 3D development capabilities comparable to commercial game engines, with comprehensive support for 3D graphics, animation, physics, audio, VR/AR, and advanced rendering techniques.*