import { createPlugin } from '@/api/plugin';
import { Sun } from '@/ui/icons';
import LightingPanel from './LightingPanel.jsx';

export default createPlugin({
  id: 'lighting-plugin',
  name: 'Lighting Panel Plugin',
  version: '1.0.0',
  description: 'Environment and lighting controls in the right panel',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[LightingPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[LightingPlugin] Starting...');
    
    api.tab('lighting', {
      title: 'Lighting',
      component: LightingPanel,
      icon: Sun,
      order: 15
    });
    
    console.log('[LightingPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[LightingPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[LightingPlugin] Disposing...');
  }
});