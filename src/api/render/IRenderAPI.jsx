/**
 * IRenderAPI - Abstract interface for all rendering engines
 * All renderers (Babylon, Three.js, WebGPU, etc.) must implement this interface
 */

export class IRenderAPI {
  constructor(canvas, options = {}) {
    this.canvas = canvas;
    this.options = options;
    this.isInitialized = false;
    this.engine = null;
    this.scene = null;
    this.activeCamera = null;
    this.objects = new Map();
    this.materials = new Map();
    this.textures = new Map();
    this.lights = new Map();
  }

  // ============= Lifecycle =============
  
  /**
   * Initialize the rendering engine
   * @returns {Promise<void>}
   */
  async initialize() {
    throw new Error('initialize() must be implemented by renderer');
  }

  /**
   * Dispose of all resources
   * @returns {Promise<void>}
   */
  async dispose() {
    throw new Error('dispose() must be implemented by renderer');
  }

  /**
   * Handle canvas resize
   * @param {number} width 
   * @param {number} height 
   */
  resize(width, height) {
    throw new Error('resize() must be implemented by renderer');
  }

  // ============= Scene Management =============

  /**
   * Create a new scene
   * @param {Object} options 
   * @returns {Object} Scene object
   */
  createScene(options = {}) {
    throw new Error('createScene() must be implemented by renderer');
  }

  /**
   * Clear the current scene
   */
  clearScene() {
    throw new Error('clearScene() must be implemented by renderer');
  }

  /**
   * Set scene background color
   * @param {Object} color - {r, g, b, a}
   */
  setSceneBackground(color) {
    throw new Error('setSceneBackground() must be implemented by renderer');
  }

  /**
   * Enable/disable fog
   * @param {Object} fogOptions 
   */
  setFog(fogOptions) {
    throw new Error('setFog() must be implemented by renderer');
  }

  // ============= Camera =============

  /**
   * Create a camera
   * @param {string} type - 'universal', 'arcRotate', 'orthographic', etc.
   * @param {Object} options 
   * @returns {Object} Camera object
   */
  createCamera(type, options = {}) {
    throw new Error('createCamera() must be implemented by renderer');
  }

  /**
   * Set active camera
   * @param {Object} camera 
   */
  setActiveCamera(camera) {
    throw new Error('setActiveCamera() must be implemented by renderer');
  }

  /**
   * Update camera position
   * @param {Object} camera 
   * @param {Object} position - {x, y, z}
   */
  setCameraPosition(camera, position) {
    throw new Error('setCameraPosition() must be implemented by renderer');
  }

  /**
   * Update camera target/lookAt
   * @param {Object} camera 
   * @param {Object} target - {x, y, z}
   */
  setCameraTarget(camera, target) {
    throw new Error('setCameraTarget() must be implemented by renderer');
  }

  // ============= Lighting =============

  /**
   * Create a light
   * @param {string} type - 'directional', 'point', 'spot', 'hemispheric', 'ambient'
   * @param {Object} options 
   * @returns {string} Light ID
   */
  createLight(type, options = {}) {
    throw new Error('createLight() must be implemented by renderer');
  }

  /**
   * Update light properties
   * @param {string} lightId 
   * @param {Object} properties 
   */
  updateLight(lightId, properties) {
    throw new Error('updateLight() must be implemented by renderer');
  }

  /**
   * Remove light from scene
   * @param {string} lightId 
   */
  removeLight(lightId) {
    throw new Error('removeLight() must be implemented by renderer');
  }

  // ============= Geometry & Meshes =============

  /**
   * Create primitive geometry
   * @param {string} type - 'box', 'sphere', 'plane', 'cylinder', 'torus', etc.
   * @param {Object} options 
   * @returns {string} Mesh ID
   */
  createPrimitive(type, options = {}) {
    throw new Error('createPrimitive() must be implemented by renderer');
  }

  /**
   * Create mesh from geometry and material
   * @param {Object} geometry 
   * @param {Object} material 
   * @returns {string} Mesh ID
   */
  createMesh(geometry, material) {
    throw new Error('createMesh() must be implemented by renderer');
  }

  /**
   * Update mesh transform
   * @param {string} meshId 
   * @param {Object} transform - {position, rotation, scale}
   */
  updateMeshTransform(meshId, transform) {
    throw new Error('updateMeshTransform() must be implemented by renderer');
  }

  /**
   * Remove mesh from scene
   * @param {string} meshId 
   */
  removeMesh(meshId) {
    throw new Error('removeMesh() must be implemented by renderer');
  }

  // ============= Materials =============

  /**
   * Create material
   * @param {string} type - 'standard', 'pbr', 'unlit', 'shader'
   * @param {Object} options 
   * @returns {string} Material ID
   */
  createMaterial(type, options = {}) {
    throw new Error('createMaterial() must be implemented by renderer');
  }

  /**
   * Update material properties
   * @param {string} materialId 
   * @param {Object} properties 
   */
  updateMaterial(materialId, properties) {
    throw new Error('updateMaterial() must be implemented by renderer');
  }

  /**
   * Apply material to mesh
   * @param {string} meshId 
   * @param {string} materialId 
   */
  applyMaterial(meshId, materialId) {
    throw new Error('applyMaterial() must be implemented by renderer');
  }

  // ============= Textures =============

  /**
   * Load texture from URL
   * @param {string} url 
   * @param {Object} options 
   * @returns {Promise<string>} Texture ID
   */
  async loadTexture(url, options = {}) {
    throw new Error('loadTexture() must be implemented by renderer');
  }

  /**
   * Create texture from data
   * @param {Uint8Array|ImageData} data 
   * @param {Object} options 
   * @returns {string} Texture ID
   */
  createTexture(data, options = {}) {
    throw new Error('createTexture() must be implemented by renderer');
  }

  /**
   * Apply texture to material
   * @param {string} materialId 
   * @param {string} textureId 
   * @param {string} channel - 'diffuse', 'normal', 'specular', etc.
   */
  applyTexture(materialId, textureId, channel) {
    throw new Error('applyTexture() must be implemented by renderer');
  }

  // ============= Models & Assets =============

  /**
   * Load 3D model
   * @param {string} url 
   * @param {Object} options 
   * @returns {Promise<string>} Model ID
   */
  async loadModel(url, options = {}) {
    throw new Error('loadModel() must be implemented by renderer');
  }

  /**
   * Load model from data
   * @param {ArrayBuffer} data 
   * @param {string} format - 'gltf', 'obj', 'fbx', etc.
   * @param {Object} options 
   * @returns {Promise<string>} Model ID
   */
  async loadModelFromData(data, format, options = {}) {
    throw new Error('loadModelFromData() must be implemented by renderer');
  }

  // ============= Rendering =============

  /**
   * Render a single frame
   */
  render() {
    throw new Error('render() must be implemented by renderer');
  }

  /**
   * Start render loop
   * @param {Function} callback - Called before each frame
   */
  startRenderLoop(callback) {
    throw new Error('startRenderLoop() must be implemented by renderer');
  }

  /**
   * Stop render loop
   */
  stopRenderLoop() {
    throw new Error('stopRenderLoop() must be implemented by renderer');
  }

  /**
   * Take screenshot
   * @param {Object} options 
   * @returns {Promise<Blob>}
   */
  async screenshot(options = {}) {
    throw new Error('screenshot() must be implemented by renderer');
  }

  // ============= Utilities =============

  /**
   * Raycast from screen coordinates
   * @param {number} x - Screen X
   * @param {number} y - Screen Y
   * @returns {Object} Hit info or null
   */
  raycast(x, y) {
    throw new Error('raycast() must be implemented by renderer');
  }

  /**
   * Convert world to screen coordinates
   * @param {Object} position - {x, y, z}
   * @returns {Object} {x, y}
   */
  worldToScreen(position) {
    throw new Error('worldToScreen() must be implemented by renderer');
  }

  /**
   * Convert screen to world coordinates
   * @param {number} x 
   * @param {number} y 
   * @param {number} depth 
   * @returns {Object} {x, y, z}
   */
  screenToWorld(x, y, depth = 0) {
    throw new Error('screenToWorld() must be implemented by renderer');
  }

  // ============= Grid & Helpers =============

  /**
   * Create grid helper
   * @param {Object} options 
   * @returns {string} Grid ID
   */
  createGrid(options = {}) {
    throw new Error('createGrid() must be implemented by renderer');
  }

  /**
   * Create axis helper
   * @param {Object} options 
   * @returns {string} Helper ID
   */
  createAxisHelper(options = {}) {
    throw new Error('createAxisHelper() must be implemented by renderer');
  }

  // ============= Post-processing =============

  /**
   * Add post-processing effect
   * @param {string} type - 'bloom', 'dof', 'ssao', 'fxaa', etc.
   * @param {Object} options 
   * @returns {string} Effect ID
   */
  addPostEffect(type, options = {}) {
    throw new Error('addPostEffect() must be implemented by renderer');
  }

  /**
   * Remove post-processing effect
   * @param {string} effectId 
   */
  removePostEffect(effectId) {
    throw new Error('removePostEffect() must be implemented by renderer');
  }

  // ============= Animation =============

  /**
   * Create animation
   * @param {Object} target 
   * @param {Object} properties 
   * @param {number} duration 
   * @param {Object} options 
   * @returns {string} Animation ID
   */
  createAnimation(target, properties, duration, options = {}) {
    throw new Error('createAnimation() must be implemented by renderer');
  }

  /**
   * Play animation
   * @param {string} animationId 
   */
  playAnimation(animationId) {
    throw new Error('playAnimation() must be implemented by renderer');
  }

  /**
   * Stop animation
   * @param {string} animationId 
   */
  stopAnimation(animationId) {
    throw new Error('stopAnimation() must be implemented by renderer');
  }

  // ============= Physics (Optional) =============

  /**
   * Enable physics
   * @param {Object} options 
   */
  enablePhysics(options = {}) {
    console.warn('Physics not implemented for this renderer');
  }

  /**
   * Add physics body
   * @param {string} meshId 
   * @param {Object} options 
   */
  addPhysicsBody(meshId, options = {}) {
    console.warn('Physics not implemented for this renderer');
  }

  // ============= Renderer Info =============

  /**
   * Get renderer name
   * @returns {string}
   */
  getRendererName() {
    throw new Error('getRendererName() must be implemented by renderer');
  }

  /**
   * Get renderer capabilities
   * @returns {Object}
   */
  getCapabilities() {
    return {
      webgl: false,
      webgl2: false,
      webgpu: false,
      maxTextureSize: 0,
      maxLights: 0,
      supportsInstancing: false,
      supportsPhysics: false,
      supportsPostProcessing: false
    };
  }

  /**
   * Get performance stats
   * @returns {Object}
   */
  getStats() {
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
}

// Export renderer types enum
export const RendererType = {
  BABYLON: 'babylon',
  THREE: 'three',
  TORUS: 'torus',
  WEBGPU: 'webgpu',
  PLAYCANVAS: 'playcanvas',
  PIXI: 'pixi',
  PHASER: 'phaser',
  CUSTOM: 'custom'
};

// Export common material types
export const MaterialType = {
  STANDARD: 'standard',
  PBR: 'pbr',
  UNLIT: 'unlit',
  SHADER: 'shader',
  TOON: 'toon',
  MATCAP: 'matcap'
};

// Export common light types
export const LightType = {
  DIRECTIONAL: 'directional',
  POINT: 'point',
  SPOT: 'spot',
  HEMISPHERIC: 'hemispheric',
  AMBIENT: 'ambient'
};

// Export common primitive types
export const PrimitiveType = {
  BOX: 'box',
  SPHERE: 'sphere',
  PLANE: 'plane',
  CYLINDER: 'cylinder',
  CONE: 'cone',
  TORUS: 'torus',
  CAPSULE: 'capsule',
  TEXT: 'text'
};