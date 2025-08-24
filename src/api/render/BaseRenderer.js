// Base renderer API that all renderers must implement

export class BaseRenderer {
  constructor(config) {
    this.id = config.id;
    this.name = config.name;
    this.config = config;
    this.initialized = false;
  }

  // Core rendering lifecycle methods
  async initialize(canvas, options = {}) {
    throw new Error('initialize() must be implemented by renderer');
  }

  async render(sceneData) {
    throw new Error('render() must be implemented by renderer');
  }

  async resize(width, height) {
    throw new Error('resize() must be implemented by renderer');
  }

  async dispose() {
    throw new Error('dispose() must be implemented by renderer');
  }

  // Scene management
  async loadScene(sceneData) {
    throw new Error('loadScene() must be implemented by renderer');
  }

  async updateScene(sceneData) {
    throw new Error('updateScene() must be implemented by renderer');
  }

  // Camera controls
  async updateCamera(cameraData) {
    throw new Error('updateCamera() must be implemented by renderer');
  }

  // Camera movement methods
  async moveCamera(direction, distance) {
    throw new Error('moveCamera() must be implemented by renderer');
  }

  async rotateCamera(deltaX, deltaY) {
    throw new Error('rotateCamera() must be implemented by renderer');
  }

  async panCamera(deltaX, deltaY) {
    throw new Error('panCamera() must be implemented by renderer');
  }

  getCameraPosition() {
    throw new Error('getCameraPosition() must be implemented by renderer');
  }

  getCameraRotation() {
    throw new Error('getCameraRotation() must be implemented by renderer');
  }

  // Lighting system
  async updateLights(lightData) {
    throw new Error('updateLights() must be implemented by renderer');
  }

  // Object manipulation
  async addObject(objectData) {
    throw new Error('addObject() must be implemented by renderer');
  }

  async removeObject(objectId) {
    throw new Error('removeObject() must be implemented by renderer');
  }

  async updateObject(objectId, objectData) {
    throw new Error('updateObject() must be implemented by renderer');
  }

  // Material system
  async updateMaterial(materialId, materialData) {
    throw new Error('updateMaterial() must be implemented by renderer');
  }

  // Performance and debugging
  getStats() {
    throw new Error('getStats() must be implemented by renderer');
  }

  async captureFrame() {
    throw new Error('captureFrame() must be implemented by renderer');
  }

  // Event handling
  onReady(callback) {
    this.readyCallback = callback;
  }

  onError(callback) {
    this.errorCallback = callback;
  }

  // Helper methods for implementations
  _notifyReady() {
    this.initialized = true;
    if (this.readyCallback) this.readyCallback();
  }

  _notifyError(error) {
    if (this.errorCallback) this.errorCallback(error);
  }

  // Utility methods
  isInitialized() {
    return this.initialized;
  }

  getConfig() {
    return this.config;
  }
}