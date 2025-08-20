import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';

/**
 * ScriptAPI - Provides a safe interface for scripts to interact with Babylon.js objects
 * This wrapper ensures scripts can only access safe methods and properties
 */
class ScriptAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.object = babylonObject;
    this._deltaTime = 0;
    
    // Bind methods to ensure proper context
    this.bindMethods();
  }
  
  bindMethods() {
    // Object manipulation methods
    this.getPosition = this.getPosition.bind(this);
    this.setPosition = this.setPosition.bind(this);
    this.getRotation = this.getRotation.bind(this);
    this.setRotation = this.setRotation.bind(this);
    this.getScale = this.getScale.bind(this);
    this.setScale = this.setScale.bind(this);
    
    // Utility methods
    this.log = this.log.bind(this);
    this.findObjectByName = this.findObjectByName.bind(this);
    this.getDeltaTime = this.getDeltaTime.bind(this);
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
    return this.scene.getEngine().getTimeMs();
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
  
  // === MATH UTILITIES ===
  
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
}

export { ScriptAPI };