import { createPlugin } from '@/api/plugin';
import { IconEye } from '@tabler/icons-solidjs';
import FogPanel from './FogPanel.jsx';
import { renderStore, renderActions } from '@/render/store';

export default createPlugin({
  id: 'fog',
  name: 'Fog Plugin',
  version: '1.0.0',
  description: 'Advanced fog controls for atmospheric effects and depth perception',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('🌫️ Fog plugin initializing...');
  },

  async onStart(api) {
    console.log('🌫️ Fog plugin starting...');
    
    // Register the fog panel as a tab
    api.tab('fog', {
      title: 'Fog',
      component: FogPanel,
      icon: IconEye,
      order: 6,
      condition: () => {
        // Always show fog tab when scene is available
        return renderStore.scene;
      }
    });

    console.log('🌫️ Fog plugin started successfully');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('🗑️ Fog plugin stopped');
  },

  async onDispose() {
    console.log('🗑️ Fog plugin disposed');
  }
});