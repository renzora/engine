import { onMount } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import { IconServer } from '@tabler/icons-solidjs';
import BridgeViewport from './BridgeViewport.jsx';
import BridgeStatus from './BridgeStatus.jsx';

export default function BridgePlugin() {
  const api = usePluginAPI();

  onMount(() => {
    // Register the bridge plugin
    api.registerPlugin('bridge-plugin', {
      name: 'Bridge Server Plugin',
      version: '1.0.0',
      description: 'Manages communication between Renzora Engine and project files',
      author: 'Renzora Engine Team'
    });

    // Register bridge viewport
    api.registerViewport('bridge-status', {
      title: 'Bridge Server Status',
      component: BridgeViewport,
      icon: IconServer,
      closable: true,
      plugin: 'bridge-plugin'
    });

    // Bridge plugin initialized with Engine API
  });

  // Return the bridge status component for use in the top bar
  return () => (
    <BridgeStatus 
      onOpenViewport={() => api.openViewport('bridge-status')} 
    />
  );
}

// Export the status component for direct use
export { default as BridgeStatus } from './BridgeStatus.jsx';
export { default as BridgeViewport } from './BridgeViewport.jsx';