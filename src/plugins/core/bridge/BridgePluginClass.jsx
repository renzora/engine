import { createPlugin } from '@/api/plugin';
import { bridgeService } from '@/api/bridge';
import { setCurrentProject } from '@/api/bridge/projects';
import BridgeStatus from './BridgeStatus.jsx';
import BridgeViewport from './BridgeViewport.jsx';
import { Server, Database, Cloud } from '@/ui/icons';

let projectSelectedHandler = null;

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

    // Listen to engine events from plugin API
    api.on('project-selected', (data) => {
      console.log('[BridgePlugin] Plugin API project selected event received:', data);
      if (data?.project) {
        setCurrentProject(data.project);
      }
    });

    // Listen to DOM events from splash screen
    projectSelectedHandler = (event) => {
      console.log('[BridgePlugin] DOM project selected event received:', event.detail);
      if (event.detail?.project) {
        setCurrentProject(event.detail.project);
      }
    };

    document.addEventListener('engine:project-selected', projectSelectedHandler);

    console.log('[BridgePlugin] Bridge server plugin started');
  },

  onUpdate() {
    // This runs every frame - could be used for connection health checks
    // Don't put heavy operations here
  },

  async onStop() {
    console.log('[BridgePlugin] Stopping bridge server plugin...');
    if (projectSelectedHandler) {
      document.removeEventListener('engine:project-selected', projectSelectedHandler);
      projectSelectedHandler = null;
    }
  },

  async onDispose() {
    console.log('[BridgePlugin] Disposing bridge server plugin...');
    if (projectSelectedHandler) {
      document.removeEventListener('engine:project-selected', projectSelectedHandler);
      projectSelectedHandler = null;
    }
  }
});