import BottomTabs from '@/plugins/editor/components/bottomPanel/BottomTabs';
import AssetLibrary from '@/plugins/editor/components/bottomPanel/AssetLibrary';
import { useSnapshot } from 'valtio';
import { globalStore } from '@/store.js';

const BottomPanel = ({
  activeTab,
  isAssetPanelOpen,
  bottomPanelHeight,
  rightPanelWidth,
  isScenePanelOpen,
  onTabChange,
  onToggleAssetPanel,
  onContextMenu,
  style = {}
}) => {
  const settings = useSnapshot(globalStore.editor.settings);
  const panelPosition = settings.editor.panelPosition || 'right';
  const isLeftPanel = panelPosition === 'left';
  const renderPanelContent = () => {
    if (!isAssetPanelOpen) return null;

    const panelStyle = { height: bottomPanelHeight - 40 };

    switch (activeTab) {
      case 'assets':
        return <AssetLibrary onContextMenu={onContextMenu} />;
      default:
        return null;
    }
  };

  return (
    <div 
      className="absolute bottom-0 pointer-events-auto no-select z-10"
      style={{
        left: isLeftPanel && isScenePanelOpen ? rightPanelWidth - 4 : 0,
        right: !isLeftPanel && isScenePanelOpen ? rightPanelWidth - 4 : 0,
        height: isAssetPanelOpen ? bottomPanelHeight : 40,
        ...style
      }}
      suppressHydrationWarning
    >
      <BottomTabs 
        activeTab={activeTab}
        onTabChange={(tabId) => {
          onTabChange(tabId);
          if (!isAssetPanelOpen) {
            onToggleAssetPanel(true);
          }
        }}
        isAssetPanelOpen={isAssetPanelOpen}
        onToggleAssetPanel={onToggleAssetPanel}
        rightPanelWidth={rightPanelWidth}
        isScenePanelOpen={isScenePanelOpen}
      />
      
      {isAssetPanelOpen && (
        <div className="flex-1 bg-gray-900 overflow-hidden" style={{ height: bottomPanelHeight - 40 }}>
          {renderPanelContent()}
        </div>
      )}
    </div>
  );
};

export default BottomPanel;