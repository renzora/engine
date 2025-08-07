import Toolbar from '@/plugins/editor/components/ui/Toolbar';
import Scene from '@/plugins/editor/components/propertiesPanel/tabs/Scene';
import Settings from '@/plugins/editor/components/propertiesPanel/tabs/Settings';
import { Icons } from '@/plugins/editor/components/Icons.jsx';
import { useSnapshot } from 'valtio';
import { globalStore } from '@/store.js';

const RightPanel = ({
  isScenePanelOpen,
  rightPanelWidth,
  bottomPanelHeight,
  isAssetPanelOpen,
  selectedRightTool,
  selectedObject,
  onToolSelect,
  onScenePanelToggle,
  onObjectSelect,
  onContextMenu,
  style = {}
}) => {
  const settings = useSnapshot(globalStore.editor.settings);
  const sceneData = useSnapshot(globalStore.editor.scene);
  const panelPosition = settings.editor.panelPosition || 'right';
  const isLeft = panelPosition === 'left';
  
  const getTabTitle = () => {
    switch (selectedRightTool) {
      case 'scene': {
        const objectCount = sceneData.objects.meshes.length + 
          (sceneData.objects.transformNodes?.length || 0) + 
          sceneData.objects.lights.length + 
          sceneData.objects.cameras.length;
        return `Scene Objects (${objectCount})`;
      }
      case 'settings': return 'Settings';
      default: {
        const objectCount = sceneData.objects.meshes.length + 
          (sceneData.objects.transformNodes?.length || 0) + 
          sceneData.objects.lights.length + 
          sceneData.objects.cameras.length;
        return `Scene Objects (${objectCount})`;
      }
    }
  };

  const renderTabContent = () => {
    switch (selectedRightTool) {
      case 'scene':
        return (
          <Scene 
            selectedObject={selectedObject}
            onObjectSelect={onObjectSelect}
            onContextMenu={onContextMenu}
          />
        );
      
      case 'settings':
        return <Settings />;
      
      default:
        return (
          <Scene 
            selectedObject={selectedObject}
            onObjectSelect={onObjectSelect}
            onContextMenu={onContextMenu}
          />
        );
    }
  };

  return (
    <div 
      className={`absolute ${isLeft ? 'left-0' : 'right-0'} top-0 bottom-0 ${isScenePanelOpen ? 'pointer-events-auto' : 'pointer-events-none'} no-select z-20`}
      style={{ 
        width: isScenePanelOpen ? rightPanelWidth : 0,
        paddingBottom: isAssetPanelOpen ? bottomPanelHeight : 40,
        ...style
      }}
      suppressHydrationWarning
    >
      {isScenePanelOpen && (
        <div 
          className={`absolute top-0 bottom-0 w-12 ${
            isLeft 
              ? (isScenePanelOpen ? 'right-1' : 'right-0')
              : (isScenePanelOpen ? 'left-1' : 'left-0')
          }`}
        >
          <Toolbar 
            selectedTool={selectedRightTool}
            onToolSelect={onToolSelect}
            scenePanelOpen={isScenePanelOpen}
            onScenePanelToggle={onScenePanelToggle}
          />
        </div>
      )}
      
      {isScenePanelOpen && (
        <div className={`absolute ${isLeft ? 'left-0 right-13' : 'left-13 right-0'} top-0 bottom-0`}>
          <div 
            className={`relative w-full h-full bg-gradient-to-b from-slate-800/95 to-slate-900/98 backdrop-blur-md ${isLeft ? 'border-r' : 'border-l'} border-slate-700/80 shadow-2xl shadow-black/30 flex flex-col pointer-events-auto no-select`}
          >
            {/* Panel Header - Gray Title Only */}
            <div className="px-3 py-2 relative">
              <div className="text-xs text-gray-400 uppercase tracking-wide">
                {getTabTitle()}
              </div>
              
              {/* Toggle Button - Far Right Edge */}
              <div className="absolute flex items-center" style={{ top: '4px', right: '-1px' }}>
                <button
                  onClick={() => {
                    onScenePanelToggle();
                    onToolSelect('select'); // Reset to default tool when closing
                  }}
                  className="w-6 h-6 text-gray-400 hover:text-blue-400 transition-colors flex items-center justify-center group relative"
                  style={{ 
                    backgroundColor: '#1e293b',
                    borderLeft: '1px solid #182236',
                    borderTop: '1px solid #182236',
                    borderBottom: '1px solid #182236',
                    borderTopLeftRadius: '6px',
                    borderBottomLeftRadius: '6px'
                  }}
                  title="Close panel"
                >
                  <div className="w-3 h-3 flex items-center justify-center">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="w-3 h-3">
                      <path d="m9 18 6-6-6-6"/>
                    </svg>
                  </div>
                  
                  {/* Tooltip */}
                  <div className="absolute right-full mr-1 top-1/2 -translate-y-1/2 bg-slate-900/95 backdrop-blur-sm border border-slate-600 text-white text-xs px-3 py-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-2xl" 
                       style={{ zIndex: 50 }}>
                    Close panel
                    {/* Arrow pointing to the button */}
                    <div className="absolute left-full top-1/2 -translate-y-1/2 w-0 h-0 border-l-4 border-l-slate-900 border-t-4 border-t-transparent border-b-4 border-b-transparent"></div>
                  </div>
                </button>
              </div>
            </div>
            
            {/* Tab Content */}
            {renderTabContent()}
          </div>
        </div>
      )}
    </div>
  );
};

export default RightPanel;