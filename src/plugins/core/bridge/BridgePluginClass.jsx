import { createPlugin } from '@/api/plugin';
import { setCurrentProject } from '@/api/bridge/projects';
import BridgeStatus from './BridgeStatus.jsx';
import BridgeViewport from './BridgeViewport.jsx';
import { IconServer, IconDatabase, IconCloud } from '@tabler/icons-solidjs';

let projectSelectedHandler = null;

export default createPlugin({
  id: 'bridge-plugin',
  name: 'Bridge Server Plugin',
  version: '1.0.0',
  description: 'Manages communication between Renzora Engine and project files',
  author: 'Renzora Engine Team',

  async onInit(api) {
    // Bridge plugin initialized
  },

  async onStart(api) {
    // Starting bridge server plugin

    // Listen to engine events from plugin API
    api.on('project-selected', (data) => {
      // Plugin API project selected event received
      if (data?.project) {
        setCurrentProject(data.project);
      }
    });

    // Listen to DOM events from splash screen
    projectSelectedHandler = (event) => {
      // DOM project selected event received
      if (event.detail?.project) {
        setCurrentProject(event.detail.project);
      }
    };

    document.addEventListener('engine:project-selected', projectSelectedHandler);

    // Bridge server plugin started
  },

  onUpdate() {
    // This runs every frame - could be used for connection health checks
    // Don't put heavy operations here
  },

  async onStop() {
    // Stopping bridge server plugin
    if (projectSelectedHandler) {
      document.removeEventListener('engine:project-selected', projectSelectedHandler);
      projectSelectedHandler = null;
    }
  },

  async onDispose() {
    // Disposing bridge server plugin
    if (projectSelectedHandler) {
      document.removeEventListener('engine:project-selected', projectSelectedHandler);
      projectSelectedHandler = null;
    }
  }
});