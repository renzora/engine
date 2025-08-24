import { createSignal } from 'solid-js';
import { createPlugin } from '@/api/plugin';
import ViewportCanvas from './viewport';

// Simple hook for engine readiness - no context needed
export const useEngineReady = () => {
  const [isEngineReady] = createSignal(true);
  return { isEngineReady };
};

// Export as WebGPU-specific renderer
export { ViewportCanvas as WebGPUViewport };

// Plugin definition for the Babylon.js WebGPU renderer
export default createPlugin({
  id: 'babylon-webgpu-renderer',
  name: 'Babylon.js WebGPU Renderer',
  version: '1.0.0',
  description: 'Babylon.js with WebGPU backend for high performance',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[WebGPU Renderer] Initializing Babylon.js WebGPU renderer...');
  },

  async onStart() {
    console.log('[WebGPU Renderer] Starting Babylon.js WebGPU renderer...');
  },

  onUpdate() {
    // Render loop updates if needed
  },

  async onStop() {
    console.log('[WebGPU Renderer] Stopping Babylon.js WebGPU renderer...');
  },

  async onDispose() {
    console.log('[WebGPU Renderer] Disposing Babylon.js WebGPU renderer...');
  }
});