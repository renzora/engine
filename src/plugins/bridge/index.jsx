import { createPlugin } from '@/api/plugin';
import { createSignal, createEffect, onMount, onCleanup, For, Show, Switch, Match } from 'solid-js';
import { CollapsibleSection } from '@/ui';
import { Settings } from '@/ui/icons';
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
      pluginAPI.hideTabs();
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
    <div className="flex flex-col h-full bg-base-100">
      <div className="flex items-center justify-between p-4 border-b border-base-300">
        <div className="flex items-center gap-3">
          <span className="text-lg font-semibold">🌉 Bridge Management</span>
          <div className={`flex items-center gap-2 px-2 py-1 rounded text-xs ${
            isOnline() ? 'bg-success/20 text-success' : 'bg-error/20 text-error'
          }`}>
            <div className={`w-2 h-2 rounded-full ${isOnline() ? 'bg-success' : 'bg-error'}`} />
            {isOnline() ? 'Online' : 'Offline'}
          </div>
        </div>
        
        <div className="flex gap-2">
          <button
            onClick={checkBridgeStatus}
            className="btn btn-sm btn-outline"
            disabled={isLoading()}
          >
            🔄 Refresh Status
          </button>
        </div>
      </div>

      <div className="flex border-b border-base-300">
        <button
          onClick={() => setActiveTab('cache')}
          className={`px-4 py-2 text-sm transition-colors ${
            activeTab() === 'cache' 
              ? 'bg-primary text-primary-content border-b-2 border-primary' 
              : 'hover:bg-base-200'
          }`}
        >
          📂 Cache Management
        </button>
        <button
          onClick={() => setActiveTab('database')}
          className={`px-4 py-2 text-sm transition-colors ${
            activeTab() === 'database' 
              ? 'bg-primary text-primary-content border-b-2 border-primary' 
              : 'hover:bg-base-200'
          }`}
        >
          🗄️ Database
        </button>
        <button
          onClick={() => setActiveTab('logs')}
          className={`px-4 py-2 text-sm transition-colors ${
            activeTab() === 'logs' 
              ? 'bg-primary text-primary-content border-b-2 border-primary' 
              : 'hover:bg-base-200'
          }`}
        >
          📜 Logs
        </button>
      </div>

      <div className="flex-1 overflow-hidden">
        <Switch>
          <Match when={activeTab() === 'cache'}>
            <div className="p-4 h-full overflow-y-auto">
              <CollapsibleSection title="Cache Statistics" defaultOpen={true}>
                <div className="grid grid-cols-2 gap-4 p-4">
                  <div className="stat bg-base-200 rounded">
                    <div className="stat-title">Cache Size</div>
                    <div className="stat-value text-lg">{bridgeStatus().cache_size || 0}</div>
                    <div className="stat-desc">bytes</div>
                  </div>
                  <div className="stat bg-base-200 rounded">
                    <div className="stat-title">Thumbnails</div>
                    <div className="stat-value text-lg">{bridgeStatus().thumbnail_count || 0}</div>
                    <div className="stat-desc">cached</div>
                  </div>
                </div>
                <div className="flex gap-2 p-4">
                  <button
                    onClick={clearCache}
                    className="btn btn-warning btn-sm"
                    disabled={isLoading()}
                  >
                    {isLoading() ? 'Clearing...' : '🗑️ Clear Cache'}
                  </button>
                  <button
                    onClick={refreshRenScriptCache}
                    className="btn btn-primary btn-sm"
                    disabled={isLoading()}
                  >
                    {isLoading() ? 'Refreshing...' : '🔄 Refresh RenScript Cache'}
                  </button>
                </div>
              </CollapsibleSection>

              <CollapsibleSection title="RenScript Search" defaultOpen={true}>
                <div className="p-4">
                  <div className="flex gap-2 mb-4">
                    <input
                      type="text"
                      placeholder="Search renscripts..."
                      value={searchTerm()}
                      onInput={(e) => setSearchTerm(e.target.value)}
                      className="input input-bordered flex-1"
                    />
                    <div className="badge badge-outline">{allScripts().length} total</div>
                  </div>
                  
                  <div className="space-y-2 max-h-64 overflow-y-auto">
                    <Show when={searchTerm().length > 0} fallback={
                      <For each={allScripts().slice(0, 20)}>
                        {(script) => (
                          <div className="p-2 bg-base-200 rounded flex justify-between items-center">
                            <div>
                              <div className="font-medium">{script.name}</div>
                              <div className="text-xs text-base-content/60">{script.full_path}</div>
                            </div>
                          </div>
                        )}
                      </For>
                    }>
                      <For each={searchResults()}>
                        {(script) => (
                          <div className="p-2 bg-primary/10 rounded flex justify-between items-center">
                            <div>
                              <div className="font-medium">{script.name}</div>
                              <div className="text-xs text-base-content/60">{script.full_path}</div>
                            </div>
                            <div className="badge badge-primary badge-sm">{script.directory}</div>
                          </div>
                        )}
                      </For>
                    </Show>
                    
                    <Show when={searchTerm().length > 0 && searchResults().length === 0}>
                      <div className="text-center text-base-content/50 py-4">
                        No scripts found for "{searchTerm()}"
                      </div>
                    </Show>
                  </div>
                </div>
              </CollapsibleSection>
            </div>
          </Match>

          <Match when={activeTab() === 'database'}>
            <div className="p-4 h-full overflow-y-auto">
              <CollapsibleSection title="SQL Query Interface" defaultOpen={true}>
                <div className="p-4">
                  <div className="mb-4">
                    <label className="label">
                      <span className="label-text">SQL Query</span>
                    </label>
                    <textarea
                      value={sqlQuery()}
                      onInput={(e) => setSqlQuery(e.target.value)}
                      className="textarea textarea-bordered w-full h-24 font-mono text-sm"
                      placeholder="Enter SQL query..."
                    />
                  </div>
                  
                  <div className="flex gap-2 mb-4">
                    <button
                      onClick={executeQuery}
                      className="btn btn-primary btn-sm"
                      disabled={isLoading() || !isOnline()}
                    >
                      {isLoading() ? 'Executing...' : '▶️ Execute Query'}
                    </button>
                    <button
                      onClick={() => setSqlQuery('SELECT * FROM scripts LIMIT 10;')}
                      className="btn btn-ghost btn-sm"
                    >
                      📋 Sample Query
                    </button>
                  </div>

                  <Show when={queryResults()}>
                    <div className="bg-base-200 rounded p-4">
                      <h4 className="font-medium mb-2">Query Results:</h4>
                      <pre className="text-xs overflow-auto max-h-64 bg-base-300 p-2 rounded">
                        {JSON.stringify(queryResults(), null, 2)}
                      </pre>
                    </div>
                  </Show>
                </div>
              </CollapsibleSection>
            </div>
          </Match>

          <Match when={activeTab() === 'logs'}>
            <div className="p-4 h-full overflow-y-auto">
              <CollapsibleSection title="Bridge Console Logs" defaultOpen={true}>
                <div className="p-4">
                  <div className="flex gap-2 mb-4">
                    <button
                      onClick={() => setLogs([])}
                      className="btn btn-ghost btn-sm"
                    >
                      🗑️ Clear Logs
                    </button>
                    <div className="badge badge-outline">{logs().length} entries</div>
                  </div>
                  
                  <div className="bg-base-300 rounded p-4 h-64 overflow-y-auto font-mono text-xs">
                    <Show when={logs().length === 0}>
                      <div className="text-base-content/50 text-center py-8">
                        No logs available. Bridge logs would appear here when integrated.
                      </div>
                    </Show>
                    <For each={logs()}>
                      {(log) => (
                        <div className={`mb-1 ${
                          log.level === 'error' ? 'text-error' :
                          log.level === 'warn' ? 'text-warning' :
                          log.level === 'info' ? 'text-info' : ''
                        }`}>
                          <span className="text-base-content/60">[{log.timestamp}]</span> {log.message}
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </CollapsibleSection>
            </div>
          </Match>
        </Switch>
      </div>

      <Show when={isOnline()}>
        <div className="p-2 border-t border-base-300 bg-base-200/50">
          <div className="flex justify-between items-center text-xs text-base-content/60">
            <span>Uptime: {Math.floor((bridgeStatus().uptime || 0) / 1000)}s</span>
            <span>Watched files: {bridgeStatus().watched_files || 0}</span>
            <span>Port: 3001</span>
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
      icon: Settings,
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