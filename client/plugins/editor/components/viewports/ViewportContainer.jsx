import { useRef, useEffect } from 'react';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";
import { Icons } from '@/plugins/editor/components/Icons';
import ViewportTabs from '@/plugins/editor/components/ui/ViewportTabs.jsx';
import RenderPlugin from '@/plugins/render/index.jsx';
import NodeEditor from '@/plugins/editor/components/viewports/NodeEditor.jsx';

const PersistentRenderViewport = ({ contextMenuHandler, showGrid }) => {
  return (
    <RenderPlugin 
      embedded={true} 
      onContextMenu={contextMenuHandler} 
      style={{ width: '100%', height: '100%' }}
    />
  );
};


const ViewportContainer = ({ 
  onContextMenu, 
  contextMenuHandler, 
  showGrid 
}) => {
  const { viewport } = useSnapshot(globalStore.editor);
  const { tabs, activeTabId } = viewport;
  const activeTab = tabs.find(tab => tab.id === activeTabId);
  const isOverlayActive = activeTab && activeTab.type !== '3d-viewport';
  
  const renderOverlayPanel = (tab) => {
    if (!tab) return null;
    
    switch (tab.type) {
      case 'node-editor':
        return (
          <div className="absolute inset-0 bg-gray-900 flex flex-col">
            <div className="flex items-center justify-between p-3 border-b border-gray-700 bg-gray-800">
              <div className="flex items-center gap-2">
                <Icons.Cog className="w-4 h-4 text-gray-400" />
                <span className="text-sm font-medium text-white">{tab.name}</span>
              </div>
              <button
                onClick={() => {
                  const threeDTab = tabs.find(t => t.type === '3d-viewport');
                  if (threeDTab) {
                    actions.editor.setActiveViewportTab(threeDTab.id);
                  }
                }}
                className="p-1 hover:bg-gray-700 rounded transition-colors"
                title="Close overlay (return to 3D view)"
              >
                <Icons.X className="w-4 h-4 text-gray-400" />
              </button>
            </div>
            
            <div className="flex-1 overflow-hidden">
              <NodeEditor 
                key={tab.id}
                tab={tab}
                objectId={tab.objectId}
              />
            </div>
          </div>
        );
        
      default:
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
          contextMenuHandler={contextMenuHandler}
          showGrid={showGrid}
        />
        
        {isOverlayActive && renderOverlayPanel(activeTab)}
      </div>
    </div>
  );
};

export default ViewportContainer;