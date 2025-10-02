import TabMenu from '@/layout/TabMenu.jsx';
import Settings from '@/pages/editor/Settings.jsx';
import Scene from '@/pages/editor/Scene.jsx';
import PanelResizer from '@/ui/PanelResizer.jsx';
import PanelToggleButton from '@/ui/PanelToggleButton.jsx';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { objectPropertiesActions } from '@/layout/stores/ViewportStore';
import { propertyTabs, propertiesPanelVisible } from '@/api/plugin';
import { Show, createMemo, createSignal } from 'solid-js';
import { renderStore } from '@/render/store.jsx';
import { IconBox } from '@tabler/icons-solidjs';

const RightPanel = () => {
  const [contextMenu, setContextMenu] = createSignal(null);
  
  // Get reactive store values
  const selection = () => editorStore.selection;
  const ui = () => editorStore.ui;
  const settings = () => editorStore.settings;
  const selectedObject = () => renderStore.selectedObject; // Get actual Babylon.js object
  const selectedObjectId = () => selection().entity; // Keep ID for other uses
  const selectedRightTool = () => ui().selectedTool;
  const rightPanelWidth = () => editorStore.ui.rightPanelWidth;
  const bottomPanelHeight = () => editorStore.ui.bottomPanelHeight;
  
  const isScenePanelOpen = () => {
    return propertiesPanelVisible() && editorStore.panels.isScenePanelOpen;
  };
  
  const panelPosition = () => settings().editor.panelPosition || 'right';
  const isLeftPanel = () => panelPosition() === 'left';

  const {
    selectEntity: setSelectedEntity, setTransformMode,
    setSelectedTool: setSelectedRightTool,
    setScenePanelOpen
  } = editorActions;

  // Panel resize functionality
  const [isResizingRight, setIsResizingRight] = createSignal(false);
  const [rightDragOffset, setRightDragOffset] = createSignal(0);
  const [isResizingTabs, setIsResizingTabs] = createSignal(false);
  const [tabContainerHeight, setTabContainerHeight] = createSignal(680);
  
  const handleRightResizeStart = (e) => {
    setIsResizingRight(true);
    // The actual panel left edge (where content starts, not including toolbar)
    const currentPanelLeft = window.innerWidth - rightPanelWidth();
    const offset = e?.clientX ? e.clientX - currentPanelLeft : 0;
    setRightDragOffset(offset);
  };
  
  const handleRightResizeEnd = () => {
    setIsResizingRight(false);
  };
  
  const handleRightResizeMove = (e) => {
    if (!isResizingRight()) return;
    
    const minPanelWidth = 250;
    const maxPanelWidth = 800;
    
    let newWidth;
    if (isLeftPanel()) {
      newWidth = e.clientX - rightDragOffset();
    } else {
      // Apply the drag offset so panel edge follows mouse cursor (same logic as bottom panel)
      newWidth = window.innerWidth - (e.clientX - rightDragOffset());
      
      // If the calculated width would be less than minimum (cursor too far right)
      // Just set to minimum width
      if (newWidth < minPanelWidth) {
        newWidth = minPanelWidth;
      }
      
      // If cursor is beyond window bounds, also set to minimum
      if (e.clientX >= window.innerWidth) {
        newWidth = minPanelWidth;
      }
    }
    
    const clampedWidth = Math.max(minPanelWidth, Math.min(newWidth, maxPanelWidth, window.innerWidth));
    editorActions.setRightPanelWidth(clampedWidth);
    
    // Resize Babylon engine to match new viewport size
    if (renderStore.engine) {
      renderStore.engine.resize();
    }
  };

  const handleObjectSelect = (objectId) => {
    setSelectedEntity(objectId);
    if (objectId) {
      // Ensure default components are set up for the selected object
      objectPropertiesActions.ensureDefaultComponents(objectId);
      
      if (objectId !== 'scene-root') {
        setTransformMode('move');
      }
      // Switch to scripts tab when an object is selected
      setSelectedRightTool('scripts');
    }
  };

  const handleTabResizeStart = (e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsResizingTabs(true);
    const startY = e.clientY;
    const startHeight = tabContainerHeight();
    
    const handleMouseMove = (e) => {
      e.preventDefault();
      e.stopPropagation();
      const deltaY = startY - e.clientY;
      
      // Calculate available space dynamically
      const rightPanelElement = document.querySelector(`[style*="width: ${rightPanelWidth()}px"]`);
      const sceneElement = rightPanelElement?.querySelector('.flex-1.min-h-0');
      
      let maxHeight = window.innerHeight - 200; // Default fallback
      
      if (rightPanelElement && sceneElement) {
        const panelRect = rightPanelElement.getBoundingClientRect();
        const sceneRect = sceneElement.getBoundingClientRect();
        
        // Reserve minimum 150px for Scene.jsx (header + some content + bottom panel)
        const minSceneHeight = 150;
        const availableSpace = panelRect.height - minSceneHeight - 24; // 24px for resize bar + padding
        maxHeight = Math.max(200, availableSpace);
      }
      
      const newHeight = Math.max(200, Math.min(maxHeight, startHeight + deltaY));
      setTabContainerHeight(newHeight);
    };
    
    const handleMouseUp = (e) => {
      e.preventDefault();
      e.stopPropagation();
      setIsResizingTabs(false);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
    
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  const handleContextMenu = (e, item, context = 'scene') => {
    if (!e) return;
    
    e.preventDefault();
    e.stopPropagation();

    const { clientX: x, clientY: y } = e;
    setContextMenu({
      position: { x, y },
      items: [
        { label: 'Create Object', action: () => console.log('Create object') },
        { label: 'Delete', action: () => console.log('Delete') }
      ],
    });
  };

  const handleRightPanelToggle = () => {
    const currentState = isScenePanelOpen();
    setScenePanelOpen(!currentState);
    
    if (!currentState && (!selectedRightTool() || selectedRightTool() === 'select')) {
      setSelectedRightTool('scene');
    }
  };
  
  const getTabTitle = createMemo(() => {
    const pluginTab = propertyTabs().get(selectedRightTool());
    if (pluginTab) {
      return pluginTab.title;
    }
    
    switch (selectedRightTool()) {
      case 'settings': return 'Settings';
      default: return 'Properties';
    }
  });

  const renderTabContent = () => {
    const pluginTab = propertyTabs().get(selectedRightTool());
    if (pluginTab && pluginTab.component) {
      const PluginComponent = pluginTab.component;
      return <PluginComponent 
        selectedObject={selectedObject()}
        onObjectSelect={handleObjectSelect}
        onContextMenu={handleContextMenu}
      />;
    }
    
    switch (selectedRightTool()) {
      case 'settings':
        return <Settings />;
      
      default:
        return (
          <div class="h-full flex flex-col items-center justify-center text-center text-base-content/60 p-4">
            <IconBox class="w-8 h-8 mb-2 opacity-40" />
            <p class="text-xs">No properties panel available</p>
          </div>
        );
    }
  };

  return (
    <Show when={propertiesPanelVisible()}>
      <div 
        className={`absolute top-0 right-0 pointer-events-auto no-select z-20`}
        style={{ 
          height: 'calc(100% - 24px)', // Subtract footer height
          width: `${rightPanelWidth()}px`,
          maxWidth: '100vw'
        }}
      >
        <Show when={isScenePanelOpen()}>
          <PanelResizer
            type="right"
            isResizing={isResizingRight}
            onResizeStart={handleRightResizeStart}
            onResizeEnd={handleRightResizeEnd}
            onResize={handleRightResizeMove}
            isLeftPanel={isLeftPanel()}
            position={{
              left: '-8px',
              top: 0,
              bottom: `${bottomPanelHeight() + 24}px`,
              width: '8px',
              zIndex: 30
            }}
            className="!bg-transparent !opacity-0 hover:!bg-transparent hover:!opacity-0"
          />
        </Show>
        
        <Show when={isScenePanelOpen()}>
          <div className="absolute inset-0 flex overflow-hidden">
            <div className="flex-1 min-w-0 overflow-hidden">
              <div className="flex flex-col h-full">
                {/* Close button - positioned inside panel */}
                <div className="absolute top-2 right-2 z-10">
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRightPanelToggle();
                    }}
                    className="w-6 h-6 text-base-content/60 hover:text-primary transition-colors flex items-center justify-center group relative"
                    style={{ 
                      'background-color': 'oklch(var(--b2))',
                      'border-left': '1px solid oklch(var(--b3))',
                      'border-top': '1px solid oklch(var(--b3))',
                      'border-bottom': '1px solid oklch(var(--b3))',
                      'border-top-left-radius': '6px',
                      'border-bottom-left-radius': '6px'
                    }}
                    title="Close panel"
                  >
                    <div className="w-3 h-3 flex items-center justify-center">
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" className="w-3 h-3">
                        <path d="m9 18 6-6-6-6"/>
                      </svg>
                    </div>
                    
                    <div className="absolute right-full mr-1 top-1/2 -translate-y-1/2 bg-base-300 backdrop-blur-sm border border-base-300 text-base-content text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl" 
                         style={{ 'z-index': 50 }}>
                      Close panel
                      <div className="absolute left-full top-1/2 -translate-y-1/2 w-0 h-0 border-l-4 border-l-base-300 border-t-4 border-t-transparent border-b-4 border-b-transparent"></div>
                    </div>
                  </button>
                </div>
                
                {/* Panel content */}
                <div className="h-full bg-base-300 border-l border-base-300 shadow-lg overflow-hidden flex">
                  {/* Scene panel - always at top */}
                  <div className="flex-1 flex flex-col">
                    <div className="flex-1 min-h-0 overflow-y-auto" style="scrollbar-width: thin;">
                      <Scene 
                        selectedObject={selectedObjectId()}
                        onObjectSelect={handleObjectSelect}
                        onContextMenu={handleContextMenu}
                        isResizing={isResizingTabs()}
                      />
                    </div>
                    
                    {/* Resize bar for tab container */}
                    <div
                      className={`h-2 cursor-row-resize transition-colors border-b-base-300 border-base-content/20 ${isResizingTabs() ? 'bg-primary/75' : 'bg-base-content/15 hover:bg-primary/75'}`}
                      onMouseDown={handleTabResizeStart}
                    />
                    
                    {/* Tab system for properties */}
                    <div className="flex bg-base-200" style={{ height: `${tabContainerHeight()}px` }}>
                      {/* Vertical toolbar inside right panel */}
                      <div className="w-auto flex-shrink-0 h-full">
                        <TabMenu 
                          selectedTool={selectedRightTool()}
                          onToolSelect={setSelectedRightTool}
                          scenePanelOpen={isScenePanelOpen()}
                          onScenePanelToggle={handleRightPanelToggle}
                          isLeftPanel={isLeftPanel()}
                          panelResize={{
                            handleRightResizeStart,
                            handleRightResizeMove,
                            handleRightResizeEnd
                          }}
                        />
                      </div>
                      
                      {/* Tab content */}
                      <div className="flex-1 min-w-0 overflow-y-auto scrollbar-thin">
                        {renderTabContent()}
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </Show>
        
        <Show when={!isScenePanelOpen()}>
          <PanelToggleButton
            onClick={() => setScenePanelOpen(true)}
            position={isLeftPanel() ? { left: 0 } : { right: 0 }}
            isLeftPanel={isLeftPanel()}
          />
        </Show>
      </div>
    </Show>
  );
};

export default RightPanel;