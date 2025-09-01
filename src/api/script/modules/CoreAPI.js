import { Vector3, Vector2, Vector4 } from '@babylonjs/core/Maths/math.vector.js';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color.js';
import { Matrix, Quaternion } from '@babylonjs/core/Maths/math.vector.js';
import { Tools } from '@babylonjs/core/Misc/tools.js';

/**
 * CoreAPI - Essential transform, visibility, and utility functions
 * Priority: HIGHEST - These are used in almost every script
 */
export class CoreAPI {
  constructor(scene, babylonObject) {
    this.scene = scene;
    this.babylonObject = babylonObject;
    this._deltaTime = 0;
  }

  // === ESSENTIAL TRANSFORM API ===
  
  getPosition() {
    if (!this.babylonObject?.position) return [0, 0, 0];
    const pos = this.babylonObject.position;
    return [pos.x, pos.y, pos.z];
  }

  position() {
    return this.getPosition();
  }

  setPosition(x, y, z) {
    if (!this.babylonObject?.position) return;
    this.babylonObject.position.set(x, y, z);
  }

  getWorldPosition() {
    if (!this.babylonObject?.getWorldMatrix) return [0, 0, 0];
    const worldMatrix = this.babylonObject.getWorldMatrix();
    const worldPos = Vector3.TransformCoordinates(Vector3.Zero(), worldMatrix);
    return [worldPos.x, worldPos.y, worldPos.z];
  }

  worldPosition() {
    return this.getWorldPosition();
  }

  getRotation() {
    if (!this.babylonObject?.rotation) return [0, 0, 0];
    const rot = this.babylonObject.rotation;
    return [rot.x, rot.y, rot.z];
  }

  rotation() {
    return this.getRotation();
  }

  setRotation(x, y, z) {
    if (!this.babylonObject?.rotation) return;
    this.babylonObject.rotation.set(x, y, z);
  }

  getWorldRotation() {
    if (!this.babylonObject?.rotationQuaternion) return [0, 0, 0, 1];
    const quat = this.babylonObject.rotationQuaternion || Quaternion.Identity();
    return [quat.x, quat.y, quat.z, quat.w];
  }

  getWorldRotationQuaternion() {
    return this.getWorldRotation();
  }

  worldRotation() {
    return this.getWorldRotationQuaternion();
  }

  getScale() {
    if (!this.babylonObject?.scaling) return [1, 1, 1];
    const scale = this.babylonObject.scaling;
    return [scale.x, scale.y, scale.z];
  }

  scale() {
    return this.getScale();
  }

  setScale(x, y, z) {
    if (!this.babylonObject?.scaling) return;
    this.babylonObject.scaling.set(x, y, z);
  }

  moveBy(x, y, z) {
    if (!this.babylonObject?.position) return;
    this.babylonObject.position.x += x;
    this.babylonObject.position.y += y;
    this.babylonObject.position.z += z;
  }

  rotateBy(x, y, z) {
    if (!this.babylonObject?.rotation) return;
    this.babylonObject.rotation.x += x;
    this.babylonObject.rotation.y += y;
    this.babylonObject.rotation.z += z;
  }

  lookAt(targetX, targetY, targetZ) {
    if (!this.babylonObject?.lookAt) return;
    const target = new Vector3(targetX, targetY, targetZ);
    this.babylonObject.lookAt(target);
  }

  // === VISIBILITY & BASIC PROPERTIES ===
  
  isVisible() {
    return this.babylonObject?.isVisible !== false;
  }

  setVisible(visible) {
    if (this.babylonObject) {
      this.babylonObject.isVisible = visible;
    }
  }

  setEnabled(enabled) {
    if (this.babylonObject?.setEnabled) {
      this.babylonObject.setEnabled(enabled);
    }
  }

  isEnabled() {
    return this.babylonObject?.isEnabled !== false;
  }

  // === TAGGING SYSTEM ===
  
  addTag(tag) {
    if (!this.babylonObject) return;
    if (!this.babylonObject.metadata) {
      this.babylonObject.metadata = {};
    }
    if (!this.babylonObject.metadata.tags) {
      this.babylonObject.metadata.tags = new Set();
    }
    this.babylonObject.metadata.tags.add(tag);
  }

  removeTag(tag) {
    if (!this.babylonObject?.metadata?.tags) return;
    this.babylonObject.metadata.tags.delete(tag);
  }

  hasTag(tag) {
    return this.babylonObject?.metadata?.tags?.has(tag) || false;
  }

  getTags() {
    return Array.from(this.babylonObject?.metadata?.tags || []);
  }

  tags() {
    return this.getTags();
  }

  // === TIME & UTILITY ===
  
  getDeltaTime() {
    return this._deltaTime;
  }

  time() {
    return this._deltaTime;
  }

  getTime() {
    return performance.now();
  }

  log(...args) {
    console.log('[RenScript]', ...args);
  }

  // === MATH UTILITIES ===
  
  random() {
    return Math.random();
  }

  randomRange(min, max) {
    return min + Math.random() * (max - min);
  }

  clamp(value, min, max) {
    return Math.min(Math.max(value, min), max);
  }

  lerp(start, end, t) {
    return start + t * (end - start);
  }

  distance(x1, y1, z1, x2, y2, z2) {
    const dx = x2 - x1;
    const dy = y2 - y1;
    const dz = z2 - z1;
    return Math.sqrt(dx * dx + dy * dy + dz * dz);
  }

  normalize(x, y, z) {
    const length = Math.sqrt(x * x + y * y + z * z);
    if (length === 0) return [0, 0, 0];
    return [x / length, y / length, z / length];
  }

  dot(x1, y1, z1, x2, y2, z2) {
    return x1 * x2 + y1 * y2 + z1 * z2;
  }

  cross(x1, y1, z1, x2, y2, z2) {
    return [
      y1 * z2 - z1 * y2,
      z1 * x2 - x1 * z2,
      x1 * y2 - y1 * x2
    ];
  }

  // === ANGLE UTILITIES ===
  
  toRadians(degrees) {
    return degrees * Math.PI / 180;
  }

  toDegrees(radians) {
    return radians * 180 / Math.PI;
  }

  // === OBJECT QUERIES ===
  
  getName() {
    return this.babylonObject?.name || '';
  }

  name() {
    return this.getName();
  }

  setName(name) {
    if (this.babylonObject) {
      this.babylonObject.name = name;
    }
  }

  getId() {
    return this.babylonObject?.id || '';
  }

  id() {
    return this.getId();
  }

  // === UPDATE DELTA TIME ===
  
  _updateDeltaTime(deltaTime) {
    this._deltaTime = deltaTime;
  }
}