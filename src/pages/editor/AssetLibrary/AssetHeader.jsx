import { Show } from 'solid-js';
import { IconGrid3x3, IconMenu2, IconPlus } from '@tabler/icons-solidjs';

function AssetHeader({ 
  _selectedAssets, 
  filteredAssets, 
  isUploading, 
  layoutMode, 
  setLayoutMode,
  _onRefresh,
  onImport
}) {
  return (
    <div class="flex items-center gap-3">
          
          
          
          <button
            onClick={onImport}
            class="flex items-center gap-1.5 px-3 py-1 text-xs rounded bg-primary text-primary-content hover:bg-primary/80 transition-colors"
          >
            <IconPlus class="w-3 h-3" />
            <span>Import</span>
          </button>
          
          <div class="flex bg-base-300 rounded overflow-hidden">
            <button
              onClick={() => setLayoutMode('grid')}
              class={`px-2 py-1 text-xs transition-colors ${
                layoutMode() === 'grid'
                  ? 'bg-primary text-primary-content'
                  : 'text-base-content/60 hover:text-base-content hover:bg-base-200'
              }`}
              title="Grid View"
            >
              <IconGrid3x3 class="w-3 h-3" />
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
              <IconMenu2 class="w-3 h-3" />
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