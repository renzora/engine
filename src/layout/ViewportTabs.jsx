import { createSignal, createEffect, createMemo, For, Show } from 'solid-js';
import { Settings, FileText, X, Star, Copy, Play, Pause, Plus, Grid3x3 } from '@/ui/icons';
import { editorStore, editorActions } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { viewportTypes } from "@/api/plugin";

const ViewportTabs = () => {
  const [isAddDropdownOpen, setIsAddDropdownOpen] = createSignal(false);
  const [dropdownPosition, setDropdownPosition] = createSignal({ x: 0, y: 0 });
  const [contextMenu, setContextMenu] = createSignal(null);
  const [editingTab, setEditingTab] = createSignal(null);
  const [editingName, setEditingName] = createSignal('');
  
  // Access store properties reactively
  const tabs = () => viewportStore.tabs;
  const activeTabId = () => viewportStore.activeTabId;
  const suspendedTabs = () => viewportStore.suspendedTabs;

  // Combine built-in and plugin viewport types
  const availableViewportTypes = createMemo(() => {
    const SceneIcon = () => <div class="text-lg">🎬</div>;
    
    const builtInTypes = [
      {
        id: '3d-viewport',
        label: 'New Scene',
        icon: SceneIcon,
        description: 'Create a new 3D scene viewport'
      },
      {
        id: 'node-editor',
        label: 'Node Editor',
        icon: Settings,
        description: 'Create a new node editor for visual scripting'
      }
    ];
    
    const pluginTypes = Array.from(viewportTypes().values());
    return [...builtInTypes, ...pluginTypes];
  });

  const getViewportIcon = (type) => {
    try {
      const viewportType = availableViewportTypes().find(v => v.id === type);
      if (viewportType && viewportType.icon && typeof viewportType.icon === 'function') {
        return viewportType.icon;
      }
      return FileText;
    } catch (error) {
      console.error('Error in getViewportIcon:', error, 'type:', type);
      return FileText;
    }
  };

  const handleAddViewport = (type) => {
    const newTabId = `viewport-${Date.now()}`;
    const viewportType = availableViewportTypes().find(v => v.id === type);
    const newTab = {
      id: newTabId,
      type: type,
      name: viewportType ? viewportType.label : 'New Viewport',
      isPinned: false,
      hasUnsavedChanges: false
    };
    console.log('🎯 Adding new viewport tab:', newTab);
    viewportActions.addViewportTab(newTab);
    viewportActions.setActiveViewportTab(newTabId);
    console.log('🎯 Active tab set to:', newTabId);
    setIsAddDropdownOpen(false);
  };

  const handleTabClick = (tabId) => {
    console.log('🎯 Tab clicked:', tabId);
    console.log('🎯 Current active tab before:', viewportStore.activeTabId);
    viewportActions.setActiveViewportTab(tabId);
    console.log('🎯 Current active tab after:', viewportStore.activeTabId);
  };

  const handleTabClose = (e, tabId) => {
    e.stopPropagation();
    viewportActions.removeViewportTab(tabId);
  };

  const handleStartRename = (tab) => {
    setEditingTab(tab.id);
    setEditingName(tab.name);
    setContextMenu(null);
  };

  const handleFinishRename = () => {
    if (editingTab() && editingName().trim()) {
      viewportActions.renameViewportTab(editingTab(), editingName().trim());
    }
    setEditingTab(null);
    setEditingName('');
  };

  const handleCancelRename = () => {
    setEditingTab(null);
    setEditingName('');
  };

  const handleRenameKeyDown = (e) => {
    if (e.key === 'Enter') {
      handleFinishRename();
    } else if (e.key === 'Escape') {
      handleCancelRename();
    }
  };

  const handleTabContextMenu = (e, tab) => {
    e.preventDefault();
    e.stopPropagation();
    
    setContextMenu({
      position: { x: e.clientX, y: e.clientY },
      tab: tab,
      items: [
        {
          label: 'Rename Tab',
          icon: FileText,
          action: () => handleStartRename(tab)
        },
        {
          label: tab.isPinned ? 'Unpin Tab' : 'Pin Tab',
          icon: tab.isPinned ? X : Star,
          action: () => viewportActions.pinViewportTab(tab.id)
        },
        {
          label: 'Duplicate Tab',
          icon: Copy,
          action: () => viewportActions.duplicateViewportTab(tab.id)
        },
        {
          label: (suspendedTabs() || []).includes(tab.id) ? 'Resume Tab' : 'Suspend Tab',
          icon: (suspendedTabs() || []).includes(tab.id) ? Play : Pause,
          action: () => {
            // TODO: Implement suspend/resume functionality
            console.log('Suspend/Resume not yet implemented');
          },
          disabled: tab.id === activeTabId()
        },
        { divider: true },
        {
          label: `New ${tab.type === '3d-viewport' ? 'Scene' : tab.type.replace('-', ' ').replace(/\b\w/g, l => l.toUpperCase())}`,
          iconComponent: getViewportIcon(tab.type),
          action: () => handleAddViewport(tab.type)
        },
        { divider: true },
        {
          label: 'Close Tab',
          icon: X,
          action: () => viewportActions.removeViewportTab(tab.id),
          disabled: tabs().length === 1
        },
        {
          label: 'Close Other Tabs',
          icon: X,
          action: () => {
            tabs().forEach(t => {
              if (t.id !== tab.id && !t.isPinned) {
                viewportActions.removeViewportTab(t.id);
              }
            });
          },
          disabled: tabs().length === 1
        },
        {
          label: 'Close All Tabs',
          icon: X,
          action: () => {
            tabs().forEach(t => {
              if (!t.isPinned) {
                viewportActions.removeViewportTab(t.id);
              }
            });
          },
          disabled: tabs().length === 1 || tabs.every(t => t.isPinned)
        }
      ]
    });
  };

  const handleMiddleClick = (e, tabId) => {
    if (e.button === 1) {
      e.preventDefault();
      viewportActions.removeViewportTab(tabId);
    }
  };

  return (
    <>
      <div className="flex items-center h-8 bg-gray-900/95 border-b border-gray-800">
        <div className="flex items-center min-w-0 flex-1 overflow-x-auto">
          <For each={tabs()}>
            {(tab) => {
              const Icon = getViewportIcon(tab.type);
              const isActive = () => tab.id === activeTabId();
              
              return (
                <div
                  classList={{
                    'group flex items-center gap-2 px-3 py-1 border-r border-gray-700 cursor-pointer transition-all select-none min-w-0 max-w-48 flex-shrink-0': true,
                    'bg-blue-600/20 border-b-2 border-b-blue-500 text-blue-300': isActive(),
                    'text-gray-400 hover:text-gray-200 hover:bg-gray-800': !isActive()
                  }}
                  onClick={() => handleTabClick(tab.id)}
                  onContextMenu={(e) => handleTabContextMenu(e, tab)}
                  onMouseDown={(e) => handleMiddleClick(e, tab.id)}
                  title={tab.name}
                >
                  <Icon className="w-4 h-4 flex-shrink-0" />
                  
                  <Show 
                    when={editingTab() === tab.id}
                    fallback={
                      <span className="text-sm font-medium truncate min-w-0">
                        {tab.name}
                      </span>
                    }
                  >
                    <input
                      type="text"
                      value={editingName()}
                      onChange={(e) => setEditingName(e.target.value)}
                      onBlur={handleFinishRename}
                      onKeyDown={handleRenameKeyDown}
                      className="text-sm font-medium bg-gray-700 border border-gray-500 rounded px-1 py-0 min-w-0 max-w-32"
                      autofocus
                      onClick={(e) => e.stopPropagation()}
                    />
                  </Show>
                  
                  <Show when={(suspendedTabs() || []).includes(tab.id)}>
                    <Pause className="w-3 h-3 text-gray-500 flex-shrink-0" title="Tab Suspended" />
                  </Show>
                  
                  <Show when={tab.isPinned}>
                    <Star className="w-3 h-3 text-yellow-500 flex-shrink-0" />
                  </Show>
                  
                  <Show when={tab.hasUnsavedChanges}>
                    <div className="w-2 h-2 bg-orange-500 rounded-full flex-shrink-0" />
                  </Show>
                  
                  <Show when={tabs().length > 1}>
                    <button
                      onClick={(e) => handleTabClose(e, tab.id)}
                      className="w-4 h-4 flex items-center justify-center rounded hover:bg-gray-600 transition-colors opacity-0 group-hover:opacity-100 flex-shrink-0"
                      title="Close Tab"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </Show>
                </div>
              );
            }}
          </For>

          <div className="relative flex-shrink-0">
            <button
              onClick={(e) => {
                const rect = e.currentTarget.getBoundingClientRect();
                setDropdownPosition({
                  x: Math.min(rect.left, window.innerWidth - 280),
                  y: rect.bottom + 4
                });
                setIsAddDropdownOpen(!isAddDropdownOpen());
              }}
              className="flex items-center px-3 py-1 text-gray-400 hover:text-gray-200 hover:bg-gray-800 transition-colors border-r border-gray-700"
              title="Add Viewport"
            >
              <Plus className="w-4 h-4" />
            </button>

            <Show when={isAddDropdownOpen()}>
              <>
                <div 
                  className="fixed inset-0 z-40"
                  onClick={() => setIsAddDropdownOpen(false)}
                />
                
                <div 
                  className="fixed w-64 bg-gray-900/98 backdrop-blur-sm border border-gray-700/50 rounded-lg shadow-xl z-50"
                  style={{
                    left: dropdownPosition().x + 'px',
                    top: dropdownPosition().y + 'px'
                  }}
                >
                  <div className="p-2">
                    <div className="text-xs text-gray-500 uppercase tracking-wide px-2 py-1 mb-1">
                      Add Viewport
                    </div>
                    <For each={availableViewportTypes()}>
                      {(viewportType) => (
                        <button
                          onClick={() => handleAddViewport(viewportType.id)}
                          className="w-full flex items-start px-3 py-2 text-sm text-gray-300 hover:bg-gray-700/50 hover:text-white rounded-md transition-colors group"
                        >
                          <div className="w-4 h-4 mr-3 mt-0.5 text-gray-400 group-hover:text-white flex-shrink-0">
                            <viewportType.icon className="w-4 h-4" />
                          </div>
                          <div className="text-left min-w-0">
                            <div className="font-medium">{viewportType.label}</div>
                            <div className="text-xs text-gray-500 group-hover:text-gray-300">
                              {viewportType.description}
                            </div>
                          </div>
                        </button>
                      )}
                    </For>
                  </div>
                </div>
              </>
            </Show>
          </div>
        </div>

        <div className="flex items-center gap-1 px-2 border-l border-gray-700 flex-shrink-0">
          <button
            className="p-1 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded transition-colors"
            title="Split Viewport"
          >
            <Grid3x3 className="w-4 h-4" />
          </button>
          <button
            className="p-1 text-gray-400 hover:text-gray-200 hover:bg-gray-800 rounded transition-colors"
            title="Viewport Settings"
          >
            <Settings className="w-4 h-4" />
          </button>
        </div>
      </div>

      <Show when={contextMenu()}>
        <>
          <div 
            className="fixed inset-0 z-50"
            onClick={() => setContextMenu(null)}
          />
          
          <div
            className="fixed z-60 bg-gray-900/98 backdrop-blur-sm border border-gray-700/50 rounded-lg shadow-xl min-w-48"
            style={{ 
              left: contextMenu().position.x + 'px', 
              top: contextMenu().position.y + 'px'
            }}
          >
            <div className="p-2">
              <For each={contextMenu().items}>
                {(item, index) => (
                  <Show
                    when={item.divider}
                    fallback={
                      <button
                        onClick={() => {
                          if (!item.disabled) {
                            item.action();
                          }
                          setContextMenu(null);
                        }}
                        disabled={item.disabled}
                        className={`w-full flex items-center px-3 py-2 text-sm rounded-md transition-colors ${
                          item.disabled 
                            ? 'text-gray-600 cursor-not-allowed'
                            : 'text-gray-300 hover:bg-gray-700/50 hover:text-white'
                        }`}
                      >
                        <Show when={item.icon || item.iconComponent}>
                          <Show when={item.iconComponent} fallback={
                            <item.icon className={`w-4 h-4 mr-3 ${
                              item.disabled ? 'text-gray-600' : 'text-gray-400'
                            }`} />
                          }>
                            <div className={`w-4 h-4 mr-3 ${
                              item.disabled ? 'text-gray-600' : 'text-gray-400'
                            }`}>
                              <item.iconComponent className="w-4 h-4" />
                            </div>
                          </Show>
                        </Show>
                        {item.label}
                      </button>
                    }
                  >
                    <div className="border-t border-gray-700/50 my-2" />
                  </Show>
                )}
              </For>
            </div>
          </div>
        </>
      </Show>
    </>
  );
};

export default ViewportTabs;