import { createPlugin } from '@/api/plugin';
import { IconCode } from '@tabler/icons-solidjs';
import ScriptsPanel from './ScriptsPanel.jsx';

export default createPlugin({
  id: 'scripts-plugin',
  name: 'Scripts Panel Plugin',
  version: '1.0.0',
  description: 'Script management and property controls in the right panel',
  author: 'Renzora Engine Team',

  async onInit() {
    // Initializing scripts plugin
  },

  async onStart(api) {
    // Starting scripts plugin
    
    api.tab('scripts', {
      title: 'Scripts',
      component: ScriptsPanel,
      icon: IconCode,
      order: 1,
      condition: (selectedObject) => {
        // Hide scripts tab for environment objects (skybox) since they don't need transforms
        return selectedObject && !selectedObject.metadata?.isEnvironmentObject;
      }
    });
    
    // Scripts plugin started
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    // Stopping scripts plugin
  },

  async onDispose() {
    // Disposing scripts plugin
  }
});