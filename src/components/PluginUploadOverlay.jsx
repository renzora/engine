import { createSignal, Show, For, onMount, createEffect } from 'solid-js';
import { usePluginAPI } from '@/api/plugin/index.jsx';
import pluginStore, { PLUGIN_STATES } from '@/stores/PluginStore.jsx';

export default function PluginUploadOverlay(props) {
  // Use external isOpen prop if provided, otherwise use internal state
  const [internalIsOpen, setInternalIsOpen] = createSignal(false);
  const isOpen = () => props.isOpen !== undefined ? props.isOpen : internalIsOpen();
  const setIsOpen = (value) => {
    if (props.onClose && !value) {
      props.onClose();
    } else if (props.isOpen === undefined) {
      setInternalIsOpen(value);
    }
  };
  const [isUploading, setIsUploading] = createSignal(false);
  const [uploadProgress, setUploadProgress] = createSignal(0);
  const [uploadStatus, setUploadStatus] = createSignal('');
  const [dragActive, setDragActive] = createSignal(false);
  const [currentView, setCurrentView] = createSignal('list'); // 'list' or 'upload'
  
  const pluginAPI = usePluginAPI();

  // Get reactive plugin list from store
  const pluginList = () => pluginStore.getAllPlugins();

  const togglePlugin = async (pluginId) => {
    const plugin = pluginStore.getPluginConfig(pluginId);
    if (!plugin) return;
    
    const newEnabledState = !plugin.enabled;
    
    try {
      if (newEnabledState) {
        // Enable plugin - the store will handle dynamic loading
        await pluginStore.setPluginEnabled(pluginId, true);
        
        // Load the plugin at runtime
        try {
          await pluginAPI.getPluginLoader().loadSinglePlugin(pluginId, plugin.path, plugin.main);
          pluginStore.setPluginState(pluginId, PLUGIN_STATES.RUNNING);
        } catch (error) {
          console.error(`Failed to load plugin ${pluginId}:`, error);
          pluginStore.setPluginError(pluginId, error);
          pluginStore.setPluginState(pluginId, PLUGIN_STATES.ERROR);
        }
      } else {
        // Disable plugin - the store will handle unloading
        await pluginStore.setPluginEnabled(pluginId, false);
      }
    } catch (error) {
      console.error(`Failed to toggle plugin ${pluginId}:`, error);
    }
  };

  const getStateColor = (state) => {
    switch (state) {
      case PLUGIN_STATES.RUNNING: return 'text-success';
      case PLUGIN_STATES.ERROR: return 'text-error';
      case PLUGIN_STATES.LOADING: return 'text-info';
      case PLUGIN_STATES.DISABLED: return 'text-base-content opacity-50';
      default: return 'text-warning';
    }
  };

  const getStateIcon = (state) => {
    switch (state) {
      case PLUGIN_STATES.RUNNING: return '✅';
      case PLUGIN_STATES.ERROR: return '❌';
      case PLUGIN_STATES.LOADING: return '⏳';
      case PLUGIN_STATES.DISABLED: return '⏸️';
      default: return '⏳';
    }
  };

  const fileToBase64 = (file) => {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.readAsDataURL(file);
      reader.onload = () => {
        // Remove the data:application/zip;base64, prefix
        const base64 = reader.result.split(',')[1];
        resolve(base64);
      };
      reader.onerror = error => reject(error);
    });
  };

  const handleFileSelect = async (file) => {
    if (!file || !file.name.endsWith('.zip')) {
      setUploadStatus('Please select a valid ZIP file');
      return;
    }

    await uploadPlugin(file);
  };

  const uploadPlugin = async (file) => {
    setIsUploading(true);
    setUploadProgress(0);
    setUploadStatus('Uploading plugin...');

    try {
      // Convert file to base64
      const base64Data = await fileToBase64(file);
      
      setUploadProgress(25);
      setUploadStatus('Converting plugin data...');

      // Create request body with base64 encoded ZIP
      const requestBody = {
        plugin: base64Data
      };

      // Upload to bridge backend
      const response = await fetch('http://localhost:3001/api/plugins/install', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(requestBody)
      });

      setUploadProgress(75);
      setUploadStatus('Installing plugin...');

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`Upload failed: ${response.statusText} - ${errorText}`);
      }

      const result = await response.json();
      setUploadProgress(90);
      setUploadStatus('Plugin installed successfully! Loading...');
      
      // Add plugin to store first
      pluginStore.addPluginConfig({
        id: result.plugin_id,
        main: result.main_file,
        path: result.plugin_path,
        priority: 1,
        enabled: true,
        name: result.plugin_name || result.plugin_id,
        description: result.plugin_description || `Dynamically loaded plugin: ${result.plugin_id}`,
        version: result.plugin_version || '1.0.0',
        author: result.plugin_author || 'Plugin Developer'
      });
      
      // Try to load the plugin immediately using the PluginAPI
      await pluginAPI.loadPluginDynamically(result.plugin_id, result.plugin_path, result.main_file);
      
      setUploadProgress(100);
      setUploadStatus(`Plugin "${result.plugin_id}" loaded successfully!`);
      
      // Save the updated config to JSON
      try {
        await pluginStore.saveConfigsToFile();
      } catch (error) {
        console.warn('Failed to save plugin config to file:', error);
      }
      
      // Close overlay after success and switch to list view
      setTimeout(() => {
        setCurrentView('list');
        setUploadStatus('');
        setUploadProgress(0);
      }, 2000);

    } catch (error) {
      console.error('Plugin upload failed:', error);
      setUploadStatus(`Upload failed: ${error.message}`);
    } finally {
      setIsUploading(false);
    }
  };


  const handleDragOver = (e) => {
    e.preventDefault();
    setDragActive(true);
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    setDragActive(false);
  };

  const handleDrop = (e) => {
    e.preventDefault();
    setDragActive(false);
    
    const files = Array.from(e.dataTransfer.files);
    if (files.length > 0) {
      handleFileSelect(files[0]);
    }
  };

  return (
    <>
      {/* Trigger Button - only show when operating in standalone mode */}
      <Show when={props.isOpen === undefined}>
        <button
          onClick={() => setIsOpen(true)}
          class="btn btn-primary btn-sm"
          title="Install Plugin"
        >
          📦 Install Plugin
        </button>
      </Show>

      {/* Overlay */}
      <Show when={isOpen()}>
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
          <div class="bg-base-100 rounded-lg p-6 w-full max-w-6xl mx-4 max-h-[90vh] overflow-hidden flex flex-col">
            <div class="flex justify-between items-center mb-4">
              <div class="flex items-center gap-4">
                <h3 class="text-lg font-semibold">Plugin Manager</h3>
                <div class="tabs tabs-boxed">
                  <button 
                    class={`tab ${currentView() === 'list' ? 'tab-active' : ''}`}
                    onClick={() => setCurrentView('list')}
                  >
                    📋 Manage Plugins
                  </button>
                  <button 
                    class={`tab ${currentView() === 'upload' ? 'tab-active' : ''}`}
                    onClick={() => setCurrentView('upload')}
                  >
                    📦 Install Plugin
                  </button>
                </div>
              </div>
              <button
                onClick={() => setIsOpen(false)}
                class="btn btn-sm btn-ghost"
                disabled={isUploading()}
              >
                ✕
              </button>
            </div>

            {/* Plugin List View */}
            <Show when={currentView() === 'list'}>
              <div class="flex-1 overflow-y-auto">
                <div class="mb-4 flex justify-between items-center">
                  <div class="stats shadow">
                    <div class="stat">
                      <div class="stat-title">Total Plugins</div>
                      <div class="stat-value text-2xl">{pluginList().length}</div>
                    </div>
                    <div class="stat">
                      <div class="stat-title">Enabled</div>
                      <div class="stat-value text-2xl text-success">{pluginList().filter(p => p.enabled).length}</div>
                    </div>
                    <div class="stat">
                      <div class="stat-title">Running</div>
                      <div class="stat-value text-2xl text-info">{pluginList().filter(p => p.state === PLUGIN_STATES.RUNNING).length}</div>
                    </div>
                  </div>
                </div>
                
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                  <For each={pluginList()}>
                    {(plugin) => (
                      <div class={`card bg-base-200 shadow-md border-2 transition-all ${
                        plugin.enabled ? 'border-success border-opacity-50' : 'border-base-300 opacity-70'
                      }`}>
                        <div class="card-body p-4">
                          <div class="flex justify-between items-start mb-2">
                            <h4 class="card-title text-sm font-bold">{plugin.name}</h4>
                            <div class="flex items-center gap-2">
                              <span class={`text-xs ${getStateColor(plugin.state)}`}>
                                {getStateIcon(plugin.state)}
                              </span>
                              <input 
                                type="checkbox" 
                                class="toggle toggle-success toggle-sm" 
                                checked={plugin.enabled}
                                onChange={() => togglePlugin(plugin.id)}
                              />
                            </div>
                          </div>
                          
                          <p class="text-xs text-base-content opacity-70 mb-2 line-clamp-2">
                            {plugin.description}
                          </p>
                          
                          <div class="text-xs space-y-1">
                            <div class="flex justify-between">
                              <span class="text-base-content opacity-60">Version:</span>
                              <span class="font-mono">{plugin.version}</span>
                            </div>
                            <div class="flex justify-between">
                              <span class="text-base-content opacity-60">Author:</span>
                              <span class="truncate ml-2">{plugin.author}</span>
                            </div>
                            <div class="flex justify-between">
                              <span class="text-base-content opacity-60">State:</span>
                              <span class={`capitalize ${getStateColor(plugin.state)}`}>
                                {plugin.state}
                              </span>
                            </div>
                          </div>
                          
                          <div class="card-actions justify-between mt-3">
                            <span class="text-xs text-base-content opacity-50 font-mono">
                              {plugin.id}
                            </span>
                            <Show when={plugin.state === PLUGIN_STATES.ERROR}>
                              <button class="btn btn-xs btn-error btn-outline"
                                      title="Plugin has errors">
                                🚨 Error
                              </button>
                            </Show>
                          </div>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </div>
            </Show>

            {/* Upload View */}
            <Show when={currentView() === 'upload'}>
              <div class="flex-1">
                <Show when={!isUploading()}>
                  {/* File Upload Area with Drag and Drop */}
                  <div
                    class={`border-2 border-dashed rounded-lg p-8 text-center transition-all duration-300 ${
                      dragActive() 
                        ? 'border-primary bg-primary bg-opacity-10 scale-105' 
                        : 'border-base-300 hover:border-primary hover:bg-base-200'
                    }`}
                    onDragOver={handleDragOver}
                    onDragLeave={handleDragLeave}
                    onDrop={handleDrop}
                  >
                    <div class="mb-6">
                      <div class={`text-6xl mb-4 transition-transform ${
                        dragActive() ? 'scale-110' : ''
                      }`}>📦</div>
                      <h4 class="text-lg font-semibold mb-2">
                        {dragActive() ? 'Drop ZIP file here!' : 'Upload Plugin'}
                      </h4>
                      <p class="text-base-content mb-2">
                        Drag and drop plugin ZIP file here or click to browse
                      </p>
                      <p class="text-sm text-base-content opacity-60">
                        Supports .zip files only
                      </p>
                    </div>
                    
                    <input
                      type="file"
                      accept=".zip"
                      onChange={(e) => handleFileSelect(e.target.files[0])}
                      class="file-input file-input-bordered file-input-primary w-full max-w-md"
                    />
                  </div>
                </Show>

                <Show when={isUploading()}>
                  {/* Upload Progress */}
                  <div class="text-center">
                    <div class="mb-4">
                      <div class="loading loading-spinner loading-lg"></div>
                    </div>
                    <div class="mb-4">
                      <progress 
                        class="progress progress-primary w-full" 
                        value={uploadProgress()} 
                        max="100"
                      ></progress>
                      <p class="text-sm mt-1">{uploadProgress()}%</p>
                    </div>
                    <p class="text-sm text-base-content">{uploadStatus()}</p>
                  </div>
                </Show>

                <Show when={uploadStatus() && !isUploading()}>
                  <div class="mt-4 p-3 rounded bg-base-200">
                    <p class="text-sm text-center">{uploadStatus()}</p>
                  </div>
                </Show>

                {/* Instructions */}
                <div class="mt-6 text-xs text-base-content opacity-60">
                  <p class="mb-1">• Plugin ZIP should contain an index.jsx file</p>
                  <p class="mb-1">• Plugin will be installed and loaded immediately</p>
                  <p>• Restart app to include in build permanently</p>
                </div>
              </div>
            </Show>
          </div>
        </div>
      </Show>
    </>
  );
}