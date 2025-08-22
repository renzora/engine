import { Show, For } from 'solid-js';
import { Folder, Cube } from '@/ui/icons';
import AssetSearch from './AssetSearch';
import FileTree from './FileTree';

function AssetSidebar({
  treePanelWidth,
  isResizing,
  viewMode,
  setViewMode,
  searchQuery,
  setSearchQuery,
  isSearching,
  folderTree,
  categoryList,
  selectedCategory,
  setSelectedCategory,
  expandedFolders,
  currentPath,
  dragOverTreeFolder,
  isInternalDrag,
  error,
  onFolderClick,
  onFolderToggle,
  onTreeDragOver,
  onTreeDragEnter,
  onTreeDragLeave,
  onTreeDrop,
  onResizeMouseDown,
  onRefresh
}) {
  return (
    <div 
      class="bg-base-300 border-r border-base-300 flex flex-col relative"
      style={{ width: `${treePanelWidth()}px` }}
    >
      <div
        class={`absolute right-0 top-0 bottom-0 w-0.5 resize-handle cursor-col-resize ${isResizing() ? 'dragging' : ''}`}
        onMouseDown={onResizeMouseDown}
      />
      <div class="px-2 py-2 border-b border-base-300">
        <div class="flex items-center gap-2">
          <div class="flex-1">
            <AssetSearch 
              searchQuery={searchQuery}
              setSearchQuery={setSearchQuery}
              isSearching={isSearching}
            />
          </div>
          <div class="flex bg-base-200 rounded overflow-hidden">
            <button
              onClick={() => setViewMode('folder')}
              class={`px-2 py-1 text-xs transition-colors ${
                viewMode() === 'folder'
                  ? 'bg-primary text-primary-content'
                  : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
              }`}
              title="Folder View"
            >
              <Folder class="w-3 h-3" />
            </button>
            <button
              onClick={() => setViewMode('type')}
              class={`px-2 py-1 text-xs transition-colors ${
                viewMode() === 'type'
                  ? 'bg-primary text-primary-content'
                  : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
              }`}
              title="Asset Type View"
            >
              <Cube class="w-3 h-3" />
            </button>
          </div>
        </div>
      </div>
      
      <div class="flex-1 overflow-y-auto scrollbar-thin">
        <Show when={viewMode() === 'folder'} fallback={
          <Show when={categoryList().length > 0} fallback={
            <div class="p-4 text-center text-base-content/50 text-xs">
              {error() ? error() : 'Loading asset categories...'}
            </div>
          }>
            <div class="space-y-0.5 p-1">
              <For each={categoryList()}>
                {(category) => (
                  <button
                    onClick={() => setSelectedCategory(category.id)}
                    class={`w-full flex items-center justify-between px-2 py-1.5 text-left text-xs rounded hover:bg-base-200 transition-colors ${
                      selectedCategory() === category.id 
                        ? 'bg-primary text-primary-content' 
                        : 'text-base-content/70 hover:text-base-content'
                    }`}
                  >
                    <span class="flex items-center">
                      <category.icon class={`w-3 h-3 mr-2 ${
                        selectedCategory() === category.id ? 'text-primary-content' : 'text-base-content/60'
                      }`} />
                      {category.label}
                    </span>
                    <span class={`text-[10px] px-1.5 py-0.5 rounded-full ${
                      selectedCategory() === category.id 
                        ? 'text-primary-content bg-primary' 
                        : 'text-base-content/60 bg-base-300'
                    }`}>{category.count}</span>
                  </button>
                )}
              </For>
            </div>
          </Show>
        }>
          <Show when={folderTree()} fallback={
            <div class="p-4 text-center text-base-content/50 text-xs">
              {(() => {
                console.log('🦀 UI Render - folderTree() is falsy:', folderTree());
                return error() ? error() : 'Loading directory tree...';
              })()}
            </div>
          }>
            <div class="py-1">
              {(() => {
                console.log('🦀 UI Render - folderTree() is truthy:', folderTree(), 'length:', folderTree()?.length);
                return null;
              })()}
              <For each={Array.isArray(folderTree()) ? folderTree() : [folderTree()]}>
                {(node) => (
                  <FileTree 
                    node={node}
                    depth={0}
                    expandedFolders={expandedFolders}
                    currentPath={currentPath}
                    dragOverTreeFolder={dragOverTreeFolder}
                    isInternalDrag={isInternalDrag}
                    viewMode={viewMode}
                    onFolderClick={onFolderClick}
                    onFolderToggle={onFolderToggle}
                    onDragOver={onTreeDragOver}
                    onDragEnter={onTreeDragEnter}
                    onDragLeave={onTreeDragLeave}
                    onDrop={onTreeDrop}
                  />
                )}
              </For>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
}

export default AssetSidebar;