import { createPlugin } from '@/api/plugin';
import { Refresh, Video, Edit, ArrowLeft, ArrowRight, Plus, Folder, File, ArrowDown, ArrowUp, Scissors, Copy, Clipboard, Trash, Cube, Download, Upload, Photo, GameController, Globe, Building, Box
} from '@/ui/icons';

export default createPlugin({
  id: 'menu-plugin',
  name: 'Menu Plugin',
  version: '1.0.0',
  description: 'Core application menu items',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[MenuPlugin] Menu plugin initialized');
  },

  async onStart(api) {
    console.log('[MenuPlugin] Registering menu items...');

    api.menu('file', {
      label: 'File',
      icon: File,
      order: 1,
      submenu: [
        { 
          id: 'new', 
          label: 'New Project', 
          icon: Plus,
          submenu: [
            { id: 'new-blank', label: 'Blank Project', icon: File },
            { id: 'new-template', label: 'From Template', icon: Folder,
              submenu: [
                { id: 'template-basic', label: 'Basic Scene', icon: Cube },
                { id: 'template-game', label: 'Game Template', icon: GameController },
                { id: 'template-arch', label: 'Architecture', icon: Building },
                { id: 'template-product', label: 'Product Viz', icon: Box }
              ]
            },
            { id: 'new-import', label: 'Import Existing', icon: ArrowDown }
          ]
        },
        { id: 'open', label: 'Open Project', icon: Folder, shortcut: 'Ctrl+O' },
        { id: 'save', label: 'Save Project', icon: File, shortcut: 'Ctrl+S' },
        { id: 'save-as', label: 'Save As...', icon: File, shortcut: 'Ctrl+Shift+S' },
        { divider: true },
        { 
          id: 'import', 
          label: 'Import', 
          icon: ArrowDown,
          submenu: [
            { id: 'import-fbx', label: 'FBX File', icon: Download },
            { id: 'import-obj', label: 'OBJ File', icon: Download },
            { id: 'import-gltf', label: 'GLTF/GLB File', icon: Download },
            { id: 'import-blend', label: 'Blender File', icon: Download },
            { divider: true },
            { id: 'import-image', label: 'Image as Plane', icon: Photo },
            { id: 'import-hdri', label: 'HDRI Environment', icon: Globe }
          ]
        },
        { 
          id: 'export', 
          label: 'Export', 
          icon: ArrowUp,
          submenu: [
            { id: 'export-scene', label: 'Export Scene', icon: Cube,
              submenu: [
                { id: 'export-fbx', label: 'FBX Format', icon: Upload },
                { id: 'export-obj', label: 'OBJ Format', icon: Upload },
                { id: 'export-gltf', label: 'GLTF Format', icon: Upload },
                { id: 'export-blend', label: 'Blender Format', icon: Upload }
              ]
            },
            { id: 'export-render', label: 'Export Render', icon: Video,
              submenu: [
                { id: 'export-png', label: 'PNG Image', icon: Photo },
                { id: 'export-jpg', label: 'JPEG Image', icon: Photo },
                { id: 'export-exr', label: 'EXR Image', icon: Photo },
                { id: 'export-animation', label: 'Animation Sequence', icon: Refresh }
              ]
            }
          ]
        },
        { divider: true },
        { id: 'recent', label: 'Recent Projects', icon: Refresh },
      ],
      onClick: () => {
        console.log('[MenuPlugin] File menu clicked');
      }
    });

    api.menu('edit', {
      label: 'Edit',
      icon: Edit,
      order: 2,
      submenu: [
        { id: 'undo', label: 'Undo', icon: ArrowLeft, shortcut: 'Ctrl+Z' },
        { id: 'redo', label: 'Redo', icon: ArrowRight, shortcut: 'Ctrl+Y' },
        { divider: true },
        { id: 'cut', label: 'Cut', icon: Scissors, shortcut: 'Ctrl+X' },
        { id: 'copy', label: 'Copy', icon: Copy, shortcut: 'Ctrl+C' },
        { id: 'paste', label: 'Paste', icon: Clipboard, shortcut: 'Ctrl+V' },
        { id: 'duplicate', label: 'Duplicate', icon: Copy, shortcut: 'Ctrl+D' },
        { id: 'delete', label: 'Delete', icon: Trash, shortcut: 'Delete' },
        { divider: true },
        { id: 'select-all', label: 'Select All', shortcut: 'Ctrl+A' },
      ],
      onClick: () => {
        console.log('[MenuPlugin] Edit menu clicked');
      }
    });

    console.log('[MenuPlugin] All menu items registered');
  }
});