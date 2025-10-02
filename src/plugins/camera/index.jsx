import { createPlugin } from '@/api/plugin';
import { IconVideo } from '@tabler/icons-solidjs';
import CameraDropdownContent from '@/ui/display/CameraDropdownContent.jsx';
import CameraPropertiesTab from './CameraPropertiesTab.jsx';

export default createPlugin({
  id: 'camera-plugin',
  name: 'Camera Helper Plugin',
  version: '1.0.0',
  description: 'Camera controls in the toolbar helper',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[CameraPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[CameraPlugin] Starting...');
    
    // Camera settings dropdown
    api.helper('camera', {
      title: 'Camera Settings',
      icon: IconVideo,
      order: 3,
      hasDropdown: true,
      dropdownComponent: CameraDropdownContent,
      dropdownWidth: 280,
      dynamicLabel: true // Enable dynamic label updates
    });
    
    // Camera properties tab - only shows for camera objects
    api.registerPropertyTab('camera-settings', {
      title: 'Camera',
      component: CameraPropertiesTab,
      icon: IconVideo,
      order: 10,
      condition: (selectedObject) => {
        return selectedObject && selectedObject.getClassName && (
          selectedObject.getClassName().includes('Camera') || 
          selectedObject.getClassName() === 'UniversalCamera' ||
          selectedObject.getClassName() === 'ArcRotateCamera' ||
          selectedObject.getClassName() === 'FreeCamera'
        );
      }
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