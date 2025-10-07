import { createPlugin } from '@/api/plugin';
import { IconSnowflake } from '@tabler/icons-solidjs';
import SnowPanel from './SnowPanel.jsx';

export default createPlugin({
  id: 'snow',
  name: 'Snow Weather Plugin',
  version: '1.0.0',
  description: 'Snow particle effects and winter weather controls',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('❄️ Snow plugin initializing...');
  },

  async onStart(api) {
    console.log('❄️ Snow plugin starting...');
    
    // Register the snow panel as a tab
    api.tab('snow', {
      title: 'Snow',
      component: SnowPanel,
      icon: IconSnowflake,
      order: 8,
      condition: () => {
        // Always show snow tab when scene is available
        return true;
      }
    });

    console.log('❄️ Snow plugin started successfully');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('🗑️ Snow plugin stopped');
  },

  async onDispose() {
    console.log('🗑️ Snow plugin disposed');
  }
});