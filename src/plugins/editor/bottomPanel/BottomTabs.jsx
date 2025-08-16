import { createSignal, createEffect, createMemo, onMount, onCleanup, Show, For } from 'solid-js';
import { IconBox, IconMenu2, IconChevronDown, IconChevronUp, IconGripVertical } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '../stores/EditorStore';
import { viewportStore } from '../stores/ViewportStore';
import { bottomPanelTabs } from '@/plugins/core/engine';

const defaultTabs = [];

const workflowTabs = {
  '3d-viewport': [],
  'default': []
};

function BottomTabs({ activeTab, onTabChange, isAssetPanelOpen, onToggleAssetPanel, rightPanelWidth, isScenePanelOpen, panelResize }) {
  // Use store directly for bottom panel state
  const bottomPanelOpen = () => editorStore.panels.isAssetPanelOpen;
  const selectedBottomTab = () => editorStore.ui.selectedBottomTab;
  // Use the store directly for reactivity instead of the prop
  const currentActiveTab = () => editorStore.ui.selectedBottomTab;
  const bottomTabOrder = () => editorStore.ui.bottomTabOrder;
  const viewport = () => viewportStore;
  const settings = () => editorStore.settings;
  const { setBottomTabOrder } = editorActions;
  const panelPosition = () => settings().editor.panelPosition || 'right';
  const isLeftPanel = () => panelPosition() === 'left';
  
  const getCurrentWorkflow = () => {
    const tabs = viewport().tabs;
    if (!tabs || tabs.length === 0) {
      return 'default';
    }
    const activeTabData = tabs.find(tab => tab.id === viewport().activeTabId);
    return activeTabData?.type || 'default';
  };
  
  const getOrderedTabs = () => {
    const currentWorkflow = getCurrentWorkflow();
    const allowedTabIds = workflowTabs[currentWorkflow] || workflowTabs['default'];
    
    // Combine default tabs with plugin bottom panel tabs
    const pluginTabs = Array.from(bottomPanelTabs().values())
      .sort((a, b) => (a.order || 0) - (b.order || 0))
      .map(tab => ({
        id: tab.id,
        label: tab.title,
        icon: tab.icon
      }));
    
    const allTabs = [...defaultTabs, ...pluginTabs];
    
    const tabsMap = allTabs.reduce((map, tab) => {
      map[tab.id] = tab;
      return map;
    }, {});
    
    let currentTabOrder = bottomTabOrder() || [];
    const missingTabs = allTabs
      .filter(tab => !currentTabOrder.includes(tab.id))
      .map(tab => tab.id);
    
    if (missingTabs.length > 0) {
      currentTabOrder = [...currentTabOrder, ...missingTabs];
      setBottomTabOrder(currentTabOrder);
    }
    
    const workflowFilteredTabs = currentTabOrder
      .filter(id => allowedTabIds.includes(id) || bottomPanelTabs().has(id))
      .map(id => tabsMap[id])
      .filter(Boolean);
    
    return workflowFilteredTabs;
  };
  
  const [allTabs, setAllTabs] = createSignal(getOrderedTabs());
  const [visibleTabs, setVisibleTabs] = createSignal(getOrderedTabs());
  
  // Update tabs when plugins register new bottom panel tabs
  createEffect(() => {
    bottomPanelTabs(); // Subscribe to changes
    const newTabs = getOrderedTabs();
    setAllTabs(newTabs);
    setVisibleTabs(newTabs);
  });
  const [overflowTabs, setOverflowTabs] = createSignal([]);
  const [showDropdown, setShowDropdown] = createSignal(false);
  const [dropdownPosition, setDropdownPosition] = createSignal({ x: 0, y: 0 });
  const [dragState, setDragState] = createSignal({
    isDragging: false,
    draggedTab: null,
    dragOverTab: null,
    dragStartX: 0,
    dragOffsetX: 0
  });
  const [dragOverOverflowButton, setDragOverOverflowButton] = createSignal(false);
  
  let containerRef;
  let tabsRef;
  let overflowButtonRef;
  let dropdownOpenTimeoutRef = null;

  createEffect(() => {
    const orderedTabs = getOrderedTabs();
    setAllTabs(orderedTabs);
  });

  createEffect(() => {
    const calculateVisibleTabs = () => {
      if (!containerRef || !tabsRef) return;
      
      const containerWidth = containerRef.offsetWidth;
      const toggleButtonWidth = 40;
      const overflowButtonWidth = 40;
      const actualAvailableWidth = containerWidth - toggleButtonWidth;
      let currentWidth = 0;
      let visibleCount = 0;
      const tabs = allTabs();
      
      for (let i = 0; i < tabs.length; i++) {
        const tabWidth = tabs[i].label.length * 7 + 50;
        if (currentWidth + tabWidth <= actualAvailableWidth) {
          currentWidth += tabWidth;
          visibleCount++;
        } else {
          break;
        }
      }
      
      if (visibleCount < tabs.length) {
        currentWidth = 0;
        visibleCount = 0;
        const availableWidthWithOverflow = actualAvailableWidth - overflowButtonWidth;
        
        for (let i = 0; i < tabs.length; i++) {
          const tabWidth = tabs[i].label.length * 7 + 50;
          if (currentWidth + tabWidth <= availableWidthWithOverflow) {
            currentWidth += tabWidth;
            visibleCount++;
          } else {
            break;
          }
        }
        
        setVisibleTabs(tabs.slice(0, Math.max(1, visibleCount)));
        setOverflowTabs(tabs.slice(Math.max(1, visibleCount)));
      } else {
        setVisibleTabs(tabs);
        setOverflowTabs([]);
      }
    };

    calculateVisibleTabs();
    window.addEventListener('resize', calculateVisibleTabs);
    
    onCleanup(() => {
      window.removeEventListener('resize', calculateVisibleTabs);
    });
  });

  createEffect(() => {
    const updateDropdownPosition = () => {
      if (showDropdown() && overflowButtonRef) {
        const rect = overflowButtonRef.getBoundingClientRect();
        setDropdownPosition({
          x: isLeftPanel() ? rect.left : rect.right,
          y: rect.top - 8
        });
      }
    };

    const handleClickOutside = (event) => {
      if (overflowButtonRef && !overflowButtonRef.contains(event.target)) {
        const dropdownElement = document.querySelector('[data-dropdown="true"]');
        if (!dropdownElement || !dropdownElement.contains(event.target)) {
          setShowDropdown(false);
        }
      }
    };

    if (showDropdown()) {
      window.addEventListener('resize', updateDropdownPosition);
      window.addEventListener('scroll', updateDropdownPosition);
      document.addEventListener('mousedown', handleClickOutside);
      
      onCleanup(() => {
        window.removeEventListener('resize', updateDropdownPosition);
        window.removeEventListener('scroll', updateDropdownPosition);
        document.removeEventListener('mousedown', handleClickOutside);
      });
    }
  });

  const handleTabClick = (tabId) => {
    if (!dragState().isDragging) {
      onTabChange(tabId);
      setShowDropdown(false);
    }
  };

  const toggleDropdown = () => {
    if (!showDropdown() && overflowButtonRef) {
      const rect = overflowButtonRef.getBoundingClientRect();
      setDropdownPosition({
        x: isLeftPanel() ? rect.left : rect.right,
        y: rect.top - 8
      });
    }
    setShowDropdown(!showDropdown());
  };

  const handleDragStart = (e, tab) => {
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/html', '');
    
    const rect = e.currentTarget.getBoundingClientRect();
    const offsetX = e.clientX - rect.left;
    
    setDragState({
      isDragging: true,
      draggedTab: tab,
      dragOverTab: null,
      dragStartX: e.clientX,
      dragOffsetX: offsetX
    });
  };

  const handleDragOver = (e, tab) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    
    const state = dragState();
    if (state.draggedTab && state.draggedTab.id !== tab.id) {
      setDragState(prev => ({ ...prev, dragOverTab: tab }));
    }
  };

  const handleDragLeave = (e) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const { clientX, clientY } = e;
    
    if (clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) {
      setDragState(prev => ({ ...prev, dragOverTab: null }));
    }
  };

  const handleDrop = (e, dropTab) => {
    e.preventDefault();
    e.stopPropagation();
    
    const state = dragState();
    if (!state.draggedTab || state.draggedTab.id === dropTab.id) {
      setDragState({
        isDragging: false,
        draggedTab: null,
        dragOverTab: null,
        dragStartX: 0,
        dragOffsetX: 0
      });
      return;
    }

    const tabs = allTabs();
    const draggedIndex = tabs.findIndex(tab => tab.id === state.draggedTab.id);
    const dropIndex = tabs.findIndex(tab => tab.id === dropTab.id);
    
    if (draggedIndex !== -1 && dropIndex !== -1 && draggedIndex !== dropIndex) {
      const newTabs = [...tabs];
      const [removed] = newTabs.splice(draggedIndex, 1);
      newTabs.splice(dropIndex, 0, removed);
      setAllTabs(newTabs);
      const newOrder = newTabs.map(tab => tab.id);
      setBottomTabOrder(newOrder);
    }

    setDragState({
      isDragging: false,
      draggedTab: null,
      dragOverTab: null,
      dragStartX: 0,
      dragOffsetX: 0
    });
  };

  const handleDragEnd = () => {
    setDragState({
      isDragging: false,
      draggedTab: null,
      dragOverTab: null,
      dragStartX: 0,
      dragOffsetX: 0
    });
    setDragOverOverflowButton(false);
    if (dropdownOpenTimeoutRef) {
      clearTimeout(dropdownOpenTimeoutRef);
      dropdownOpenTimeoutRef = null;
    }
  };

  onCleanup(() => {
    if (dropdownOpenTimeoutRef) {
      clearTimeout(dropdownOpenTimeoutRef);
    }
  });

  const currentWorkflow = getCurrentWorkflow();

  return (
    <div ref={containerRef} class="h-10 bg-slate-900 border-t border-slate-700 border-b border-slate-700 flex items-center relative z-50">
      <div ref={tabsRef} class="flex flex-1 overflow-hidden">
        <For each={visibleTabs()}>
          {(tab) => {
            const isDragged = () => dragState().draggedTab?.id === tab.id;
            const isDragOver = () => dragState().dragOverTab?.id === tab.id;
            const isActive = () => currentActiveTab() === tab.id;
            
            return (
              <button
                draggable
                onClick={() => handleTabClick(tab.id)}
                onDragStart={(e) => handleDragStart(e, tab)}
                onDragOver={(e) => handleDragOver(e, tab)}
                onDragLeave={handleDragLeave}
                onDrop={(e) => handleDrop(e, tab)}
                onDragEnd={handleDragEnd}
                classList={{
                  'relative flex items-center px-4 py-2.5 text-sm font-medium transition-all whitespace-nowrap select-none': true,
                  'opacity-50 cursor-grabbing': isDragged(),
                  'hover:bg-slate-800 cursor-grab': !isDragged(),
                  'text-blue-400': isActive(),
                  'text-gray-400 hover:text-gray-200': !isActive()
                }}
                style={{
                  transform: isDragged() ? 'scale(0.95)' : 'scale(1)',
                }}
              >
                <tab.icon class="w-4 h-4 mr-2" />
                {tab.label}
                
                <Show when={isActive()}>
                  <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500"></div>
                </Show>
                
                <Show when={isDragOver()}>
                  <div class="absolute inset-y-0 left-0 w-0.5 bg-blue-500 z-10"></div>
                </Show>
              </button>
            );
          }}
        </For>
        
        {/* Empty space that can be dragged to resize */}
        <div 
          class="flex-1 cursor-row-resize"
          onMouseDown={(e) => {
            if (!panelResize) return;
            e.preventDefault();
            panelResize.handleBottomResizeStart();
            
            const handleMouseMove = (e) => {
              e.preventDefault();
              panelResize.handleBottomResizeMove(e, { isAssetPanelOpen });
            };

            const handleMouseUp = (e) => {
              e.preventDefault();
              panelResize.handleBottomResizeEnd();
              document.removeEventListener('mousemove', handleMouseMove);
              document.removeEventListener('mouseup', handleMouseUp);
            };

            document.addEventListener('mousemove', handleMouseMove);
            document.addEventListener('mouseup', handleMouseUp);
          }}
        />
        
        <Show when={overflowTabs().length > 0}>
          <div class="relative">
            <button
              ref={overflowButtonRef}
              onClick={toggleDropdown}
              onDragEnter={(e) => {
                e.preventDefault();
                if (dragState().draggedTab) {
                  setDragOverOverflowButton(true);
                  if (dropdownOpenTimeoutRef) {
                    clearTimeout(dropdownOpenTimeoutRef);
                  }
                  dropdownOpenTimeoutRef = setTimeout(() => {
                    if (!showDropdown()) {
                      const rect = overflowButtonRef?.getBoundingClientRect();
                      if (rect) {
                        setDropdownPosition({
                          x: isLeftPanel() ? rect.left : rect.right,
                          y: rect.top - 8
                        });
                        setShowDropdown(true);
                      }
                    }
                  }, 500);
                }
              }}
              onDragLeave={(e) => {
                const rect = e.currentTarget.getBoundingClientRect();
                const { clientX, clientY } = e;
                
                if (clientX < rect.left || clientX > rect.right || clientY < rect.top || clientY > rect.bottom) {
                  setDragOverOverflowButton(false);
                  if (dropdownOpenTimeoutRef) {
                    clearTimeout(dropdownOpenTimeoutRef);
                    dropdownOpenTimeoutRef = null;
                  }
                }
              }}
              classList={{
                'relative flex items-center px-3 py-2.5 text-sm font-medium transition-colors': true,
                'bg-blue-600/20 border border-blue-500': dragOverOverflowButton(),
                'hover:bg-slate-800': !dragOverOverflowButton(),
                'text-blue-400': overflowTabs().some(tab => tab.id === currentActiveTab()),
                'text-gray-400 hover:text-gray-200': !overflowTabs().some(tab => tab.id === currentActiveTab())
              }}
            >
              <IconMenu2 class="w-4 h-4" />
              
              <Show when={overflowTabs().some(tab => tab.id === currentActiveTab())}>
                <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500"></div>
              </Show>
            </button>
          </div>
        </Show>
      </div>
      
      <div class="flex items-center pr-1">
        {/* Drag handle button */}
        <Show when={panelResize}>
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              panelResize.handleBottomResizeStart();
              
              const handleMouseMove = (e) => {
                e.preventDefault();
                panelResize.handleBottomResizeMove(e, { isAssetPanelOpen });
              };

              const handleMouseUp = (e) => {
                e.preventDefault();
                panelResize.handleBottomResizeEnd();
                document.removeEventListener('mousemove', handleMouseMove);
                document.removeEventListener('mouseup', handleMouseUp);
              };

              document.addEventListener('mousemove', handleMouseMove);
              document.addEventListener('mouseup', handleMouseUp);
            }}
            class="p-1.5 hover:bg-slate-800 rounded transition-colors text-gray-400 hover:text-white cursor-row-resize mr-1"
            title="Drag to resize panel"
          >
            <IconGripVertical class="w-4 h-4" />
          </button>
        </Show>
        
        {/* Toggle button */}
        <button 
          onClick={() => {
            const currentState = bottomPanelOpen();
            onToggleAssetPanel(!currentState);
          }}
          class="p-1.5 hover:bg-slate-800 rounded transition-colors text-gray-400 hover:text-white"
          title={bottomPanelOpen() ? 'Hide panel' : 'Show panel'}
        >
          {bottomPanelOpen() ? (
            <IconChevronDown class="w-4 h-4" />
          ) : (
            <IconChevronUp class="w-4 h-4" />
          )}
        </button>
      </div>
      
      <Show when={showDropdown() && overflowTabs().length > 0}>
        <div 
          class="fixed bg-slate-800 border border-slate-700 rounded-lg shadow-2xl shadow-black/50 min-w-48 pointer-events-auto"
          data-dropdown="true"
          style={{
            left: `${dropdownPosition().x}px`,
            top: `${dropdownPosition().y}px`,
            transform: isLeftPanel() ? 'translate(0%, -100%)' : 'translate(-100%, -100%)',
            'z-index': 9999
          }}
        >
          <For each={overflowTabs()}>
            {(tab) => {
              const isDragged = () => dragState().draggedTab?.id === tab.id;
              const isDragOver = () => dragState().dragOverTab?.id === tab.id;
              const isActive = () => currentActiveTab() === tab.id;
              
              return (
                <button
                  draggable
                  onClick={() => handleTabClick(tab.id)}
                  onDragStart={(e) => handleDragStart(e, tab)}
                  onDragOver={(e) => handleDragOver(e, tab)}
                  onDragLeave={handleDragLeave}
                  onDrop={(e) => handleDrop(e, tab)}
                  onDragEnd={handleDragEnd}
                  classList={{
                    'w-full flex items-center px-3 py-2 text-sm font-medium transition-all first:rounded-t-lg last:rounded-b-lg select-none': true,
                    'opacity-50 cursor-grabbing': isDragged(),
                    'hover:bg-slate-700 cursor-grab': !isDragged(),
                    'text-blue-400 bg-slate-700/50': isActive(),
                    'text-gray-300 hover:text-gray-200': !isActive()
                  }}
                  style={{
                    transform: isDragged() ? 'scale(0.95)' : 'scale(1)',
                  }}
                >
                  <tab.icon class="w-4 h-4 mr-2" />
                  {tab.label}
                  
                  <Show when={isDragOver()}>
                    <div class="absolute inset-y-0 left-0 w-0.5 bg-blue-500 z-10"></div>
                  </Show>
                </button>
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
}

export default BottomTabs;