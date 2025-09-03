import { createPlugin } from '@/api/plugin';
import { Box } from '@/ui/icons';
import ObjectProperties from '@/pages/editor/objectProperties.jsx';

export default createPlugin({
  id: 'object-properties-plugin',
  name: 'Object Properties Plugin',
  version: '1.0.0',
  description: 'Object properties panel for selected entities',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[ObjectPropertiesPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[ObjectPropertiesPlugin] Starting...');
    
    api.tab('object-properties', {
      title: 'Properties',
      component: ObjectProperties,
      icon: Box,
      order: 1
    });
    
    console.log('[ObjectPropertiesPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[ObjectPropertiesPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[ObjectPropertiesPlugin] Disposing...');
  }
});