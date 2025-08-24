import { createSignal } from 'solid-js';
import { createPlugin } from '@/api/plugin';
import ViewportCanvas from './viewport';

// Simple hook for engine readiness - no context needed
export const useEngineReady = () => {
  const [isEngineReady] = createSignal(true);
  return { isEngineReady };
};

// Export as WebGL-specific renderer
export { ViewportCanvas as WebGLViewport };

// Plugin definition for the Babylon.js WebGL renderer
export default createPlugin({
  id: 'babylon-webgl-renderer',
  name: 'Babylon.js WebGL Renderer',
  version: '1.0.0',
  description: 'Babylon.js with WebGL backend for maximum compatibility',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[WebGL Renderer] Initializing Babylon.js WebGL renderer...');
  },

  async onStart() {
    console.log('[WebGL Renderer] Starting Babylon.js WebGL renderer...');
  },

  onUpdate() {
    // Render loop updates if needed
  },

  async onStop() {
    console.log('[WebGL Renderer] Stopping Babylon.js WebGL renderer...');
  },

  async onDispose() {
    console.log('[WebGL Renderer] Disposing Babylon.js WebGL renderer...');
  }
});