import { Show } from 'solid-js';
import { IconSearch } from '@tabler/icons-solidjs';

function AssetSearch({ searchQuery, setSearchQuery, isSearching }) {
  return (
    <div class="relative">
      <Show when={isSearching()} fallback={
        <IconSearch class="w-3 h-3 absolute left-2 top-1.5 text-base-content/40" />
      }>
        <div class="w-3 h-3 absolute left-2 top-1.5 animate-spin">
          <div class="w-3 h-3 border border-base-content/40 border-t-primary rounded-full"></div>
        </div>
      </Show>
      <input
        type="text"
        placeholder="Search"
        value={searchQuery()}
        onInput={(e) => setSearchQuery(e.target.value)}
        class="w-full pl-6 pr-2 py-1 bg-base-200 border border-base-300 rounded text-xs text-base-content placeholder-base-content/50 focus:outline-none focus:border-primary transition-colors"
      />
    </div>
  );
}

export default AssetSearch;