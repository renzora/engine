import { BaseComponent } from './BaseComponent.jsx';

// Built-in components
import { MeshComponent } from './MeshComponent.jsx';
import { MaterialComponent } from './MaterialComponent.jsx';
import { RigidBodyComponent } from './RigidBodyComponent.jsx';
import { AudioSourceComponent } from './AudioSourceComponent.jsx';
import { ScriptComponent } from './ScriptComponent.jsx';
import { LightComponent } from './LightComponent.jsx';
import { CameraComponent } from './CameraComponent.jsx';
import { ParticleSystemComponent } from './ParticleSystemComponent.jsx';
import { AnimationComponent } from './AnimationComponent.jsx';

/**
 * Registry for all component types in the engine
 * Manages component registration, creation, and validation
 */
export class ComponentRegistry {
  constructor() {
    this.components = new Map();
    this.componentSchemas = new Map();
    
    // Register built-in components
    this.registerBuiltInComponents();
    
    console.log('📋 ComponentRegistry: Initialized');
  }

  /**
   * Register all built-in engine components
   */
  registerBuiltInComponents() {
    // Core rendering components
    this.registerComponent('mesh', MeshComponent);
    this.registerComponent('material', MaterialComponent);
    this.registerComponent('light', LightComponent);
    this.registerComponent('camera', CameraComponent);
    this.registerComponent('particles', ParticleSystemComponent);
    
    // Physics components
    this.registerComponent('rigidBody', RigidBodyComponent);
    this.registerComponent('rigidbody', RigidBodyComponent); // Alias
    
    // Audio components
    this.registerComponent('audioSource', AudioSourceComponent);
    this.registerComponent('audio', AudioSourceComponent); // Alias
    
    // Animation components
    this.registerComponent('animation', AnimationComponent);
    this.registerComponent('animator', AnimationComponent); // Alias
    
    // Scripting components
    this.registerComponent('script', ScriptComponent);
    this.registerComponent('renscript', ScriptComponent); // Alias
    
    console.log(`📋 Registered ${this.components.size} built-in components`);
  }

  /**
   * Register a new component type
   */
  registerComponent(name, ComponentClass, options = {}) {
    // Validate component class
    if (!ComponentClass || typeof ComponentClass !== 'function') {
      throw new Error(`Invalid component class for ${name}`);
    }

    // Ensure component extends BaseComponent
    if (!this.isValidComponentClass(ComponentClass)) {
      throw new Error(`Component ${name} must extend BaseComponent`);
    }

    // Register the component
    this.components.set(name, {
      ComponentClass,
      options: {
        category: options.category || 'custom',
        description: options.description || '',
        icon: options.icon || 'component',
        hidden: options.hidden || false,
        ...options
      }
    });

    // Register schema if available
    if (ComponentClass.getSchema && typeof ComponentClass.getSchema === 'function') {
      this.componentSchemas.set(name, ComponentClass.getSchema());
    }

    console.log(`📋 Registered component: ${name} (${options.category || 'custom'})`);
    return true;
  }

  /**
   * Unregister a component type
   */
  unregisterComponent(name) {
    const removed = this.components.delete(name);
    this.componentSchemas.delete(name);
    
    if (removed) {
      console.log(`📋 Unregistered component: ${name}`);
    }
    
    return removed;
  }

  /**
   * Get a component class by name
   */
  getComponent(name) {
    const component = this.components.get(name);
    return component ? component.ComponentClass : null;
  }

  /**
   * Get component metadata by name
   */
  getComponentMetadata(name) {
    const component = this.components.get(name);
    return component ? component.options : null;
  }

  /**
   * Get component schema by name
   */
  getComponentSchema(name) {
    return this.componentSchemas.get(name) || null;
  }

  /**
   * Check if a component type exists
   */
  hasComponent(name) {
    return this.components.has(name);
  }

  /**
   * Get all registered component names
   */
  getComponentNames() {
    return Array.from(this.components.keys());
  }

  /**
   * Get all components in a category
   */
  getComponentsByCategory(category) {
    const result = [];
    this.components.forEach((component, name) => {
      if (component.options.category === category) {
        result.push({
          name,
          ComponentClass: component.ComponentClass,
          metadata: component.options
        });
      }
    });
    return result;
  }

  /**
   * Get all component categories
   */
  getCategories() {
    const categories = new Set();
    this.components.forEach(component => {
      categories.add(component.options.category);
    });
    return Array.from(categories).sort();
  }

  /**
   * Create a component instance
   */
  createComponent(name, babylonObject, data = {}, bridge = null) {
    const ComponentClass = this.getComponent(name);
    if (!ComponentClass) {
      console.error(`❌ Unknown component type: ${name}`);
      return null;
    }

    try {
      const component = new ComponentClass(babylonObject, data, bridge);
      return component;
    } catch (error) {
      console.error(`❌ Failed to create component ${name}:`, error);
      return null;
    }
  }

  /**
   * Validate component data against its schema
   */
  validateComponentData(name) {
    const schema = this.getComponentSchema(name);
    if (!schema) {
      return { valid: true, errors: [] };
    }

    // TODO: Implement proper JSON schema validation
    // For now, return valid
    return { valid: true, errors: [] };
  }

  /**
   * Check if a class is a valid component (extends BaseComponent)
   */
  isValidComponentClass(ComponentClass) {
    // Check if it's a function/class
    if (typeof ComponentClass !== 'function') {
      return false;
    }

    // Check prototype chain for BaseComponent
    let prototype = ComponentClass.prototype;
    while (prototype) {
      if (prototype.constructor === BaseComponent) {
        return true;
      }
      prototype = Object.getPrototypeOf(prototype);
      
      // Prevent infinite loops
      if (prototype === Object.prototype || prototype === Function.prototype) {
        break;
      }
    }

    return false;
  }

  /**
   * Get component documentation/help info
   */
  getComponentHelp(name) {
    const metadata = this.getComponentMetadata(name);
    const schema = this.getComponentSchema(name);
    
    if (!metadata) {
      return null;
    }

    return {
      name,
      category: metadata.category,
      description: metadata.description,
      icon: metadata.icon,
      schema: schema,
      properties: schema ? Object.keys(schema.properties || {}) : []
    };
  }

  /**
   * Get all component help information
   */
  getAllComponentHelp() {
    const help = {};
    this.getComponentNames().forEach(name => {
      const componentHelp = this.getComponentHelp(name);
      if (componentHelp) {
        help[name] = componentHelp;
      }
    });
    return help;
  }

  /**
   * Import component definitions from external sources (plugins)
   */
  async importComponents(componentDefinitions) {
    const imported = [];
    
    for (const [name, definition] of Object.entries(componentDefinitions)) {
      try {
        // Handle different import formats
        let ComponentClass;
        
        if (typeof definition === 'function') {
          ComponentClass = definition;
        } else if (definition.class) {
          ComponentClass = definition.class;
        } else if (definition.module) {
          // Dynamic import
          const module = await import(definition.module);
          ComponentClass = module.default || module[definition.export || name];
        } else {
          console.warn(`⚠️ Invalid component definition for ${name}`);
          continue;
        }

        // Register the component
        this.registerComponent(name, ComponentClass, definition.options || {});
        imported.push(name);
        
      } catch (error) {
        console.error(`❌ Failed to import component ${name}:`, error);
      }
    }

    console.log(`📦 Imported ${imported.length} components:`, imported);
    return imported;
  }

  /**
   * Export component definitions for serialization
   */
  exportComponents() {
    const exported = {};
    
    this.components.forEach((component, name) => {
      exported[name] = {
        category: component.options.category,
        description: component.options.description,
        schema: this.getComponentSchema(name)
      };
    });

    return exported;
  }

  /**
   * Get registry statistics
   */
  getStats() {
    const categories = this.getCategories();
    const categoryStats = {};
    
    categories.forEach(category => {
      categoryStats[category] = this.getComponentsByCategory(category).length;
    });

    return {
      totalComponents: this.components.size,
      categories: categories.length,
      categoryBreakdown: categoryStats,
      hasSchemas: this.componentSchemas.size
    };
  }

  /**
   * Clear all registered components (except built-ins if preserveBuiltIns is true)
   */
  clear(preserveBuiltIns = true) {
    if (preserveBuiltIns) {
      // Remove only non-built-in components
      const builtInCategories = ['rendering', 'physics', 'audio', 'animation', 'scripting', 'core'];
      const toRemove = [];
      
      this.components.forEach((component, name) => {
        if (!builtInCategories.includes(component.options.category)) {
          toRemove.push(name);
        }
      });
      
      toRemove.forEach(name => this.unregisterComponent(name));
      console.log(`🧹 Cleared ${toRemove.length} custom components`);
    } else {
      // Clear everything
      const count = this.components.size;
      this.components.clear();
      this.componentSchemas.clear();
      console.log(`🧹 Cleared all ${count} components`);
    }
  }

  /**
   * Dispose the registry
   */
  dispose() {
    console.log('🗑️ ComponentRegistry: Disposing...');
    this.clear(false);
    console.log('✅ ComponentRegistry: Disposed');
  }
}

// Export singleton instance
export const componentRegistry = new ComponentRegistry();
export default ComponentRegistry;