import { createSignal } from 'solid-js';
import { createPlugin } from '@/api/plugin';
import BabylonNativeViewport from './BabylonNativeViewport';

// Simple hook for engine readiness - no context needed
export const useEngineReady = () => {
  const [isEngineReady] = createSignal(true);
  return { isEngineReady };
};

// Export as Babylon Native renderer
export { BabylonNativeViewport };

// Plugin definition for the Babylon Native renderer
export default createPlugin({
  id: 'babylon-native-renderer',
  name: 'Babylon Native Renderer',
  version: '1.0.0',
  description: 'Babylon.js with native C++ backend (DirectX/Metal/Vulkan)',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[Native Renderer] Initializing Babylon Native renderer...');
  },

  async onStart() {
    console.log('[Native Renderer] Starting Babylon Native renderer...');
  },

  onUpdate() {
    // Render loop updates if needed
  },

  async onStop() {
    console.log('[Native Renderer] Stopping Babylon Native renderer...');
  },

  async onDispose() {
    console.log('[Native Renderer] Disposing Babylon Native renderer...');
  }
});