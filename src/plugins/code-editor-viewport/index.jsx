import { createPlugin } from '@/api/plugin';
import { IconCode } from '@tabler/icons-solidjs';
import { createEffect } from 'solid-js';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';
import CodeEditorViewport from './CodeEditorViewport.jsx';

export default createPlugin({
  id: 'code-editor-viewport-plugin',
  name: 'Code Editor Viewport Plugin',
  version: '1.0.0',
  description: 'Code editor viewport for editing scripts and text files',
  author: 'Renzora Engine Team',

  async onInit(_api) {
    console.log('[CodeEditorViewportPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[CodeEditorViewportPlugin] Starting...');
    
    api.viewport('code-editor', {
      label: 'Code Editor',
      component: CodeEditorViewport,
      icon: IconCode,
      description: 'Code editor for scripts and text files'
    });
    
    // Monitor active tab and control toolbar visibility
    const effect = createEffect(() => {
      // Get reactive values from store
      const activeTabId = viewportStore.activeTabId;
      const tabs = viewportStore.tabs;
      const activeTab = tabs.find(tab => tab.id === activeTabId);
      
      console.log('[CodeEditorViewportPlugin] Effect triggered - activeTabId:', activeTabId, 'activeTab type:', activeTab?.type);
      
      // Force hide toolbar whenever any code editor tab is active
      if (activeTab && activeTab.type === 'code-editor') {
        console.log('[CodeEditorViewportPlugin] Code editor active - hiding toolbar');
        // Immediate and delayed calls to ensure toolbar is hidden
        api.hideToolbar();
        requestAnimationFrame(() => api.hideToolbar());
        setTimeout(() => api.hideToolbar(), 10);
        setTimeout(() => api.hideToolbar(), 100);
      } else if (activeTab) {
        console.log('[CodeEditorViewportPlugin] Other viewport active - showing toolbar');
        api.showToolbar();
      }
    });
    
    // Store the cleanup function for onStop
    api.toolbarEffect = effect;
    
    console.log('[CodeEditorViewportPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop(api) {
    console.log('[CodeEditorViewportPlugin] Stopping...');
    
    // Clean up the toolbar effect
    if (api.toolbarEffect) {
      api.toolbarEffect();
      api.toolbarEffect = null;
    }
    
    // Restore toolbar visibility when plugin stops
    const { pluginAPI } = await import('@/api/plugin');
    pluginAPI.showToolbar();
  },

  async onDispose() {
    console.log('[CodeEditorViewportPlugin] Disposing...');
    
    // Ensure toolbar is restored
    const { pluginAPI } = await import('@/api/plugin');
    pluginAPI.showToolbar();
  }
});