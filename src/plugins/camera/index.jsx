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
    
    // Initialize camera store to start global vignette effects
    try {
      await import('./cameraStore.jsx');
      console.log('[CameraPlugin] Camera store initialized');
    } catch (error) {
      console.warn('[CameraPlugin] Failed to initialize camera store:', error);
    }
    
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
    
    // Clean up vignette post-processes
    try {
      const { cleanupVignette } = await import('./cameraStore.jsx');
      cleanupVignette();
      console.log('[CameraPlugin] Cleaned up vignette post-processes');
    } catch (error) {
      console.warn('[CameraPlugin] Failed to cleanup vignette:', error);
    }
  },

  async onDispose() {
    console.log('[CameraPlugin] Disposing...');
    
    // Clean up vignette post-processes
    try {
      const { cleanupVignette } = await import('./cameraStore.jsx');
      cleanupVignette();
    } catch (error) {
      console.warn('[CameraPlugin] Failed to cleanup vignette:', error);
    }
  }
});