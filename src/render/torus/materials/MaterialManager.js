/**
 * Material Manager - Handles material creation and properties
 */
export class MaterialManager {
  constructor() {
    this.gl = null;
    this.materials = new Map();
  }

  async initialize(gl) {
    this.gl = gl;
    console.log('[Torus Materials] Manager initialized');
  }

  createMaterial(id, type, options = {}) {
    const material = {
      id,
      type,
      options,
      properties: {
        color: options.color || { r: 1, g: 1, b: 1 },
        ...options
      }
    };
    
    this.materials.set(id, material);
    return material;
  }

  getMaterial(id) {
    return this.materials.get(id);
  }

  getCount() {
    return this.materials.size;
  }

  async dispose() {
    this.materials.clear();
    console.log('[Torus Materials] Disposed');
  }
}