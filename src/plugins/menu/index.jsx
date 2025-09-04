import { createPlugin } from '@/api/plugin';
import { IconRefresh, IconVideo, IconEdit, IconArrowLeft, IconArrowRight, IconPlus, IconFolder, IconFile, IconArrowDown, IconArrowUp, IconScissors, IconCopy, IconClipboard, IconTrash, IconCube, IconDownload, IconUpload, IconPhoto, IconDeviceGamepad2, IconWorld, IconDeviceDesktop, IconBox, IconCircle, IconCylinder, IconSquare, IconRecord, IconChairDirector, IconNetwork, IconBridge, IconHelp, IconHeadphones, IconBrandYoutube, IconBrandDiscord, IconBook, IconInfoCircle
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
          icon: IconPlus
        },
        { id: 'open', label: 'Open Project', icon: IconFolder, shortcut: 'Ctrl+O' },
        { id: 'save', label: 'Save Project', icon: IconFile, shortcut: 'Ctrl+S' },
        { id: 'save-as', label: 'Save As...', icon: IconFile, shortcut: 'Ctrl+Shift+S' },
        { divider: true },
        { 
          id: 'import', 
          label: 'Import', 
          icon: IconArrowDown,
          action: () => {
            document.dispatchEvent(new CustomEvent('engine:open-model-importer'));
          }
        },
        { 
          id: 'export', 
          label: 'Export Game', 
          icon: IconDeviceGamepad2,
          submenu: [
            { id: 'export-web', label: 'Web', icon: IconWorld },
            { id: 'export-desktop', label: 'Desktop', icon: IconDeviceDesktop }
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

    api.menu('create', {
      label: 'Create',
      icon: IconPlus,
      order: 3,
      submenu: [
        { id: 'create-scene', label: 'Scene', icon: IconChairDirector },
        { 
          id: 'mesh', 
          label: 'Mesh', 
          icon: IconCube,
          submenu: [
            { id: 'add-cube', label: 'Cube', icon: IconBox },
            { id: 'add-plane', label: 'Plane', icon: IconSquare },
            { id: 'add-cylinder', label: 'Cylinder', icon: IconCylinder },
            { id: 'add-sphere', label: 'Sphere', icon: IconCircle },
            { id: 'add-torus', label: 'Torus', icon: IconRecord }
          ]
        }
      ]
    });

    api.menu('viewports', {
      label: 'Viewports',
      icon: IconChairDirector,
      order: 4,
      submenu: [
        { id: 'viewport-node-editor', label: 'Node Editor', icon: IconNetwork },
        { id: 'viewport-bridge', label: 'Bridge', icon: IconBridge },
        { id: 'viewport-web-browser', label: 'Web Browser', icon: IconWorld, 
          action: () => {
            const api = document.querySelector('[data-plugin-api]')?.__pluginAPI;
            if (api) {
              api.open('web-browser', { label: 'Web Browser' });
            }
          }
        }
      ]
    });

    api.menu('help', {
      label: 'Help',
      icon: IconHelp,
      order: 5,
      submenu: [
        { id: 'help-support', label: 'Support', icon: IconHeadphones },
        { id: 'help-youtube', label: 'YouTube', icon: IconBrandYoutube },
        { id: 'help-discord', label: 'Discord', icon: IconBrandDiscord },
        { id: 'help-documentation', label: 'Documentation', icon: IconBook },
        { id: 'help-about', label: 'About', icon: IconInfoCircle }
      ]
    });

    console.log('[MenuPlugin] All menu items registered');
  }
});