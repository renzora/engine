import { createPlugin } from '@/api/plugin';
import { IconCode } from '@tabler/icons-solidjs';
import ScriptsPanel from './ScriptsPanel.jsx';

export default createPlugin({
  id: 'scripts-plugin',
  name: 'Scripts Panel Plugin',
  version: '1.0.0',
  description: 'Script management and property controls in the right panel',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[ScriptsPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[ScriptsPlugin] Starting...');
    
    api.tab('scripts', {
      title: 'Scripts',
      component: ScriptsPanel,
      icon: IconCode,
      order: 2
    });
    
    console.log('[ScriptsPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[ScriptsPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[ScriptsPlugin] Disposing...');
  }
});