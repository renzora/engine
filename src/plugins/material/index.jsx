import { createPlugin } from '@/api/plugin';
import { IconPalette } from '@tabler/icons-solidjs';
import MaterialPanel from './MaterialPanel.jsx';

export default createPlugin({
  id: 'material-plugin',
  name: 'Material Settings Plugin',
  version: '1.0.0',
  description: 'Material and color assignment controls in the scene panel',
  author: 'Renzora Engine Team',

  async onInit(api) {
    // Initializing material plugin
  },

  async onStart(api) {
    // Starting material plugin
    
    api.tab('material', {
      title: 'Material',
      component: MaterialPanel,
      icon: IconPalette,
      order: 3
    });
    
    // Material plugin started
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    // Stopping material plugin
  },

  async onDispose() {
    // Disposing material plugin
  }
});