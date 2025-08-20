import { createContext, useContext, createSignal, createEffect, onCleanup } from 'solid-js';
import { RendererType } from './IRenderAPI';

// Context for the active renderer
const RenderContext = createContext();

// Available renderers registry
const rendererRegistry = new Map();

/**
 * Register a renderer implementation
 * @param {string} type - Renderer type from RendererType enum
 * @param {Function} rendererClass - Class that extends IRenderAPI
 */
export function registerRenderer(type, rendererClass) {
  rendererRegistry.set(type, rendererClass);
  console.log(`[RenderContext] Registered renderer: ${type}`);
}

/**
 * Get available renderers
 * @returns {Array} Array of registered renderer types
 */
export function getAvailableRenderers() {
  const renderers = Array.from(rendererRegistry.keys());
  console.log('[RenderContext] getAvailableRenderers called, registry has:', renderers);
  console.log('[RenderContext] Registry map:', rendererRegistry);
  return renderers;
}

/**
 * RenderProvider - Provides rendering context to the application
 */
export function RenderProvider(props) {
  const [activeRenderer, setActiveRenderer] = createSignal(null);
  const [rendererType, setRendererType] = createSignal(props.defaultRenderer || RendererType.TORUS);
  const [isLoading, setIsLoading] = createSignal(false);
  const [error, setError] = createSignal(null);
  const [canvas, setCanvas] = createSignal(null);

  // Renderer switching function
  const switchRenderer = async (newType, options = {}) => {
    console.log(`[RenderContext] Switching renderer to: ${newType}`);
    setIsLoading(true);
    setError(null);

    try {
      // Dispose current renderer
      if (activeRenderer()) {
        console.log('[RenderContext] Disposing current renderer...');
        await activeRenderer().dispose();
        
        // Wait for WebGL context cleanup
        console.log('[RenderContext] Waiting for WebGL context cleanup...');
        await new Promise(resolve => setTimeout(resolve, 100));
      }

      // Get renderer class
      const RendererClass = rendererRegistry.get(newType);
      if (!RendererClass) {
        throw new Error(`Renderer type '${newType}' not registered`);
      }

      // Create new renderer instance
      const renderer = new RendererClass(canvas(), options);
      
      // Initialize renderer
      console.log(`[RenderContext] Initializing ${newType} renderer...`);
      await renderer.initialize();

      // Update state
      setActiveRenderer(renderer);
      setRendererType(newType);
      
      console.log(`[RenderContext] Successfully switched to ${newType} renderer`);
      
      // Notify listeners
      if (props.onRendererChange) {
        props.onRendererChange(newType, renderer);
      }

      return renderer;
    } catch (err) {
      console.error('[RenderContext] Failed to switch renderer:', err);
      setError(err.message);
      throw err;
    } finally {
      setIsLoading(false);
    }
  };

  // Initialize default renderer when canvas is ready
  createEffect(() => {
    const canvasElement = canvas();
    if (canvasElement && !activeRenderer()) {
      switchRenderer(rendererType());
    }
  });

  // Cleanup on unmount
  onCleanup(async () => {
    if (activeRenderer()) {
      await activeRenderer().dispose();
    }
  });

  // Context value
  const contextValue = {
    // State
    renderer: activeRenderer,
    rendererType,
    isLoading,
    error,
    canvas,
    
    // Actions
    setCanvas,
    switchRenderer,
    getAvailableRenderers,
    
    // Renderer API proxy methods
    render: () => activeRenderer()?.render(),
    createScene: (options) => activeRenderer()?.createScene(options),
    createCamera: (type, options) => activeRenderer()?.createCamera(type, options),
    createLight: (type, options) => activeRenderer()?.createLight(type, options),
    createPrimitive: (type, options) => activeRenderer()?.createPrimitive(type, options),
    createMaterial: (type, options) => activeRenderer()?.createMaterial(type, options),
    loadModel: (url, options) => activeRenderer()?.loadModel(url, options),
    loadTexture: (url, options) => activeRenderer()?.loadTexture(url, options),
    raycast: (x, y) => activeRenderer()?.raycast(x, y),
    screenshot: (options) => activeRenderer()?.screenshot(options),
    getStats: () => activeRenderer()?.getStats(),
    getCapabilities: () => activeRenderer()?.getCapabilities()
  };

  return (
    <RenderContext.Provider value={contextValue}>
      {props.children}
    </RenderContext.Provider>
  );
}

/**
 * Hook to use render context
 * @returns {Object} Render context value
 */
export function useRenderContext() {
  const context = useContext(RenderContext);
  if (!context) {
    throw new Error('useRenderContext must be used within RenderProvider');
  }
  return context;
}

/**
 * Hook to get active renderer
 * @returns {IRenderAPI} Active renderer instance
 */
export function useRenderer() {
  const { renderer } = useRenderContext();
  return renderer();
}

/**
 * Hook for renderer switching
 * @returns {Object} Renderer switching utilities
 */
export function useRendererSwitcher() {
  const { rendererType, switchRenderer, getAvailableRenderers, isLoading, error } = useRenderContext();
  
  return {
    currentRenderer: rendererType,
    availableRenderers: getAvailableRenderers,
    switchRenderer,
    isLoading,
    error
  };
}