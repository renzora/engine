import { Show, createEffect, createMemo, Switch, Match } from 'solid-js';
import BottomTabs from './BottomTabs';
import AssetLibrary from './AssetLibrary';
import PanelResizer from '../ui/PanelResizer';
import { editorStore } from '../stores/EditorStore';
import { bottomPanelTabs, bottomPanelVisible, propertiesPanelVisible } from '@/plugins/core/engine';

const BottomPanel = ({
  activeTab,
  isAssetPanelOpen,
  bottomPanelHeight,
  rightPanelWidth,
  isScenePanelOpen,
  panelResize,
  onTabChange,
  onToggleAssetPanel,
  onContextMenu,
  style = {}
}) => {
  const settings = () => editorStore.settings;
  const panelPosition = () => settings().editor.panelPosition || 'right';
  const isLeftPanel = () => panelPosition() === 'left';
  
  
  // Get the active tab from the store directly for reactivity
  const currentActiveTab = () => editorStore.ui.selectedBottomTab;
  
  const getPanelHeight = () => {
    const height = typeof bottomPanelHeight === 'function' ? bottomPanelHeight() : bottomPanelHeight;
    const assetPanelOpen = typeof isAssetPanelOpen === 'function' ? isAssetPanelOpen() : isAssetPanelOpen;
    const finalHeight = assetPanelOpen ? height : 40;
    return finalHeight;
  };
  
  const getRightWidth = () => {
    const width = typeof rightPanelWidth === 'function' ? rightPanelWidth() : rightPanelWidth;
    const scenePanelOpen = typeof isScenePanelOpen === 'function' ? isScenePanelOpen() : isScenePanelOpen;
    return width;
  };

  const getPositioning = () => {
    const scenePanelOpen = typeof isScenePanelOpen === 'function' ? isScenePanelOpen() : isScenePanelOpen;
    const leftPos = isLeftPanel() && scenePanelOpen && propertiesPanelVisible() ? `${getRightWidth()}px` : '0';
    const rightPos = !isLeftPanel() && scenePanelOpen && propertiesPanelVisible() ? `${getRightWidth()}px` : '0';
    const heightVal = `${getPanelHeight()}px`;
    
    return { left: leftPos, right: rightPos, height: heightVal };
  };
  
  return (
    <Show when={bottomPanelVisible()}>
      <div 
        class="absolute bottom-0 pointer-events-auto no-select z-[60]"
        style={{
          ...getPositioning(),
          ...style
        }}
      >
      <Show when={panelResize}>
        <PanelResizer
          type="bottom"
          isResizing={panelResize.isResizingBottom}
          onResizeStart={panelResize.handleBottomResizeStart}
          onResizeEnd={panelResize.handleBottomResizeEnd}
          onResize={(e) => panelResize.handleBottomResizeMove(e, { isAssetPanelOpen })}
          position={{
            top: '-8px',
            left: '0',
            right: '0',
            height: '8px',
            zIndex: 9999
          }}
          className="hover:h-3"
        />
      </Show>
      
      <BottomTabs 
        activeTab={activeTab}
        onTabChange={(tabId) => {
          onTabChange(tabId);
          const assetPanelOpen = typeof isAssetPanelOpen === 'function' ? isAssetPanelOpen() : isAssetPanelOpen;
          if (!assetPanelOpen) {
            onToggleAssetPanel(true);
          }
        }}
        isAssetPanelOpen={typeof isAssetPanelOpen === 'function' ? isAssetPanelOpen() : isAssetPanelOpen}
        onToggleAssetPanel={onToggleAssetPanel}
        rightPanelWidth={rightPanelWidth}
        isScenePanelOpen={typeof isScenePanelOpen === 'function' ? isScenePanelOpen() : isScenePanelOpen}
        panelResize={panelResize}
      />
      
      <Show when={typeof isAssetPanelOpen === 'function' ? isAssetPanelOpen() : isAssetPanelOpen}>
        <div class="flex-1 bg-gray-900 overflow-hidden" style={{ height: `${getPanelHeight() - 40}px` }}>
          <Switch>
            <Match when={currentActiveTab() === 'assets'}>
              <AssetLibrary onContextMenu={onContextMenu} />
            </Match>
            <Match when={bottomPanelTabs().get(currentActiveTab())}>
              {(() => {
                const tab = bottomPanelTabs().get(currentActiveTab());
                if (tab && tab.component) {
                  const Component = tab.component;
                  return <Component />;
                }
                return <div class="p-4 text-gray-400">Loading plugin content...</div>;
              })()}
            </Match>
            <Match when={true}>
              <div class="p-4 text-gray-400">
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