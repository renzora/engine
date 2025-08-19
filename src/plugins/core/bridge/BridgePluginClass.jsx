import { createPlugin } from '@/api/plugin';
import { bridgeService } from './index.jsx';
import BridgeStatus from './BridgeStatus.jsx';
import BridgeViewport from './BridgeViewport.jsx';
import { Server, Database, Cloud } from '@/ui/icons';

export default createPlugin({
  id: 'bridge-plugin',
  name: 'Bridge Server Plugin',
  version: '1.0.0',
  description: 'Manages communication between Renzora Engine and project files',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[BridgePlugin] Initializing bridge server connection...');
    console.log('[BridgePlugin] Bridge plugin initialized');
  },

  async onStart(api) {
    console.log('[BridgePlugin] Starting bridge server plugin...');

    // Listen to engine events
    api.on('project-selected', (data) => {
      console.log('[BridgePlugin] Project selected event received:', data);
      if (data?.project) {
        bridgeService.setCurrentProject(data.project);
      }
    });

    console.log('[BridgePlugin] Bridge server plugin started');
  },

  onUpdate() {
    // This runs every frame - could be used for connection health checks
    // Don't put heavy operations here
  },

  async onStop() {
    console.log('[BridgePlugin] Stopping bridge server plugin...');
    // Cleanup bridge connections if needed
  },

  async onDispose() {
    console.log('[BridgePlugin] Disposing bridge server plugin...');
    // Final cleanup
  }
});