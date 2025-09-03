import { createPlugin } from '@/api/plugin';
import { Grid3x3 } from '@/ui/icons';
import GridPanel from './GridPanel.jsx';

export default createPlugin({
  id: 'grid-plugin',
  name: 'Grid Panel Plugin',
  version: '1.0.0',
  description: 'Grid settings and snapping controls in the right panel',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[GridPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[GridPlugin] Starting...');
    
    api.tab('grid', {
      title: 'Grid',
      component: GridPanel,
      icon: Grid3x3,
      order: 8
    });
    
    console.log('[GridPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[GridPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[GridPlugin] Disposing...');
  }
});