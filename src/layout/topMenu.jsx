import { createSignal, createEffect, onCleanup, createMemo, For, Show } from 'solid-js';
import { ArrowDown, ArrowUp, Refresh, ChevronRight } from '@/ui/icons';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { topMenuItems } from '@/api/plugin';

function TopMenu() {
  const [activeMenu, setActiveMenu] = createSignal(null);
  const [isSaving, setIsSaving] = createSignal(false);
  const [lastSync, setLastSync] = createSignal(null);
  const [hasUnsavedChanges, setHasUnsavedChanges] = createSignal(false);
  const [showSyncTooltip, setShowSyncTooltip] = createSignal(false);
  const [showUpdateTooltip, setShowUpdateTooltip] = createSignal(false);
  const [showProjectManager, setShowProjectManager] = createSignal(false);
  const [menuPosition, setMenuPosition] = createSignal(null);
  const [showRendererDropdown, setShowRendererDropdown] = createSignal(false);
  const [rendererDropdownPosition, setRendererDropdownPosition] = createSignal(null);

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
            <span class="text-gray-400">
              Renzora Engine v1.0.0
            </span>
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
          </div>
          
          <div class="flex items-center gap-2">
            <span class="text-gray-600">•</span>
            <span 
              class="text-orange-400 text-xs font-medium flex items-center gap-1 cursor-pointer relative"
              onMouseEnter={() => setShowUpdateTooltip(true)}
              onMouseLeave={() => setShowUpdateTooltip(false)}
            >
              Renzora Engine r1
              <ArrowDown class="w-3 h-3" />
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
                {(settings().viewport?.renderingEngine || 'webgl').toUpperCase()}
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
        </div>
      </div>
      
      <Show when={activeMenu() && menuPosition()}>
        <div 
          class="dropdown-content fixed w-56 bg-gradient-to-br from-gray-900/98 to-gray-950/98 backdrop-blur-sm rounded-lg shadow-[0_20px_25px_-5px_rgba(0,0,0,0.4)] z-[110] border border-gray-700/50"
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
                    <div class="relative group/item">
                      <button
                        class="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-gradient-to-r hover:from-blue-600/90 hover:to-blue-500/90 hover:text-white flex items-center justify-between transition-all duration-150 relative rounded-md hover:shadow-lg"
                        onClick={() => item.submenu ? null : handleItemClick(item)}
                      >
                        <div class="flex items-center gap-2">
                          <Show when={item.icon}>
                            <span class="w-4 h-4 flex items-center justify-center text-gray-400 group-hover/item:text-white">
                              <Show when={item.id === 'save' && isSaving()} fallback={<item.icon class="w-3.5 h-3.5" />}>
                                <div class="w-3 h-3 border-2 border-gray-400 border-t-transparent rounded-full animate-spin" />
                              </Show>
                            </span>
                          </Show>
                          <span class="font-normal">
                            {item.id === 'save' && isSaving() ? 'Saving...' : item.label}
                          </span>
                        </div>
                        <Show when={item.shortcut}>
                          <span class="ml-auto text-xs text-gray-500 group-hover/item:text-gray-300">{item.shortcut}</span>
                        </Show>
                        <Show when={item.submenu}>
                          <ChevronRight class="w-3 h-3 text-gray-400 group-hover/item:text-white ml-auto" />
                        </Show>
                      </button>
                      
                      {/* Nested submenu - CSS hover activated */}
                      <Show when={item.submenu}>
                        <div class="absolute left-full top-0 -ml-1 w-56 bg-gradient-to-br from-gray-900/98 to-gray-950/98 backdrop-blur-sm rounded-lg shadow-[0_20px_25px_-5px_rgba(0,0,0,0.4)] border border-gray-700/50 opacity-0 invisible group-hover/item:opacity-100 group-hover/item:visible transition-all duration-200 z-[120] before:absolute before:inset-y-0 before:-left-1 before:w-2 before:content-['']">
                          <div class="p-1">
                            <For each={item.submenu}>
                              {(subItem) => (
                                <>
                                  <Show when={subItem.divider}>
                                    <div class="border-t border-gray-700/50 my-1 mx-2" />
                                  </Show>
                                  <Show when={!subItem.divider}>
                                    <div class="relative group/subitem">
                                      <button
                                        class="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-gradient-to-r hover:from-blue-600/90 hover:to-blue-500/90 hover:text-white flex items-center justify-between transition-all duration-150 rounded-md"
                                        onClick={() => subItem.submenu ? null : handleItemClick(subItem)}
                                      >
                                        <div class="flex items-center gap-2">
                                          <Show when={subItem.icon}>
                                            <span class="w-4 h-4 flex items-center justify-center text-gray-400 group-hover/subitem:text-white">
                                              <subItem.icon class="w-3.5 h-3.5" />
                                            </span>
                                          </Show>
                                          <span class="font-normal">{subItem.label}</span>
                                        </div>
                                        <Show when={subItem.shortcut}>
                                          <span class="ml-auto text-xs text-gray-500 group-hover/subitem:text-gray-300">{subItem.shortcut}</span>
                                        </Show>
                                        <Show when={subItem.submenu}>
                                          <ChevronRight class="w-3 h-3 text-gray-400 group-hover/subitem:text-white ml-auto" />
                                        </Show>
                                      </button>
                                      
                                      {/* Third level submenu */}
                                      <Show when={subItem.submenu}>
                                        <div class="absolute left-full top-0 -ml-1 w-56 bg-gradient-to-br from-gray-900/98 to-gray-950/98 backdrop-blur-sm rounded-lg shadow-[0_20px_25px_-5px_rgba(0,0,0,0.4)] border border-gray-700/50 opacity-0 invisible group-hover/subitem:opacity-100 group-hover/subitem:visible transition-all duration-200 z-[130] before:absolute before:inset-y-0 before:-left-1 before:w-2 before:content-['']">
                                          <div class="p-1">
                                            <For each={subItem.submenu}>
                                              {(thirdItem) => (
                                                <button
                                                  class="w-full px-3 py-1.5 text-left text-sm text-gray-300 hover:bg-gradient-to-r hover:from-blue-600/90 hover:to-blue-500/90 hover:text-white flex items-center gap-2 transition-all duration-150 rounded-md"
                                                  onClick={() => handleItemClick(thirdItem)}
                                                >
                                                  <Show when={thirdItem.icon}>
                                                    <span class="w-4 h-4 flex items-center justify-center text-gray-400">
                                                      <thirdItem.icon class="w-3.5 h-3.5" />
                                                    </span>
                                                  </Show>
                                                  <span class="font-normal">{thirdItem.label}</span>
                                                  <Show when={thirdItem.shortcut}>
                                                    <span class="ml-auto text-xs text-gray-500">{thirdItem.shortcut}</span>
                                                  </Show>
                                                </button>
                                              )}
                                            </For>
                                          </div>
                                        </div>
                                      </Show>
                                    </div>
                                  </Show>
                                </>
                              )}
                            </For>
                          </div>
                        </div>
                      </Show>
                    </div>
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
                  settings().viewport?.renderingEngine === option.id
                    ? 'bg-green-600/90 text-white'
                    : 'text-gray-300 hover:bg-gray-900/60 hover:text-white'
                }`}
              >
                {option.label}
                <Show when={settings().viewport?.renderingEngine === option.id}>
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
    </>
  );
}

export default TopMenu;