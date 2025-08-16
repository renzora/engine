import { Plugin } from '@/plugins/core/engine/Plugin.jsx';
import EditorPlugin from './index.jsx';
import DevNotice from '@/components/DevNotice';
import Scene from './propertiesPanel/tabs/Scene';
import GridDropdownContent from './ui/GridDropdownContent';
import CameraDropdownContent from './ui/CameraDropdownContent';
import { 
  IconCode, 
  IconPalette,
  IconCube,
  IconPlus,
  IconSettings,
  IconMaximize,
  IconGrid3x3,
  IconVideo
} from '@tabler/icons-solidjs';

export default class EditorPluginClass extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
    this.frameCount = 0;
  }

  getId() {
    return 'editor-plugin';
  }

  getName() {
    return 'Editor Plugin';
  }

  getVersion() {
    return '1.0.0';
  }

  getDescription() {
    return 'Core editor functionality for the Renzora Engine';
  }

  getAuthor() {
    return 'Renzora Engine Team';
  }

  async onInit() {
    console.log('[EditorPlugin] Initializing editor plugin...');
    
    // Register editor theme
    this.registerTheme('editor-theme', {
      name: 'Editor Theme',
      description: 'Default theme for the editor interface',
      colors: {
        primary: '#6366f1',
        secondary: '#8b5cf6',
        accent: '#06b6d4'
      },
      cssVariables: {
        '--editor-primary': '#6366f1',
        '--editor-secondary': '#8b5cf6',
        '--editor-grid': '#374151'
      }
    });

    console.log('[EditorPlugin] Editor plugin initialized');
  }

  async onStart() {
    console.log('[EditorPlugin] Starting editor plugin...');
    
    // Register editor top menu items
    this.registerTopMenuItem('editor-tools', {
      label: 'Tools',
      icon: IconCode,
      order: 10,
      onClick: () => {
        console.log('[EditorPlugin] Tools menu clicked');
        // Could open tools panel or show tools menu
      }
    });

    this.registerTopMenuItem('editor-view', {
      label: 'View',
      icon: IconCube,
      order: 20,
      onClick: () => {
        console.log('[EditorPlugin] View menu clicked');
        // Could control viewport options
      }
    });

    // Register scene hierarchy property tab
    console.log('[EditorPlugin] About to register scene property tab');
    const result = this.registerPropertyTab('scene', {
      title: 'Scene',
      component: (props) => (
        <Scene 
          selectedObject={props.selectedObject}
          onObjectSelect={props.onObjectSelect}
          onContextMenu={props.onContextMenu}
        />
      ),
      icon: IconCube,
      order: 1
    });
    console.log('[EditorPlugin] Scene property tab registration result:', result);


    // Register assets bottom panel tab
    this.registerBottomPanelTab('assets', {
      title: 'Assets',
      component: () => (
        <div class="h-full flex bg-slate-900">
          <div class="w-64 border-r border-slate-700 p-4">
            <h3 class="font-semibold text-white mb-4">Asset Browser</h3>
            <div class="space-y-2">
              <div class="bg-slate-800 p-3 rounded cursor-pointer hover:bg-slate-700">
                <div class="text-sm text-white font-medium">Models</div>
                <div class="text-xs text-gray-400">3D Meshes</div>
              </div>
              <div class="bg-slate-800 p-3 rounded cursor-pointer hover:bg-slate-700">
                <div class="text-sm text-white font-medium">Textures</div>
                <div class="text-xs text-gray-400">Images & Materials</div>
              </div>
              <div class="bg-slate-800 p-3 rounded cursor-pointer hover:bg-slate-700">
                <div class="text-sm text-white font-medium">Audio</div>
                <div class="text-xs text-gray-400">Sound Effects</div>
              </div>
            </div>
          </div>
          
          <div class="flex-1 p-4">
            <h3 class="font-semibold text-white mb-4">Asset Details</h3>
            <div class="text-center text-gray-400 mt-8">
              <p>Select an asset to view details</p>
            </div>
          </div>
        </div>
      ),
      icon: IconPalette,
      order: 1,
      defaultHeight: 300
    });

    // Listen to engine events
    this.on('object-selected', (data) => {
      console.log('[EditorPlugin] Object selected event received:', data);
    });

    // Start update loop tracking
    this.addUpdateCallback(() => {
      this.frameCount++;
      if (this.frameCount % 3600 === 0) { // Every 60 seconds at 60fps
        console.log(`[EditorPlugin] Running for ${this.frameCount / 60} seconds`);
      }
    });

    // Register toolbar buttons for right side menu
    this.registerToolbarButton('add', {
      title: 'Add',
      icon: IconPlus,
      section: 'right',
      order: 10,
      onClick: () => {
        console.log('[EditorPlugin] Add toolbar button clicked');
        // Handle add functionality
      }
    });

    // Register Camera Helper as dropdown button
    this.registerToolbarButton('camera-helper', {
      title: 'Camera Settings',
      icon: IconVideo,
      section: 'right',
      order: 15,
      hasDropdown: true,
      dropdownComponent: CameraDropdownContent,
      dropdownWidth: 256 // w-64 = 256px
    });

    // Register Grid Helper as dropdown button
    this.registerToolbarButton('grid-helper', {
      title: 'Grid Settings',
      icon: IconGrid3x3,
      section: 'right',
      order: 20,
      hasDropdown: true,
      dropdownComponent: GridDropdownContent,
      dropdownWidth: 288 // w-72 = 288px
    });

    this.registerToolbarButton('settings', {
      title: 'Settings',
      icon: IconSettings,
      section: 'right',
      order: 30,
      onClick: () => {
        console.log('[EditorPlugin] Settings toolbar button clicked');
        // Handle settings functionality
      }
    });

    this.registerToolbarButton('fullscreen', {
      title: 'Fullscreen',
      icon: IconMaximize,
      section: 'right',
      order: 40,
      onClick: () => {
        console.log('[EditorPlugin] Fullscreen toolbar button clicked');
        // Handle fullscreen toggle
        if (!document.fullscreenElement) {
          document.documentElement.requestFullscreen().catch(err => {
            console.log(`Error attempting to enable fullscreen: ${err.message}`);
          });
        } else {
          document.exitFullscreen();
        }
      }
    });

    console.log('[EditorPlugin] Editor plugin started');
  }

  onUpdate() {
    // This runs every frame (60fps) - handle real-time editor updates
    // Don't put heavy operations here
  }

  async onStop() {
    console.log('[EditorPlugin] Stopping editor plugin...');
    // Cleanup editor state
  }

  async onDispose() {
    console.log('[EditorPlugin] Disposing editor plugin...');
    this.frameCount = 0;
  }
}