import { createSignal, createEffect, onCleanup, createMemo, For, Show } from 'solid-js';
import { 
  IconPointer, IconArrowsMove, IconRotateClockwise2, IconMaximize, 
  IconCamera, IconEdit, IconArrowLeft, IconArrowRight, IconPlus, 
  IconFolder, IconFile, IconArrowDown, IconArrowUp, IconRefresh, 
  IconScissors, IconCopy, IconClipboard, IconTrash, IconSettings, 
  IconGridDots, IconSun, IconCube 
} from '@tabler/icons-solidjs';
import { editorStore, editorActions } from "@/plugins/editor/stores/EditorStore";
import { viewportStore, viewportActions } from "@/plugins/editor/stores/ViewportStore";
import { bridgeService } from "@/plugins/core/bridge";
import { BridgeStatus } from "@/plugins/core/bridge";
import { topMenuItems } from "@/plugins/core/engine";
import { getCurrentWindow } from '@tauri-apps/api/window';
import { UpdateModal } from "@/components/UpdateModal";

function TopBarMenu() {
  const [activeMenu, setActiveMenu] = createSignal(null);
  const [isSaving, setIsSaving] = createSignal(false);
  const [lastSync, setLastSync] = createSignal(null);
  const [hasUnsavedChanges, setHasUnsavedChanges] = createSignal(false);
  const [showSyncTooltip, setShowSyncTooltip] = createSignal(false);
  const [showUpdateTooltip, setShowUpdateTooltip] = createSignal(false);
  const [showProjectManager, setShowProjectManager] = createSignal(false);
  const [showUpdateModal, setShowUpdateModal] = createSignal(false);
  const [selectedTool, setSelectedTool] = createSignal('select');
  const [flashingTool, setFlashingTool] = createSignal(null);
  const [menuPosition, setMenuPosition] = createSignal(null);
  const [showRendererDropdown, setShowRendererDropdown] = createSignal(false);
  const [rendererDropdownPosition, setRendererDropdownPosition] = createSignal(null);
  const [isMaximized, setIsMaximized] = createSignal(false);

  // Click outside detection for dropdowns
  createEffect(() => {
    const handleClickOutside = (event) => {
      const target = event.target;
      const isMenuButton = target.closest('.menu-button');
      const isDropdownContent = target.closest('.dropdown-content');
      
      if (!isMenuButton && !isDropdownContent) {
        // Close all dropdowns and menus
        setActiveMenu(null);
        setMenuPosition(null);
        setShowRendererDropdown(false);
        setRendererDropdownPosition(null);
      }
    };

    if (activeMenu() || showRendererDropdown()) {
      document.addEventListener('mousedown', handleClickOutside);
      onCleanup(() => {
        document.removeEventListener('mousedown', handleClickOutside);
      });
    }
  });

  const calculateDropdownPosition = (buttonRect, dropdownWidth = 192) => {
    const viewportWidth = window.innerWidth;
    const margin = 8;
    
    // Try to center the dropdown under the button
    let left = buttonRect.left + (buttonRect.width / 2) - (dropdownWidth / 2);
    
    // Check if dropdown would go off the right edge
    if (left + dropdownWidth + margin > viewportWidth) {
      left = viewportWidth - dropdownWidth - margin;
    }
    
    // Check if dropdown would go off the left edge
    if (left < margin) {
      left = margin;
    }
    
    return {
      left,
      top: buttonRect.bottom + 4
    };
  };

  // Check if we're running in Tauri
  const isTauri = () => {
    try {
      // Try to get current window - if it works, we're in Tauri
      getCurrentWindow();
      return true;
    } catch {
      return false;
    }
  };

  const transformMode = createMemo(() => editorStore.selection.transformMode);
  const currentProject = createMemo(() => bridgeService.getCurrentProject());
  const settings = createMemo(() => editorStore.settings);

  const handleSave = async () => {
    if (isSaving()) return;
    
    try {
      setIsSaving(true);
      // TODO: Implement actual save functionality
      editorActions.addConsoleMessage('Save not implemented', 'warning');
    } catch (error) {
      console.error('Save failed:', error);
      editorActions.addConsoleMessage('Failed to save project', 'error');
    } finally {
      setIsSaving(false);
    }
  };

  // Tauri window control functions
  const handleMinimize = async () => {
    try {
      console.log('Minimize clicked');
      const currentWindow = getCurrentWindow();
      await currentWindow.minimize();
      console.log('Minimize successful');
    } catch (error) {
      console.error('Minimize failed:', error);
    }
  };

  const handleMaximize = async () => {
    try {
      console.log('Maximize clicked');
      const currentWindow = getCurrentWindow();
      await currentWindow.toggleMaximize();
      console.log('Maximize successful');
    } catch (error) {
      console.error('Maximize failed:', error);
    }
  };

  const handleClose = async () => {
    try {
      console.log('Close clicked');
      const currentWindow = getCurrentWindow();
      await currentWindow.close();
      console.log('Close successful');
    } catch (error) {
      console.error('Close failed:', error);
    }
  };

  createEffect(() => {
    const project = bridgeService.getCurrentProject();
    if (project?.loaded) {
      setLastSync(new Date(project.loaded));
    }
  });

  // Track window maximize state in Tauri
  createEffect(async () => {
    if (isTauri()) {
      try {
        const currentWindow = getCurrentWindow();
        
        // Check initial maximized state
        const maximized = await currentWindow.isMaximized();
        setIsMaximized(maximized);
        
        // Listen for window resize events
        await currentWindow.listen('tauri://resize', async () => {
          const maximized = await currentWindow.isMaximized();
          setIsMaximized(maximized);
        });
      } catch (error) {
        console.error('Failed to setup window state tracking:', error);
      }
    }
  });

  createEffect(() => {
    const checkUnsavedChanges = () => {
      setHasUnsavedChanges(false); // TODO: Implement change tracking
    };

    checkUnsavedChanges();
    const interval = setInterval(checkUnsavedChanges, 1000);
    return () => clearInterval(interval);
  });

  const formatLastSync = (date) => {
    if (!date) return 'Never synced';
    
    const now = new Date();
    const diff = now - date;
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);
    
    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    return `${days}d ago`;
  };

  const getSyncStatusInfo = createMemo(() => {
    if (hasUnsavedChanges()) {
      return {
        color: 'bg-yellow-500',
        tooltip: 'Unsaved changes - will auto-save soon'
      };
    }
    return {
      color: 'bg-green-500',
      tooltip: `Last sync: ${formatLastSync(lastSync())}`
    };
  });

  const tools = [
    { id: 'select', icon: IconPointer, title: 'Select' },
    { id: 'move', icon: IconArrowsMove, title: 'Move' },
    { id: 'rotate', icon: IconRotateClockwise2, title: 'Rotate' },
    { id: 'scale', icon: IconMaximize, title: 'Scale' },
    { divider: true },
    { id: 'camera', icon: IconCamera, title: 'Camera' },
    { id: 'paint', icon: IconEdit, title: 'Paint' },
    { divider: true },
    { id: 'undo', icon: IconArrowLeft, title: 'Undo' },
    { id: 'redo', icon: IconArrowRight, title: 'Redo' },
  ];

  const getEffectiveSelectedTool = () => {
    if (['select', 'move', 'rotate', 'scale'].includes(transformMode())) {
      return transformMode();
    }
    return selectedTool();
  };

  const handleToolSelect = (toolId) => {
    if (toolId === 'undo' || toolId === 'redo') {
      setFlashingTool(toolId);
      setTimeout(() => setFlashingTool(null), 200);
      console.log(`${toolId} action triggered`);
    } else {
      setSelectedTool(toolId);
      editorActions.setTransformMode(toolId);
    }
  };

  const rendererOptions = [
    { id: 'webgl', label: 'WebGL' },
    { id: 'webgpu', label: 'WebGPU' }
  ];

  const handleRendererChange = (rendererId) => {
    editorActions.updateViewportSettings({ renderingEngine: rendererId });
    setShowRendererDropdown(false);
    editorActions.addConsoleMessage(`Switched to ${rendererId.toUpperCase()} renderer`, 'success');
  };

  // Create dynamic menu structure from plugin extensions only
  const menuStructure = createMemo(() => {
    const pluginMenuItems = topMenuItems();
    const pluginMenuArray = Array.from(pluginMenuItems.values())
      .sort((a, b) => (a.order || 0) - (b.order || 0));
    
    const menuStructure = {};
    
    // Add plugin menu items as top-level menus
    pluginMenuArray.forEach(item => {
      menuStructure[item.label] = item.submenu || [
        { 
          id: item.id, 
          label: item.label, 
          icon: item.icon,
          action: item.onClick 
        }
      ];
    });

    return menuStructure;
  });

  const handleMenuClick = (menuName, event) => {
    console.log('Menu clicked:', menuName, 'Current active:', activeMenu());
    if (activeMenu() === menuName) {
      console.log('Closing menu:', menuName);
      setActiveMenu(null);
      setMenuPosition(null);
    } else {
      console.log('Opening menu:', menuName);
      // Close renderer dropdown if open
      setShowRendererDropdown(false);
      setRendererDropdownPosition(null);
      
      const rect = event.currentTarget.getBoundingClientRect();
      const position = calculateDropdownPosition(rect, 224); // Menu width is 224px (w-56)
      setMenuPosition({
        left: position.left,
        top: rect.bottom + 1
      });
      setActiveMenu(menuName);
    }
  };

  const handleItemClick = (item) => {
    setActiveMenu(null);
    setMenuPosition(null);
    if (item.action) {
      item.action();
    } else if (['new', 'open', 'export'].includes(item.id)) {
      setShowProjectManager(true);
    } else if (item.id === 'subdivision') {
      editorActions.addConsoleMessage('Subdivision Surface applied to selected object', 'success');
    } else if (item.id === 'mirror') {
      editorActions.addConsoleMessage('Mirror Modifier applied to selected object', 'success');
    } else if (item.id === 'settings') {
      editorActions.addConsoleMessage('Settings functionality removed', 'info');
    } else {
      console.log('Menu item clicked:', item.id);
      editorActions.addConsoleMessage(`Menu action: ${item.label}`, 'info');
    }
  };

  return (
    <>
      <div 
        class="relative w-full h-8 bg-gray-900/95 backdrop-blur-sm border-b border-gray-800 flex items-center px-2"
        data-tauri-drag-region
      >
        <div 
          style={{
            '-webkit-app-region': 'no-drag'
          }}
          class="flex items-center"
        >
          <For each={Object.entries(menuStructure())}>
            {([menuName, items]) => (
              <div class="relative inline-block">
                <button
                  onClick={(e) => handleMenuClick(menuName, e)}
                  onMouseEnter={(e) => {
                    console.log('Hovering over menu:', menuName, 'Current active menu:', activeMenu());
                    if (activeMenu()) {
                      console.log('Switching from', activeMenu(), 'to', menuName);
                      // Close renderer dropdown if open
                      setShowRendererDropdown(false);
                      setRendererDropdownPosition(null);
                      
                      const rect = e.currentTarget.getBoundingClientRect();
                      const position = calculateDropdownPosition(rect, 224);
                      setMenuPosition({
                        left: position.left,
                        top: rect.bottom + 1
                      });
                      setActiveMenu(menuName);
                    } else {
                      console.log('No active menu, not switching');
                    }
                  }}
                  class={`menu-button px-3 py-1 text-sm text-gray-300 hover:bg-gray-700/50 rounded transition-colors ${
                    activeMenu() === menuName ? 'bg-gray-700/50' : ''
                  }`}
                >
                  {menuName}
                </button>
              </div>
            )}
          </For>
        </div>
        
        <div class="flex-1" />
        <div 
          class="flex items-center gap-3 text-xs text-gray-500"
          style={{
            '-webkit-app-region': 'no-drag'
          }}
        >
          <div class="flex items-center gap-2">
            <BridgeStatus />
            <span class="text-gray-400">
              {currentProject()?.name || 'Renzora Engine v1.0.0'}
            </span>
            <Show when={currentProject()?.name}>
              <div 
                class={`w-1.5 h-1.5 ${getSyncStatusInfo().color} rounded-full cursor-pointer relative`}
                onMouseEnter={() => setShowSyncTooltip(true)}
                onMouseLeave={() => setShowSyncTooltip(false)}
              >
                <Show when={showSyncTooltip()}>
                  <div class="absolute bottom-full left-1/2 transform -translate-x-1/2 mb-2 px-2 py-1 bg-gray-900/95 text-white text-xs rounded whitespace-nowrap z-[120]">
                    {getSyncStatusInfo().tooltip}
                    <div class="absolute top-full left-1/2 transform -translate-x-1/2 border-4 border-transparent border-t-gray-900/95" />
                  </div>
                </Show>
              </div>
            </Show>
          </div>
          
          <Show when={currentProject()?.name}>
            <div class="flex items-center gap-2">
              <span class="text-gray-600">•</span>
              <span 
                class="text-orange-400 text-xs font-medium flex items-center gap-1 cursor-pointer relative"
                onMouseEnter={() => setShowUpdateTooltip(true)}
                onMouseLeave={() => setShowUpdateTooltip(false)}
                onClick={() => setShowUpdateModal(true)}
              >
                Renzora Engine r1
                <IconArrowDown class="w-3 h-3" />
                <Show when={showUpdateTooltip()}>
                  <div class="absolute right-full top-1/2 transform -translate-y-1/2 mr-2 px-2 py-1 bg-gray-900/95 text-white text-xs rounded whitespace-nowrap z-[120]">
                    Update to r2
                    <div class="absolute left-full top-1/2 transform -translate-y-1/2 border-4 border-transparent border-l-gray-900/95" />
                  </div>
                </Show>
              </span>
              <span class="text-gray-600">•</span>
              <div class="relative">
                <button
                  onClick={(e) => {
                    if (showRendererDropdown()) {
                      setShowRendererDropdown(false);
                      setRendererDropdownPosition(null);
                    } else {
                      // Close menu if open
                      setActiveMenu(null);
                      setMenuPosition(null);
                      
                      const rect = e.currentTarget.getBoundingClientRect();
                      const position = calculateDropdownPosition(rect, 128);
                      setRendererDropdownPosition(position);
                      setShowRendererDropdown(true);
                    }
                  }}
                  class="menu-button text-blue-400 font-medium hover:text-blue-300 transition-colors px-2 py-1 rounded hover:bg-gray-700/50 flex items-center gap-1"
                >
                  {(settings().viewport.renderingEngine || 'webgl').toUpperCase()}
                  <svg 
                    class={`w-3 h-3 transition-transform ${showRendererDropdown() ? 'rotate-180' : ''}`} 
                    fill="currentColor" 
                    viewBox="0 0 20 20"
                  >
                    <path fill-rule="evenodd" d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z" clip-rule="evenodd" />
                  </svg>
                </button>
              </div>
            </div>
          </Show>

          {/* Tauri Window Controls */}
          <Show when={isTauri()}>
            <div class="flex ml-4">
              <button 
                class="w-11 h-8 flex items-center justify-center text-gray-400 hover:bg-gray-700/50 hover:text-white transition-colors"
                onClick={handleMinimize}
                title="Minimize"
              >
                <svg width="10" height="10" viewBox="0 0 10 10">
                  <rect x="0" y="4" width="10" height="1" fill="currentColor" />
                </svg>
              </button>
              
              <button 
                class="w-11 h-8 flex items-center justify-center text-gray-400 hover:bg-gray-700/50 hover:text-white transition-colors"
                onClick={handleMaximize}
                title={isMaximized() ? "Restore Down" : "Maximize"}
              >
                {isMaximized() ? (
                  <svg width="10" height="10" viewBox="0 0 10 10">
                    <rect x="1" y="1" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
                    <rect x="2" y="0" width="7" height="7" fill="none" stroke="currentColor" stroke-width="1" />
                  </svg>
                ) : (
                  <svg width="10" height="10" viewBox="0 0 10 10">
                    <rect x="0" y="0" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1" />
                  </svg>
                )}
              </button>
              
              <button 
                class="w-11 h-8 flex items-center justify-center text-gray-400 hover:bg-red-600 hover:text-white transition-colors"
                onClick={handleClose}
                title="Close"
              >
                <svg width="10" height="10" viewBox="0 0 10 10">
                  <line x1="0" y1="0" x2="10" y2="10" stroke="currentColor" stroke-width="1" />
                  <line x1="10" y1="0" x2="0" y2="10" stroke="currentColor" stroke-width="1" />
                </svg>
              </button>
            </div>
          </Show>
        </div>
      </div>
      
      <Show when={activeMenu() && menuPosition()}>
        <div 
          class="dropdown-content fixed w-56 bg-gradient-to-br from-gray-900/98 to-gray-950/98 backdrop-blur-sm rounded-lg shadow-[0_20px_25px_-5px_rgba(0,0,0,0.4)] overflow-hidden z-[110] border border-gray-700/50"
          style={{
            left: menuPosition().left + 'px',
            top: menuPosition().top + 'px'
          }}
        >
          <div class="p-1">
            <For each={menuStructure()[activeMenu()]}>
              {(item, index) => (
                <>
                  <Show when={item.divider}>
                    <div class="border-t border-gray-700/50 my-1 mx-2" />
                  </Show>
                  <Show when={!item.divider}>
                    <button
                      class="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-gradient-to-r hover:from-blue-600/90 hover:to-blue-500/90 hover:text-white flex items-center justify-between transition-all duration-150 group relative rounded-md hover:shadow-lg"
                      onClick={() => handleItemClick(item)}
                    >
                      <div class="flex items-center gap-2">
                        <Show when={item.icon}>
                          <span class="w-4 h-4 flex items-center justify-center text-gray-400 group-hover:text-white">
                            <Show when={item.id === 'save' && isSaving()} fallback={<item.icon class="w-3.5 h-3.5" />}>
                              <div class="w-3 h-3 border-2 border-gray-400 border-t-transparent rounded-full animate-spin" />
                            </Show>
                          </span>
                        </Show>
                        <span class="font-normal">
                          {item.id === 'save' && isSaving() ? 'Saving...' : item.label}
                        </span>
                      </div>
                    </button>
                  </Show>
                </>
              )}
            </For>
          </div>
        </div>
      </Show>
      
      <Show when={showRendererDropdown() && rendererDropdownPosition()}>
        <div 
          class="dropdown-content fixed w-32 bg-gray-800/95 backdrop-blur-sm rounded-lg shadow-xl border border-gray-600/50 z-[210]"
          style={{
            left: rendererDropdownPosition().left + 'px',
            top: rendererDropdownPosition().top + 'px'
          }}
        >
          <For each={rendererOptions}>
            {(option) => (
              <button
                onClick={() => handleRendererChange(option.id)}
                class={`w-full px-3 py-2 text-left text-sm transition-colors flex items-center gap-2 first:rounded-t-lg last:rounded-b-lg ${
                  settings().viewport.renderingEngine === option.id
                    ? 'bg-green-600/90 text-white'
                    : 'text-gray-300 hover:bg-gray-900/60 hover:text-white'
                }`}
              >
                {option.label}
                <Show when={settings().viewport.renderingEngine === option.id}>
                  <svg class="w-3 h-3 ml-auto" fill="currentColor" viewBox="0 0 20 20" stroke-width="2">
                    <path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" />
                  </svg>
                </Show>
              </button>
            )}
          </For>
        </div>
      </Show>

      <Show when={showProjectManager()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div class="bg-slate-800 p-6 rounded-xl">
            <h2 class="text-white mb-4">Project Manager</h2>
            <p class="text-gray-300 mb-4">Project manager coming soon...</p>
            <button 
              onClick={() => setShowProjectManager(false)}
              class="px-4 py-2 bg-blue-600 text-white rounded"
            >
              Close
            </button>
          </div>
        </div>
      </Show>

      <UpdateModal 
        show={showUpdateModal()} 
        onClose={() => setShowUpdateModal(false)} 
      />
    </>
  );
}

export default TopBarMenu;