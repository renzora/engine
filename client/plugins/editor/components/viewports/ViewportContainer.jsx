import { useRef, useEffect } from 'react';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";
import { Icons } from '@/plugins/editor/components/Icons';

// Import viewport components
import ViewportTabs from '@/plugins/editor/components/ui/ViewportTabs.jsx';
import ErrorBoundary from '../ErrorBoundary.jsx';

// Import 3D viewport components
import RenderPlugin from '@/plugins/render/index.jsx';
import NodeEditor from '@/plugins/editor/components/viewports/NodeEditor.jsx';

// Always-active 3D viewport that stays mounted
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
  
  // Check if the active tab is an overlay type (non-3D viewport)
  const isOverlayActive = activeTab && activeTab.type !== '3d-viewport';
  
  const renderOverlayPanel = (tab) => {
    if (!tab) return null;
    
    switch (tab.type) {
      case 'node-editor':
        return (
          <div className="absolute inset-0 bg-gray-900 flex flex-col">
            {/* Overlay header with close button */}
            <div className="flex items-center justify-between p-3 border-b border-gray-700 bg-gray-800">
              <div className="flex items-center gap-2">
                <Icons.Cog className="w-4 h-4 text-gray-400" />
                <span className="text-sm font-medium text-white">{tab.name}</span>
              </div>
              <button
                onClick={() => {
                  // Switch back to a 3D viewport tab
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
            
            {/* Node editor content */}
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
      {/* Viewport Tabs */}
      <ViewportTabs />
      
      {/* Viewport Content - Always shows 3D viewport with optional overlay */}
      <div 
        className="flex-1 relative overflow-hidden"
        onContextMenu={(e) => e.preventDefault()}
      >
        {/* Always-active 3D viewport (background) */}
        <PersistentRenderViewport
          contextMenuHandler={contextMenuHandler}
          showGrid={showGrid}
        />
        
        {/* Overlay panel for non-3D viewport tabs */}
        {isOverlayActive && renderOverlayPanel(activeTab)}
      </div>
    </div>
  );
};

export default ViewportContainer;