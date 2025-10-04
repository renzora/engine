import { Show, createEffect, createMemo, Switch, Match, createSignal, onMount, onCleanup } from 'solid-js';
import BottomTabs from './BottomTabs.jsx';
import AssetLibrary from '@/pages/editor/AssetLibrary';
import PanelResizer from '@/ui/PanelResizer.jsx';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { bottomPanelTabs, bottomPanelVisible, propertiesPanelVisible } from '@/api/plugin';
import { useViewportContextMenu } from '@/ui/ViewportContextMenu.jsx';
import { renderStore } from '@/render/store.jsx';

const BottomPanel = () => {
  const { showContextMenu } = useViewportContextMenu();
  const [contextMenu, setContextMenu] = createSignal(null);
  
  // Handle window resize to ensure panel height stays within bounds
  onMount(() => {
    const handleWindowResize = () => {
      const currentHeight = editorStore.ui.bottomPanelHeight;
      const footerHeight = 24;
      const maxHeight = Math.floor((window.innerHeight - footerHeight) * 0.8);
      if (currentHeight > maxHeight) {
        editorActions.setBottomPanelHeight(maxHeight);
      }
    };
    
    window.addEventListener('resize', handleWindowResize);
    onCleanup(() => window.removeEventListener('resize', handleWindowResize));
  });

  // Recalculate viewport when bottom panel visibility or position changes
  createEffect(() => {
    // Watch for changes in panel state and position
    const panelOpen = isAssetPanelOpen();
    const panelHeight = bottomPanelHeight();
    
    // Trigger resize for any bottom panel changes (open/close/resize)
    setTimeout(() => {
      if (renderStore.engine) {
        renderStore.engine.resize();
        console.log('🔄 Viewport recalculated for bottom panel change');
      }
    }, 50); // Slightly longer delay to ensure positioning is complete
  });
  
  // Get reactive store values
  const ui = () => editorStore.ui;
  const settings = () => editorStore.settings;
  const activeTab = () => ui().selectedBottomTab;
  const rightPanelWidth = () => editorStore.ui.rightPanelWidth;
  const bottomPanelHeight = () => editorStore.ui.bottomPanelHeight;
  
  const isAssetPanelOpen = () => {
    return bottomPanelVisible() && editorStore.panels.isAssetPanelOpen;
  };
  
  const isScenePanelOpen = () => {
    return propertiesPanelVisible() && editorStore.panels.isScenePanelOpen;
  };
  
  const panelPosition = () => settings().editor.panelPosition || 'right';
  const isLeftPanel = () => panelPosition() === 'left';

  const {
    setSelectedBottomTab: setActiveTab,
    setAssetPanelOpen
  } = editorActions;

  // Panel resize functionality
  const [isResizingBottom, setIsResizingBottom] = createSignal(false);
  const [dragOffset, setDragOffset] = createSignal(0);
  
  const handleBottomResizeStart = (e) => {
    setIsResizingBottom(true);
    // Calculate offset between mouse and current panel edge, accounting for footer
    const footerHeight = 24;
    const currentPanelTop = window.innerHeight - bottomPanelHeight() - footerHeight;
    setDragOffset(e?.clientY ? e.clientY - currentPanelTop : 0);
  };
  
  const handleBottomResizeEnd = () => {
    setIsResizingBottom(false);
  };
  
  const handleBottomResizeMove = (e) => {
    if (!isResizingBottom()) return;
    
    // Apply the drag offset so panel edge follows mouse cursor, accounting for footer
    const footerHeight = 24;
    const newHeight = window.innerHeight - footerHeight - (e.clientY - dragOffset());
    const maxHeight = (window.innerHeight - footerHeight) * 0.8;
    const clampedHeight = Math.max(100, Math.min(newHeight, maxHeight));
    editorActions.setBottomPanelHeight(clampedHeight);
    
    // Resize Babylon engine to match new viewport size
    if (renderStore.engine) {
      renderStore.engine.resize();
    }
  };

  const handleContextMenu = (e, item, context = 'bottom-panel', currentPath = '') => {
    showContextMenu(e, item, context, currentPath);
  };
  
  const currentActiveTab = () => activeTab();
  
  const getPanelHeight = () => {
    return isAssetPanelOpen() ? bottomPanelHeight() : 32;
  };
  
  const getPositioning = () => {
    const leftPos = isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0';
    const rightPos = !isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0';
    const heightVal = `${getPanelHeight()}px`;
    const footerHeight = 24; // 24px footer height
    const bottomPos = isAssetPanelOpen() ? `${footerHeight + 1}px` : `${footerHeight + 1}px`;
    
    return { left: leftPos, right: rightPos, height: heightVal, bottom: bottomPos };
  };
  
  return (
    <Show when={bottomPanelVisible()}>
      <div 
        class="absolute pointer-events-auto no-select z-[60]"
        style={getPositioning()}
      >
      <PanelResizer
        type="bottom"
        isResizing={isResizingBottom}
        onResizeStart={handleBottomResizeStart}
        onResizeEnd={handleBottomResizeEnd}
        onResize={handleBottomResizeMove}
        position={{
          top: '-8px',
          left: '0',
          right: '0',
          height: '8px',
          zIndex: 9999
        }}
        className="!bg-transparent !opacity-0 hover:!bg-transparent hover:!opacity-0"
      />
      
      <div class="flex-1 bg-base-200/90 backdrop-blur-sm overflow-hidden flex flex-col" style={{ height: `${getPanelHeight() - 1}px` }}>
        <BottomTabs 
          activeTab={activeTab()}
          onTabChange={(tabId) => {
            setActiveTab(tabId);
            if (!isAssetPanelOpen()) {
              setAssetPanelOpen(true);
            }
          }}
          isAssetPanelOpen={isAssetPanelOpen()}
          onToggleAssetPanel={(newState) => {
            const currentState = isAssetPanelOpen();
            const targetState = newState !== undefined ? newState : !currentState;
            setAssetPanelOpen(targetState);
          }}
          rightPanelWidth={rightPanelWidth()}
          isScenePanelOpen={isScenePanelOpen()}
          panelResize={{
            handleBottomResizeStart,
            handleBottomResizeMove,
            handleBottomResizeEnd
          }}
        />
        
        <Show when={isAssetPanelOpen()}>
          <div class="flex-1 overflow-hidden">
          <Switch>
            <Match when={currentActiveTab() === 'assets'}>
              <AssetLibrary onContextMenu={handleContextMenu} />
            </Match>
            <Match when={bottomPanelTabs().get(currentActiveTab())}>
              {(() => {
                const tab = bottomPanelTabs().get(currentActiveTab());
                if (tab && tab.component) {
                  const Component = tab.component;
                  return <Component />;
                }
                return <div class="p-4 text-base-content/60">Loading plugin content...</div>;
              })()}
            </Match>
            <Match when={true}>
              <div class="p-4 text-red-500 font-bold text-xl">
                🔥 HMR TEST: No content available for tab: {currentActiveTab()}
              </div>
            </Match>
          </Switch>
          </div>
        </Show>
      </div>
      </div>
    </Show>
  );
};

export default BottomPanel;