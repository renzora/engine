import { createPlugin } from '@/api/plugin';
import { IconGridDots } from '@tabler/icons-solidjs';
import GridSettingsDropdown from '@/ui/display/GridSettingsDropdown.jsx';

export default createPlugin({
  id: 'grid-plugin',
  name: 'Grid & Snapping Plugin',
  version: '1.0.0',
  description: 'Grid settings, object snapping, and gizmo snapping controls',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[GridPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[GridPlugin] Starting...');
    
    api.helper('grid-settings', {
      title: 'Grid & Snapping Settings',
      icon: IconGridDots,
      order: 4,
      hasDropdown: true,
      dropdownComponent: GridSettingsDropdown,
      dropdownWidth: 320
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