// Automatic renderer discovery and management system

import { rendererAPI } from '../api/render/RendererAPI.js';

class RendererManager {
  constructor() {
    this.renderers = new Map();
    this.activeRenderer = null;
    this.initialized = false;
    this.api = rendererAPI;
  }

  async discoverRenderers() {
    console.log('🔍 Discovering available renderers...');
    
    // Auto-discover renderers by scanning render directory
    const rendererConfigs = await this.loadRendererConfigs();
    
    for (const config of rendererConfigs) {
      await this.registerRenderer(config);
    }
    
    console.log(`✅ Discovered ${this.renderers.size} renderers:`, 
      Array.from(this.renderers.keys()));
    
    this.initialized = true;
    return Array.from(this.renderers.values());
  }

  async loadRendererConfigs() {
    // In a real implementation, this would dynamically scan directories
    // For now, we'll import known renderer configs
    const configs = [];
    
    try {
      const { default: webglConfig } = await import('./babylon-webgl/renderer.config.js');
      configs.push(webglConfig);
    } catch (e) {
      console.warn('WebGL renderer config not found:', e);
    }
    
    try {
      const { default: webgpuConfig } = await import('./babylon-webgpu/renderer.config.js');
      configs.push(webgpuConfig);
    } catch (e) {
      console.warn('WebGPU renderer config not found:', e);
    }
    
    try {
      const { default: nativeConfig } = await import('./babylon-native/renderer.config.js');
      configs.push(nativeConfig);
    } catch (e) {
      console.warn('Babylon Native renderer config not found:', e);
    }
    
    try {
      const { default: vulkanConfig } = await import('./custom-vulkan/renderer.config.js');
      configs.push(vulkanConfig);
    } catch (e) {
      console.warn('Custom Vulkan renderer config not found:', e);
    }
    
    try {
      const { default: threejsConfig } = await import('./threejs/renderer.config.js');
      configs.push(threejsConfig);
    } catch (e) {
      console.warn('Three.js renderer config not found:', e);
    }
    
    try {
      const { default: playcanvasConfig } = await import('./playcanvas/renderer.config.js');
      configs.push(playcanvasConfig);
    } catch (e) {
      console.warn('PlayCanvas renderer config not found:', e);
    }
    
    try {
      const { default: pixijsConfig } = await import('./pixijs/renderer.config.js');
      configs.push(pixijsConfig);
    } catch (e) {
      console.warn('PixiJS renderer config not found:', e);
    }
    
    try {
      const { default: phaserConfig } = await import('./phaser/renderer.config.js');
      configs.push(phaserConfig);
    } catch (e) {
      console.warn('Phaser renderer config not found:', e);
    }
    
    try {
      const { default: melonjsConfig } = await import('./melonjs/renderer.config.js');
      configs.push(melonjsConfig);
    } catch (e) {
      console.warn('MelonJS renderer config not found:', e);
    }
    
    return configs;
  }

  async registerRenderer(config) {
    try {
      // Load the viewport component
      const viewportModule = await import(`./${config.id}/${config.viewport}`);
      const ViewportComponent = viewportModule.default || viewportModule[Object.keys(viewportModule)[0]];
      
      // Load the renderer API implementation
      const rendererModule = await import(`./${config.id}/${
        config.id === 'custom-vulkan' ? 'VulkanRenderer.js' : 
        config.id === 'babylon-native' ? 'BabylonNativeRenderer.js' : 
        config.id === 'babylon-webgpu' ? 'WebGPURenderer.js' : 
        config.id === 'threejs' ? 'ThreeRenderer.js' :
        config.id === 'playcanvas' ? 'PlayCanvasRenderer.js' :
        config.id === 'pixijs' ? 'PixiRenderer.js' :
        config.id === 'phaser' ? 'PhaserRenderer.js' :
        config.id === 'melonjs' ? 'MelonRenderer.js' :
        'WebGLRenderer.js'
      }`);
      const RendererClass = rendererModule.default || Object.values(rendererModule)[0];
      
      // Check platform compatibility
      const isCompatible = await this.checkCompatibility(config);
      
      const renderer = {
        ...config,
        component: ViewportComponent,
        rendererClass: RendererClass,
        available: isCompatible,
        status: isCompatible ? 'ready' : 'incompatible'
      };
      
      this.renderers.set(config.id, renderer);
      
      // Register with API layer if compatible
      if (isCompatible) {
        const rendererInstance = new RendererClass(config);
        this.api.registerRenderer(config.id, rendererInstance);
      }
      
      console.log(`📦 Registered renderer: ${config.name} (${renderer.status})`);
      
    } catch (error) {
      console.error(`❌ Failed to register renderer ${config.id}:`, error);
    }
  }

  async checkCompatibility(config) {
    // Check platform requirements
    if (config.platform && !config.platform.includes(this.getCurrentPlatform())) {
      return false;
    }
    
    // Check specific requirements
    if (config.requirements) {
      if (config.requirements.webgpu && !navigator.gpu) {
        return false;
      }
      
      if (config.requirements.native_only && !this.isTauriEnvironment()) {
        return false;
      }
      
      if (config.requirements.vulkan && !await this.checkVulkanSupport()) {
        return false;
      }
    }
    
    return true;
  }

  getCurrentPlatform() {
    return this.isTauriEnvironment() ? 'tauri' : 'web';
  }

  isTauriEnvironment() {
    return typeof window !== 'undefined' && window.__TAURI_INTERNALS__;
  }

  async checkVulkanSupport() {
    if (!this.isTauriEnvironment()) return false;
    
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke('check_vulkan_support');
    } catch {
      return false;
    }
  }

  getAvailableRenderers() {
    return Array.from(this.renderers.values()).filter(r => r.available);
  }

  getRenderer(id) {
    return this.renderers.get(id);
  }

  async switchRenderer(rendererId) {
    const renderer = this.renderers.get(rendererId);
    if (!renderer || !renderer.available) {
      throw new Error(`Renderer ${rendererId} not available`);
    }
    
    console.log(`🔄 Switching to renderer: ${renderer.name}`);
    this.activeRenderer = rendererId;
    
    // Switch renderer in API layer
    await this.api.setActiveRenderer(rendererId);
    
    // Emit renderer change event
    if (typeof window !== 'undefined') {
      window.dispatchEvent(new CustomEvent('renderer-changed', {
        detail: { 
          renderer: rendererId,
          config: renderer
        }
      }));
    }
    
    return renderer;
  }

  getActiveRenderer() {
    return this.activeRenderer ? this.renderers.get(this.activeRenderer) : null;
  }
}

// Global renderer manager instance
export const rendererManager = new RendererManager();

// Auto-initialize when module loads
if (typeof window !== 'undefined') {
  rendererManager.discoverRenderers().catch(console.error);
}