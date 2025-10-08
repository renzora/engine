import { createPlugin } from '@/api/plugin';
import { IconStars } from '@tabler/icons-solidjs';
import StarsPanel from './StarsPanel.jsx';

export default createPlugin({
  id: 'stars',
  name: 'Stars Weather Plugin',
  version: '1.0.0',
  description: 'Night sky stars and celestial effects',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('⭐ Stars plugin initializing...');
  },

  async onStart(api) {
    console.log('⭐ Stars plugin starting...');
    
    // Register the stars panel as a tab
    api.tab('stars', {
      title: 'Stars',
      component: StarsPanel,
      icon: IconStars,
      order: 9,
      condition: () => {
        // Always show stars tab when scene is available
        return true;
      }
    });

    console.log('⭐ Stars plugin started successfully');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('🗑️ Stars plugin stopped');
  },

  async onDispose() {
    console.log('🗑️ Stars plugin disposed');
  }
});