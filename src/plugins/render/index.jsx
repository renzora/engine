import { createPlugin } from '@/api/plugin';
import { IconAdjustments } from '@tabler/icons-solidjs';
import RenderPanel from './RenderPanel.jsx';

export default createPlugin({
  id: 'render-plugin',
  name: 'Render Settings Plugin',
  version: '1.0.0',
  description: 'Object render settings including shadows and collision in the scene panel',
  author: 'Renzora Engine Team',

  async onInit(_api) {
    // Initializing render plugin
  },

  async onStart(api) {
    // Starting render plugin
    
    api.tab('render', {
      title: 'Render',
      component: RenderPanel,
      icon: IconAdjustments,
      order: 2,
      condition: (selectedObject) => {
        // Hide render tab (shadows/collision) for cameras, lights, and scene root since they don't cast shadows
        const isCamera = selectedObject && selectedObject.getClassName && (
          selectedObject.getClassName().includes('Camera') || 
          selectedObject.getClassName() === 'UniversalCamera' ||
          selectedObject.getClassName() === 'ArcRotateCamera' ||
          selectedObject.getClassName() === 'FreeCamera'
        );
        const isLight = selectedObject && selectedObject.getClassName && (
          selectedObject.getClassName().includes('Light') ||
          selectedObject.metadata?.isLightContainer
        );
        const isSceneRoot = selectedObject && selectedObject.getClassName && 
          selectedObject.getClassName() === 'Scene';
          
        return selectedObject && !isCamera && !isLight && !isSceneRoot;
      }
    });
    
    // Render plugin started
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    // Stopping render plugin
  },

  async onDispose() {
    // Disposing render plugin
  }
});