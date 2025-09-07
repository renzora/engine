import { createSignal, createEffect, Show, For } from 'solid-js';
import { getWebSocketClient } from '@/api/websocket/WebSocketClient.jsx';

export default function ServerConfigDialog(props) {
  const [config, setConfig] = createSignal(null);
  const [foundPaths, setFoundPaths] = createSignal([]);
  const [loading, setLoading] = createSignal(false);
  const [customPath, setCustomPath] = createSignal('');
  const [customProjectsPath, setCustomProjectsPath] = createSignal('');
  const [scanning, setScanning] = createSignal(false);

  const client = getWebSocketClient();

  // Load current configuration
  createEffect(async () => {
    if (props.open) {
      setLoading(true);
      try {
        const response = await client.getServerConfig();
        if (response.data && !response.data.error) {
          setConfig(response.data.config);
          setCustomPath(response.data.config.base_path);
          setCustomProjectsPath(response.data.config.projects_path);
        }
      } catch (error) {
        console.error('Failed to load server config:', error);
      } finally {
        setLoading(false);
      }
    }
  });

  const scanForEngines = async () => {
    setScanning(true);
    try {
      const response = await client.scanForEngineRoots();
      if (response.data && !response.data.error) {
        setFoundPaths(response.data.found_paths || []);
      }
    } catch (error) {
      console.error('Failed to scan for engines:', error);
    } finally {
      setScanning(false);
    }
  };

  const setBasePath = async (path) => {
    setLoading(true);
    try {
      const response = await client.setBasePath(path);
      if (response.data && response.data.success) {
        // Refresh config
        const configResponse = await client.getServerConfig();
        if (configResponse.data && !configResponse.data.error) {
          setConfig(configResponse.data.config);
        }
        console.log('✅ Base path updated successfully');
      } else {
        console.error('❌ Failed to update base path:', response.data?.error);
      }
    } catch (error) {
      console.error('Failed to update base path:', error);
    } finally {
      setLoading(false);
    }
  };

  const setProjectsPath = async (path) => {
    setLoading(true);
    try {
      const response = await client.setProjectsPath(path);
      if (response.data && response.data.success) {
        // Refresh config
        const configResponse = await client.getServerConfig();
        if (configResponse.data && !configResponse.data.error) {
          setConfig(configResponse.data.config);
        }
        console.log('✅ Projects path updated successfully');
      } else {
        console.error('❌ Failed to update projects path:', response.data?.error);
      }
    } catch (error) {
      console.error('Failed to update projects path:', error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Show when={props.open}>
      <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
        <div class="bg-base-100 rounded-lg shadow-xl max-w-2xl w-full mx-4 max-h-[90vh] overflow-y-auto">
          <div class="p-6">
            <div class="flex justify-between items-center mb-6">
              <h2 class="text-2xl font-bold">🔧 Server Configuration</h2>
              <button 
                class="btn btn-sm btn-circle btn-ghost"
                onClick={() => props.onClose?.()}
              >
                ✕
              </button>
            </div>

            <Show when={loading()}>
              <div class="flex items-center gap-2 mb-4">
                <span class="loading loading-spinner loading-sm"></span>
                <span>Loading configuration...</span>
              </div>
            </Show>

            <Show when={config() && !loading()}>
              <div class="space-y-6">
                {/* Current Configuration */}
                <div class="card bg-base-200">
                  <div class="card-body">
                    <h3 class="card-title">📋 Current Configuration</h3>
                    <div class="space-y-2 text-sm">
                      <div><strong>Server:</strong> {config().host}:{config().port}</div>
                      <div><strong>Version:</strong> {config().version}</div>
                      <div><strong>Base Path:</strong> {config().base_path}</div>
                      <div><strong>Projects Path:</strong> {config().projects_path}</div>
                    </div>
                  </div>
                </div>

                {/* Scan for Engine Roots */}
                <div class="card bg-base-200">
                  <div class="card-body">
                    <h3 class="card-title">🔍 Auto-Detect Engine Location</h3>
                    <p class="text-sm opacity-70">
                      Scan your system for Renzora engine installations
                    </p>
                    
                    <div class="card-actions">
                      <button 
                        class="btn btn-primary btn-sm"
                        classList={{ 'loading': scanning() }}
                        onClick={scanForEngines}
                        disabled={scanning()}
                      >
                        {scanning() ? 'Scanning...' : '🔍 Scan System'}
                      </button>
                    </div>

                    <Show when={foundPaths().length > 0}>
                      <div class="mt-4">
                        <h4 class="font-semibold mb-2">Found Engine Locations:</h4>
                        <div class="space-y-2">
                          <For each={foundPaths()}>
                            {(path) => (
                              <div class="flex justify-between items-center bg-base-100 p-3 rounded">
                                <span class="font-mono text-sm">{path}</span>
                                <button 
                                  class="btn btn-xs btn-primary"
                                  onClick={() => setBasePath(path)}
                                >
                                  Use This
                                </button>
                              </div>
                            )}
                          </For>
                        </div>
                      </div>
                    </Show>
                  </div>
                </div>

                {/* Manual Configuration */}
                <div class="card bg-base-200">
                  <div class="card-body">
                    <h3 class="card-title">⚙️ Manual Configuration</h3>
                    
                    <div class="space-y-4">
                      <div class="form-control">
                        <label class="label">
                          <span class="label-text">Engine Base Path</span>
                        </label>
                        <div class="join">
                          <input
                            type="text"
                            class="input input-bordered join-item flex-1"
                            placeholder="C:\path\to\engine"
                            value={customPath()}
                            onInput={(e) => setCustomPath(e.target.value)}
                          />
                          <button 
                            class="btn btn-primary join-item"
                            onClick={() => setBasePath(customPath())}
                            disabled={!customPath() || loading()}
                          >
                            Set
                          </button>
                        </div>
                        <label class="label">
                          <span class="label-text-alt">Path to the main engine directory (contains src/, package.json)</span>
                        </label>
                      </div>

                      <div class="form-control">
                        <label class="label">
                          <span class="label-text">Projects Directory</span>
                        </label>
                        <div class="join">
                          <input
                            type="text"
                            class="input input-bordered join-item flex-1"
                            placeholder="C:\path\to\projects"
                            value={customProjectsPath()}
                            onInput={(e) => setCustomProjectsPath(e.target.value)}
                          />
                          <button 
                            class="btn btn-primary join-item"
                            onClick={() => setProjectsPath(customProjectsPath())}
                            disabled={!customProjectsPath() || loading()}
                          >
                            Set
                          </button>
                        </div>
                        <label class="label">
                          <span class="label-text-alt">Directory where projects are stored</span>
                        </label>
                      </div>
                    </div>
                  </div>
                </div>

                {/* Actions */}
                <div class="flex gap-2 justify-end">
                  <button 
                    class="btn btn-ghost"
                    onClick={() => props.onClose?.()}
                  >
                    Close
                  </button>
                  <button 
                    class="btn btn-primary"
                    onClick={() => {
                      console.log('🔄 Configuration updated');
                      props.onClose?.();
                    }}
                  >
                    Apply & Close
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </div>
    </Show>
  );
}