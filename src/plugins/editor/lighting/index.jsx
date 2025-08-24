import { createPlugin } from '@/api/plugin';
import { Sun } from '@/ui/icons';
import Lighting from '@/pages/editor/Lighting.jsx';

export default createPlugin({
  id: 'lighting-plugin',
  name: 'Lighting Plugin',
  version: '1.0.0',
  description: 'Scene lighting controls and environment settings',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[LightingPlugin] Lighting plugin initialized');
  },

  async onStart(api) {
    console.log('[LightingPlugin] Registering lighting tab...');

    // Register property tab
    api.tab('lighting', {
      title: 'Lighting',
      icon: Sun,
      component: Lighting,
      order: 12,
      plugin: 'lighting-plugin'
    });

    console.log('[LightingPlugin] Lighting tab registered');
  }
});