import { IRenderAPI, MaterialType, LightType, PrimitiveType } from '../../api/IRenderAPI.js';
import Phaser from 'phaser';

/**
 * Phaser implementation of IRenderAPI
 * Note: Phaser is primarily a 2D game engine with some 3D capabilities
 */
export class PhaserRenderer extends IRenderAPI {
  constructor(canvas, options = {}) {
    super(canvas, options);
    this.game = null;
    this.mainScene = null;
    this.renderLoopCallback = null;
    this.nextId = 1;
    this.gameObjects = new Map();
    this.graphics = new Map();
    this.sprites = new Map();
  }

  // ============= Lifecycle =============

  async initialize() {
    if (this.isInitialized) return;

    try {
      console.log('[PhaserRenderer] Starting initialization...');
      
      // Check WebGL availability
      const testCanvas = document.createElement('canvas');
      const gl = testCanvas.getContext('webgl2') || testCanvas.getContext('webgl');
      if (!gl) {
        throw new Error('WebGL not supported');
      }
      console.log('[PhaserRenderer] WebGL context available');
      
      const self = this;
      
      class MainScene extends Phaser.Scene {
        constructor() {
          super({ key: 'MainScene' });
          self.mainScene = this;
        }

        create() {
          // Set background color
          this.cameras.main.setBackgroundColor('#1e1e1e');
          
          // Create simple test graphics
          const graphics = this.add.graphics();
          graphics.fillStyle(0x00ff00);
          graphics.fillRect(350, 250, 100, 100);
          
          console.log('[PhaserRenderer] Test rectangle created');
        }
      }

      // Simple Phaser config
      const config = {
        type: Phaser.WEBGL,
        canvas: this.canvas,
        width: this.canvas.clientWidth || 800,
        height: this.canvas.clientHeight || 600,
        backgroundColor: '#1e1e1e',
        scene: MainScene
      };

      console.log('[PhaserRenderer] Creating Phaser game...');
      this.game = new Phaser.Game(config);
      
      this.isInitialized = true;
      console.log('[PhaserRenderer] Initialized successfully');
    } catch (error) {
      console.error('[PhaserRenderer] Initialization failed:', error);
      throw error;
    }
  }

  async dispose() {
    if (this.game) {
      console.log('[PhaserRenderer] Starting disposal...');
      
      try {
        // Destroy the game and clean up WebGL context
        this.game.destroy(true, false); // removeCanvas = false to preserve the canvas element
        this.game = null;
        this.mainScene = null;
        
        // Force garbage collection if available
        if (window.gc) {
          window.gc();
        }
        
        console.log('[PhaserRenderer] Game destroyed');
      } catch (error) {
        console.error('[PhaserRenderer] Error during disposal:', error);
      }
    }
    
    // Clear all references
    this.gameObjects.clear();
    this.graphics.clear();
    this.sprites.clear();
    this.objects.clear();
    this.materials.clear();
    this.textures.clear();
    
    this.isInitialized = false;
    console.log('[PhaserRenderer] Disposed completely');
  }

  resize(width, height) {
    if (this.game) {
      this.game.scale.resize(width, height);
    }
  }

  // ============= Renderer Info =============

  getRendererName() {
    return 'Phaser';
  }

  getCapabilities() {
    return {
      webgl: true,
      webgl2: false,
      webgpu: false,
      maxTextureSize: 4096,
      maxLights: 10, // Limited lighting in 2D
      supportsInstancing: false,
      supportsPhysics: true,
      supportsPostProcessing: true
    };
  }

  getStats() {
    if (this.game) {
      const fps = this.game.loop.actualFps || 0;
      return {
        fps: fps,
        frameTime: fps > 0 ? 1000 / fps : 0,
        drawCalls: 0, // Would need custom tracking
        triangles: 0, // Not applicable for 2D
        meshes: this.objects.size,
        materials: this.materials.size,
        textures: this.textures.size
      };
    }
    
    return {
      fps: 0,
      frameTime: 0,
      drawCalls: 0,
      triangles: 0,
      meshes: 0,
      materials: 0,
      textures: 0
    };
  }

  // ============= Stub Methods =============
  // These are required by IRenderAPI but simplified for basic functionality

  createScene(options = {}) { return this.mainScene; }
  clearScene() { console.warn('[PhaserRenderer] clearScene not implemented'); }
  setSceneBackground(color) { console.warn('[PhaserRenderer] setSceneBackground not implemented'); }
  setFog(fogOptions) { console.warn('[PhaserRenderer] setFog not implemented'); }
  
  createCamera(type, options = {}) { return this.mainScene?.cameras.main; }
  setActiveCamera(camera) { this.activeCamera = camera; }
  setCameraPosition(camera, position) { console.warn('[PhaserRenderer] setCameraPosition not implemented'); }
  setCameraTarget(camera, target) { console.warn('[PhaserRenderer] setCameraTarget not implemented'); }
  
  createLight(type, options = {}) { return `light_${this.nextId++}`; }
  updateLight(lightId, properties) { console.warn('[PhaserRenderer] updateLight not implemented'); }
  removeLight(lightId) { console.warn('[PhaserRenderer] removeLight not implemented'); }
  
  createPrimitive(type, options = {}) { return `primitive_${this.nextId++}`; }
  createMesh(geometry, material) { return `mesh_${this.nextId++}`; }
  updateMeshTransform(meshId, transform) { console.warn('[PhaserRenderer] updateMeshTransform not implemented'); }
  removeMesh(meshId) { console.warn('[PhaserRenderer] removeMesh not implemented'); }
  
  createMaterial(type, options = {}) { return `material_${this.nextId++}`; }
  updateMaterial(materialId, properties) { console.warn('[PhaserRenderer] updateMaterial not implemented'); }
  applyMaterial(meshId, materialId) { console.warn('[PhaserRenderer] applyMaterial not implemented'); }
  
  async loadTexture(url, options = {}) { return `texture_${this.nextId++}`; }
  createTexture(data, options = {}) { return `texture_${this.nextId++}`; }
  applyTexture(materialId, textureId, channel) { console.warn('[PhaserRenderer] applyTexture not implemented'); }
  
  async loadModel(url, options = {}) { return `model_${this.nextId++}`; }
  async loadModelFromData(data, format, options = {}) { return null; }
  
  render() { /* Phaser renders automatically */ }
  startRenderLoop(callback) { this.renderLoopCallback = callback; }
  stopRenderLoop() { this.renderLoopCallback = null; }
  async screenshot(options = {}) { return null; }
  
  raycast(x, y) { return null; }
  worldToScreen(position) { return { x: 0, y: 0 }; }
  screenToWorld(x, y, depth = 0) { return { x: 0, y: 0, z: 0 }; }
  
  createGrid(options = {}) { return `grid_${this.nextId++}`; }
  createAxisHelper(options = {}) { return `axis_${this.nextId++}`; }
  
  addPostEffect(type, options = {}) { return `effect_${this.nextId++}`; }
  removePostEffect(effectId) { console.warn('[PhaserRenderer] removePostEffect not implemented'); }
  
  createAnimation(target, properties, duration, options = {}) { return `animation_${this.nextId++}`; }
  playAnimation(animationId) { console.warn('[PhaserRenderer] playAnimation not implemented'); }
  stopAnimation(animationId) { console.warn('[PhaserRenderer] stopAnimation not implemented'); }
  
  enablePhysics(options = {}) { console.warn('[PhaserRenderer] enablePhysics not implemented'); }
  addPhysicsBody(meshId, options = {}) { console.warn('[PhaserRenderer] addPhysicsBody not implemented'); }

  // ============= Helper Methods =============

  _colorToHex(color) {
    if (typeof color === 'number') return color;
    if (typeof color === 'string') return parseInt(color.replace('#', '0x'));
    
    const r = Math.floor((color.r || 0) * 255);
    const g = Math.floor((color.g || 0) * 255);
    const b = Math.floor((color.b || 0) * 255);
    
    return (r << 16) + (g << 8) + b;
  }
}