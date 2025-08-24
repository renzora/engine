import { Show } from 'solid-js';
import { Grid, Menu, Refresh, CodeSlash, Plus } from '@/ui/icons';

function AssetHeader({ 
  selectedAssets, 
  filteredAssets, 
  isUploading, 
  layoutMode, 
  setLayoutMode,
  onRefresh,
  onCodeToggle,
  isCodeEditorOpen = false,
  onImport
}) {
  return (
    <div class="flex items-center gap-3">
          <Show when={selectedAssets().size > 0}>
            <span class="text-xs text-primary font-medium bg-primary/20 px-2 py-1 rounded">
              {selectedAssets().size} selected
            </span>
          </Show>
          <span class="text-xs text-base-content/60">{filteredAssets().length} items</span>
          
          <Show when={isUploading()}>
            <div class="flex items-center gap-2 transition-all duration-300 opacity-100">
              <div class="w-20 h-1.5 bg-base-300 rounded-full overflow-hidden">
                <div class="h-full bg-primary rounded-full animate-pulse" style={{ width: '100%' }} />
              </div>
              <span class="text-xs text-base-content/60">Uploading...</span>
            </div>
          </Show>
          
          
          <button
            onClick={onImport}
            class="flex items-center gap-1.5 px-3 py-1 text-xs rounded bg-primary text-primary-content hover:bg-primary/80 transition-colors"
          >
            <Plus class="w-3 h-3" />
            <span>Import</span>
          </button>
          
          <div class="flex bg-base-300 rounded overflow-hidden">
            <button
              onClick={onCodeToggle}
              class={`px-2 py-1 text-xs transition-colors ${
                isCodeEditorOpen()
                  ? 'bg-primary text-primary-content'
                  : 'text-base-content/60 hover:text-base-content hover:bg-base-200'
              }`}
              title="Toggle Code Editor"
            >
              <CodeSlash class="w-3 h-3" />
            </button>
            <button
              onClick={() => setLayoutMode('grid')}
              class={`px-2 py-1 text-xs transition-colors ${
                layoutMode() === 'grid'
                  ? 'bg-primary text-primary-content'
                  : 'text-base-content/60 hover:text-base-content hover:bg-base-200'
              }`}
              title="Grid View"
            >
              <Grid class="w-3 h-3" />
            </button>
            <button
              onClick={() => setLayoutMode('list')}
              class={`px-2 py-1 text-xs transition-colors ${
                layoutMode() === 'list'
                  ? 'bg-primary text-primary-content'
                  : 'text-base-content/60 hover:text-base-content hover:bg-base-200'
              }`}
              title="List View"
            >
              <Menu class="w-3 h-3" />
            </button>
          </div>
          
          <Show when={isUploading()} fallback={
            <Show when={filteredAssets().length > 0}>
            </Show>
          }>
            <div class="flex items-center gap-1.5 text-primary/80 bg-primary/10 px-2 py-1 rounded-md border border-primary/20">
              <div class="w-2 h-2 bg-primary rounded-full animate-spin" />
              <span class="text-xs font-medium">Uploading...</span>
            </div>
          </Show>
    </div>
  );
}

export default AssetHeader;