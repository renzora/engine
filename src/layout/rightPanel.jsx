import Toolbar from '@/components/Toolbars/Toolbar.jsx';
import Settings from '@/pages/editor/Settings.jsx';
import PanelResizer from '@/ui/PanelResizer.jsx';
import { editorStore } from '@/layout/stores/EditorStore';
import { propertyTabs } from '@/api/plugin';
import { Show, createMemo } from 'solid-js';

const RightPanel = (props) => {
  const settings = editorStore.settings;
  const panelPosition = settings.editor.panelPosition || 'right';
  const isLeft = panelPosition === 'left';
  
  console.log('🔵 RightPanel props:', { 
    isScenePanelOpen: props.isScenePanelOpen, 
    rightPanelWidth: props.rightPanelWidth, 
    rightPanelWidthType: typeof props.rightPanelWidth,
    isLeft 
  });
  
  const getTabTitle = createMemo(() => {
    // Check if it's a plugin tab first
    const pluginTab = propertyTabs().get(props.selectedRightTool);
    if (pluginTab) {
      return pluginTab.title;
    }
    
    // Handle built-in tabs
    switch (props.selectedRightTool) {
      case 'settings': return 'Settings';
      default: return 'Properties';
    }
  });

  const renderTabContent = () => {
    // Check if it's a plugin tab first
    const pluginTab = propertyTabs().get(props.selectedRightTool);
    if (pluginTab && pluginTab.component) {
      const PluginComponent = pluginTab.component;
      return <PluginComponent 
        selectedObject={props.selectedObject}
        onObjectSelect={props.onObjectSelect}
        onContextMenu={props.onContextMenu}
      />;
    }
    
    // Handle built-in tabs
    switch (props.selectedRightTool) {
      case 'settings':
        return <Settings />;
      
      default:
        return (
          <div class="p-4 text-center text-gray-400">
            <p>No properties panel available</p>
          </div>
        );
    }
  };

  return (
    <div 
      className={`absolute top-0 right-0 pointer-events-auto no-select z-20`}
      style={{ 
        height: '100%',
        width: `${props.rightPanelWidth}px`
      }}
    >
      {props.isScenePanelOpen && (
        <PanelResizer
          type="right"
          isResizing={props.panelResize?.isResizingRight}
          onResizeStart={props.panelResize?.handleRightResizeStart}
          onResizeEnd={props.panelResize?.handleRightResizeEnd}
          onResize={(e) => props.panelResize?.handleRightResizeMove(e, { 
            isScenePanelOpen: props.isScenePanelOpen, 
            isLeftPanel: props.isLeftPanel 
          })}
          isLeftPanel={props.isLeftPanel}
          position={{
            left: '-8px',
            top: 0,
            bottom: props.bottomPanelVisible ? (props.isAssetPanelOpen ? `${props.bottomPanelHeight}px` : '40px') : '0px',
            width: '8px',
            zIndex: 30
          }}
        />
      )}
      
      {props.isScenePanelOpen && (
        <div className="absolute inset-0 flex">
          <div className="w-auto flex-shrink-1">
            <Toolbar 
              selectedTool={props.selectedRightTool}
              onToolSelect={props.onToolSelect}
              scenePanelOpen={props.isScenePanelOpen}
              onScenePanelToggle={props.onScenePanelToggle}
              panelResize={props.panelResize}
              isLeftPanel={props.isLeftPanel}
            />
          </div>
          
          <div className="flex-1">
            <div 
              className={`relative w-full h-full bg-gradient-to-b from-slate-800/95 to-slate-900/98 backdrop-blur-md border-l border-slate-700/80 shadow-2xl shadow-black/30 flex flex-col pointer-events-auto no-select`}
            >
            <div className="px-3 py-2 relative">
              <div className="text-xs text-gray-400 uppercase tracking-wide">
                {getTabTitle()}
              </div>
              
              <div className="absolute flex items-center" style={{ top: '4px', right: '-1px' }}>
                <button
                  onClick={() => {
                    props.onScenePanelToggle();
                  }}
                  className="w-6 h-6 text-gray-400 hover:text-blue-400 transition-colors flex items-center justify-center group relative"
                  style={{ 
                    'background-color': '#1e293b',
                    'border-left': '1px solid #182236',
                    'border-top': '1px solid #182236',
                    'border-bottom': '1px solid #182236',
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
                  
                  <div className="absolute right-full mr-1 top-1/2 -translate-y-1/2 bg-slate-900/95 backdrop-blur-sm border border-slate-600 text-white text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl" 
                       style={{ 'z-index': 50 }}>
                    Close panel
                    <div className="absolute left-full top-1/2 -translate-y-1/2 w-0 h-0 border-l-4 border-l-slate-900 border-t-4 border-t-transparent border-b-4 border-b-transparent"></div>
                  </div>
                </button>
              </div>
            </div>
            
              {renderTabContent()}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default RightPanel;