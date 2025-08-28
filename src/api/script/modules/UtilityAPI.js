// === UTILITY API MODULE ===

import {
  Vector2,
  Vector3,
  Vector4,
  Quaternion,
  Matrix,
  Color3,
  Color4,
  Angle,
  Tools,
  Epsilon,
  Space,
  Axis,
  Path3D,
  Curve3,
  BezierCurve,
  Arc2,
  Polygon,
  PointerEventTypes,
  ActionManager,
  ExecuteCodeAction,
  InterpolateValueAction,
  SetValueAction,
  IncrementValueAction,
  PlayAnimationAction,
  StopAnimationAction,
  DoNothingAction,
  CombineAction,
  SetStateAction,
  SetParentAction,
  PlaySoundAction,
  StopSoundAction
} from '@babylonjs/core';

export class UtilityAPI {
  constructor(scene) {
    this.scene = scene;
  }

  // === MATH UTILITIES ===

  clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }

  lerp(start, end, t) {
    return start + (end - start) * this.clamp(t, 0, 1);
  }

  inverseLerp(start, end, value) {
    return this.clamp((value - start) / (end - start), 0, 1);
  }

  smoothStep(edge0, edge1, x) {
    const t = this.clamp((x - edge0) / (edge1 - edge0), 0, 1);
    return t * t * (3 - 2 * t);
  }

  remap(value, fromMin, fromMax, toMin, toMax) {
    const t = this.inverseLerp(fromMin, fromMax, value);
    return this.lerp(toMin, toMax, t);
  }

  randomRange(min, max) {
    return Math.random() * (max - min) + min;
  }

  randomInt(min, max) {
    return Math.floor(this.randomRange(min, max + 1));
  }

  randomChoice(array) {
    if (!array || array.length === 0) return null;
    return array[this.randomInt(0, array.length - 1)];
  }

  // === VECTOR UTILITIES ===

  createVector2(x = 0, y = 0) {
    return new Vector2(x, y);
  }

  createVector3(x = 0, y = 0, z = 0) {
    return new Vector3(x, y, z);
  }

  createVector4(x = 0, y = 0, z = 0, w = 0) {
    return new Vector4(x, y, z, w);
  }

  vectorDistance(vec1, vec2) {
    if (vec1.length === 2) {
      return Vector2.Distance(new Vector2(...vec1), new Vector2(...vec2));
    } else if (vec1.length === 3) {
      return Vector3.Distance(new Vector3(...vec1), new Vector3(...vec2));
    }
    return 0;
  }

  vectorLerp(vec1, vec2, t) {
    if (vec1.length === 2) {
      const result = Vector2.Lerp(new Vector2(...vec1), new Vector2(...vec2), t);
      return [result.x, result.y];
    } else if (vec1.length === 3) {
      const result = Vector3.Lerp(new Vector3(...vec1), new Vector3(...vec2), t);
      return [result.x, result.y, result.z];
    }
    return vec1;
  }

  vectorNormalize(vec) {
    if (vec.length === 2) {
      const result = new Vector2(...vec).normalize();
      return [result.x, result.y];
    } else if (vec.length === 3) {
      const result = new Vector3(...vec).normalize();
      return [result.x, result.y, result.z];
    }
    return vec;
  }

  vectorCross(vec1, vec2) {
    const v1 = new Vector3(...vec1);
    const v2 = new Vector3(...vec2);
    const result = Vector3.Cross(v1, v2);
    return [result.x, result.y, result.z];
  }

  vectorDot(vec1, vec2) {
    if (vec1.length === 2) {
      return Vector2.Dot(new Vector2(...vec1), new Vector2(...vec2));
    } else if (vec1.length === 3) {
      return Vector3.Dot(new Vector3(...vec1), new Vector3(...vec2));
    }
    return 0;
  }

  // === COLOR UTILITIES ===

  createColor3(r = 1, g = 1, b = 1) {
    return new Color3(r, g, b);
  }

  createColor4(r = 1, g = 1, b = 1, a = 1) {
    return new Color4(r, g, b, a);
  }

  colorLerp(color1, color2, t) {
    if (color1.length === 3) {
      const result = Color3.Lerp(new Color3(...color1), new Color3(...color2), t);
      return [result.r, result.g, result.b];
    } else if (color1.length === 4) {
      const result = Color4.Lerp(new Color4(...color1), new Color4(...color2), t);
      return [result.r, result.g, result.b, result.a];
    }
    return color1;
  }

  colorFromHex(hex) {
    const color = Color3.FromHexString(hex);
    return [color.r, color.g, color.b];
  }

  colorToHex(color) {
    const color3 = new Color3(...color);
    return color3.toHexString();
  }

  colorFromHSV(h, s, v) {
    const color = Color3.FromHSV(h, s, v);
    return [color.r, color.g, color.b];
  }

  colorToHSV(color) {
    const color3 = new Color3(...color);
    return color3.toHSV();
  }

  // === ANGLE UTILITIES ===

  degreesToRadians(degrees) {
    return Angle.FromDegrees(degrees).radians();
  }

  radiansToDegrees(radians) {
    return Angle.FromRadians(radians).degrees();
  }

  normalizeAngle(angle) {
    while (angle > Math.PI) angle -= 2 * Math.PI;
    while (angle < -Math.PI) angle += 2 * Math.PI;
    return angle;
  }

  angleBetweenVectors(vec1, vec2) {
    const v1 = new Vector3(...vec1).normalize();
    const v2 = new Vector3(...vec2).normalize();
    return Math.acos(this.clamp(Vector3.Dot(v1, v2), -1, 1));
  }

  // === TRANSFORMATION UTILITIES ===

  createQuaternion(x = 0, y = 0, z = 0, w = 1) {
    return new Quaternion(x, y, z, w);
  }

  quaternionFromEuler(x, y, z) {
    const result = Quaternion.RotationYawPitchRoll(y, x, z);
    return [result.x, result.y, result.z, result.w];
  }

  quaternionToEuler(quat) {
    const quaternion = new Quaternion(...quat);
    const euler = quaternion.toEulerAngles();
    return [euler.x, euler.y, euler.z];
  }

  quaternionSlerp(quat1, quat2, t) {
    const result = Quaternion.Slerp(new Quaternion(...quat1), new Quaternion(...quat2), t);
    return [result.x, result.y, result.z, result.w];
  }

  transformPoint(point, matrix) {
    const vec = new Vector3(...point);
    const mat = Array.isArray(matrix) ? Matrix.FromArray(matrix) : matrix;
    const result = Vector3.TransformCoordinates(vec, mat);
    return [result.x, result.y, result.z];
  }

  transformDirection(direction, matrix) {
    const vec = new Vector3(...direction);
    const mat = Array.isArray(matrix) ? Matrix.FromArray(matrix) : matrix;
    const result = Vector3.TransformNormal(vec, mat);
    return [result.x, result.y, result.z];
  }

  // === CURVE UTILITIES ===

  createBezierCurve(points) {
    if (points.length < 4 || points.length % 3 !== 1) {
      console.warn('Bezier curve requires 4, 7, 10, ... points (3n+1)');
      return null;
    }
    
    const pathPoints = points.map(p => new Vector3(...p));
    return BezierCurve.CreateCubic(pathPoints[0], pathPoints[1], pathPoints[2], pathPoints[3]);
  }

  sampleCurve(curve, t) {
    if (!curve) return [0, 0, 0];
    
    const point = curve.getPointAt(this.clamp(t, 0, 1));
    return [point.x, point.y, point.z];
  }

  getCurveLength(points) {
    if (!points || points.length < 2) return 0;
    
    let length = 0;
    for (let i = 1; i < points.length; i++) {
      length += this.vectorDistance(points[i - 1], points[i]);
    }
    return length;
  }

  simplifyPath(points, tolerance = 0.1) {
    if (!points || points.length < 3) return points;
    
    // Douglas-Peucker algorithm
    const douglasPeucker = (points, tolerance) => {
      const first = points[0];
      const last = points[points.length - 1];
      
      if (points.length < 3) return points;
      
      let maxDistance = 0;
      let maxIndex = 0;
      
      for (let i = 1; i < points.length - 1; i++) {
        const distance = this.pointToLineDistance(points[i], first, last);
        if (distance > maxDistance) {
          maxDistance = distance;
          maxIndex = i;
        }
      }
      
      if (maxDistance > tolerance) {
        const left = douglasPeucker(points.slice(0, maxIndex + 1), tolerance);
        const right = douglasPeucker(points.slice(maxIndex), tolerance);
        return left.slice(0, -1).concat(right);
      } else {
        return [first, last];
      }
    };
    
    return douglasPeucker(points, tolerance);
  }

  pointToLineDistance(point, lineStart, lineEnd) {
    const p = new Vector3(...point);
    const a = new Vector3(...lineStart);
    const b = new Vector3(...lineEnd);
    
    const ab = b.subtract(a);
    const ap = p.subtract(a);
    const cross = Vector3.Cross(ab, ap);
    
    return cross.length() / ab.length();
  }

  // === TIME UTILITIES ===

  getEngineTime() {
    return this.scene.getEngine().getTimeInMilliseconds();
  }

  getDeltaTime() {
    return this.scene.getEngine().getDeltaTime();
  }

  createTimer(duration, callback, repeat = false) {
    const startTime = this.getEngineTime();
    
    const check = () => {
      const elapsed = this.getEngineTime() - startTime;
      if (elapsed >= duration) {
        callback();
        if (repeat) {
          return this.createTimer(duration, callback, repeat);
        }
      } else {
        requestAnimationFrame(check);
      }
    };
    
    requestAnimationFrame(check);
    return { startTime, duration, repeat };
  }

  // === INPUT UTILITIES ===

  isKeyPressed(keyCode) {
    return this.scene.actionManager?.getCurrentKey(keyCode) || false;
  }

  getMousePosition() {
    const canvas = this.scene.getEngine().getRenderingCanvas();
    if (!canvas) return [0, 0];
    
    const rect = canvas.getBoundingClientRect();
    return [
      this.scene.pointerX - rect.left,
      this.scene.pointerY - rect.top
    ];
  }

  getMouseWorldPosition(groundMesh = null) {
    const canvas = this.scene.getEngine().getRenderingCanvas();
    if (!canvas) return [0, 0, 0];
    
    const pickInfo = this.scene.pick(this.scene.pointerX, this.scene.pointerY, (mesh) => {
      return groundMesh ? mesh === groundMesh : mesh.isPickable;
    });
    
    if (pickInfo.hit && pickInfo.pickedPoint) {
      return [pickInfo.pickedPoint.x, pickInfo.pickedPoint.y, pickInfo.pickedPoint.z];
    }
    
    return [0, 0, 0];
  }

  // === SCREEN UTILITIES ===

  worldToScreen(worldPosition, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return [0, 0];
    
    const engine = this.scene.getEngine();
    const viewport = targetCamera.viewport;
    
    const worldPos = new Vector3(...worldPosition);
    const screenPos = Vector3.Project(
      worldPos,
      Matrix.Identity(),
      targetCamera.getViewMatrix().multiply(targetCamera.getProjectionMatrix()),
      viewport.toGlobal(engine.getRenderWidth(), engine.getRenderHeight())
    );
    
    return [screenPos.x, screenPos.y];
  }

  screenToWorld(screenX, screenY, distance = 10, camera = null) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return [0, 0, 0];
    
    const ray = this.scene.createPickingRay(screenX, screenY, Matrix.Identity(), targetCamera);
    const point = ray.origin.add(ray.direction.scale(distance));
    
    return [point.x, point.y, point.z];
  }

  getScreenSize() {
    const engine = this.scene.getEngine();
    return {
      width: engine.getRenderWidth(),
      height: engine.getRenderHeight()
    };
  }

  // === GEOMETRY UTILITIES ===

  calculateTriangleArea(p1, p2, p3) {
    const a = new Vector3(...p1);
    const b = new Vector3(...p2);
    const c = new Vector3(...p3);
    
    const ab = b.subtract(a);
    const ac = c.subtract(a);
    const cross = Vector3.Cross(ab, ac);
    
    return cross.length() * 0.5;
  }

  calculateMeshSurfaceArea(mesh) {
    if (!mesh || !mesh.getVerticesData) return 0;
    
    const positions = mesh.getVerticesData('position');
    const indices = mesh.getIndices();
    
    if (!positions || !indices) return 0;
    
    let totalArea = 0;
    
    for (let i = 0; i < indices.length; i += 3) {
      const i1 = indices[i] * 3;
      const i2 = indices[i + 1] * 3;
      const i3 = indices[i + 2] * 3;
      
      const p1 = [positions[i1], positions[i1 + 1], positions[i1 + 2]];
      const p2 = [positions[i2], positions[i2 + 1], positions[i2 + 2]];
      const p3 = [positions[i3], positions[i3 + 1], positions[i3 + 2]];
      
      totalArea += this.calculateTriangleArea(p1, p2, p3);
    }
    
    return totalArea;
  }

  findNearestMesh(position, meshes = null) {
    const targetMeshes = meshes || this.scene.meshes;
    const pos = new Vector3(...position);
    
    let nearestMesh = null;
    let nearestDistance = Infinity;
    
    targetMeshes.forEach(mesh => {
      const distance = Vector3.Distance(pos, mesh.position);
      if (distance < nearestDistance) {
        nearestDistance = distance;
        nearestMesh = mesh;
      }
    });
    
    return { mesh: nearestMesh, distance: nearestDistance };
  }

  // === ACTION MANAGER UTILITIES ===

  createActionManager(mesh) {
    if (!mesh) return null;
    
    if (!mesh.actionManager) {
      mesh.actionManager = new ActionManager(this.scene);
    }
    return mesh.actionManager;
  }

  addClickAction(mesh, callback) {
    if (!mesh || !callback) return false;
    
    const actionManager = this.createActionManager(mesh);
    actionManager.registerAction(new ExecuteCodeAction(
      ActionManager.OnPickTrigger,
      callback
    ));
    return true;
  }

  addHoverAction(mesh, onEnter, onExit = null) {
    if (!mesh || !onEnter) return false;
    
    const actionManager = this.createActionManager(mesh);
    
    actionManager.registerAction(new ExecuteCodeAction(
      ActionManager.OnPointerOverTrigger,
      onEnter
    ));
    
    if (onExit) {
      actionManager.registerAction(new ExecuteCodeAction(
        ActionManager.OnPointerOutTrigger,
        onExit
      ));
    }
    
    return true;
  }

  addKeyAction(keyCode, callback) {
    if (!this.scene.actionManager) {
      this.scene.actionManager = new ActionManager(this.scene);
    }
    
    this.scene.actionManager.registerAction(new ExecuteCodeAction(
      { trigger: ActionManager.OnKeyDownTrigger, parameter: keyCode },
      callback
    ));
    return true;
  }

  // === PERFORMANCE UTILITIES ===

  optimizeScene() {
    let optimizations = 0;
    
    // Freeze world matrices for static meshes
    this.scene.meshes.forEach(mesh => {
      if (mesh.position.length() === 0 && mesh.rotation.length() === 0 && mesh.scaling.length() === 3) {
        mesh.freezeWorldMatrix();
        optimizations++;
      }
    });
    
    // Merge materials with same properties
    const materialGroups = new Map();
    this.scene.materials.forEach(material => {
      const key = this.getMaterialSignature(material);
      if (!materialGroups.has(key)) {
        materialGroups.set(key, []);
      }
      materialGroups.get(key).push(material);
    });
    
    // Dispose duplicate materials
    materialGroups.forEach(materials => {
      if (materials.length > 1) {
        const keepMaterial = materials[0];
        for (let i = 1; i < materials.length; i++) {
          // Replace material references
          this.scene.meshes.forEach(mesh => {
            if (mesh.material === materials[i]) {
              mesh.material = keepMaterial;
              optimizations++;
            }
          });
          materials[i].dispose();
        }
      }
    });
    
    return optimizations;
  }

  getMaterialSignature(material) {
    // Create a signature based on material properties
    return [
      material.getClassName(),
      material.diffuseColor?.toString() || '',
      material.specularColor?.toString() || '',
      material.emissiveColor?.toString() || '',
      material.alpha || 1
    ].join('|');
  }

  // === UTILITY FUNCTIONS ===

  generateGUID() {
    return Tools.GenerateUUID();
  }

  deepCopy(obj) {
    return JSON.parse(JSON.stringify(obj));
  }

  debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
      const later = () => {
        clearTimeout(timeout);
        func(...args);
      };
      clearTimeout(timeout);
      timeout = setTimeout(later, wait);
    };
  }

  throttle(func, limit) {
    let inThrottle;
    return function executedFunction(...args) {
      if (!inThrottle) {
        func.apply(this, args);
        inThrottle = true;
        setTimeout(() => inThrottle = false, limit);
      }
    };
  }

  // === COORDINATE CONVERSION ===

  localToWorld(localPosition, mesh) {
    if (!mesh) return localPosition;
    
    const local = new Vector3(...localPosition);
    const world = Vector3.TransformCoordinates(local, mesh.getWorldMatrix());
    return [world.x, world.y, world.z];
  }

  worldToLocal(worldPosition, mesh) {
    if (!mesh) return worldPosition;
    
    const world = new Vector3(...worldPosition);
    const invMatrix = mesh.getWorldMatrix().invert();
    const local = Vector3.TransformCoordinates(world, invMatrix);
    return [local.x, local.y, local.z];
  }

  // === ARRAY UTILITIES ===

  shuffle(array) {
    const shuffled = [...array];
    for (let i = shuffled.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
    }
    return shuffled;
  }

  chunk(array, size) {
    const chunks = [];
    for (let i = 0; i < array.length; i += size) {
      chunks.push(array.slice(i, i + size));
    }
    return chunks;
  }

  unique(array) {
    return [...new Set(array)];
  }

  groupBy(array, keyFn) {
    return array.reduce((groups, item) => {
      const key = keyFn(item);
      if (!groups[key]) groups[key] = [];
      groups[key].push(item);
      return groups;
    }, {});
  }

  // === VALIDATION ===

  isValidVector3(vec) {
    return Array.isArray(vec) && vec.length === 3 && vec.every(n => typeof n === 'number' && !isNaN(n));
  }

  isValidColor(color) {
    return Array.isArray(color) && 
           (color.length === 3 || color.length === 4) && 
           color.every(n => typeof n === 'number' && n >= 0 && n <= 1);
  }

  isValidQuaternion(quat) {
    return Array.isArray(quat) && quat.length === 4 && quat.every(n => typeof n === 'number' && !isNaN(n));
  }

  // === FORMAT CONVERSION ===

  arrayToVector3(arr) {
    if (!this.isValidVector3(arr)) return Vector3.Zero();
    return new Vector3(...arr);
  }

  vector3ToArray(vec3) {
    return [vec3.x, vec3.y, vec3.z];
  }

  arrayToColor3(arr) {
    if (!this.isValidColor(arr)) return Color3.White();
    return new Color3(...arr.slice(0, 3));
  }

  color3ToArray(color3) {
    return [color3.r, color3.g, color3.b];
  }

  // === HELPER FUNCTIONS ===

  wait(milliseconds) {
    return new Promise(resolve => setTimeout(resolve, milliseconds));
  }

  nextFrame() {
    return new Promise(resolve => requestAnimationFrame(resolve));
  }

  executeNextFrame(callback) {
    requestAnimationFrame(callback);
    return true;
  }

  // === SCENE UTILITIES ===

  findObjectsByTag(tag) {
    const results = [];
    
    // Check meshes
    this.scene.meshes.forEach(mesh => {
      if (mesh.getTags && mesh.getTags().includes(tag)) {
        results.push({ type: 'mesh', object: mesh });
      }
    });
    
    // Check lights
    this.scene.lights.forEach(light => {
      if (light.getTags && light.getTags().includes(tag)) {
        results.push({ type: 'light', object: light });
      }
    });
    
    // Check cameras
    this.scene.cameras.forEach(camera => {
      if (camera.getTags && camera.getTags().includes(tag)) {
        results.push({ type: 'camera', object: camera });
      }
    });
    
    return results;
  }

  addTagToObject(object, tag) {
    if (!object || !object.addTags) return false;
    object.addTags(tag);
    return true;
  }

  removeTagFromObject(object, tag) {
    if (!object || !object.removeTags) return false;
    object.removeTags(tag);
    return true;
  }

  // === STRING UTILITIES ===

  formatNumber(number, decimals = 2) {
    return Number(number).toFixed(decimals);
  }

  formatBytes(bytes) {
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    if (bytes === 0) return '0 Bytes';
    
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const value = bytes / Math.pow(1024, i);
    
    return `${value.toFixed(1)} ${sizes[i]}`;
  }

  formatTime(seconds) {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const secs = Math.floor(seconds % 60);
    
    if (hours > 0) {
      return `${hours}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
    } else {
      return `${minutes}:${secs.toString().padStart(2, '0')}`;
    }
  }
}