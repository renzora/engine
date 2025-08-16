import { Plugin } from '@/plugins/core/engine/Plugin.jsx';
import { bridgeService } from './index.jsx';
import BridgeStatus from './BridgeStatus.jsx';
import BridgeViewport from './BridgeViewport.jsx';
import { IconServerBolt, IconDatabase, IconCloud } from '@tabler/icons-solidjs';

export default class BridgePluginClass extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
    this.bridgeService = bridgeService;
  }

  getId() {
    return 'bridge-plugin';
  }

  getName() {
    return 'Bridge Server Plugin';
  }

  getVersion() {
    return '1.0.0';
  }

  getDescription() {
    return 'Manages communication between Renzora Engine and project files';
  }

  getAuthor() {
    return 'Renzora Engine Team';
  }

  async onInit() {
    console.log('[BridgePlugin] Initializing bridge server connection...');
    
    // Register bridge theme
    this.registerTheme('bridge-theme', {
      name: 'Bridge Connection Theme',
      description: 'A theme optimized for bridge server communication',
      colors: {
        primary: '#0ea5e9',
        secondary: '#06b6d4',
        accent: '#22d3ee'
      },
      cssVariables: {
        '--bridge-primary': '#0ea5e9',
        '--bridge-secondary': '#06b6d4'
      }
    });

    console.log('[BridgePlugin] Bridge plugin initialized');
  }

  async onStart() {
    console.log('[BridgePlugin] Starting bridge server plugin...');
    
    // Register bridge top menu item
    this.registerTopMenuItem('bridge-status', {
      label: 'Bridge',
      icon: IconServerBolt,
      order: 5,
      onClick: () => {
        console.log('[BridgePlugin] Bridge menu clicked - could show status modal');
        // Could open bridge status modal/dialog
      }
    });

    // Register bridge property tab
    this.registerPropertyTab('bridge-properties', {
      title: 'Bridge Config',
      component: () => (
        <div class="p-4 space-y-4">
          <h3 class="font-semibold text-white">Bridge Configuration</h3>
          <div class="space-y-3">
            <div>
              <label class="block text-sm text-gray-300 mb-1">Server URL</label>
              <input 
                type="text" 
                value="http://localhost:3001"
                class="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded text-white"
                readonly
              />
            </div>
            <div>
              <label class="flex items-center gap-2 text-sm text-gray-300">
                <input type="checkbox" class="rounded" checked />
                Auto-reconnect on failure
              </label>
            </div>
            <button class="w-full px-3 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded">
              Restart Bridge Connection
            </button>
          </div>
        </div>
      ),
      icon: IconDatabase,
      order: 3
    });

    // Register bridge console tab
    this.registerBottomPanelTab('bridge-console', {
      title: 'Bridge Console',
      component: () => (
        <div class="h-full flex flex-col bg-slate-900">
          <div class="flex items-center justify-between p-3 border-b border-slate-700">
            <h3 class="font-semibold text-white">Bridge Server Console</h3>
            <button class="px-2 py-1 bg-gray-600 hover:bg-gray-700 text-white text-xs rounded">
              Clear
            </button>
          </div>
          <div class="flex-1 overflow-auto p-3 space-y-1">
            <div class="text-xs font-mono text-green-400">
              <span class="text-gray-500">{new Date().toLocaleTimeString()}</span>
              <span class="text-green-500 font-bold ml-2">[INFO]</span>
              <span class="ml-2">Bridge server connected on port 3001</span>
            </div>
            <div class="text-xs font-mono text-blue-400">
              <span class="text-gray-500">{new Date().toLocaleTimeString()}</span>
              <span class="text-blue-500 font-bold ml-2">[DEBUG]</span>
              <span class="ml-2">File watcher service started</span>
            </div>
          </div>
        </div>
      ),
      icon: IconCloud,
      order: 10,
      defaultHeight: 250
    });

    // Listen to engine events
    this.on('project-selected', (data) => {
      console.log('[BridgePlugin] Project selected event received:', data);
      if (data?.project) {
        this.bridgeService.setCurrentProject(data.project);
      }
    });

    console.log('[BridgePlugin] Bridge server plugin started');
  }

  onUpdate() {
    // This runs every frame - could be used for connection health checks
    // Don't put heavy operations here
  }

  async onStop() {
    console.log('[BridgePlugin] Stopping bridge server plugin...');
    // Cleanup bridge connections if needed
  }

  async onDispose() {
    console.log('[BridgePlugin] Disposing bridge server plugin...');
    // Final cleanup
  }
}