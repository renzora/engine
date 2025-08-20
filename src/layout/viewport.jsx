import { editorStore } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { Settings, X } from '@/ui/icons';
import ViewportTabs from './ViewportTabs.jsx';
import { ViewportCanvas } from '@/render/babylonjs';
import { viewportTypes, propertiesPanelVisible, bottomPanelVisible } from "@/api/plugin";
import { Show, createMemo, createSignal } from 'solid-js';

const PersistentRenderViewport = (props) => {
  return (
    <ViewportCanvas
      onContextMenu={props.contextMenuHandler} 
      style={{ width: '100%', height: '100%' }}
    />
  );
};

const Viewport = () => {
  const [contextMenu, setContextMenu] = createSignal(null);
  
  // Get reactive store values
  const ui = () => editorStore.ui;
  const settings = () => editorStore.settings;
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
  
  const getViewportPositioning = () => {
    const top = '0px';
    const left = isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const right = !isLeftPanel() && isScenePanelOpen() && propertiesPanelVisible() ? `${rightPanelWidth()}px` : '0px';
    const bottom = isAssetPanelOpen() ? `${bottomPanelHeight()}px` : (bottomPanelVisible() ? '40px' : '0px');
    
    return { top, left, right, bottom };
  };
  
  const activeTab = createMemo(() => {
    const tab = viewportStore.tabs.find(tab => tab.id === viewportStore.activeTabId);
    console.log('🎯 ViewportContainer - Active tab:', tab);
    console.log('🎯 ViewportContainer - All tabs:', viewportStore.tabs);
    console.log('🎯 ViewportContainer - Active tab ID:', viewportStore.activeTabId);
    return tab;
  });
  const isOverlayActive = createMemo(() => {
    const active = activeTab() && activeTab().type !== '3d-viewport';
    console.log('🎯 ViewportContainer - Overlay active:', active, activeTab()?.type);
    return active;
  });
  
  const renderOverlayPanel = (tab) => {
    if (!tab) return null;
    
    console.log('🎯 Rendering overlay panel for tab type:', tab.type);
    
    switch (tab.type) {
        
      default:
        // Check if this is a plugin viewport type
        console.log('🎯 Checking for plugin viewport type:', tab.type);
        console.log('🎯 Available viewport types:', Array.from(viewportTypes().keys()));
        const pluginViewportType = viewportTypes().get(tab.type);
        console.log('🎯 Found plugin viewport type:', pluginViewportType);
        if (pluginViewportType && pluginViewportType.component) {
          const PluginComponent = pluginViewportType.component;
          console.log('🎯 Rendering plugin component for:', tab.type);
          
          // Special handling for splash viewport - no header
          if (tab.type === 'splash-viewport') {
            return (
              <div className="absolute inset-0 bg-base-100">
                <PluginComponent tab={tab} />
              </div>
            );
          }
          
          return (
            <div className="absolute inset-0 bg-base-100 flex flex-col">
              <div className="flex items-center justify-between p-3 border-b border-base-300 bg-base-200">
                <div className="flex items-center gap-2">
                  <Show when={pluginViewportType.icon} fallback={<Settings class="w-4 h-4 text-base-content/60" />}>
                    <div class="w-4 h-4 text-base-content/60">
                      <pluginViewportType.icon class="w-4 h-4" />
                    </div>
                  </Show>
                  <span className="text-sm font-medium text-base-content">{tab.name}</span>
                </div>
                <button
                  onClick={() => {
                    const threeDTab = viewportStore.tabs.find(t => t.type === '3d-viewport');
                    if (threeDTab) {
                      viewportActions.setActiveViewportTab(threeDTab.id);
                    }
                  }}
                  className="p-1 hover:bg-base-300 rounded transition-colors"
                  title="Close overlay (return to 3D view)"
                >
                  <X class="w-4 h-4 text-base-content/60" />
                </button>
              </div>
              
              <div className="flex-1 overflow-hidden">
                <PluginComponent tab={tab} />
              </div>
            </div>
          );
        }
        
        return (
          <div className="absolute inset-0 bg-base-100 flex items-center justify-center">
            <div className="text-center">
              <div className="text-lg text-base-content/60 mb-2">Unknown Overlay</div>
              <div className="text-sm text-base-content/50">Overlay type "{tab.type}" not found</div>
            </div>
          </div>
        );
    }
  };

  return (
    <div 
      class="absolute pointer-events-auto viewport-container"
      style={getViewportPositioning()}
    >
      <div className="w-full h-full flex flex-col bg-base-100">
        <ViewportTabs />
        <div 
          className="flex-1 relative overflow-hidden"
          onContextMenu={(e) => e.preventDefault()}
        >
          <PersistentRenderViewport
            contextMenuHandler={() => handleContextMenu}
            showGrid={true}
          />
          
          <Show when={isOverlayActive()}>
            {renderOverlayPanel(activeTab())}
          </Show>
        </div>
      </div>
    </div>
  );
};

export default Viewport;