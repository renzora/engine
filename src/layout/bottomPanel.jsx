import { Show, createEffect, createMemo, Switch, Match, createSignal, onMount, onCleanup } from 'solid-js';
import BottomTabs from './BottomTabs.jsx';
import AssetLibrary from '@/pages/editor/AssetLibrary';
import PanelResizer from '@/ui/PanelResizer.jsx';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { bottomPanelTabs, bottomPanelVisible, propertiesPanelVisible } from '@/api/plugin';

const BottomPanel = () => {
  const [contextMenu, setContextMenu] = createSignal(null);
  
  // Handle window resize to ensure panel height stays within bounds
  onMount(() => {
    const handleWindowResize = () => {
      const currentHeight = editorStore.ui.bottomPanelHeight;
      const maxHeight = Math.floor(window.innerHeight * 0.8);
      if (currentHeight > maxHeight) {
        editorActions.setBottomPanelHeight(maxHeight);
      }
    };
    
    window.addEventListener('resize', handleWindowResize);
    onCleanup(() => window.removeEventListener('resize', handleWindowResize));
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
  
  const handleBottomResizeStart = (e) => {
    setIsResizingBottom(true);
  };
  
  const handleBottomResizeEnd = () => {
    setIsResizingBottom(false);
  };
  
  const handleBottomResizeMove = (e) => {
    if (!isResizingBottom()) return;
    
    const newHeight = window.innerHeight - e.clientY;
    const clampedHeight = Math.max(100, Math.min(newHeight, window.innerHeight * 0.8));
    editorActions.setBottomPanelHeight(clampedHeight);
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
  
  const currentActiveTab = () => activeTab();
  
  const getPanelHeight = () => {
    return isAssetPanelOpen() ? bottomPanelHeight() : 40;
  };
  
  const getPositioning = () => {
    const leftPos = isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0';
    const rightPos = !isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0';
    const heightVal = `${getPanelHeight()}px`;
    
    return { left: leftPos, right: rightPos, height: heightVal };
  };
  
  return (
    <Show when={bottomPanelVisible()}>
      <div 
        class="absolute bottom-0 pointer-events-auto no-select z-[60]"
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
        className="hover:h-3"
      />
      
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
      />
      
      <Show when={isAssetPanelOpen()}>
        <div class="flex-1 bg-base-200 overflow-hidden" style={{ height: `${getPanelHeight() - 40}px` }}>
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
              <div class="p-4 text-base-content/60">
                No content available for tab: {currentActiveTab()}
              </div>
            </Match>
          </Switch>
        </div>
      </Show>
      </div>
    </Show>
  );
};

export default BottomPanel;