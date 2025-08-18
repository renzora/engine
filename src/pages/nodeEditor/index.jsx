import { onMount, onCleanup } from 'solid-js';
import { usePluginAPI } from '@/api/plugin';
import NodeEditor from '../editor/nodeEditor/index.jsx';

const NodeIcon = () => <div>🔗</div>;

export default function NodeEditorPage() {
  onMount(() => {
    const pluginAPI = usePluginAPI();
    
    // Register NodeEditor as a viewport type so it can be opened in the main viewport
    pluginAPI.registerViewportType('node-editor', {
      label: 'Node Editor',
      component: NodeEditor,
      icon: NodeIcon,
      description: 'Visual node-based scripting interface'
    });
    
    onCleanup(() => {
      // TODO: Add unregister methods to plugin API
    });
  });

  return null; // This component just registers with the plugin API
}