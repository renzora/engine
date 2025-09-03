import { createPlugin } from '@/api/plugin';
import { IconRefresh, IconVideo, IconEdit, IconArrowLeft, IconArrowRight, IconPlus, IconFolder, IconFile, IconArrowDown, IconArrowUp, IconScissors, IconCopy, IconClipboard, IconTrash, IconCube, IconDownload, IconUpload, IconPhoto, IconDeviceGamepad2, IconWorld, IconBox
} from '@tabler/icons-solidjs';

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
      icon: IconFile,
      order: 1,
      submenu: [
        { 
          id: 'new', 
          label: 'New Project', 
          icon: IconPlus,
          submenu: [
            { id: 'new-blank', label: 'Blank Project', icon: IconFile },
            { id: 'new-template', label: 'From Template', icon: IconFolder,
              submenu: [
                { id: 'template-basic', label: 'Basic Scene', icon: IconCube },
                { id: 'template-game', label: 'Game Template', icon: IconDeviceGamepad2 },
                { id: 'template-arch', label: 'Architecture', icon: IconBox },
                { id: 'template-product', label: 'Product Viz', icon: IconBox }
              ]
            },
            { id: 'new-import', label: 'Import Existing', icon: IconArrowDown }
          ]
        },
        { id: 'open', label: 'Open Project', icon: IconFolder, shortcut: 'Ctrl+O' },
        { id: 'save', label: 'Save Project', icon: IconFile, shortcut: 'Ctrl+S' },
        { id: 'save-as', label: 'Save As...', icon: IconFile, shortcut: 'Ctrl+Shift+S' },
        { divider: true },
        { 
          id: 'import', 
          label: 'Import', 
          icon: IconArrowDown,
          submenu: [
            { id: 'import-model', label: 'Model Importer...', icon: IconDownload, 
              action: () => {
                document.dispatchEvent(new CustomEvent('engine:open-model-importer'));
              }
            },
            { divider: true },
            { id: 'import-fbx', label: 'FBX File', icon: IconDownload },
            { id: 'import-obj', label: 'OBJ File', icon: IconDownload },
            { id: 'import-gltf', label: 'GLTF/GLB File', icon: IconDownload },
            { id: 'import-blend', label: 'Blender File', icon: IconDownload },
            { divider: true },
            { id: 'import-image', label: 'Image as Plane', icon: IconPhoto },
            { id: 'import-hdri', label: 'HDRI Environment', icon: IconWorld }
          ]
        },
        { 
          id: 'export', 
          label: 'Export', 
          icon: IconArrowUp,
          submenu: [
            { id: 'export-scene', label: 'Export Scene', icon: IconCube,
              submenu: [
                { id: 'export-fbx', label: 'FBX Format', icon: IconUpload },
                { id: 'export-obj', label: 'OBJ Format', icon: IconUpload },
                { id: 'export-gltf', label: 'GLTF Format', icon: IconUpload },
                { id: 'export-blend', label: 'Blender Format', icon: IconUpload }
              ]
            },
            { id: 'export-render', label: 'Export Render', icon: IconVideo,
              submenu: [
                { id: 'export-png', label: 'PNG Image', icon: IconPhoto },
                { id: 'export-jpg', label: 'JPEG Image', icon: IconPhoto },
                { id: 'export-exr', label: 'EXR Image', icon: IconPhoto },
                { id: 'export-animation', label: 'Animation Sequence', icon: IconRefresh }
              ]
            }
          ]
        },
        { divider: true },
        { id: 'recent', label: 'Recent Projects', icon: IconRefresh },
      ],
      onClick: () => {
        console.log('[MenuPlugin] File menu clicked');
      }
    });

    api.menu('edit', {
      label: 'Edit',
      icon: IconEdit,
      order: 2,
      submenu: [
        { id: 'undo', label: 'Undo', icon: IconArrowLeft, shortcut: 'Ctrl+Z' },
        { id: 'redo', label: 'Redo', icon: IconArrowRight, shortcut: 'Ctrl+Y' },
        { divider: true },
        { id: 'cut', label: 'Cut', icon: IconScissors, shortcut: 'Ctrl+X' },
        { id: 'copy', label: 'Copy', icon: IconCopy, shortcut: 'Ctrl+C' },
        { id: 'paste', label: 'Paste', icon: IconClipboard, shortcut: 'Ctrl+V' },
        { id: 'duplicate', label: 'Duplicate', icon: IconCopy, shortcut: 'Ctrl+D' },
        { id: 'delete', label: 'Delete', icon: IconTrash, shortcut: 'Delete' },
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