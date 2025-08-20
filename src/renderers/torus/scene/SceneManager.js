import { MathUtils } from '../math/MathUtils.js';

/**
 * Scene Manager - Handles cameras, lights, and scene rendering
 */
export class SceneManager {
  constructor() {
    this.cameras = new Map();
    this.lights = new Map();
    this.meshes = new Map();
    this.activeCamera = null;
    
    // Matrices
    this.viewMatrix = new Float32Array(16);
    this.projectionMatrix = new Float32Array(16);
    
    // Default camera setup
    this.cameraPosition = [0, 5, 10];
    this.cameraTarget = [0, 0, 0];
    this.cameraUp = [0, 1, 0];
  }

  async initialize() {
    this.updateViewMatrix();
    this.updateProjectionMatrix(800, 600); // Default size
    console.log('[Torus Scene] Manager initialized');
  }

  createScene(options = {}) {
    return { id: 'scene', options };
  }

  clearScene() {
    this.meshes.clear();
  }

  createCamera(type, options = {}) {
    const camera = { type, options };
    if (options.position) {
      this.cameraPosition = [options.position.x, options.position.y, options.position.z];
    }
    if (options.lookAt) {
      this.cameraTarget = [options.lookAt.x, options.lookAt.y, options.lookAt.z];
    }
    this.updateViewMatrix();
    return camera;
  }

  setActiveCamera(camera) {
    this.activeCamera = camera;
  }

  createLight(id, type, options = {}) {
    const light = { id, type, options };
    this.lights.set(id, light);
    return light;
  }

  createMesh(id, geometry, options = {}) {
    const mesh = {
      id,
      geometry,
      position: options.position || { x: 0, y: 0, z: 0 },
      rotation: options.rotation || { x: 0, y: 0, z: 0 },
      scale: options.scale || { x: 1, y: 1, z: 1 },
      color: options.color || { r: 1, g: 1, b: 1 },
      visible: true,
      type: options.type || 'mesh'
    };
    
    this.meshes.set(id, mesh);
    return mesh;
  }

  updateViewMatrix() {
    MathUtils.lookAt(this.viewMatrix, this.cameraPosition, this.cameraTarget, this.cameraUp);
  }

  updateProjectionMatrix(width = 800, height = 600) {
    const aspect = width / height;
    const fov = Math.PI / 4; // 45 degrees
    const near = 0.1;
    const far = 1000.0;
    
    MathUtils.perspective(this.projectionMatrix, fov, aspect, near, far);
  }

  getViewMatrix() {
    return this.viewMatrix;
  }

  getProjectionMatrix() {
    return this.projectionMatrix;
  }

  render(renderQueue, viewMatrix, projectionMatrix) {
    // Stub implementation - actual rendering handled elsewhere for now
    console.log(`[Torus Scene] Rendering ${renderQueue.length} objects`);
  }

  async dispose() {
    this.cameras.clear();
    this.lights.clear();
    this.meshes.clear();
    console.log('[Torus Scene] Disposed');
  }
}