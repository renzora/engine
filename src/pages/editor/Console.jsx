import { createSignal, createEffect, For } from 'solid-js';
import { Terminal, X, Clear, Filter, Search } from '@/ui/icons';

function Console() {
  const [logs, setLogs] = createSignal([
    { id: 1, type: 'info', message: 'Engine initialized successfully', timestamp: Date.now() },
    { id: 2, type: 'warning', message: 'WebGL context created with fallback settings', timestamp: Date.now() - 1000 },
    { id: 3, type: 'error', message: 'Failed to load texture: missing_texture.png', timestamp: Date.now() - 2000 },
  ]);
  const [filter, setFilter] = createSignal('all');
  const [searchTerm, setSearchTerm] = createSignal('');

  const filteredLogs = () => {
    return logs().filter(log => {
      const matchesFilter = filter() === 'all' || log.type === filter();
      const matchesSearch = !searchTerm() || log.message.toLowerCase().includes(searchTerm().toLowerCase());
      return matchesFilter && matchesSearch;
    });
  };

  const clearLogs = () => {
    setLogs([]);
  };

  const getLogIcon = (type) => {
    switch (type) {
      case 'error': return '❌';
      case 'warning': return '⚠️';
      case 'info': return 'ℹ️';
      default: return '📝';
    }
  };

  const getLogColor = (type) => {
    switch (type) {
      case 'error': return 'text-error';
      case 'warning': return 'text-warning';
      case 'info': return 'text-info';
      default: return 'text-base-content';
    }
  };

  const formatTimestamp = (timestamp) => {
    return new Date(timestamp).toLocaleTimeString();
  };

  return (
    <div class="h-full flex flex-col bg-base-100">
      {/* Console Header */}
      <div class="flex items-center justify-between p-3 border-b border-base-300">
        <div class="flex items-center space-x-2">
          <Terminal class="w-4 h-4 text-base-content/60" />
          <span class="text-sm font-medium text-base-content">Console</span>
        </div>
        
        <div class="flex items-center space-x-2">
          {/* Search */}
          <div class="flex items-center space-x-1">
            <Search class="w-3 h-3 text-base-content/40" />
            <input
              type="text"
              placeholder="Search logs..."
              class="input input-xs input-bordered w-32 text-xs"
              value={searchTerm()}
              onInput={(e) => setSearchTerm(e.target.value)}
            />
          </div>
          
          {/* Filter */}
          <select
            class="select select-xs select-bordered text-xs"
            value={filter()}
            onChange={(e) => setFilter(e.target.value)}
          >
            <option value="all">All</option>
            <option value="info">Info</option>
            <option value="warning">Warning</option>
            <option value="error">Error</option>
          </select>
          
          {/* Clear */}
          <button
            class="btn btn-xs btn-ghost"
            onClick={clearLogs}
            title="Clear console"
          >
            <Clear class="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Console Content */}
      <div class="flex-1 overflow-y-auto p-2 font-mono text-xs leading-relaxed">
        <For each={filteredLogs()}>
          {(log) => (
            <div class="flex items-start space-x-2 py-1 hover:bg-base-200/50 px-2 rounded">
              <span class="flex-shrink-0 mt-0.5">
                {getLogIcon(log.type)}
              </span>
              <span class="flex-shrink-0 text-base-content/40 text-[10px] mt-0.5">
                {formatTimestamp(log.timestamp)}
              </span>
              <span class={`flex-1 ${getLogColor(log.type)}`}>
                {log.message}
              </span>
            </div>
          )}
        </For>
        
        {filteredLogs().length === 0 && (
          <div class="text-center text-base-content/40 py-8">
            No logs to display
          </div>
        )}
      </div>
      
      {/* Console Input */}
      <div class="border-t border-base-300 p-2">
        <div class="flex items-center space-x-2">
          <span class="text-base-content/60 text-xs">></span>
          <input
            type="text"
            placeholder="Enter command..."
            class="input input-xs input-ghost flex-1 font-mono"
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                const command = e.target.value.trim();
                if (command) {
                  setLogs(prev => [...prev, {
                    id: Date.now(),
                    type: 'info',
                    message: `> ${command}`,
                    timestamp: Date.now()
                  }]);
                  e.target.value = '';
                }
              }
            }}
          />
        </div>
      </div>
    </div>
  );
}

export default Console;