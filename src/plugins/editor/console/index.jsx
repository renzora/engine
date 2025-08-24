import { createPlugin } from '@/api/plugin';
import { Terminal } from '@/ui/icons';
import Console from '@/pages/editor/Console.jsx';

export default createPlugin({
  id: 'console-plugin',
  name: 'Console Plugin',
  version: '1.0.0',
  description: 'Developer console for logs, errors, and commands',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[ConsolePlugin] Console plugin initialized');
  },

  async onStart(api) {
    console.log('[ConsolePlugin] Registering console panel...');

    // Register bottom panel tab
    api.panel('console', {
      title: 'Console',
      icon: Terminal,
      component: Console,
      order: 1,
      defaultHeight: 300,
      plugin: 'console-plugin'
    });

    console.log('[ConsolePlugin] Console panel registered');
  }
});