import { createPlugin } from '@/api/plugin';
import { TestPipe, Bug, Flask, Code } from '@/ui/icons';

function TestConsole() {
  return (
    <div class="p-4 bg-gray-900 text-white h-full">
      <h3 class="font-semibold text-white mb-4">Test Plugin Console</h3>
      <div class="bg-black p-3 rounded border h-64 overflow-y-auto font-mono text-sm">
        <div class="text-green-400">[TestPlugin] Console initialized</div>
        <div class="text-blue-400">[TestPlugin] Ready for testing</div>
        <div class="text-yellow-400">[TestPlugin] Using new function-based API!</div>
      </div>
    </div>
  );
}

export default createPlugin({
  id: 'test-plugin',
  name: 'Test Plugin',
  version: '1.0.0',
  description: 'A test plugin demonstrating the new function-based API',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[TestPlugin] Initializing test plugin...');
  },

  async onStart(api) {
    console.log('[TestPlugin] Starting test plugin...');
  
    // Register a test menu
    api.menu('test', {
      label: 'Test',
      icon: TestPipe,
      order: 100,
      submenu: [
        {
          id: 'debug',
          label: 'Debug Tools',
          icon: Bug,
          onClick: () => console.log('[TestPlugin] Debug tools clicked')
        },
        {
          id: 'test-tools',
          label: 'Test Tools',
          icon: Flask,
          onClick: () => console.log('[TestPlugin] Test tools clicked')
        }
      ]
    });

    // Register a test console panel
    api.panel('test-console', {
      title: 'Test Console',
      icon: Code,
      component: TestConsole,
      defaultHeight: 300
    });

    // Register a test theme
    api.theme('test-theme', {
      name: 'Test Theme',
      description: 'A test theme for development',
      cssVariables: {
        '--test-color': '#00ff00'
      }
    });

    console.log('[TestPlugin] Test plugin started with new API!');
  },

  onUpdate() {
    // Test update logic
  },

  async onStop() {
    console.log('[TestPlugin] Stopping test plugin...');
  },

  async onDispose() {
    console.log('[TestPlugin] Disposing test plugin...');
  }
});