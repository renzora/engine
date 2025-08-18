import { createPlugin } from '@/api/plugin';
import NodeEditor from '@/pages/editor/nodeEditor/index.jsx';

const NodeIcon = () => <div>🔗</div>;

export default createPlugin({
  id: 'node-editor-plugin',
  name: 'Node Editor Plugin',
  version: '1.0.0',
  description: 'Visual node-based scripting interface',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[NodeEditorPlugin] Initializing node editor plugin...');
  },

  async onStart(api) {
    console.log('[NodeEditorPlugin] Starting node editor plugin...');
    
    // Register NodeEditor as a viewport type
    api.viewport('node-editor', {
      label: 'Node Editor',
      component: NodeEditor,
      icon: NodeIcon,
      description: 'Visual node-based scripting interface'
    });

    console.log('[NodeEditorPlugin] Node editor plugin started');
  },

  async onStop() {
    console.log('[NodeEditorPlugin] Stopping node editor plugin...');
  },

  async onDispose() {
    console.log('[NodeEditorPlugin] Disposing node editor plugin...');
  }
});