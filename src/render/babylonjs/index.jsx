import { createSignal } from 'solid-js';
import { createPlugin } from '@/api/plugin';
import ViewportCanvas from './viewport';

// Simple hook for engine readiness - no context needed
export const useEngineReady = () => {
  const [isEngineReady] = createSignal(true);
  return { isEngineReady };
};

// Direct export of ViewportCanvas - no wrapper needed
export { ViewportCanvas };

// Plugin definition for the Babylon.js renderer
export default createPlugin({
  id: 'core-render-plugin',
  name: 'Core Render Plugin',
  version: '1.0.0',
  description: 'Core rendering functionality for Renzora Engine',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[RenderPlugin] Initializing core render plugin...');
  },

  async onStart() {
    console.log('[RenderPlugin] Starting core render plugin...');
  },

  onUpdate() {
    // Render loop updates if needed
  },

  async onStop() {
    console.log('[RenderPlugin] Stopping core render plugin...');
  },

  async onDispose() {
    console.log('[RenderPlugin] Disposing core render plugin...');
  }
});