import { IRenderAPI, MaterialType, LightType, PrimitiveType } from '../../api/IRenderAPI.js';
import * as PIXI from 'pixi.js';

/**
 * PixiJS implementation of IRenderAPI
 * Note: PixiJS is primarily a 2D renderer with some 3D capabilities
 */
export class PixiRenderer extends IRenderAPI {
  constructor(canvas, options = {}) {
    super(canvas, options);
    this.app = null;
    this.renderLoopCallback = null;
    this.nextId = 1;
    this.sprites = new Map();
    this.containers = new Map();
    this.graphics = new Map();
  }

  // ============= Lifecycle =============

  async initialize() {
    if (this.isInitialized) return;

    try {
      console.log('[PixiRenderer] Starting initialization...');
      
      // Check WebGL availability
      const canvas = document.createElement('canvas');
      const gl = canvas.getContext('webgl2') || canvas.getContext('webgl');
      if (!gl) {
        throw new Error('WebGL not supported');
      }
      console.log('[PixiRenderer] WebGL context available');
      
      // Create PixiJS application
      this.app = new PIXI.Application();
      
      await this.app.init({
        canvas: this.canvas,
        width: this.canvas.clientWidth || 800,
        height: this.canvas.clientHeight || 600,
        antialias: true,
        resolution: 1,
        autoDensity: false,
        backgroundColor: 0x1e1e1e
      });

      console.log('[PixiRenderer] PIXI app initialized, canvas size:', this.app.screen.width, 'x', this.app.screen.height);

      // Create main container for scene
      this.scene = new PIXI.Container();
      this.app.stage.addChild(this.scene);

      // Set up a scale factor to convert from 3D units to pixels
      this.unitScale = 100; // 1 3D unit = 100 pixels
      
      // Position scene at center of screen
      this.scene.position.set(this.app.screen.width / 2, this.app.screen.height / 2);
      
      // Create a simple test rectangle to verify rendering
      const testRect = new PIXI.Graphics();
      testRect.rect(-50, -50, 100, 100);
      testRect.fill({ color: 0x00ff00, alpha: 1 });
      this.scene.addChild(testRect);
      
      console.log('[PixiRenderer] Test rectangle added to scene');

      this.isInitialized = true;
      console.log('[PixiRenderer] Initialized successfully');
    } catch (error) {
      console.error('[PixiRenderer] Initialization failed:', error);
      throw error;
    }
  }

  async dispose() {
    if (this.app) {
      console.log('[PixiRenderer] Starting disposal...');
      
      // Stop any running loops
      this.app.ticker.stop();
      
      // Destroy the application and clean up WebGL context
      this.app.destroy(true, { 
        children: true, 
        texture: true, 
        baseTexture: true,
        context: true // This should clean up the WebGL context
      });
      this.app = null;
      
      // Force garbage collection if available
      if (window.gc) {
        window.gc();
      }
      
      console.log('[PixiRenderer] Application destroyed');
    }
    
    // Clear all references
    this.sprites.clear();
    this.containers.clear();
    this.graphics.clear();
    this.objects.clear();
    this.materials.clear();
    this.textures.clear();
    this.scene = null;
    this.activeCamera = null;
    
    this.isInitialized = false;
    console.log('[PixiRenderer] Disposed completely');
  }

  resize(width, height) {
    if (this.app) {
      this.app.renderer.resize(width, height);
    }
  }

  // ============= Scene Management =============

  createScene(options = {}) {
    if (!this.scene) {
      this.scene = new PIXI.Container();
      this.app.stage.addChild(this.scene);
    }
    return this.scene;
  }

  clearScene() {
    if (this.scene) {
      this.scene.removeChildren();
      this.sprites.clear();
      this.containers.clear();
      this.graphics.clear();
      this.objects.clear();
    }
  }

  setSceneBackground(color) {
    if (this.app) {
      const hexColor = this._colorToHex(color);
      this.app.renderer.background.color = hexColor;
    }
  }

  setFog(fogOptions) {
    // PixiJS doesn't have built-in fog support
    console.warn('[PixiRenderer] Fog is not supported in PixiJS');
  }

  // ============= Camera (Simulated for 2D) =============

  createCamera(type, options = {}) {
    // In PixiJS, we simulate camera with viewport transforms
    const defaultX = this.app ? this.app.screen.width / 2 : 400;
    const defaultY = this.app ? this.app.screen.height / 2 : 300;
    
    this.activeCamera = {
      x: options.position?.x || defaultX,
      y: options.position?.y || defaultY,
      zoom: options.zoom || 1,
      rotation: options.rotation || 0
    };
    return this.activeCamera;
  }

  setActiveCamera(camera) {
    this.activeCamera = camera;
    this._updateCameraTransform();
  }

  setCameraPosition(camera, position) {
    if (camera) {
      camera.x = position.x;
      camera.y = position.y;
      this._updateCameraTransform();
    }
  }

  setCameraTarget(camera, target) {
    // In 2D, we just move the camera to look at the target
    if (camera) {
      camera.x = target.x;
      camera.y = target.y;
      this._updateCameraTransform();
    }
  }

  _updateCameraTransform() {
    if (this.scene && this.activeCamera && this.app) {
      this.scene.position.set(
        this.app.screen.width / 2 - this.activeCamera.x * this.activeCamera.zoom,
        this.app.screen.height / 2 - this.activeCamera.y * this.activeCamera.zoom
      );
      this.scene.scale.set(this.activeCamera.zoom);
      this.scene.rotation = this.activeCamera.rotation;
    }
  }

  // ============= Lighting (Not applicable for 2D) =============

  createLight(type, options = {}) {
    console.warn('[PixiRenderer] Traditional 3D lighting is not supported in PixiJS');
    return `light_${this.nextId++}`;
  }

  updateLight(lightId, properties) {
    console.warn('[PixiRenderer] Traditional 3D lighting is not supported in PixiJS');
  }

  removeLight(lightId) {
    console.warn('[PixiRenderer] Traditional 3D lighting is not supported in PixiJS');
  }

  // ============= Geometry & Meshes =============

  createPrimitive(type, options = {}) {
    const id = `primitive_${this.nextId++}`;
    const graphics = new PIXI.Graphics();
    
    const color = this._colorToHex(options.color || { r: 1, g: 1, b: 1 });
    const alpha = options.alpha || 1;
    const scale = this.unitScale || 100;

    switch (type) {
      case 'box':
      case 'plane':
        const width = (options.width || 1) * scale;
        const height = (options.height || 1) * scale;
        graphics.rect(-width/2, -height/2, width, height);
        graphics.fill({ color, alpha });
        break;
        
      case 'sphere':
        const radius = (options.radius || 0.5) * scale;
        graphics.circle(0, 0, radius);
        graphics.fill({ color, alpha });
        break;
        
      case 'cylinder':
        // Approximate with ellipse
        const radiusX = (options.radiusTop || options.radius || 0.5) * scale;
        const radiusY = (options.height || 1) * scale;
        graphics.ellipse(0, 0, radiusX, radiusY/2);
        graphics.fill({ color, alpha });
        break;
        
      case 'torus':
        // Draw as ring - outer circle
        const outerRadius = (options.radius || 0.5) * scale;
        const innerRadius = (options.tube || 0.2) * scale;
        graphics.circle(0, 0, outerRadius);
        graphics.fill({ color, alpha });
        // Inner hole
        graphics.circle(0, 0, innerRadius);
        graphics.cut();
        break;
        
      default:
        graphics.rect(-scale/2, -scale/2, scale, scale);
        graphics.fill({ color, alpha });
    }
    
    // Set position (convert from 3D units to pixels)
    if (options.position) {
      graphics.position.set(
        (options.position.x || 0) * scale, 
        -(options.position.y || 0) * scale  // Invert Y for 2D (Y goes down in screen space)
      );
    }
    
    // Set rotation (z-axis only in 2D)
    if (options.rotation) {
      graphics.rotation = options.rotation.z || 0;
    }
    
    // Set scale
    if (options.scale) {
      const scaleValue = typeof options.scale === 'number' ? options.scale : (options.scale.x || 1);
      graphics.scale.set(scaleValue, scaleValue);
    }
    
    this.scene.addChild(graphics);
    this.graphics.set(id, graphics);
    this.objects.set(id, graphics);
    
    return id;
  }

  createMesh(geometry, material) {
    console.warn('[PixiRenderer] Mesh creation is limited in PixiJS');
    return this.createPrimitive('box');
  }

  updateMeshTransform(meshId, transform) {
    const obj = this.objects.get(meshId);
    if (obj) {
      const scale = this.unitScale || 100;
      if (transform.position) {
        obj.position.set(
          (transform.position.x || 0) * scale, 
          -(transform.position.y || 0) * scale
        );
      }
      if (transform.rotation) {
        obj.rotation = transform.rotation.z || 0;
      }
      if (transform.scale) {
        const scaleValue = typeof transform.scale === 'number' ? transform.scale : (transform.scale.x || 1);
        obj.scale.set(scaleValue, scaleValue);
      }
    }
  }

  removeMesh(meshId) {
    const obj = this.objects.get(meshId);
    if (obj) {
      obj.destroy();
      this.objects.delete(meshId);
      this.graphics.delete(meshId);
      this.sprites.delete(meshId);
    }
  }

  // ============= Materials =============

  createMaterial(type, options = {}) {
    // PixiJS doesn't have traditional materials, return a color/style object
    const id = `material_${this.nextId++}`;
    this.materials.set(id, {
      type,
      color: options.color || { r: 1, g: 1, b: 1 },
      alpha: options.alpha || 1,
      ...options
    });
    return id;
  }

  updateMaterial(materialId, properties) {
    const material = this.materials.get(materialId);
    if (material) {
      Object.assign(material, properties);
    }
  }

  applyMaterial(meshId, materialId) {
    const obj = this.objects.get(meshId);
    const material = this.materials.get(materialId);
    if (obj && material && obj instanceof PIXI.Graphics) {
      // Redraw with new material properties
      obj.clear();
      obj.rect(-50, -50, 100, 100);
      obj.fill({ 
        color: this._colorToHex(material.color), 
        alpha: material.alpha 
      });
    }
  }

  // ============= Textures =============

  async loadTexture(url, options = {}) {
    const id = `texture_${this.nextId++}`;
    const texture = await PIXI.Assets.load(url);
    this.textures.set(id, texture);
    return id;
  }

  createTexture(data, options = {}) {
    const id = `texture_${this.nextId++}`;
    const texture = PIXI.Texture.from(data);
    this.textures.set(id, texture);
    return id;
  }

  applyTexture(materialId, textureId, channel) {
    // In PixiJS, textures are applied directly to sprites
    console.warn('[PixiRenderer] Texture application works differently in PixiJS - use sprites instead');
  }

  // ============= Models & Assets =============

  async loadModel(url, options = {}) {
    // PixiJS doesn't support 3D models, load as sprite instead
    console.warn('[PixiRenderer] 3D models not supported, loading as sprite');
    const id = `model_${this.nextId++}`;
    const texture = await PIXI.Assets.load(url);
    const sprite = new PIXI.Sprite(texture);
    
    if (options.position) {
      sprite.position.set(options.position.x || 0, options.position.y || 0);
    }
    
    this.scene.addChild(sprite);
    this.sprites.set(id, sprite);
    this.objects.set(id, sprite);
    
    return id;
  }

  async loadModelFromData(data, format, options = {}) {
    console.warn('[PixiRenderer] 3D models not supported in PixiJS');
    return null;
  }

  // ============= Rendering =============

  render() {
    if (this.app) {
      this.app.render();
    }
  }

  startRenderLoop(callback) {
    this.renderLoopCallback = callback;
    
    if (this.app) {
      this.app.ticker.add((delta) => {
        if (this.renderLoopCallback) {
          this.renderLoopCallback(delta);
        }
      });
    }
  }

  stopRenderLoop() {
    if (this.app && this.renderLoopCallback) {
      this.app.ticker.remove(this.renderLoopCallback);
      this.renderLoopCallback = null;
    }
  }

  async screenshot(options = {}) {
    if (this.app) {
      const renderTexture = PIXI.RenderTexture.create({
        width: this.app.screen.width,
        height: this.app.screen.height
      });
      
      this.app.renderer.render(this.app.stage, { renderTexture });
      const canvas = this.app.renderer.extract.canvas(renderTexture);
      
      return new Promise((resolve) => {
        canvas.toBlob(resolve, options.format || 'image/png', options.quality || 1);
      });
    }
    return null;
  }

  // ============= Utilities =============

  raycast(x, y) {
    // Simple hit testing in PixiJS
    const point = new PIXI.Point(x, y);
    const hitObject = this.app.renderer.plugins.interaction?.hitTest(point);
    
    if (hitObject) {
      return {
        object: hitObject,
        point: { x, y, z: 0 },
        distance: 0
      };
    }
    return null;
  }

  worldToScreen(position) {
    if (!this.app || !this.activeCamera) return { x: 0, y: 0 };
    
    // In 2D, world coords are screen coords with camera transform
    const screenX = (position.x - this.activeCamera.x) * this.activeCamera.zoom + this.app.screen.width / 2;
    const screenY = (position.y - this.activeCamera.y) * this.activeCamera.zoom + this.app.screen.height / 2;
    return { x: screenX, y: screenY };
  }

  screenToWorld(x, y, depth = 0) {
    if (!this.app || !this.activeCamera) return { x: 0, y: 0, z: 0 };
    
    // Inverse of worldToScreen
    const worldX = (x - this.app.screen.width / 2) / this.activeCamera.zoom + this.activeCamera.x;
    const worldY = (y - this.app.screen.height / 2) / this.activeCamera.zoom + this.activeCamera.y;
    return { x: worldX, y: worldY, z: 0 };
  }

  // ============= Grid & Helpers =============

  createGrid(options = {}) {
    const id = `grid_${this.nextId++}`;
    const graphics = new PIXI.Graphics();
    
    const scale = this.unitScale || 100;
    const size = (options.size || 10) * scale;
    const divisions = options.divisions || 10;
    const step = size / divisions;
    const color = this._colorToHex(options.color || { r: 0.3, g: 0.3, b: 0.3 });
    
    // Draw grid lines
    for (let i = 0; i <= divisions; i++) {
      const pos = -size/2 + i * step;
      // Vertical lines
      graphics.moveTo(pos, -size/2);
      graphics.lineTo(pos, size/2);
      // Horizontal lines
      graphics.moveTo(-size/2, pos);
      graphics.lineTo(size/2, pos);
    }
    
    graphics.stroke({ width: 1, color, alpha: 0.5 });
    
    this.scene.addChild(graphics);
    this.graphics.set(id, graphics);
    
    return id;
  }

  createAxisHelper(options = {}) {
    const id = `axis_${this.nextId++}`;
    const graphics = new PIXI.Graphics();
    const scale = this.unitScale || 100;
    const size = (options.size || 1) * scale;
    
    // X axis - red (pointing right)
    graphics.moveTo(0, 0);
    graphics.lineTo(size, 0);
    graphics.stroke({ width: 2, color: 0xff0000 });
    
    // Y axis - green (pointing up in 3D, but down in 2D screen space)
    graphics.moveTo(0, 0);
    graphics.lineTo(0, -size);
    graphics.stroke({ width: 2, color: 0x00ff00 });
    
    this.scene.addChild(graphics);
    this.graphics.set(id, graphics);
    
    return id;
  }

  // ============= Post-processing =============

  addPostEffect(type, options = {}) {
    console.warn('[PixiRenderer] Post-processing effects limited in PixiJS');
    return `effect_${this.nextId++}`;
  }

  removePostEffect(effectId) {
    console.warn('[PixiRenderer] Post-processing effects limited in PixiJS');
  }

  // ============= Animation =============

  createAnimation(target, properties, duration, options = {}) {
    console.warn('[PixiRenderer] Use GSAP or similar for animations with PixiJS');
    return `animation_${this.nextId++}`;
  }

  playAnimation(animationId) {
    console.warn('[PixiRenderer] Animation system not implemented');
  }

  stopAnimation(animationId) {
    console.warn('[PixiRenderer] Animation system not implemented');
  }

  // ============= Renderer Info =============

  getRendererName() {
    return 'PixiJS';
  }

  getCapabilities() {
    return {
      webgl: true,
      webgl2: this.app?.renderer.context.webGLVersion === 2,
      webgpu: false,
      maxTextureSize: 4096,
      maxLights: 0, // No traditional lighting
      supportsInstancing: true,
      supportsPhysics: false,
      supportsPostProcessing: true
    };
  }

  getStats() {
    return {
      fps: this.app?.ticker.FPS || 0,
      frameTime: this.app?.ticker.deltaMS || 0,
      drawCalls: 0, // Would need to track manually
      triangles: 0, // Not applicable for 2D
      meshes: this.objects.size,
      materials: this.materials.size,
      textures: this.textures.size
    };
  }

  // ============= Helper Methods =============

  _colorToHex(color) {
    if (typeof color === 'number') return color;
    const r = Math.floor((color.r || 0) * 255);
    const g = Math.floor((color.g || 0) * 255);
    const b = Math.floor((color.b || 0) * 255);
    return (r << 16) + (g << 8) + b;
  }
}