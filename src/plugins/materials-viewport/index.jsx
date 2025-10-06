import { createPlugin } from '@/api/plugin';
import { IconPalette } from '@tabler/icons-solidjs';
import { createEffect, onCleanup } from 'solid-js';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';
import MaterialsViewport from './MaterialsViewport.jsx';

export default createPlugin({
  id: 'materials-viewport-plugin',
  name: 'Materials Viewport Plugin',
  version: '1.0.0',
  description: 'Material library and preview viewport similar to Unreal Engine',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[MaterialsViewportPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[MaterialsViewportPlugin] Starting...');
    
    api.viewport('materials', {
      label: 'Materials',
      component: MaterialsViewport,
      icon: IconPalette,
      description: 'Material library, editor and preview viewport'
    });
    
    // Monitor active tab and control toolbar visibility
    const effect = createEffect(() => {
      const activeTabId = viewportStore.activeTabId;
      const activeTab = viewportStore.tabs.find(tab => tab.id === activeTabId);
      
      if (activeTab && activeTab.type === 'materials') {
        // Materials viewport is active - hide toolbar
        api.hideToolbar();
      } else {
        // Materials viewport is not active - show toolbar
        api.showToolbar();
      }
    });
    
    // Store the cleanup function for onStop
    this.toolbarEffect = effect;
    
    console.log('[MaterialsViewportPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[MaterialsViewportPlugin] Stopping...');
    
    // Clean up the toolbar effect
    if (this.toolbarEffect) {
      this.toolbarEffect();
      this.toolbarEffect = null;
    }
    
    // Restore toolbar visibility when plugin stops
    const { pluginAPI } = await import('@/api/plugin');
    pluginAPI.showToolbar();
  },

  async onDispose() {
    console.log('[MaterialsViewportPlugin] Disposing...');
    
    // Ensure toolbar is restored
    const { pluginAPI } = await import('@/api/plugin');
    pluginAPI.showToolbar();
  }
});