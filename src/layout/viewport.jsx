import { editorStore } from "@/layout/stores/EditorStore";
import { viewportStore, viewportActions } from "@/layout/stores/ViewportStore";
import { Settings, X } from '@/ui/icons';
import ViewportTabs from './ViewportTabs.jsx';
import { RenderProvider } from '@/plugins/core/render';
import NodeEditor from '@/pages/editor/nodeEditor/index.jsx';
import { viewportTypes } from "@/api/plugin";
import { Show, createMemo } from 'solid-js';

const PersistentRenderViewport = (props) => {
  return (
    <RenderProvider 
      embedded={true} 
      onContextMenu={props.contextMenuHandler} 
      style={{ width: '100%', height: '100%' }}
    />
  );
};

const Viewport = (props) => {
  
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
      case 'node-editor':
        return (
          <div className="absolute inset-0 bg-gray-900 flex flex-col">
            <div className="flex items-center justify-between p-3 border-b border-gray-700 bg-gray-800">
              <div className="flex items-center gap-2">
                <Settings class="w-4 h-4 text-gray-400" />
                <span className="text-sm font-medium text-white">{tab.name}</span>
              </div>
              <button
                onClick={() => {
                  const threeDTab = viewportStore.tabs.find(t => t.type === '3d-viewport');
                  if (threeDTab) {
                    viewportActions.setActiveViewportTab(threeDTab.id);
                  }
                }}
                className="p-1 hover:bg-gray-700 rounded transition-colors"
                title="Close overlay (return to 3D view)"
              >
                <X class="w-4 h-4 text-gray-400" />
              </button>
            </div>
            
            <div className="flex-1 overflow-hidden">
              <NodeEditor 
                tab={tab}
                objectId={tab.objectId}
              />
            </div>
          </div>
        );
        
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
              <div className="absolute inset-0 bg-gray-900">
                <PluginComponent tab={tab} />
              </div>
            );
          }
          
          return (
            <div className="absolute inset-0 bg-gray-900 flex flex-col">
              <div className="flex items-center justify-between p-3 border-b border-gray-700 bg-gray-800">
                <div className="flex items-center gap-2">
                  <Show when={pluginViewportType.icon} fallback={<Settings class="w-4 h-4 text-gray-400" />}>
                    <div class="w-4 h-4 text-gray-400">
                      <pluginViewportType.icon class="w-4 h-4" />
                    </div>
                  </Show>
                  <span className="text-sm font-medium text-white">{tab.name}</span>
                </div>
                <button
                  onClick={() => {
                    const threeDTab = viewportStore.tabs.find(t => t.type === '3d-viewport');
                    if (threeDTab) {
                      viewportActions.setActiveViewportTab(threeDTab.id);
                    }
                  }}
                  className="p-1 hover:bg-gray-700 rounded transition-colors"
                  title="Close overlay (return to 3D view)"
                >
                  <X class="w-4 h-4 text-gray-400" />
                </button>
              </div>
              
              <div className="flex-1 overflow-hidden">
                <PluginComponent tab={tab} />
              </div>
            </div>
          );
        }
        
        return (
          <div className="absolute inset-0 bg-gray-900 flex items-center justify-center">
            <div className="text-center">
              <div className="text-lg text-gray-400 mb-2">Unknown Overlay</div>
              <div className="text-sm text-gray-500">Overlay type "{tab.type}" not found</div>
            </div>
          </div>
        );
    }
  };

  return (
    <div className="w-full h-full flex flex-col bg-gray-900">
      <ViewportTabs />
      <div 
        className="flex-1 relative overflow-hidden"
        onContextMenu={(e) => e.preventDefault()}
      >
        <PersistentRenderViewport
          contextMenuHandler={props.contextMenuHandler}
          showGrid={props.showGrid}
        />
        
        <Show when={isOverlayActive()}>
          {renderOverlayPanel(activeTab())}
        </Show>
      </div>
    </div>
  );
};

export default Viewport;