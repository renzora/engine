import { createPlugin } from '@/api/plugin';
import { IconWorld } from '@tabler/icons-solidjs';
import WebBrowserViewport from './WebBrowserViewport.jsx';

export default createPlugin({
  id: 'web-browser-plugin',
  name: 'Web Browser Viewport Plugin',
  version: '1.0.0',
  description: 'Web browser viewport for browsing websites',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[WebBrowserPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[WebBrowserPlugin] Starting...');
    
    api.viewport('web-browser', {
      label: 'Web Browser',
      component: WebBrowserViewport,
      icon: IconWorld,
      description: 'Browse websites in a viewport'
    });
    
    console.log('[WebBrowserPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[WebBrowserPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[WebBrowserPlugin] Disposing...');
  }
});