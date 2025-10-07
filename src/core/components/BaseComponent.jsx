/**
 * Base component class for all engine components
 * Components add functionality to game objects (mesh, physics, audio, etc.)
 */
export class BaseComponent {
  constructor(babylonObject, data = {}, bridge = null) {
    this.babylonObject = babylonObject;
    this.bridge = bridge;
    this.data = { ...this.getDefaultData(), ...data };
    this.isActive = true;
    this.babylonResources = []; // Track created Babylon.js resources for cleanup
    
    // Component metadata
    this.componentType = this.constructor.name;
    this.componentId = `${this.componentType}_${Date.now()}_${Math.random().toString(36).substr(2, 5)}`;
    
    console.log(`🔧 Component created: ${this.componentType} for ${babylonObject.name}`);
  }

  /**
   * Override in subclasses to provide default data structure
   */
  getDefaultData() {
    return {};
  }

  /**
   * Override in subclasses to provide component schema for validation
   */
  static getSchema() {
    return {
      type: 'object',
      properties: {},
      required: []
    };
  }

  /**
   * Called when component is first created
   * Override in subclasses for initialization logic
   */
  onCreate() {
    // Override in subclasses
  }

  /**
   * Called every frame when the component is active
   * Override in subclasses for update logic
   */
  onUpdate(_deltaTime) {
    // Override in subclasses
  }

  /**
   * Called when component is being removed
   * Override in subclasses for cleanup logic
   */
  onDestroy() {
    // Override in subclasses
  }

  /**
   * Called when component data is updated
   * Override in subclasses to handle data changes
   */
  onDataUpdated(_newData) {
    // Override in subclasses
  }

  /**
   * Update component data and trigger update handlers
   */
  updateData(newData) {
    const oldData = { ...this.data };
    this.data = { ...this.data, ...newData };
    
    try {
      this.onDataUpdated(newData, oldData);
    } catch (error) {
      console.error(`❌ Error updating component ${this.componentType}:`, error);
    }
  }

  /**
   * Set component active/inactive
   */
  setActive(active) {
    if (this.isActive === active) return;
    
    this.isActive = active;
    
    if (active) {
      this.onActivated();
    } else {
      this.onDeactivated();
    }
  }

  /**
   * Called when component becomes active
   */
  onActivated() {
    // Override in subclasses
  }

  /**
   * Called when component becomes inactive
   */
  onDeactivated() {
    // Override in subclasses
  }

  /**
   * Register a Babylon.js resource for automatic cleanup
   */
  registerResource(resource) {
    if (resource && !this.babylonResources.includes(resource)) {
      this.babylonResources.push(resource);
    }
  }

  /**
   * Unregister a Babylon.js resource
   */
  unregisterResource(resource) {
    const index = this.babylonResources.indexOf(resource);
    if (index !== -1) {
      this.babylonResources.splice(index, 1);
    }
  }

  /**
   * Get the engine object ID for this component's babylon object
   */
  getEngineObjectId() {
    return this.bridge ? this.bridge.engineMap.get(this.babylonObject) : null;
  }

  /**
   * Get API methods to expose to RenScript
   * Override in subclasses to expose component-specific functionality
   */
  getRenScriptAPI() {
    return {
      // Base component API
      getId: () => this.componentId,
      getType: () => this.componentType,
      getData: () => ({ ...this.data }),
      updateData: (newData) => this.updateData(newData),
      setActive: (active) => this.setActive(active),
      isActive: () => this.isActive,
      
      // Babylon object access
      getBabylonObject: () => this.babylonObject,
      
      // Transform shortcuts (common to all components)
      getPosition: () => this.babylonObject.position,
      setPosition: (x, y, z) => {
        if (typeof x === 'object') {
          this.babylonObject.position = x;
        } else {
          this.babylonObject.position.set(x || 0, y || 0, z || 0);
        }
      },
      getRotation: () => this.babylonObject.rotation,
      setRotation: (x, y, z) => {
        if (typeof x === 'object') {
          this.babylonObject.rotation = x;
        } else {
          this.babylonObject.rotation.set(x || 0, y || 0, z || 0);
        }
      },
      getScale: () => this.babylonObject.scaling,
      setScale: (x, y, z) => {
        if (typeof x === 'object') {
          this.babylonObject.scaling = x;
        } else if (typeof x === 'number' && y === undefined) {
          this.babylonObject.scaling.setAll(x);
        } else {
          this.babylonObject.scaling.set(x || 1, y || 1, z || 1);
        }
      }
    };
  }

  /**
   * Serialize component data for saving
   */
  serialize() {
    return {
      componentType: this.componentType,
      data: { ...this.data },
      isActive: this.isActive
    };
  }

  /**
   * Deserialize component data when loading
   */
  static deserialize(babylonObject, serializedData, bridge) {
    const component = new this(babylonObject, serializedData.data, bridge);
    component.setActive(serializedData.isActive !== false);
    return component;
  }

  /**
   * Validate component data against schema
   */
  validateData(_data) {
    const _schema = this.constructor.getSchema();
    // TODO: Implement JSON schema validation
    return { valid: true, errors: [] };
  }

  /**
   * Dispose component and cleanup resources
   */
  dispose() {
    console.log(`🗑️ Disposing component: ${this.componentType}`);
    
    // Call destroy handler
    try {
      this.onDestroy();
    } catch (error) {
      console.error(`❌ Error in onDestroy for ${this.componentType}:`, error);
    }
    
    // Cleanup Babylon.js resources
    this.babylonResources.forEach(resource => {
      try {
        if (resource && typeof resource.dispose === 'function' && !resource.isDisposed()) {
          resource.dispose();
        }
      } catch (error) {
        console.error(`❌ Error disposing resource:`, error);
      }
    });
    
    this.babylonResources = [];
    this.babylonObject = null;
    this.bridge = null;
  }

  /**
   * Get component info for debugging
   */
  getDebugInfo() {
    return {
      componentType: this.componentType,
      componentId: this.componentId,
      isActive: this.isActive,
      dataKeys: Object.keys(this.data),
      resourceCount: this.babylonResources.length,
      babylonObjectName: this.babylonObject?.name || 'unknown'
    };
  }
}

export default BaseComponent;