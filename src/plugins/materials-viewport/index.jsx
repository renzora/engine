import { createPlugin } from '@/api/plugin';
import { IconPalette } from '@tabler/icons-solidjs';
import MaterialsViewport from './MaterialsViewport.jsx';

export default createPlugin({
  id: 'materials-viewport-plugin',
  name: 'Materials Viewport Plugin',
  version: '1.0.0',
  description: 'Material library and preview viewport similar to Unreal Engine',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[MaterialsViewportPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[MaterialsViewportPlugin] Starting...');
    
    api.viewport('materials', {
      label: 'Materials',
      component: MaterialsViewport,
      icon: IconPalette,
      description: 'Material library, editor and preview viewport'
    });
    
    console.log('[MaterialsViewportPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[MaterialsViewportPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[MaterialsViewportPlugin] Disposing...');
  }
});