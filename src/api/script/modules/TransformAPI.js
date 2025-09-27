// === TRANSFORM API MODULE ===
/**
 * TransformAPI - Advanced transform operations for position, rotation, scale
 * Provides comprehensive transform utilities beyond basic CoreAPI transforms
 * Priority: HIGH - Essential for object manipulation and animation
 */

import { 
  Vector3, 
  Quaternion, 
  Matrix, 
  Space,
  Tools,
  Animation,
  AnimationKeys,
  BezierCurveEase,
  CircleEase,
  BackEase,
  BounceEase,
  CubicEase,
  ElasticEase,
  ExponentialEase,
  PowerEase,
  QuadraticEase,
  QuarticEase,
  QuinticEase,
  SineEase
} from '@babylonjs/core';

export class TransformAPI {
  constructor(scene, babylonObject = null) {
    this.scene = scene;
    this.babylonObject = babylonObject;
    this.mesh = babylonObject; // Alias for backward compatibility
  }

  // === POSITION METHODS ===

  /**
   * Move object to absolute world position
   * @param {number} x - X coordinate
   * @param {number} y - Y coordinate  
   * @param {number} z - Z coordinate
   */
  moveTo(x, y, z) {
    if (!this.babylonObject?.position) return false;
    this.babylonObject.position.set(x, y, z);
    return true;
  }

  /**
   * Move object by relative offset
   * @param {number} x - X offset
   * @param {number} y - Y offset
   * @param {number} z - Z offset
   */
  moveBy(x, y, z) {
    if (!this.babylonObject?.position) return false;
    this.babylonObject.position.addInPlace(new Vector3(x, y, z));
    return true;
  }

  /**
   * Move object forward based on its current rotation
   * @param {number} distance - Distance to move forward
   */
  moveForward(distance = 1) {
    if (!this.babylonObject) return false;
    const forward = this.babylonObject.getDirection ? 
      this.babylonObject.getDirection(Vector3.Forward()) : 
      Vector3.Forward();
    this.babylonObject.position.addInPlace(forward.scale(distance));
    return true;
  }

  /**
   * Move object backward based on its current rotation
   * @param {number} distance - Distance to move backward
   */
  moveBackward(distance = 1) {
    return this.moveForward(-distance);
  }

  /**
   * Move object right based on its current rotation
   * @param {number} distance - Distance to move right
   */
  moveRight(distance = 1) {
    if (!this.babylonObject) return false;
    const right = this.babylonObject.getDirection ? 
      this.babylonObject.getDirection(Vector3.Right()) : 
      Vector3.Right();
    this.babylonObject.position.addInPlace(right.scale(distance));
    return true;
  }

  /**
   * Move object left based on its current rotation
   * @param {number} distance - Distance to move left
   */
  moveLeft(distance = 1) {
    return this.moveRight(-distance);
  }

  /**
   * Move object up based on its current rotation
   * @param {number} distance - Distance to move up
   */
  moveUp(distance = 1) {
    if (!this.babylonObject) return false;
    const up = this.babylonObject.getDirection ? 
      this.babylonObject.getDirection(Vector3.Up()) : 
      Vector3.Up();
    this.babylonObject.position.addInPlace(up.scale(distance));
    return true;
  }

  /**
   * Move object down based on its current rotation
   * @param {number} distance - Distance to move down
   */
  moveDown(distance = 1) {
    return this.moveUp(-distance);
  }

  /**
   * Get distance to another object or position
   * @param {Object|Array} target - Babylon object or [x,y,z] array
   * @returns {number} Distance
   */
  distanceTo(target) {
    if (!this.babylonObject?.position) return 0;
    
    let targetPos;
    if (Array.isArray(target)) {
      targetPos = new Vector3(target[0], target[1], target[2]);
    } else if (target?.position) {
      targetPos = target.position;
    } else {
      return 0;
    }
    
    return Vector3.Distance(this.babylonObject.position, targetPos);
  }

  // === ROTATION METHODS ===

  /**
   * Rotate to absolute rotation in radians
   * @param {number} x - X rotation in radians
   * @param {number} y - Y rotation in radians
   * @param {number} z - Z rotation in radians
   */
  rotateTo(x, y, z) {
    if (!this.babylonObject?.rotation) return false;
    this.babylonObject.rotation.set(x, y, z);
    return true;
  }

  /**
   * Rotate to absolute rotation in degrees
   * @param {number} x - X rotation in degrees
   * @param {number} y - Y rotation in degrees
   * @param {number} z - Z rotation in degrees
   */
  rotateToDegs(x, y, z) {
    return this.rotateTo(
      Tools.ToRadians(x),
      Tools.ToRadians(y),
      Tools.ToRadians(z)
    );
  }

  /**
   * Rotate by relative amounts in radians
   * @param {number} x - X rotation offset in radians
   * @param {number} y - Y rotation offset in radians
   * @param {number} z - Z rotation offset in radians
   */
  rotateBy(x, y, z) {
    if (!this.babylonObject?.rotation) return false;
    this.babylonObject.rotation.addInPlace(new Vector3(x, y, z));
    return true;
  }

  /**
   * Rotate by relative amounts in degrees
   * @param {number} x - X rotation offset in degrees
   * @param {number} y - Y rotation offset in degrees
   * @param {number} z - Z rotation offset in degrees
   */
  rotateByDegs(x, y, z) {
    return this.rotateBy(
      Tools.ToRadians(x),
      Tools.ToRadians(y),
      Tools.ToRadians(z)
    );
  }

  /**
   * Look at target position or object
   * @param {Object|Array} target - Babylon object or [x,y,z] array
   * @param {Array} up - Up vector [x,y,z], defaults to [0,1,0]
   */
  lookAt(target, up = [0, 1, 0]) {
    if (!this.babylonObject?.lookAt) return false;
    
    let targetPos;
    if (Array.isArray(target)) {
      targetPos = new Vector3(target[0], target[1], target[2]);
    } else if (target?.position) {
      targetPos = target.position;
    } else {
      return false;
    }
    
    this.babylonObject.lookAt(targetPos, 0, 0, 0, Space.WORLD);
    return true;
  }

  /**
   * Get rotation in degrees
   * @returns {Array} [x, y, z] rotation in degrees
   */
  getRotationDegrees() {
    if (!this.babylonObject?.rotation) return [0, 0, 0];
    const rot = this.babylonObject.rotation;
    return [
      Tools.ToDegrees(rot.x),
      Tools.ToDegrees(rot.y),
      Tools.ToDegrees(rot.z)
    ];
  }

  // === SCALE METHODS ===

  /**
   * Get current scale values
   * @returns {Array} [x, y, z] scale values
   */
  getScale() {
    if (!this.babylonObject?.scaling) return [1, 1, 1];
    const scale = this.babylonObject.scaling;
    return [scale.x, scale.y, scale.z];
  }

  /**
   * Alias for getScale()
   * @returns {Array} [x, y, z] scale values
   */
  scale() {
    return this.getScale();
  }

  /**
   * Scale to absolute size
   * @param {number} x - X scale
   * @param {number} y - Y scale (optional, defaults to x)
   * @param {number} z - Z scale (optional, defaults to x)
   */
  scaleTo(x, y = null, z = null) {
    if (!this.babylonObject?.scaling) return false;
    y = y !== null ? y : x;
    z = z !== null ? z : x;
    this.babylonObject.scaling.set(x, y, z);
    return true;
  }

  /**
   * Scale by relative multiplier
   * @param {number} x - X scale multiplier
   * @param {number} y - Y scale multiplier (optional, defaults to x)
   * @param {number} z - Z scale multiplier (optional, defaults to x)
   */
  scaleBy(x, y = null, z = null) {
    if (!this.babylonObject?.scaling) return false;
    y = y !== null ? y : x;
    z = z !== null ? z : x;
    const current = this.babylonObject.scaling;
    this.babylonObject.scaling.set(
      current.x * x,
      current.y * y,
      current.z * z
    );
    return true;
  }

  /**
   * Scale uniformly (all axes same value)
   * @param {number} scale - Uniform scale value
   */
  scaleUniform(scale) {
    return this.scaleTo(scale, scale, scale);
  }

  // === ANIMATION HELPERS ===

  /**
   * Animate position smoothly
   * @param {Array} targetPos - Target position [x, y, z]
   * @param {number} duration - Duration in milliseconds
   * @param {string} easing - Easing type ('linear', 'ease', 'bounce', etc.)
   * @returns {Animation} Animation object
   */
  animateToPosition(targetPos, duration = 1000, easing = 'ease') {
    if (!this.babylonObject?.position) return null;

    const animation = Animation.CreateAndStartAnimation(
      `pos_${this.babylonObject.name}_${Date.now()}`,
      this.babylonObject,
      'position',
      60, // 60 FPS
      Math.floor(duration / 1000 * 60), // Convert to frames
      this.babylonObject.position.clone(),
      new Vector3(targetPos[0], targetPos[1], targetPos[2]),
      Animation.ANIMATIONLOOPMODE_CONSTANT,
      this._getEasingFunction(easing),
      null
    );

    return animation;
  }

  /**
   * Animate rotation smoothly
   * @param {Array} targetRot - Target rotation [x, y, z] in degrees
   * @param {number} duration - Duration in milliseconds
   * @param {string} easing - Easing type
   * @returns {Animation} Animation object
   */
  animateToRotation(targetRot, duration = 1000, easing = 'ease') {
    if (!this.babylonObject?.rotation) return null;

    const targetRadians = new Vector3(
      Tools.ToRadians(targetRot[0]),
      Tools.ToRadians(targetRot[1]),
      Tools.ToRadians(targetRot[2])
    );

    const animation = Animation.CreateAndStartAnimation(
      `rot_${this.babylonObject.name}_${Date.now()}`,
      this.babylonObject,
      'rotation',
      60,
      Math.floor(duration / 1000 * 60),
      this.babylonObject.rotation.clone(),
      targetRadians,
      Animation.ANIMATIONLOOPMODE_CONSTANT,
      this._getEasingFunction(easing),
      null
    );

    return animation;
  }

  /**
   * Animate scale smoothly
   * @param {Array|number} targetScale - Target scale [x, y, z] or uniform scale
   * @param {number} duration - Duration in milliseconds
   * @param {string} easing - Easing type
   * @returns {Animation} Animation object
   */
  animateToScale(targetScale, duration = 1000, easing = 'ease') {
    if (!this.babylonObject?.scaling) return null;

    let targetVec;
    if (Array.isArray(targetScale)) {
      targetVec = new Vector3(targetScale[0], targetScale[1], targetScale[2]);
    } else {
      targetVec = new Vector3(targetScale, targetScale, targetScale);
    }

    const animation = Animation.CreateAndStartAnimation(
      `scale_${this.babylonObject.name}_${Date.now()}`,
      this.babylonObject,
      'scaling',
      60,
      Math.floor(duration / 1000 * 60),
      this.babylonObject.scaling.clone(),
      targetVec,
      Animation.ANIMATIONLOOPMODE_CONSTANT,
      this._getEasingFunction(easing),
      null
    );

    return animation;
  }

  // === TRANSFORM MATRIX OPERATIONS ===

  /**
   * Get world matrix of the object
   * @returns {Matrix} World transformation matrix
   */
  getWorldMatrix() {
    if (!this.babylonObject?.getWorldMatrix) return Matrix.Identity();
    return this.babylonObject.getWorldMatrix();
  }

  /**
   * Get world position (different from local position if object has parent)
   * @returns {Array} World position [x, y, z]
   */
  getWorldPosition() {
    if (!this.babylonObject?.getAbsolutePosition) return [0, 0, 0];
    const pos = this.babylonObject.getAbsolutePosition();
    return [pos.x, pos.y, pos.z];
  }

  /**
   * Transform point from local to world coordinates
   * @param {Array} localPoint - Local point [x, y, z]
   * @returns {Array} World point [x, y, z]
   */
  localToWorld(localPoint) {
    if (!this.babylonObject?.getWorldMatrix) return localPoint;
    const localVec = new Vector3(localPoint[0], localPoint[1], localPoint[2]);
    const worldVec = Vector3.TransformCoordinates(localVec, this.getWorldMatrix());
    return [worldVec.x, worldVec.y, worldVec.z];
  }

  /**
   * Transform point from world to local coordinates
   * @param {Array} worldPoint - World point [x, y, z]
   * @returns {Array} Local point [x, y, z]
   */
  worldToLocal(worldPoint) {
    if (!this.babylonObject?.getWorldMatrix) return worldPoint;
    const worldVec = new Vector3(worldPoint[0], worldPoint[1], worldPoint[2]);
    const inverseMatrix = Matrix.Invert(this.getWorldMatrix());
    const localVec = Vector3.TransformCoordinates(worldVec, inverseMatrix);
    return [localVec.x, localVec.y, localVec.z];
  }

  // === UTILITY METHODS ===

  /**
   * Get the forward direction vector of the object
   * @returns {Array} Forward direction [x, y, z]
   */
  getForwardDirection() {
    if (!this.babylonObject) return [0, 0, 1];
    const forward = this.babylonObject.getDirection ? 
      this.babylonObject.getDirection(Vector3.Forward()) : 
      Vector3.Forward();
    return [forward.x, forward.y, forward.z];
  }

  /**
   * Get the right direction vector of the object
   * @returns {Array} Right direction [x, y, z]
   */
  getRightDirection() {
    if (!this.babylonObject) return [1, 0, 0];
    const right = this.babylonObject.getDirection ? 
      this.babylonObject.getDirection(Vector3.Right()) : 
      Vector3.Right();
    return [right.x, right.y, right.z];
  }

  /**
   * Get the up direction vector of the object
   * @returns {Array} Up direction [x, y, z]
   */
  getUpDirection() {
    if (!this.babylonObject) return [0, 1, 0];
    const up = this.babylonObject.getDirection ? 
      this.babylonObject.getDirection(Vector3.Up()) : 
      Vector3.Up();
    return [up.x, up.y, up.z];
  }

  /**
   * Reset transform to identity (position 0,0,0, rotation 0,0,0, scale 1,1,1)
   */
  resetTransform() {
    if (!this.babylonObject) return false;
    if (this.babylonObject.position) this.babylonObject.position.set(0, 0, 0);
    if (this.babylonObject.rotation) this.babylonObject.rotation.set(0, 0, 0);
    if (this.babylonObject.scaling) this.babylonObject.scaling.set(1, 1, 1);
    return true;
  }

  /**
   * Copy transform from another object
   * @param {Object} sourceObject - Source Babylon object
   */
  copyTransformFrom(sourceObject) {
    if (!this.babylonObject || !sourceObject) return false;
    
    if (sourceObject.position && this.babylonObject.position) {
      this.babylonObject.position.copyFrom(sourceObject.position);
    }
    if (sourceObject.rotation && this.babylonObject.rotation) {
      this.babylonObject.rotation.copyFrom(sourceObject.rotation);
    }
    if (sourceObject.scaling && this.babylonObject.scaling) {
      this.babylonObject.scaling.copyFrom(sourceObject.scaling);
    }
    
    return true;
  }

  // === PRIVATE HELPER METHODS ===

  /**
   * Get easing function by name
   * @private
   */
  _getEasingFunction(easingType) {
    switch (easingType.toLowerCase()) {
      case 'linear': return null;
      case 'ease': return new BezierCurveEase(0.25, 0.1, 0.25, 1);
      case 'bounce': return new BounceEase();
      case 'back': return new BackEase();
      case 'elastic': return new ElasticEase();
      case 'exponential': return new ExponentialEase();
      case 'power': return new PowerEase();
      case 'quadratic': return new QuadraticEase();
      case 'quartic': return new QuarticEase();
      case 'quintic': return new QuinticEase();
      case 'sine': return new SineEase();
      case 'circle': return new CircleEase();
      case 'cubic': return new CubicEase();
      default: return new BezierCurveEase(0.25, 0.1, 0.25, 1);
    }
  }
}