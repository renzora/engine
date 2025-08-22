import { Show, For } from 'solid-js';
import AssetItem from './AssetItem';

function AssetGrid({ 
  layoutMode,
  filteredAssets,
  assetGridRef,
  isAssetSelected,
  hoveredItem,
  setHoveredItem,
  toggleAssetSelection,
  handleAssetDoubleClick,
  isInternalDrag,
  setIsInternalDrag,
  selectedAssets,
  setSelectedAssets,
  lastSelectedAsset,
  setLastSelectedAsset,
  setDragOverFolder,
  setDragOverTreeFolder,
  setDragOverBreadcrumb,
  loadedAssets,
  preloadingAssets,
  failedAssets,
  setFailedAssets,
  setPreloadingAssets,
  setLoadedAssets,
  getExtensionStyle
}) {
  return (
    <Show when={filteredAssets().length > 0}>
      <Show when={layoutMode() === 'grid'} fallback={
        <div class="space-y-0">
          <For each={filteredAssets()}>
            {(asset, index) => (
              <AssetItem 
                asset={asset}
                index={index}
                layoutMode={layoutMode}
                isAssetSelected={isAssetSelected}
                hoveredItem={hoveredItem}
                setHoveredItem={setHoveredItem}
                toggleAssetSelection={toggleAssetSelection}
                handleAssetDoubleClick={handleAssetDoubleClick}
                isInternalDrag={isInternalDrag}
                setIsInternalDrag={setIsInternalDrag}
                selectedAssets={selectedAssets}
                setSelectedAssets={setSelectedAssets}
                lastSelectedAsset={lastSelectedAsset}
                setLastSelectedAsset={setLastSelectedAsset}
                filteredAssets={filteredAssets}
                setDragOverFolder={setDragOverFolder}
                setDragOverTreeFolder={setDragOverTreeFolder}
                setDragOverBreadcrumb={setDragOverBreadcrumb}
                loadedAssets={loadedAssets}
                preloadingAssets={preloadingAssets}
                failedAssets={failedAssets}
                setFailedAssets={setFailedAssets}
                setPreloadingAssets={setPreloadingAssets}
                setLoadedAssets={setLoadedAssets}
                getExtensionStyle={getExtensionStyle}
              />
            )}
          </For>
        </div>
      }>
        <div 
          ref={assetGridRef}
          class="grid grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-8 2xl:grid-cols-10 gap-3 relative"
        >
          <For each={filteredAssets()}>
            {(asset) => (
              <AssetItem 
                asset={asset}
                layoutMode={layoutMode}
                isAssetSelected={isAssetSelected}
                hoveredItem={hoveredItem}
                setHoveredItem={setHoveredItem}
                toggleAssetSelection={toggleAssetSelection}
                handleAssetDoubleClick={handleAssetDoubleClick}
                isInternalDrag={isInternalDrag}
                setIsInternalDrag={setIsInternalDrag}
                selectedAssets={selectedAssets}
                setSelectedAssets={setSelectedAssets}
                lastSelectedAsset={lastSelectedAsset}
                setLastSelectedAsset={setLastSelectedAsset}
                filteredAssets={filteredAssets}
                setDragOverFolder={setDragOverFolder}
                setDragOverTreeFolder={setDragOverTreeFolder}
                setDragOverBreadcrumb={setDragOverBreadcrumb}
                loadedAssets={loadedAssets}
                preloadingAssets={preloadingAssets}
                failedAssets={failedAssets}
                setFailedAssets={setFailedAssets}
                setPreloadingAssets={setPreloadingAssets}
                setLoadedAssets={setLoadedAssets}
                getExtensionStyle={getExtensionStyle}
              />
            )}
          </For>
        </div>
      </Show>
    </Show>
  );
}

export default AssetGrid;