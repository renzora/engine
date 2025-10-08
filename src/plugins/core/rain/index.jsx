import { createPlugin } from '@/api/plugin';
import { IconCloudRain } from '@tabler/icons-solidjs';
import RainPanel from './RainPanel.jsx';

export default createPlugin({
  id: 'rain',
  name: 'Rain Weather Plugin',
  version: '1.0.0',
  description: 'Rain particle effects and weather controls',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('🌧️ Rain plugin initializing...');
  },

  async onStart(api) {
    console.log('🌧️ Rain plugin starting...');
    
    // Register the rain panel as a tab
    api.tab('rain', {
      title: 'Rain',
      component: RainPanel,
      icon: IconCloudRain,
      order: 7,
      condition: () => {
        // Always show rain tab when scene is available
        return true;
      }
    });

    console.log('🌧️ Rain plugin started successfully');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('🗑️ Rain plugin stopped');
  },

  async onDispose() {
    console.log('🗑️ Rain plugin disposed');
  }
});