import { createPlugin } from '@/api/plugin';
import { createSignal, createEffect, onMount, onCleanup, For, Show, Switch, Match } from 'solid-js';
import { CollapsibleSection } from '@/ui';
import { IconSettings } from '@tabler/icons-solidjs';
import { viewportStore } from '@/layout/stores/ViewportStore.jsx';
import { editorActions } from '@/layout/stores/EditorStore.jsx';
import { pluginAPI } from '@/api/plugin';
import BridgeFooterButton from './BridgeFooterButton.jsx';

function BridgeViewport({ tab }) {
  const [activeTab, setActiveTab] = createSignal('cache');
  const [isOnline, setIsOnline] = createSignal(false);
  const [bridgeStatus, setBridgeStatus] = createSignal({});
  const [cacheStats, setCacheStats] = createSignal({});
  const [searchTerm, setSearchTerm] = createSignal('');
  const [searchResults, setSearchResults] = createSignal([]);
  const [allScripts, setAllScripts] = createSignal([]);
  const [sqlQuery, setSqlQuery] = createSignal('SELECT * FROM scripts LIMIT 10;');
  const [queryResults, setQueryResults] = createSignal(null);
  const [logs, setLogs] = createSignal([]);
  const [isLoading, setIsLoading] = createSignal(false);

  let statusInterval;

  const checkBridgeStatus = async () => {
    try {
      const response = await fetch('http://localhost:3001/health');
      if (response.ok) {
        const status = await response.json();
        setBridgeStatus(status);
        setIsOnline(true);
      } else {
        setIsOnline(false);
      }
    } catch (error) {
      setIsOnline(false);
    }
  };

  const loadCacheStats = async () => {
    try {
      const response = await fetch('http://localhost:3001/scripts/cache/stats');
      if (response.ok) {
        const stats = await response.json();
        setCacheStats(stats);
      }
    } catch (error) {
      console.warn('Failed to load cache stats:', error);
    }
  };

  const loadAllScripts = async () => {
    try {
      const response = await fetch('http://localhost:3001/renscripts');
      if (response.ok) {
        const scripts = await response.json();
        setAllScripts(scripts);
      }
    } catch (error) {
      console.warn('Failed to load scripts:', error);
    }
  };

  const searchScripts = async (term) => {
    if (!term.trim()) {
      setSearchResults([]);
      return;
    }
    
    try {
      const response = await fetch(`http://localhost:3001/renscripts/search?q=${encodeURIComponent(term)}`);
      if (response.ok) {
        const results = await response.json();
        setSearchResults(results);
      }
    } catch (error) {
      console.warn('Failed to search scripts:', error);
    }
  };

  const clearCache = async () => {
    setIsLoading(true);
    try {
      const response = await fetch('http://localhost:3001/scripts/cache/clear', {
        method: 'POST'
      });
      if (response.ok) {
        console.log('Cache cleared successfully');
        await loadCacheStats();
      } else {
        console.error('Failed to clear cache');
      }
    } catch (error) {
      console.error('Error clearing cache:', error.message);
    } finally {
      setIsLoading(false);
    }
  };

  const refreshRenScriptCache = async () => {
    setIsLoading(true);
    try {
      const response = await fetch('http://localhost:3001/renscripts/cache/refresh', {
        method: 'POST'
      });
      if (response.ok) {
        console.log('RenScript cache refreshed successfully');
        await loadAllScripts();
      } else {
        console.error('Failed to refresh RenScript cache');
      }
    } catch (error) {
      console.error('Error refreshing cache:', error.message);
    } finally {
      setIsLoading(false);
    }
  };

  const executeQuery = async () => {
    const query = sqlQuery().trim();
    if (!query) return;

    setIsLoading(true);
    try {
      const response = await fetch('http://localhost:3001/database/query', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query })
      });
      
      if (response.ok) {
        const results = await response.json();
        setQueryResults(results);
        console.log('Query executed successfully');
      } else {
        const error = await response.text();
        console.error('Query failed:', error);
        setQueryResults({ error });
      }
    } catch (error) {
      console.error('Query error:', error.message);
      setQueryResults({ error: error.message });
    } finally {
      setIsLoading(false);
    }
  };

  createEffect(() => {
    const term = searchTerm();
    const timeoutId = setTimeout(() => {
      searchScripts(term);
    }, 300);
    
    onCleanup(() => clearTimeout(timeoutId));
  });

  // Hide UI elements when this bridge management viewport is active
  createEffect(() => {
    const activeTabId = viewportStore.activeTabId;
    const isThisTabActive = activeTabId === tab?.id;
    
    if (isThisTabActive) {
      pluginAPI.hidePanel();
      pluginAPI.hideProps();
      pluginAPI.hideMenu();
      pluginAPI.showTabs();
    }
  });

  onMount(() => {
    checkBridgeStatus();
    loadCacheStats();
    loadAllScripts();
    
    statusInterval = setInterval(checkBridgeStatus, 10000);
  });

  onCleanup(() => {
    if (statusInterval) {
      clearInterval(statusInterval);
    }
  });

  return (
    <div className="flex flex-col h-full bg-gradient-to-br from-base-100 to-base-200/50">
      {/* Header */}
      <div className="flex items-center justify-between p-6 border-b border-base-300/50 bg-base-100/80 backdrop-blur-sm">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-gradient-to-br from-primary/20 to-secondary/20 rounded-xl border border-primary/30">
              <IconSettings className="w-5 h-5 text-primary" />
            </div>
            <div>
              <h1 className="text-xl font-bold text-base-content">Bridge Management</h1>
              <p className="text-sm text-base-content/60">Server monitoring and cache control</p>
            </div>
          </div>
          
          <div className={`flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium transition-all ${
            isOnline() 
              ? 'bg-success/10 text-success border border-success/30' 
              : 'bg-error/10 text-error border border-error/30'
          }`}>
            <div className={`w-2 h-2 rounded-full ${isOnline() ? 'bg-success animate-pulse' : 'bg-error'}`} />
            {isOnline() ? 'Connected' : 'Disconnected'}
          </div>
        </div>
        
        <button
          onClick={checkBridgeStatus}
          className="btn btn-sm btn-primary btn-outline gap-2"
          disabled={isLoading()}
        >
          <IconSettings className="w-4 h-4" />
          {isLoading() ? 'Refreshing...' : 'Refresh'}
        </button>
      </div>

      {/* Tab Navigation */}
      <div className="flex bg-base-100/60 border-b border-base-300/30">
        <button
          onClick={() => setActiveTab('cache')}
          className={`px-6 py-3 text-sm font-medium transition-all relative ${
            activeTab() === 'cache' 
              ? 'text-primary bg-primary/5 border-b-2 border-primary' 
              : 'text-base-content/60 hover:text-base-content hover:bg-base-100/50'
          }`}
        >
          Cache & Search
        </button>
        <button
          onClick={() => setActiveTab('database')}
          className={`px-6 py-3 text-sm font-medium transition-all relative ${
            activeTab() === 'database' 
              ? 'text-primary bg-primary/5 border-b-2 border-primary' 
              : 'text-base-content/60 hover:text-base-content hover:bg-base-100/50'
          }`}
        >
          Database
        </button>
        <button
          onClick={() => setActiveTab('logs')}
          className={`px-6 py-3 text-sm font-medium transition-all relative ${
            activeTab() === 'logs' 
              ? 'text-primary bg-primary/5 border-b-2 border-primary' 
              : 'text-base-content/60 hover:text-base-content hover:bg-base-100/50'
          }`}
        >
          System Logs
        </button>
      </div>

      <div className="flex-1 overflow-hidden">
        <Switch>
          <Match when={activeTab() === 'cache'}>
            <div className="p-6 h-full overflow-y-auto space-y-6">
              {/* Stats Cards */}
              <div className="grid grid-cols-3 gap-4">
                <div className="bg-gradient-to-br from-base-100 to-base-200 p-4 rounded-xl border border-base-300/50 shadow-sm">
                  <div className="flex items-center gap-3 mb-2">
                    <div className="p-2 bg-primary/10 rounded-lg">
                      <IconSettings className="w-4 h-4 text-primary" />
                    </div>
                    <div>
                      <div className="text-xs text-base-content/60 uppercase tracking-wide">Cache Size</div>
                      <div className="text-lg font-bold text-base-content">{((bridgeStatus().cache_size || 0) / 1024).toFixed(1)} KB</div>
                    </div>
                  </div>
                </div>
                
                <div className="bg-gradient-to-br from-base-100 to-base-200 p-4 rounded-xl border border-base-300/50 shadow-sm">
                  <div className="flex items-center gap-3 mb-2">
                    <div className="p-2 bg-secondary/10 rounded-lg">
                      <IconSettings className="w-4 h-4 text-secondary" />
                    </div>
                    <div>
                      <div className="text-xs text-base-content/60 uppercase tracking-wide">Thumbnails</div>
                      <div className="text-lg font-bold text-base-content">{bridgeStatus().thumbnail_count || 0}</div>
                    </div>
                  </div>
                </div>
                
                <div className="bg-gradient-to-br from-base-100 to-base-200 p-4 rounded-xl border border-base-300/50 shadow-sm">
                  <div className="flex items-center gap-3 mb-2">
                    <div className="p-2 bg-accent/10 rounded-lg">
                      <IconSettings className="w-4 h-4 text-accent" />
                    </div>
                    <div>
                      <div className="text-xs text-base-content/60 uppercase tracking-wide">Scripts</div>
                      <div className="text-lg font-bold text-base-content">{allScripts().length}</div>
                    </div>
                  </div>
                </div>
              </div>

              {/* Actions */}
              <div className="bg-base-100 p-4 rounded-xl border border-base-300/50 shadow-sm">
                <h3 className="text-sm font-semibold text-base-content mb-3">Cache Operations</h3>
                <div className="flex gap-3">
                  <button
                    onClick={clearCache}
                    className="btn btn-warning btn-sm gap-2"
                    disabled={isLoading()}
                  >
                    <IconSettings className="w-4 h-4" />
                    {isLoading() ? 'Clearing...' : 'Clear Cache'}
                  </button>
                  <button
                    onClick={refreshRenScriptCache}
                    className="btn btn-primary btn-sm gap-2"
                    disabled={isLoading()}
                  >
                    <IconSettings className="w-4 h-4" />
                    {isLoading() ? 'Refreshing...' : 'Refresh Scripts'}
                  </button>
                </div>
              </div>

              {/* Search Interface */}
              <div className="bg-base-100 p-4 rounded-xl border border-base-300/50 shadow-sm">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-sm font-semibold text-base-content">RenScript Search</h3>
                  <div className="badge badge-primary badge-sm">{allScripts().length} total</div>
                </div>
                
                <div className="mb-4">
                  <input
                    type="text"
                    placeholder="Search scripts by name or path..."
                    value={searchTerm()}
                    onInput={(e) => setSearchTerm(e.target.value)}
                    className="input input-bordered w-full"
                  />
                </div>
                
                <div className="space-y-2 max-h-80 overflow-y-auto">
                  <Show when={searchTerm().length > 0} fallback={
                    <For each={allScripts().slice(0, 15)}>
                      {(script) => (
                        <div className="p-3 bg-base-100/30 rounded-lg border border-base-300/30 hover:bg-base-100/50 transition-colors">
                          <div className="font-medium text-sm text-base-content">{script.name}</div>
                          <div className="text-xs text-base-content/60 mt-1">{script.full_path}</div>
                        </div>
                      )}
                    </For>
                  }>
                    <For each={searchResults()}>
                      {(script) => (
                        <div className="p-3 bg-primary/5 rounded-lg border border-primary/20 hover:bg-primary/10 transition-colors">
                          <div className="flex justify-between items-start">
                            <div className="flex-1">
                              <div className="font-medium text-sm text-base-content">{script.name}</div>
                              <div className="text-xs text-base-content/60 mt-1">{script.full_path}</div>
                            </div>
                            <div className="badge badge-primary badge-xs ml-2">{script.directory}</div>
                          </div>
                        </div>
                      )}
                    </For>
                  </Show>
                  
                  <Show when={searchTerm().length > 0 && searchResults().length === 0}>
                    <div className="text-center text-base-content/50 py-8">
                      <div className="text-base-content/40 mb-2">No results found</div>
                      <div className="text-sm">Try searching for "{searchTerm()}" with different terms</div>
                    </div>
                  </Show>
                </div>
              </div>
            </div>
          </Match>

          <Match when={activeTab() === 'database'}>
            <div className="p-6 h-full overflow-y-auto space-y-6">
              {/* Query Interface */}
              <div className="bg-base-100 p-6 rounded-xl border border-base-300/50 shadow-sm">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-semibold text-base-content">SQL Query Interface</h3>
                  <div className={`badge ${isOnline() ? 'badge-success' : 'badge-error'} badge-sm`}>
                    {isOnline() ? 'Database Online' : 'Database Offline'}
                  </div>
                </div>
                
                <div className="space-y-4">
                  <div>
                    <label className="text-sm font-medium text-base-content/80 mb-2 block">
                      SQL Query
                    </label>
                    <textarea
                      value={sqlQuery()}
                      onInput={(e) => setSqlQuery(e.target.value)}
                      className="textarea textarea-bordered w-full h-32 font-mono text-sm bg-base-100/30"
                      placeholder="SELECT * FROM scripts WHERE name LIKE '%camera%';"
                    />
                  </div>
                  
                  <div className="flex gap-3">
                    <button
                      onClick={executeQuery}
                      className="btn btn-primary gap-2"
                      disabled={isLoading() || !isOnline()}
                    >
                      <IconSettings className="w-4 h-4" />
                      {isLoading() ? 'Executing...' : 'Execute Query'}
                    </button>
                    <button
                      onClick={() => setSqlQuery('SELECT * FROM scripts LIMIT 10;')}
                      className="btn btn-ghost gap-2"
                    >
                      <IconSettings className="w-4 h-4" />
                      Sample Query
                    </button>
                  </div>
                </div>
              </div>

              {/* Results */}
              <Show when={queryResults()}>
                <div className="bg-base-100 p-6 rounded-xl border border-base-300/50 shadow-sm">
                  <h4 className="text-lg font-semibold text-base-content mb-4">Query Results</h4>
                  <div className="bg-base-100/30 rounded-lg p-4 max-h-96 overflow-auto">
                    <pre className="text-xs font-mono text-base-content whitespace-pre-wrap">
                      {JSON.stringify(queryResults(), null, 2)}
                    </pre>
                  </div>
                </div>
              </Show>
            </div>
          </Match>

          <Match when={activeTab() === 'logs'}>
            <div className="p-6 h-full overflow-y-auto">
              <div className="bg-base-100 p-6 rounded-xl border border-base-300/50 shadow-sm h-full flex flex-col">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-semibold text-base-content">System Logs</h3>
                  <div className="flex items-center gap-3">
                    <div className="badge badge-outline">{logs().length} entries</div>
                    <button
                      onClick={() => setLogs([])}
                      className="btn btn-ghost btn-sm gap-2"
                    >
                      <IconSettings className="w-4 h-4" />
                      Clear Logs
                    </button>
                  </div>
                </div>
                
                <div className="flex-1 bg-base-300/30 rounded-lg border border-base-300/50 overflow-hidden">
                  <div className="h-full overflow-y-auto p-4 font-mono text-sm">
                    <Show when={logs().length === 0}>
                      <div className="text-center text-base-content/50 py-12">
                        <div className="text-base-content/40 mb-2">No logs available</div>
                        <div className="text-xs">Bridge logs will appear here when integrated</div>
                      </div>
                    </Show>
                    <For each={logs()}>
                      {(log) => (
                        <div className={`mb-2 p-2 rounded ${
                          log.level === 'error' ? 'bg-error/10 text-error' :
                          log.level === 'warn' ? 'bg-warning/10 text-warning' :
                          log.level === 'info' ? 'bg-info/10 text-info' : 'bg-base-100/30'
                        }`}>
                          <span className="text-base-content/60 text-xs">[{log.timestamp}]</span>
                          <span className="ml-2">{log.message}</span>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </div>
            </div>
          </Match>
        </Switch>
      </div>

      <Show when={isOnline()}>
        <div className="p-4 border-t border-base-300/30 bg-base-100/80 backdrop-blur-sm">
          <div className="flex justify-between items-center text-sm">
            <div className="flex items-center gap-4 text-base-content/60">
              <span className="flex items-center gap-2">
                <div className="w-2 h-2 bg-success rounded-full animate-pulse" />
                Uptime: {Math.floor((bridgeStatus().uptime || 0) / 1000)}s
              </span>
              <span>Files: {bridgeStatus().watched_files || 0}</span>
              <span className="font-mono">:3001</span>
            </div>
            <div className="text-xs text-base-content/40">
              Bridge Server v{bridgeStatus().version || '1.0.0'}
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}

export default createPlugin({
  id: 'bridge-plugin',
  name: 'Bridge Management Plugin',
  version: '1.0.0',
  description: 'Bridge server management and monitoring tools',
  author: 'Renzora Engine Team',

  async onInit(api) {
    console.log('[BridgeManagementPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[BridgeManagementPlugin] Starting...');
    
    console.log('🌉 BridgeViewport component type:', typeof BridgeViewport);
    console.log('🌉 BridgeViewport component value:', BridgeViewport);
    
    // Register the viewport type for bridge management
    api.viewport('bridge-mgmt', {
      label: 'Bridge Management',
      component: BridgeViewport,
      icon: IconSettings,
      description: 'Bridge server management and monitoring'
    });

    // Register footer button with online status
    api.footer('bridge-status', {
      component: BridgeFooterButton,
      priority: 100
    });
    
    console.log('[BridgeManagementPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[BridgeManagementPlugin] Stopping...');
  },

  async onDispose() {
    console.log('[BridgeManagementPlugin] Disposing...');
  }
});