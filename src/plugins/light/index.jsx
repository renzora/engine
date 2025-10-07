import { createPlugin } from '@/api/plugin';
import { IconBulb } from '@tabler/icons-solidjs';
import LightPanel from './LightPanel.jsx';

export default createPlugin({
  id: 'light-plugin',
  name: 'Light Settings Plugin',
  version: '1.0.0',
  description: 'Light properties and settings controls in the scene panel',
  author: 'Renzora Engine Team',

  async onInit() {
    // Initializing light plugin
    console.log('[LightPlugin] Initializing...');
  },

  async onStart(api) {
    // Starting light plugin
    console.log('[LightPlugin] Starting...');
    
    api.tab('light', {
      title: 'Light',
      component: LightPanel,
      icon: IconBulb,
      order: 4,
      condition: (selectedObject) => {
        return selectedObject && selectedObject.metadata?.isLightContainer;
      }
    });
    
    // Light plugin started
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    // Stopping light plugin
  },

  async onDispose() {
    // Disposing light plugin
  }
});