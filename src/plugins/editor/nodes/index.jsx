import { createPlugin } from '@/api/plugin';
import Nodes from '@/pages/editor/Nodes.jsx';

// Create node editor icon
const NodesIcon = (props) => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" {...props}>
    <circle cx="5" cy="6" r="3"/>
    <circle cx="19" cy="18" r="3"/>
    <circle cx="12" cy="12" r="3"/>
    <path d="M8 6h5"/>
    <path d="M15 12h2"/>
    <path d="M9 15l3-3"/>
  </svg>
);

export default createPlugin({
  id: 'nodes-plugin',
  name: 'Nodes Plugin',
  version: '1.0.0',
  description: 'Node-based visual scripting and material editor',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[NodesPlugin] Nodes plugin initialized');
  },

  async onStart(api) {
    console.log('[NodesPlugin] Registering nodes panel...');

    // Register bottom panel tab
    api.panel('nodes', {
      title: 'Nodes',
      icon: NodesIcon,
      component: Nodes,
      order: 6,
      defaultHeight: 500,
      plugin: 'nodes-plugin'
    });

    console.log('[NodesPlugin] Nodes panel registered');
  }
});