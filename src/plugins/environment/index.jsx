import { createPlugin } from '@/api/plugin';
import { IconPhotoHexagon } from '@tabler/icons-solidjs';
import LightingPanel from './LightingPanel.jsx';

export default createPlugin({
  id: 'environment-plugin',
  name: 'Environment Panel Plugin',
  version: '1.0.0',
  description: 'Environment and lighting controls in the right panel',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[EnvironmentPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[EnvironmentPlugin] Starting...');
    
    api.tab('environment', {
      title: 'Environment',
      component: LightingPanel,
      icon: IconPhotoHexagon,
      order: 4
    });
    
    console.log('[EnvironmentPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[EnvironmentPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[EnvironmentPlugin] Disposing...');
  }
});