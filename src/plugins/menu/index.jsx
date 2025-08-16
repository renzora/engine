import { Plugin } from '@/plugins/core/engine/Plugin.jsx';
import { 
  IconPointer, IconArrowsMove, IconRotateClockwise2, IconMaximize, 
  IconCamera, IconEdit, IconArrowLeft, IconArrowRight, IconPlus, 
  IconFolder, IconFile, IconArrowDown, IconArrowUp, IconRefresh, 
  IconScissors, IconCopy, IconClipboard, IconTrash, IconSettings, 
  IconGridDots, IconSun, IconCube 
} from '@tabler/icons-solidjs';

export default class MenuPluginClass extends Plugin {
  constructor(engineAPI) {
    super(engineAPI);
  }

  getId() {
    return 'menu-plugin';
  }

  getName() {
    return 'Menu Plugin';
  }

  getVersion() {
    return '1.0.0';
  }

  getDescription() {
    return 'Core application menu items';
  }

  getAuthor() {
    return 'Renzora Engine Team';
  }

  async onInit() {
    console.log('[MenuPlugin] Menu plugin initialized');
  }

  async onStart() {
    console.log('[MenuPlugin] Registering menu items...');
    
    // File Menu
    this.registerTopMenuItem('file', {
      label: 'File',
      icon: IconFile,
      order: 1,
      submenu: [
        { id: 'new', label: 'New Project', icon: IconPlus },
        { id: 'open', label: 'Open Project', icon: IconFolder },
        { id: 'save', label: 'Save Project', icon: IconFile },
        { id: 'save-as', label: 'Save As...', icon: IconFile },
        { divider: true },
        { id: 'import', label: 'Import', icon: IconArrowDown },
        { id: 'export', label: 'Export', icon: IconArrowUp },
        { divider: true },
        { id: 'recent', label: 'Recent Projects', icon: IconRefresh },
      ],
      onClick: () => {
        console.log('[MenuPlugin] File menu clicked');
      }
    });

    // Edit Menu
    this.registerTopMenuItem('edit', {
      label: 'Edit',
      icon: IconEdit,
      order: 2,
      submenu: [
        { id: 'undo', label: 'Undo', icon: IconArrowLeft },
        { id: 'redo', label: 'Redo', icon: IconArrowRight },
        { divider: true },
        { id: 'cut', label: 'Cut', icon: IconScissors },
        { id: 'copy', label: 'Copy', icon: IconCopy },
        { id: 'paste', label: 'Paste', icon: IconClipboard },
        { id: 'duplicate', label: 'Duplicate', icon: IconCopy },
        { id: 'delete', label: 'Delete', icon: IconTrash },
        { divider: true },
        { id: 'select-all', label: 'Select All' },
      ],
      onClick: () => {
        console.log('[MenuPlugin] Edit menu clicked');
      }
    });

    // View Menu
    this.registerTopMenuItem('view', {
      label: 'View',
      icon: IconCube,
      order: 3,
      submenu: [
        { id: 'wireframe', label: 'Wireframe Mode' },
        { id: 'solid', label: 'Solid Mode' },
        { id: 'material', label: 'Material Preview' },
        { id: 'rendered', label: 'Rendered Mode' },
        { divider: true },
        { id: 'grid', label: 'Show Grid' },
        { id: 'axes', label: 'Show Axes' },
        { id: 'statistics', label: 'Show Statistics' },
        { divider: true },
        { id: 'fullscreen', label: 'Fullscreen' },
      ],
      onClick: () => {
        console.log('[MenuPlugin] View menu clicked');
      }
    });

    // Tools Menu
    this.registerTopMenuItem('tools', {
      label: 'Tools',
      icon: IconPointer,
      order: 4,
      submenu: [
        { id: 'select', label: 'Select Tool', icon: IconPointer },
        { id: 'move', label: 'Move Tool', icon: IconArrowsMove },
        { id: 'rotate', label: 'Rotate Tool', icon: IconRotateClockwise2 },
        { id: 'scale', label: 'Scale Tool', icon: IconMaximize },
        { divider: true },
        { id: 'subdivision', label: 'Subdivision Surface', icon: IconGridDots },
        { id: 'mirror', label: 'Mirror Modifier', icon: IconCopy },
        { divider: true },
        { id: 'camera', label: 'Camera Tool', icon: IconCamera },
        { id: 'light', label: 'Light Tool', icon: IconSun },
        { id: 'mesh', label: 'Add Mesh', icon: IconCube },
      ],
      onClick: () => {
        console.log('[MenuPlugin] Tools menu clicked');
      }
    });

    // Window Menu
    this.registerTopMenuItem('window', {
      label: 'Window',
      icon: IconSettings,
      order: 5,
      submenu: [
        { id: 'scene-panel', label: 'Scene Panel' },
        { id: 'properties-panel', label: 'Properties Panel' },
        { id: 'assets-panel', label: 'Assets Panel' },
        { id: 'console-panel', label: 'Console Panel' },
        { divider: true },
        { id: 'settings', label: 'Settings', icon: IconSettings },
        { id: 'reset-layout', label: 'Reset Layout' },
      ],
      onClick: () => {
        console.log('[MenuPlugin] Window menu clicked');
      }
    });

    console.log('[MenuPlugin] All menu items registered');
  }
}