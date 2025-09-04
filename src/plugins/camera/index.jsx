import { createPlugin } from '@/api/plugin';
import { IconVideo } from '@tabler/icons-solidjs';
import CameraPanel from './CameraPanel.jsx';

export default createPlugin({
  id: 'camera-plugin',
  name: 'Camera Panel Plugin',
  version: '1.0.0',
  description: 'Camera controls in the right panel',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[CameraPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[CameraPlugin] Starting...');
    
    api.tab('camera', {
      title: 'Camera',
      component: CameraPanel,
      icon: IconVideo,
      order: 3
    });
    
    console.log('[CameraPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[CameraPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[CameraPlugin] Disposing...');
  }
});