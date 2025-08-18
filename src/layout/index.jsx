import { createSignal, Show, onMount } from 'solid-js';
import { editorStore, editorActions } from './stores/EditorStore';
import { propertiesPanelVisible, bottomPanelVisible } from '@/api/plugin';
import { createPanelResize } from '@/pages/editor/hooks/usePanelResize';
import TopMenu from './topMenu.jsx';
import HorizontalToolbar from './horizontalToolbar.jsx';
import Viewport from './viewport.jsx';
import RightPanel from './rightPanel.jsx';
import BottomPanel from './bottomPanel.jsx';
import PanelToggleButton from '@/ui/PanelToggleButton.jsx';

const Layout = () => {
  const [mounted, setMounted] = createSignal(false);
  const [contextMenu, setContextMenu] = createSignal(null);

  // Get reactive store values
  const selection = () => editorStore.selection;
  const ui = () => editorStore.ui;
  const panels = () => editorStore.panels;
  const settings = () => editorStore.settings;
  const selectedObject = () => selection().entity;
  const selectedRightTool = () => ui().selectedTool;
  const activeTab = () => ui().selectedBottomTab;
  const rightPanelWidth = () => editorStore.ui.rightPanelWidth;
  const bottomPanelHeight = () => editorStore.ui.bottomPanelHeight;
  
  const isScenePanelOpen = () => {
    return propertiesPanelVisible() && editorStore.panels.isScenePanelOpen;
  };
  
  const isAssetPanelOpen = () => {
    return bottomPanelVisible() && editorStore.panels.isAssetPanelOpen;
  };
  
  const panelPosition = () => settings().editor.panelPosition || 'right';
  const isLeftPanel = () => panelPosition() === 'left';

  const {
    selectEntity: setSelectedEntity, setContextMenuHandler, setTransformMode,
    setSelectedTool: setSelectedRightTool, setSelectedBottomTab: setActiveTab,
    setScenePanelOpen, setAssetPanelOpen
  } = editorActions;

  const panelResize = createPanelResize(editorActions);

  const handleObjectSelect = (objectId) => {
    setSelectedEntity(objectId);
    if (objectId) {
      setTransformMode('move');
    }
  };

  const handleContextMenu = (e, item, context = 'scene') => {
    if (!e) return;
    
    e.preventDefault();
    e.stopPropagation();

    const { clientX: x, clientY: y } = e;
    // Simple context menu implementation
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
    
    // Ensure we have a valid tool selected when opening the panel
    if (!currentState && (!selectedRightTool() || selectedRightTool() === 'select')) {
      setSelectedRightTool('scene');
    }
  };

  onMount(() => {
    setMounted(true);
  });

  return (
    <Show when={mounted()} fallback={<div />}>
      <div class="fixed inset-0 flex flex-col pointer-events-none z-10" onContextMenu={(e) => e.preventDefault()}>
        <div class="flex-shrink-0 pointer-events-auto z-50">
          <TopMenu />
          <HorizontalToolbar />
        </div>
      
        <div class="flex-1 relative overflow-hidden pointer-events-auto">
          <div 
            class="absolute pointer-events-auto viewport-container"
            style={{
              top: '0px',
              left: isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px',
              right: !isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px',
              bottom: isAssetPanelOpen() ? `${bottomPanelHeight()}px` : (bottomPanelVisible() ? '40px' : '0px'),
            }}
          >
            <Viewport 
              onContextMenu={(e) => e.preventDefault()}
              contextMenuHandler={() => handleContextMenu}
              showGrid={true}
              style={{ width: '100%', height: '100%', display: 'block' }}
            />
          </div>
          
          <Show when={propertiesPanelVisible()}>
            <RightPanel
              isScenePanelOpen={isScenePanelOpen()}
              rightPanelWidth={rightPanelWidth()}
              bottomPanelHeight={bottomPanelHeight()}
              isAssetPanelOpen={isAssetPanelOpen()}
              bottomPanelVisible={bottomPanelVisible()}
              selectedRightTool={selectedRightTool()}
              selectedObject={selectedObject()}
              panelResize={panelResize}
              isLeftPanel={isLeftPanel()}
              onToolSelect={setSelectedRightTool}
              onScenePanelToggle={handleRightPanelToggle}
              onObjectSelect={handleObjectSelect}
              onContextMenu={handleContextMenu}
            />
          </Show>

          <Show when={propertiesPanelVisible() && !isScenePanelOpen()}>
            <PanelToggleButton
              onClick={() => setScenePanelOpen(true)}
              position={isLeftPanel() ? { left: 0 } : { right: 0 }}
              isLeftPanel={isLeftPanel()}
            />
          </Show>
          
          <BottomPanel
            activeTab={activeTab()}
            isAssetPanelOpen={isAssetPanelOpen}
            bottomPanelHeight={bottomPanelHeight}
            rightPanelWidth={rightPanelWidth}
            isScenePanelOpen={isScenePanelOpen}
            panelResize={panelResize}
            onTabChange={(tabId) => {
              setActiveTab(tabId);
              if (!isAssetPanelOpen()) {
                setAssetPanelOpen(true);
              }
            }}
            onToggleAssetPanel={(newState) => {
              const currentState = isAssetPanelOpen();
              const targetState = newState !== undefined ? newState : !currentState;
              setAssetPanelOpen(targetState);
            }}
            onContextMenu={handleContextMenu}
          />
        </div>
      </div>
    </Show>
  );
};

export default Layout;