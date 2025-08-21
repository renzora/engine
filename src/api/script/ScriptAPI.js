import { Vector3, Vector2, Vector4 } from '@babylonjs/core/Maths/math.vector.js';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color.js';
import { Matrix, Quaternion } from '@babylonjs/core/Maths/math.vector.js';
import { Tools } from '@babylonjs/core/Misc/tools.js';
import { Animation } from '@babylonjs/core/Animations/animation.js';
import { AnimationGroup } from '@babylonjs/core/Animations/animationGroup.js';
import { Sound } from '@babylonjs/core/Audio/sound.js';
import { Ray } from '@babylonjs/core/Culling/ray.js';
import { PickingInfo } from '@babylonjs/core/Collisions/pickingInfo.js';

/**
 * ScriptAPI - Provides a safe interface for scripts to interact with Babylon.js objects
 * This wrapper ensures scripts can only access safe methods and properties
 */
class ScriptAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
    this._deltaTime = 0;
    
    // Bind methods to ensure proper context
    this.bindMethods();
  }
  
  // Property accessor for babylon object - allows updates
  get object() {
    return this.babylonObject;
  }
  
  bindMethods() {
    // Transform methods
    this.getPosition = this.getPosition.bind(this);
    this.setPosition = this.setPosition.bind(this);
    this.getRotation = this.getRotation.bind(this);
    this.setRotation = this.setRotation.bind(this);
    this.getScale = this.getScale.bind(this);
    this.setScale = this.setScale.bind(this);
    this.moveBy = this.moveBy.bind(this);
    this.rotateBy = this.rotateBy.bind(this);
    this.lookAt = this.lookAt.bind(this);
    
    // Material methods
    this.setColor = this.setColor.bind(this);
    this.getColor = this.getColor.bind(this);
    this.setMaterialProperty = this.setMaterialProperty.bind(this);
    this.getMaterialProperty = this.getMaterialProperty.bind(this);
    
    // Animation methods
    this.animate = this.animate.bind(this);
    this.stopAnimation = this.stopAnimation.bind(this);
    this.pauseAnimation = this.pauseAnimation.bind(this);
    this.resumeAnimation = this.resumeAnimation.bind(this);
    
    // Physics methods
    this.setPhysicsImpostor = this.setPhysicsImpostor.bind(this);
    this.applyImpulse = this.applyImpulse.bind(this);
    this.setLinearVelocity = this.setLinearVelocity.bind(this);
    this.setAngularVelocity = this.setAngularVelocity.bind(this);
    
    // Scene query methods
    this.findObjectByName = this.findObjectByName.bind(this);
    this.findObjectsByTag = this.findObjectsByTag.bind(this);
    this.raycast = this.raycast.bind(this);
    this.getObjectsInRadius = this.getObjectsInRadius.bind(this);
    
    // Audio methods
    this.playSound = this.playSound.bind(this);
    this.stopSound = this.stopSound.bind(this);
    this.setSoundVolume = this.setSoundVolume.bind(this);
    
    // Input methods
    this.isKeyPressed = this.isKeyPressed.bind(this);
    this.isMouseButtonPressed = this.isMouseButtonPressed.bind(this);
    this.getMousePosition = this.getMousePosition.bind(this);
    
    // Utility methods
    this.log = this.log.bind(this);
    this.getDeltaTime = this.getDeltaTime.bind(this);
    this.getTime = this.getTime.bind(this);
    this.random = this.random.bind(this);
    this.clamp = this.clamp.bind(this);
    this.lerp = this.lerp.bind(this);
  }
  
  /**
   * Update delta time (called by ScriptManager)
   */
  _updateDeltaTime(deltaTime) {
    this._deltaTime = deltaTime;
  }
  
  // === OBJECT TRANSFORM API ===
  
  /**
   * Get the position of the object
   * @returns {Array} [x, y, z] position array
   */
  getPosition() {
    if (!this.object.position) return [0, 0, 0];
    return [this.object.position.x, this.object.position.y, this.object.position.z];
  }
  
  /**
   * Set the position of the object
   * @param {number|Array} x - X coordinate or [x, y, z] array
   * @param {number} y - Y coordinate (if x is not array)
   * @param {number} z - Z coordinate (if x is not array)
   */
  setPosition(x, y, z) {
    if (!this.object.position) return;
    
    if (Array.isArray(x)) {
      this.object.position.x = x[0] || 0;
      this.object.position.y = x[1] || 0;
      this.object.position.z = x[2] || 0;
    } else {
      this.object.position.x = x || 0;
      this.object.position.y = y || 0;
      this.object.position.z = z || 0;
    }
  }
  
  /**
   * Get the rotation of the object in radians
   * @returns {Array} [x, y, z] rotation array
   */
  getRotation() {
    if (!this.object.rotation) return [0, 0, 0];
    return [this.object.rotation.x, this.object.rotation.y, this.object.rotation.z];
  }
  
  /**
   * Set the rotation of the object in radians
   * @param {number|Array} x - X rotation or [x, y, z] array
   * @param {number} y - Y rotation (if x is not array)
   * @param {number} z - Z rotation (if x is not array)
   */
  setRotation(x, y, z) {
    if (!this.object.rotation) return;
    
    if (Array.isArray(x)) {
      this.object.rotation.x = x[0] || 0;
      this.object.rotation.y = x[1] || 0;
      this.object.rotation.z = x[2] || 0;
    } else {
      this.object.rotation.x = x || 0;
      this.object.rotation.y = y || 0;
      this.object.rotation.z = z || 0;
    }
  }
  
  /**
   * Get the scale of the object
   * @returns {Array} [x, y, z] scale array
   */
  getScale() {
    if (!this.object.scaling) return [1, 1, 1];
    return [this.object.scaling.x, this.object.scaling.y, this.object.scaling.z];
  }
  
  /**
   * Set the scale of the object
   * @param {number|Array} x - X scale or [x, y, z] array
   * @param {number} y - Y scale (if x is not array)
   * @param {number} z - Z scale (if x is not array)
   */
  setScale(x, y, z) {
    if (!this.object.scaling) return;
    
    if (Array.isArray(x)) {
      this.object.scaling.x = x[0] || 1;
      this.object.scaling.y = x[1] || 1;
      this.object.scaling.z = x[2] || 1;
    } else {
      this.object.scaling.x = x || 1;
      this.object.scaling.y = y || 1;
      this.object.scaling.z = z || 1;
    }
  }
  
  /**
   * Move the object by a relative amount
   * @param {number|Array} x - X offset or [x, y, z] array
   * @param {number} y - Y offset (if x is not array)
   * @param {number} z - Z offset (if x is not array)
   */
  moveBy(x, y, z) {
    const currentPos = this.getPosition();
    
    if (Array.isArray(x)) {
      this.setPosition(
        currentPos[0] + (x[0] || 0),
        currentPos[1] + (x[1] || 0),
        currentPos[2] + (x[2] || 0)
      );
    } else {
      this.setPosition(
        currentPos[0] + (x || 0),
        currentPos[1] + (y || 0),
        currentPos[2] + (z || 0)
      );
    }
  }
  
  /**
   * Rotate the object by a relative amount
   * @param {number|Array} x - X rotation offset or [x, y, z] array
   * @param {number} y - Y rotation offset (if x is not array)
   * @param {number} z - Z rotation offset (if x is not array)
   */
  rotateBy(x, y, z) {
    const currentRot = this.getRotation();
    
    if (Array.isArray(x)) {
      this.setRotation(
        currentRot[0] + (x[0] || 0),
        currentRot[1] + (x[1] || 0),
        currentRot[2] + (x[2] || 0)
      );
    } else {
      this.setRotation(
        currentRot[0] + (x || 0),
        currentRot[1] + (y || 0),
        currentRot[2] + (z || 0)
      );
    }
  }
  
  // === VISIBILITY API ===
  
  /**
   * Get the visibility of the object
   * @returns {boolean} True if visible
   */
  isVisible() {
    if (this.object.isVisible !== undefined) {
      return this.object.isVisible;
    }
    if (this.object.isEnabled) {
      return this.object.isEnabled();
    }
    return true;
  }
  
  /**
   * Set the visibility of the object
   * @param {boolean} visible - Whether the object should be visible
   */
  setVisible(visible) {
    if (this.object.isVisible !== undefined) {
      this.object.isVisible = !!visible;
    } else if (this.object.setEnabled) {
      this.object.setEnabled(!!visible);
    }
  }
  
  // === MATERIAL API ===
  
  /**
   * Set the color of the object (if it has a material)
   * @param {number|Array} r - Red component (0-1) or [r, g, b] array
   * @param {number} g - Green component (0-1) (if r is not array)
   * @param {number} b - Blue component (0-1) (if r is not array)
   */
  setColor(r, g, b) {
    if (!this.object.material) return false;
    
    let red, green, blue;
    if (Array.isArray(r)) {
      red = r[0] || 0;
      green = r[1] || 0;
      blue = r[2] || 0;
    } else {
      red = r || 0;
      green = g || 0;
      blue = b || 0;
    }
    
    // Clamp values between 0 and 1
    red = Math.max(0, Math.min(1, red));
    green = Math.max(0, Math.min(1, green));
    blue = Math.max(0, Math.min(1, blue));
    
    if (this.object.material.diffuseColor) {
      this.object.material.diffuseColor = new Color3(red, green, blue);
    }
    
    return true;
  }
  
  // === SCENE QUERY API ===
  
  /**
   * Find an object in the scene by name
   * @param {string} name - Name of the object to find
   * @returns {Object|null} ScriptAPI wrapper for the found object, or null
   */
  findObjectByName(name) {
    // Search in all object types
    const allObjects = [
      ...this.scene.meshes,
      ...this.scene.transformNodes,
      ...this.scene.lights,
      ...this.scene.cameras
    ];
    
    const foundObject = allObjects.find(obj => obj.name === name);
    if (foundObject) {
      return new ScriptAPI(this.scene, foundObject);
    }
    
    return null;
  }
  
  // === TIME API ===
  
  /**
   * Get the delta time for this frame
   * @returns {number} Delta time in seconds
   */
  getDeltaTime() {
    return this._deltaTime;
  }
  
  /**
   * Get the current time since the scene started
   * @returns {number} Time in milliseconds
   */
  getTime() {
    // Use performance.now() for consistent timing
    return performance.now();
  }
  
  // === UTILITY API ===
  
  /**
   * Log a message to the console with script context
   * @param {...*} args - Arguments to log
   */
  log(...args) {
    console.log(`[Script:${this.object.name}]`, ...args);
  }
  
  /**
   * Create a Vector3 object
   * @param {number} x - X component
   * @param {number} y - Y component
   * @param {number} z - Z component
   * @returns {Vector3} Babylon.js Vector3 object
   */
  createVector3(x = 0, y = 0, z = 0) {
    return new Vector3(x, y, z);
  }
  
  /**
   * Create a Color3 object
   * @param {number} r - Red component (0-1)
   * @param {number} g - Green component (0-1)
   * @param {number} b - Blue component (0-1)
   * @returns {Color3} Babylon.js Color3 object
   */
  createColor3(r = 0, g = 0, b = 0) {
    return new Color3(r, g, b);
  }
  
  // === ENHANCED TRANSFORM API ===
  
  /**
   * Make the object look at a target position or object
   * @param {Array|Object} target - Target position [x,y,z] or object with getPosition()
   * @param {Array} up - Up vector [x,y,z] (optional, defaults to [0,1,0])
   */
  lookAt(target, up = [0, 1, 0]) {
    if (!this.object.lookAt) return false;
    
    let targetPos;
    if (Array.isArray(target)) {
      targetPos = new Vector3(target[0], target[1], target[2]);
    } else if (target.getPosition) {
      const pos = target.getPosition();
      targetPos = new Vector3(pos[0], pos[1], pos[2]);
    } else {
      return false;
    }
    
    const upVector = new Vector3(up[0], up[1], up[2]);
    this.object.lookAt(targetPos, 0, 0, 0, upVector);
    return true;
  }
  
  /**
   * Get world position (considering parent transforms)
   * @returns {Array} [x, y, z] world position
   */
  getWorldPosition() {
    if (!this.object.getAbsolutePosition) {
      return this.getPosition();
    }
    const worldPos = this.object.getAbsolutePosition();
    return [worldPos.x, worldPos.y, worldPos.z];
  }
  
  /**
   * Get world rotation as quaternion
   * @returns {Array} [x, y, z, w] quaternion
   */
  getWorldRotationQuaternion() {
    if (!this.object.rotationQuaternion) {
      const euler = this.getRotation();
      const quat = Quaternion.FromEulerAngles(euler[0], euler[1], euler[2]);
      return [quat.x, quat.y, quat.z, quat.w];
    }
    const quat = this.object.rotationQuaternion;
    return [quat.x, quat.y, quat.z, quat.w];
  }
  
  // === ENHANCED MATERIAL API ===
  
  /**
   * Get the color of the object
   * @returns {Array} [r, g, b] color array (0-1)
   */
  getColor() {
    if (!this.object.material || !this.object.material.diffuseColor) {
      return [1, 1, 1];
    }
    const color = this.object.material.diffuseColor;
    return [color.r, color.g, color.b];
  }
  
  /**
   * Set emissive color of the object (glow effect)
   * @param {number} r - Red component (0-1)
   * @param {number} g - Green component (0-1)
   * @param {number} b - Blue component (0-1)
   */
  setEmissiveColor(r, g, b) {
    if (!this.object.material) {
      this.log('No material found on object for setEmissiveColor');
      return false;
    }
    
    try {
      this.object.material.emissiveColor = new Color3(r, g, b);
      return true;
    } catch (error) {
      this.log('Failed to set emissive color:', error);
      return false;
    }
  }
  
  /**
   * Get emissive color of the object
   * @returns {Array} [r, g, b] emissive color array (0-1)
   */
  getEmissiveColor() {
    if (!this.object.material || !this.object.material.emissiveColor) {
      return [0, 0, 0];
    }
    const color = this.object.material.emissiveColor;
    return [color.r, color.g, color.b];
  }
  
  /**
   * Set material property
   * @param {string} property - Property name (roughness, metallic, emissiveColor, etc.)
   * @param {*} value - Property value
   */
  setMaterialProperty(property, value) {
    if (!this.object.material) return false;
    
    try {
      if (property.includes('Color') && Array.isArray(value)) {
        this.object.material[property] = new Color3(value[0], value[1], value[2]);
      } else {
        this.object.material[property] = value;
      }
      return true;
    } catch (error) {
      this.log('Failed to set material property:', property, error);
      return false;
    }
  }
  
  /**
   * Get material property
   * @param {string} property - Property name
   * @returns {*} Property value
   */
  getMaterialProperty(property) {
    if (!this.object.material) return null;
    
    const value = this.object.material[property];
    if (value && value.r !== undefined && value.g !== undefined && value.b !== undefined) {
      return [value.r, value.g, value.b];
    }
    return value;
  }
  
  // === ANIMATION API ===
  
  /**
   * Animate a property
   * @param {string} property - Property to animate (position, rotation, scale, etc.)
   * @param {Array} targetValue - Target value [x, y, z]
   * @param {number} duration - Animation duration in seconds
   * @param {string} easing - Easing function (linear, easeInOut, etc.)
   * @returns {Animation} Animation object
   */
  animate(property, targetValue, duration = 1.0, easing = 'easeInOut') {
    if (!this.scene.beginAnimation) return null;
    
    try {
      const frameRate = 60;
      const totalFrames = Math.floor(duration * frameRate);
      
      const animation = new Animation(
        `${property}Animation`,
        property,
        frameRate,
        Animation.ANIMATIONTYPE_VECTOR3,
        Animation.ANIMATIONLOOPMODE_CONSTANT
      );
      
      const keys = [];
      const startValue = this.object[property].clone();
      const endValue = new Vector3(targetValue[0], targetValue[1], targetValue[2]);
      
      keys.push({ frame: 0, value: startValue });
      keys.push({ frame: totalFrames, value: endValue });
      
      animation.setKeys(keys);
      
      // Set easing function
      switch (easing) {
        case 'easeInOut':
          animation.setEasingFunction(new BABYLON.CubicEase());
          break;
        case 'easeIn':
          animation.setEasingFunction(new BABYLON.QuadraticEase());
          break;
        case 'bounceOut':
          animation.setEasingFunction(new BABYLON.BounceEase());
          break;
      }
      
      this.object.animations = this.object.animations || [];
      this.object.animations.push(animation);
      
      const animatable = this.scene.beginAnimation(this.object, 0, totalFrames, false);
      return animatable;
      
    } catch (error) {
      this.log('Animation failed:', error);
      return null;
    }
  }
  
  /**
   * Stop all animations on this object
   */
  stopAnimation() {
    if (this.scene.stopAnimation) {
      this.scene.stopAnimation(this.object);
    }
  }
  
  /**
   * Pause all animations on this object
   */
  pauseAnimation() {
    if (this.scene.pauseAnimation) {
      this.scene.pauseAnimation(this.object);
    }
  }
  
  /**
   * Resume all animations on this object
   */
  resumeAnimation() {
    if (this.scene.resumeAnimation) {
      this.scene.resumeAnimation(this.object);
    }
  }
  
  // === PHYSICS API ===
  
  /**
   * Set physics impostor for the object
   * @param {string} type - Impostor type (box, sphere, cylinder, mesh, etc.)
   * @param {Object} options - Physics options {mass, restitution, friction}
   */
  setPhysicsImpostor(type = 'box', options = {}) {
    if (!this.scene.getPhysicsEngine) {
      this.log('Physics engine not available');
      return false;
    }
    
    try {
      // Handle both object and individual parameter style
      let physicsOptions;
      if (typeof options === 'object' && options !== null) {
        physicsOptions = {
          mass: options.mass || 1,
          restitution: options.restitution || 0.3,
          friction: options.friction || 0.8,
          ...options
        };
      } else {
        // Use defaults if no options provided
        physicsOptions = {
          mass: 1,
          restitution: 0.3,
          friction: 0.8
        };
      }
      
      let impostorType;
      switch (type.toLowerCase()) {
        case 'box': impostorType = BABYLON.PhysicsImpostor.BoxImpostor; break;
        case 'sphere': impostorType = BABYLON.PhysicsImpostor.SphereImpostor; break;
        case 'cylinder': impostorType = BABYLON.PhysicsImpostor.CylinderImpostor; break;
        case 'mesh': impostorType = BABYLON.PhysicsImpostor.MeshImpostor; break;
        default: impostorType = BABYLON.PhysicsImpostor.BoxImpostor; break;
      }
      
      this.object.physicsImpostor = new BABYLON.PhysicsImpostor(
        this.object,
        impostorType,
        physicsOptions,
        this.scene
      );
      
      return true;
    } catch (error) {
      this.log('Failed to set physics impostor:', error);
      return false;
    }
  }
  
  /**
   * Apply impulse to the object
   * @param {Array} force - Force vector [x, y, z]
   * @param {Array} contactPoint - Contact point [x, y, z] (optional)
   */
  applyImpulse(force, contactPoint = null) {
    if (!this.object.physicsImpostor) {
      this.log('No physics impostor found');
      return false;
    }
    
    const forceVector = new Vector3(force[0], force[1], force[2]);
    const contact = contactPoint ? new Vector3(contactPoint[0], contactPoint[1], contactPoint[2]) : null;
    
    this.object.physicsImpostor.applyImpulse(forceVector, contact || this.object.getAbsolutePosition());
    return true;
  }
  
  /**
   * Set linear velocity
   * @param {Array} velocity - Velocity vector [x, y, z]
   */
  setLinearVelocity(velocity) {
    if (!this.object.physicsImpostor) return false;
    
    const velocityVector = new Vector3(velocity[0], velocity[1], velocity[2]);
    this.object.physicsImpostor.setLinearVelocity(velocityVector);
    return true;
  }
  
  /**
   * Set angular velocity
   * @param {Array} velocity - Angular velocity [x, y, z]
   */
  setAngularVelocity(velocity) {
    if (!this.object.physicsImpostor) return false;
    
    const velocityVector = new Vector3(velocity[0], velocity[1], velocity[2]);
    this.object.physicsImpostor.setAngularVelocity(velocityVector);
    return true;
  }
  
  // === ENHANCED SCENE QUERY API ===
  
  /**
   * Find objects by tag
   * @param {string} tag - Tag to search for
   * @returns {Array} Array of ScriptAPI wrappers for found objects
   */
  findObjectsByTag(tag) {
    const results = [];
    const allObjects = [
      ...this.scene.meshes,
      ...this.scene.transformNodes,
      ...this.scene.lights,
      ...this.scene.cameras
    ];
    
    allObjects.forEach(obj => {
      if (obj.metadata && obj.metadata.tags && obj.metadata.tags.includes(tag)) {
        results.push(new ScriptAPI(this.scene, obj));
      }
    });
    
    return results;
  }
  
  /**
   * Perform raycast from object
   * @param {Array} direction - Ray direction [x, y, z]
   * @param {number} maxDistance - Maximum ray distance
   * @param {Array} excludeObjects - Objects to exclude from raycast
   * @returns {Object|null} Hit information {hit: boolean, distance: number, object: ScriptAPI}
   */
  raycast(direction, maxDistance = 100, excludeObjects = []) {
    const origin = this.object.getAbsolutePosition();
    const dir = new Vector3(direction[0], direction[1], direction[2]).normalize();
    const ray = new Ray(origin, dir, maxDistance);
    
    const hit = this.scene.pickWithRay(ray, (mesh) => {
      return !excludeObjects.includes(mesh) && mesh !== this.object;
    });
    
    if (hit && hit.hit) {
      return {
        hit: true,
        distance: hit.distance,
        point: [hit.pickedPoint.x, hit.pickedPoint.y, hit.pickedPoint.z],
        normal: hit.getNormal ? [hit.getNormal().x, hit.getNormal().y, hit.getNormal().z] : null,
        object: new ScriptAPI(this.scene, hit.pickedMesh)
      };
    }
    
    return { hit: false, distance: maxDistance, point: null, normal: null, object: null };
  }
  
  /**
   * Get objects within radius
   * @param {number} radius - Search radius
   * @param {Array} objectTypes - Types to include ['mesh', 'light', 'camera']
   * @returns {Array} Array of {object: ScriptAPI, distance: number}
   */
  getObjectsInRadius(radius, objectTypes = ['mesh']) {
    const results = [];
    const myPos = this.object.getAbsolutePosition();
    
    // Handle case where objectTypes might not be provided or is not an array
    let types = objectTypes;
    if (!Array.isArray(objectTypes)) {
      types = ['mesh']; // Default to mesh objects
    }
    
    const collections = [];
    if (types.includes('mesh')) collections.push(this.scene.meshes);
    if (types.includes('light')) collections.push(this.scene.lights);
    if (types.includes('camera')) collections.push(this.scene.cameras);
    
    collections.forEach(collection => {
      collection.forEach(obj => {
        if (obj === this.object) return;
        
        const objPos = obj.getAbsolutePosition ? obj.getAbsolutePosition() : obj.position;
        if (!objPos) return;
        
        const distance = Vector3.Distance(myPos, objPos);
        if (distance <= radius) {
          results.push({
            object: new ScriptAPI(this.scene, obj),
            distance: distance
          });
        }
      });
    });
    
    return results.sort((a, b) => a.distance - b.distance);
  }
  
  // === AUDIO API ===
  
  /**
   * Play a sound
   * @param {string} soundPath - Path to sound file
   * @param {Object} options - Sound options {volume, loop, spatial}
   * @returns {Sound|null} Sound object
   */
  playSound(soundPath, options = {}) {
    try {
      const soundOptions = {
        volume: options.volume || 1.0,
        loop: options.loop || false,
        spatialSound: options.spatial || false,
        maxDistance: options.maxDistance || 100,
        rolloffFactor: options.rolloffFactor || 1,
        ...options
      };
      
      if (soundOptions.spatialSound) {
        soundOptions.spatialSound = true;
        soundOptions.panningModel = 'HRTF';
        soundOptions.distanceModel = 'linear';
      }
      
      const sound = new Sound(`sound_${Date.now()}`, soundPath, this.scene, null, soundOptions);
      
      if (soundOptions.spatialSound && this.object.position) {
        sound.setPosition(this.object.position);
      }
      
      sound.play();
      return sound;
      
    } catch (error) {
      this.log('Failed to play sound:', error);
      return null;
    }
  }
  
  /**
   * Stop a sound
   * @param {Sound} sound - Sound object to stop
   */
  stopSound(sound) {
    if (sound && sound.stop) {
      sound.stop();
    }
  }
  
  /**
   * Set sound volume
   * @param {Sound} sound - Sound object
   * @param {number} volume - Volume (0-1)
   */
  setSoundVolume(sound, volume) {
    if (sound && sound.setVolume) {
      sound.setVolume(Math.max(0, Math.min(1, volume)));
    }
  }
  
  // === INPUT API ===
  
  /**
   * Check if a key is currently pressed
   * @param {string} key - Key code or name
   * @returns {boolean} True if key is pressed
   */
  isKeyPressed(key) {
    const engine = this.scene.getEngine();
    if (!engine.keyboardEventTypes) return false;
    
    // Simple key state tracking would need to be implemented in the engine
    // This is a placeholder for the concept
    return false;
  }
  
  /**
   * Check if a mouse button is pressed
   * @param {number} button - Mouse button (0=left, 1=middle, 2=right)
   * @returns {boolean} True if button is pressed
   */
  isMouseButtonPressed(button = 0) {
    // Would need mouse state tracking implementation
    return false;
  }
  
  /**
   * Get mouse position
   * @returns {Array} [x, y] mouse position
   */
  getMousePosition() {
    const engine = this.scene.getEngine();
    // Would need mouse position tracking implementation
    return [0, 0];
  }
  
  // === ENHANCED MATH UTILITIES ===
  
  /**
   * Generate random number
   * @param {number} min - Minimum value
   * @param {number} max - Maximum value
   * @returns {number} Random number
   */
  random(min = 0, max = 1) {
    return Math.random() * (max - min) + min;
  }
  
  /**
   * Clamp value between min and max
   * @param {number} value - Value to clamp
   * @param {number} min - Minimum value
   * @param {number} max - Maximum value
   * @returns {number} Clamped value
   */
  clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }
  
  /**
   * Linear interpolation between two values
   * @param {number} start - Start value
   * @param {number} end - End value
   * @param {number} t - Interpolation factor (0-1)
   * @returns {number} Interpolated value
   */
  lerp(start, end, t) {
    return start + (end - start) * Math.max(0, Math.min(1, t));
  }
  
  /**
   * Convert degrees to radians
   * @param {number} degrees - Angle in degrees
   * @returns {number} Angle in radians
   */
  toRadians(degrees) {
    return degrees * Math.PI / 180;
  }
  
  /**
   * Convert radians to degrees
   * @param {number} radians - Angle in radians
   * @returns {number} Angle in degrees
   */
  toDegrees(radians) {
    return radians * 180 / Math.PI;
  }
  
  /**
   * Calculate distance between two positions
   * @param {Array} pos1 - First position [x, y, z]
   * @param {Array} pos2 - Second position [x, y, z]
   * @returns {number} Distance
   */
  distance(pos1, pos2) {
    const dx = pos2[0] - pos1[0];
    const dy = pos2[1] - pos1[1];
    const dz = pos2[2] - pos1[2];
    return Math.sqrt(dx * dx + dy * dy + dz * dz);
  }
  
  /**
   * Normalize a vector
   * @param {Array} vector - Vector [x, y, z]
   * @returns {Array} Normalized vector [x, y, z]
   */
  normalize(vector) {
    const length = Math.sqrt(vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2]);
    if (length === 0) return [0, 0, 0];
    return [vector[0] / length, vector[1] / length, vector[2] / length];
  }
  
  /**
   * Calculate dot product of two vectors
   * @param {Array} vec1 - First vector [x, y, z]
   * @param {Array} vec2 - Second vector [x, y, z]
   * @returns {number} Dot product
   */
  dot(vec1, vec2) {
    return vec1[0] * vec2[0] + vec1[1] * vec2[1] + vec1[2] * vec2[2];
  }
  
  /**
   * Calculate cross product of two vectors
   * @param {Array} vec1 - First vector [x, y, z]
   * @param {Array} vec2 - Second vector [x, y, z]
   * @returns {Array} Cross product [x, y, z]
   */
  cross(vec1, vec2) {
    return [
      vec1[1] * vec2[2] - vec1[2] * vec2[1],
      vec1[2] * vec2[0] - vec1[0] * vec2[2],
      vec1[0] * vec2[1] - vec1[1] * vec2[0]
    ];
  }
  
  // === ADVANCED OBJECT METHODS ===
  
  /**
   * Clone this object
   * @param {string} name - Name for the cloned object
   * @param {Array} position - Position for clone [x, y, z] (optional)
   * @returns {ScriptAPI|null} ScriptAPI for cloned object
   */
  clone(name, position = null) {
    if (!this.object.clone) return null;
    
    try {
      const cloned = this.object.clone(name);
      if (position) {
        cloned.position = new Vector3(position[0], position[1], position[2]);
      }
      return new ScriptAPI(this.scene, cloned);
    } catch (error) {
      this.log('Failed to clone object:', error);
      return null;
    }
  }
  
  /**
   * Dispose this object
   */
  dispose() {
    if (this.object.dispose) {
      this.object.dispose();
    }
  }
  
  /**
   * Set object metadata
   * @param {string} key - Metadata key
   * @param {*} value - Metadata value
   */
  setMetadata(key, value) {
    if (!this.object.metadata) {
      this.object.metadata = {};
    }
    this.object.metadata[key] = value;
  }
  
  /**
   * Get object metadata
   * @param {string} key - Metadata key
   * @returns {*} Metadata value
   */
  getMetadata(key) {
    return this.object.metadata ? this.object.metadata[key] : null;
  }
  
  // === SCRIPT PROPERTIES API ===
  
  /**
   * Get script properties definition
   * @returns {Array} Array of property definitions
   */
  getScriptProperties() {
    return this._scriptProperties || [];
  }
  
  /**
   * Get script properties organized by section
   * @returns {Object} Object with section names as keys and property arrays as values
   */
  getScriptPropertiesBySection() {
    const properties = this.getScriptProperties();
    const sections = {};
    
    properties.forEach(prop => {
      const sectionName = prop.section || 'General';
      if (!sections[sectionName]) {
        sections[sectionName] = [];
      }
      sections[sectionName].push(prop);
    });
    
    return sections;
  }
  
  /**
   * Set script property value
   * @param {string} propertyName - Name of the property
   * @param {*} value - New value for the property
   */
  setScriptProperty(propertyName, value) {
    if (!this.object.metadata) {
      this.object.metadata = {};
    }
    if (!this.object.metadata.scriptProperties) {
      this.object.metadata.scriptProperties = {};
    }
    
    this.object.metadata.scriptProperties[propertyName] = value;
    
    // Also set it on the script instance if we have a reference
    if (this._scriptInstance) {
      this._scriptInstance[propertyName] = value;
    }
  }
  
  /**
   * Get script property value
   * @param {string} propertyName - Name of the property
   * @returns {*} Property value
   */
  getScriptProperty(propertyName) {
    if (this.object.metadata && this.object.metadata.scriptProperties) {
      return this.object.metadata.scriptProperties[propertyName];
    }
    return null;
  }
  
  /**
   * Initialize script properties with default values
   */
  initializeScriptProperties() {
    const properties = this.getScriptProperties();
    
    properties.forEach(prop => {
      const currentValue = this.getScriptProperty(prop.name);
      if (currentValue === null || currentValue === undefined) {
        // Set default value if not already set
        const defaultValue = this.evaluatePropertyDefault(prop.defaultValue);
        this.setScriptProperty(prop.name, defaultValue);
      }
    });
  }
  
  /**
   * Initialize script properties on the script instance itself
   * @param {Object} scriptInstance - The script instance to initialize
   */
  initializeScriptInstanceProperties(scriptInstance) {
    const properties = this.getScriptProperties();
    
    properties.forEach(prop => {
      // Get value from metadata or use default
      let value = this.getScriptProperty(prop.name);
      if (value === null || value === undefined) {
        value = this.evaluatePropertyDefault(prop.defaultValue);
        this.setScriptProperty(prop.name, value);
      }
      
      // Set the property directly on the script instance
      scriptInstance[prop.name] = value;
    });
  }
  
  /**
   * Evaluate property default value expression
   * @private
   */
  evaluatePropertyDefault(expression) {
    if (expression === null || expression === undefined) return null;
    
    try {
      // Handle string literals
      if (typeof expression === 'string' && expression.startsWith('"') && expression.endsWith('"')) {
        return expression.slice(1, -1);
      }
      
      // Handle numeric literals
      if (!isNaN(expression)) {
        return parseFloat(expression);
      }
      
      // Handle boolean literals
      if (expression === 'true') return true;
      if (expression === 'false') return false;
      
      // For more complex expressions, try to evaluate safely
      // This is a simplified evaluation - in production you'd want a proper expression evaluator
      return expression;
    } catch (error) {
      this.log('Failed to evaluate property default:', expression, error);
      return null;
    }
  }
  
  /**
   * Add tag to object
   * @param {string} tag - Tag to add
   */
  addTag(tag) {
    if (!this.object.metadata) {
      this.object.metadata = {};
    }
    if (!this.object.metadata.tags) {
      this.object.metadata.tags = [];
    }
    if (!this.object.metadata.tags.includes(tag)) {
      this.object.metadata.tags.push(tag);
    }
  }
  
  /**
   * Remove tag from object
   * @param {string} tag - Tag to remove
   */
  removeTag(tag) {
    if (this.object.metadata && this.object.metadata.tags) {
      this.object.metadata.tags = this.object.metadata.tags.filter(t => t !== tag);
    }
  }
  
  /**
   * Check if object has tag
   * @param {string} tag - Tag to check
   * @returns {boolean} True if object has tag
   */
  hasTag(tag) {
    return this.object.metadata && this.object.metadata.tags && this.object.metadata.tags.includes(tag);
  }
}

export { ScriptAPI };