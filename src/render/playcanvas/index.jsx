import { IRenderAPI, MaterialType, LightType, PrimitiveType } from '../../api/IRenderAPI.js';
import * as pc from 'playcanvas';

/**
 * PlayCanvas implementation of IRenderAPI
 */
export class PlayCanvasRenderer extends IRenderAPI {
  constructor(canvas, options = {}) {
    super(canvas, options);
    this.app = null;
    this.renderLoopCallback = null;
    this.nextId = 1;
    this.entities = new Map();
  }

  // ============= Lifecycle =============

  async initialize() {
    if (this.isInitialized) return;

    try {
      console.log('[PlayCanvasRenderer] Starting initialization...');
      
      // Check WebGL availability
      const testCanvas = document.createElement('canvas');
      const gl = testCanvas.getContext('webgl2') || testCanvas.getContext('webgl');
      if (!gl) {
        throw new Error('WebGL not supported');
      }
      console.log('[PlayCanvasRenderer] WebGL context available');
      
      // Create PlayCanvas application with minimal options
      this.app = new pc.Application(this.canvas);

      // Basic configuration
      this.app.scene.ambientLight = new pc.Color(0.2, 0.2, 0.2);
      
      console.log('[PlayCanvasRenderer] App created, starting...');
      
      // Start the application
      this.app.start();
      
      console.log('[PlayCanvasRenderer] App started');

      // Create simple camera
      const cameraEntity = new pc.Entity('Camera');
      cameraEntity.addComponent('camera', {
        clearColor: new pc.Color(0.1, 0.1, 0.1)
      });
      cameraEntity.setPosition(3, 3, 3);
      cameraEntity.lookAt(0, 0, 0);
      this.app.root.addChild(cameraEntity);
      this.activeCamera = cameraEntity;
      
      console.log('[PlayCanvasRenderer] Camera created');

      // Create simple light
      const lightEntity = new pc.Entity('Light');
      lightEntity.addComponent('light', {
        type: pc.LIGHTTYPE_DIRECTIONAL,
        color: new pc.Color(1, 1, 1),
        intensity: 1
      });
      lightEntity.setPosition(5, 5, 5);
      this.app.root.addChild(lightEntity);
      
      console.log('[PlayCanvasRenderer] Light created');

      // Create simple test box
      const boxEntity = new pc.Entity('TestBox');
      boxEntity.addComponent('model', { type: 'box' });
      const material = new pc.StandardMaterial();
      material.diffuse = new pc.Color(0, 1, 0);
      material.update();
      boxEntity.model.material = material;
      boxEntity.setPosition(0, 0, 0);
      this.app.root.addChild(boxEntity);
      
      console.log('[PlayCanvasRenderer] Test box created');

      this.isInitialized = true;
      console.log('[PlayCanvasRenderer] Initialized successfully');
    } catch (error) {
      console.error('[PlayCanvasRenderer] Initialization failed:', error);
      throw error;
    }
  }

  async dispose() {
    if (this.app) {
      console.log('[PlayCanvasRenderer] Starting disposal...');
      
      try {
        // Stop the application first
        if (this.app.systems) {
          this.app.systems.destroy();
        }
        
        // Destroy all entities
        if (this.app.root) {
          this.app.root.destroy();
        }
        
        // Destroy the graphics device and WebGL context
        if (this.app.graphicsDevice) {
          this.app.graphicsDevice.destroy();
        }
        
        // Destroy the application
        this.app.destroy();
        this.app = null;
        
        // Force garbage collection if available
        if (window.gc) {
          window.gc();
        }
        
        console.log('[PlayCanvasRenderer] Application destroyed');
      } catch (error) {
        console.error('[PlayCanvasRenderer] Error during disposal:', error);
      }
    }
    
    // Clear all references
    this.entities.clear();
    this.objects.clear();
    this.materials.clear();
    this.textures.clear();
    this.lights.clear();
    this.scene = null;
    this.activeCamera = null;
    
    this.isInitialized = false;
    console.log('[PlayCanvasRenderer] Disposed completely');
  }

  resize(width, height) {
    if (this.app) {
      this.app.resizeCanvas(width, height);
    }
  }

  // ============= Renderer Info =============

  getRendererName() {
    return 'PlayCanvas';
  }

  getCapabilities() {
    const device = this.app?.graphicsDevice;
    return {
      webgl: true,
      webgl2: device?.webgl2 || false,
      webgpu: false,
      maxTextureSize: device?.maxTextureSize || 4096,
      maxLights: 32,
      supportsInstancing: true,
      supportsPhysics: true,
      supportsPostProcessing: true
    };
  }

  getStats() {
    const stats = this.app?.stats;
    return {
      fps: stats?.frame.fps || 0,
      frameTime: stats?.frame.ms || 0,
      drawCalls: stats?.drawCalls.total || 0,
      triangles: stats?.triangles || 0,
      meshes: this.objects.size,
      materials: this.materials.size,
      textures: this.textures.size
    };
  }

  // ============= Stub Methods =============
  // These are required by IRenderAPI but simplified for basic functionality

  createScene(options = {}) { return this.app?.scene || null; }
  clearScene() { console.warn('[PlayCanvasRenderer] clearScene not implemented'); }
  setSceneBackground(color) { console.warn('[PlayCanvasRenderer] setSceneBackground not implemented'); }
  setFog(fogOptions) { console.warn('[PlayCanvasRenderer] setFog not implemented'); }
  
  createCamera(type, options = {}) { return this.activeCamera; }
  setActiveCamera(camera) { this.activeCamera = camera; }
  setCameraPosition(camera, position) { console.warn('[PlayCanvasRenderer] setCameraPosition not implemented'); }
  setCameraTarget(camera, target) { console.warn('[PlayCanvasRenderer] setCameraTarget not implemented'); }
  
  createLight(type, options = {}) { return `light_${this.nextId++}`; }
  updateLight(lightId, properties) { console.warn('[PlayCanvasRenderer] updateLight not implemented'); }
  removeLight(lightId) { console.warn('[PlayCanvasRenderer] removeLight not implemented'); }
  
  createPrimitive(type, options = {}) { return `primitive_${this.nextId++}`; }
  createMesh(geometry, material) { return `mesh_${this.nextId++}`; }
  updateMeshTransform(meshId, transform) { console.warn('[PlayCanvasRenderer] updateMeshTransform not implemented'); }
  removeMesh(meshId) { console.warn('[PlayCanvasRenderer] removeMesh not implemented'); }
  
  createMaterial(type, options = {}) { return `material_${this.nextId++}`; }
  updateMaterial(materialId, properties) { console.warn('[PlayCanvasRenderer] updateMaterial not implemented'); }
  applyMaterial(meshId, materialId) { console.warn('[PlayCanvasRenderer] applyMaterial not implemented'); }
  
  async loadTexture(url, options = {}) { return `texture_${this.nextId++}`; }
  createTexture(data, options = {}) { return `texture_${this.nextId++}`; }
  applyTexture(materialId, textureId, channel) { console.warn('[PlayCanvasRenderer] applyTexture not implemented'); }
  
  async loadModel(url, options = {}) { return `model_${this.nextId++}`; }
  async loadModelFromData(data, format, options = {}) { return null; }
  
  render() { /* PlayCanvas renders automatically */ }
  startRenderLoop(callback) { this.renderLoopCallback = callback; }
  stopRenderLoop() { this.renderLoopCallback = null; }
  async screenshot(options = {}) { return null; }
  
  raycast(x, y) { return null; }
  worldToScreen(position) { return { x: 0, y: 0 }; }
  screenToWorld(x, y, depth = 0) { return { x: 0, y: 0, z: 0 }; }
  
  createGrid(options = {}) { return `grid_${this.nextId++}`; }
  createAxisHelper(options = {}) { return `axis_${this.nextId++}`; }
  
  addPostEffect(type, options = {}) { return `effect_${this.nextId++}`; }
  removePostEffect(effectId) { console.warn('[PlayCanvasRenderer] removePostEffect not implemented'); }
  
  createAnimation(target, properties, duration, options = {}) { return `animation_${this.nextId++}`; }
  playAnimation(animationId) { console.warn('[PlayCanvasRenderer] playAnimation not implemented'); }
  stopAnimation(animationId) { console.warn('[PlayCanvasRenderer] stopAnimation not implemented'); }
  
  enablePhysics(options = {}) { console.warn('[PlayCanvasRenderer] enablePhysics not implemented'); }
  addPhysicsBody(meshId, options = {}) { console.warn('[PlayCanvasRenderer] addPhysicsBody not implemented'); }
}