import { createSignal, createEffect, onMount, For, Show } from 'solid-js';
import { IconPlus, IconFileText, IconLink, IconEdit, IconTrash, IconCode, IconDeviceFloppy, IconX } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { objectPropertiesActions } from '@/layout/stores/ViewportStore';
import ScriptCreationDialog from './ScriptCreationDialog.jsx';
import { bridgeService as projects } from '@/plugins/core/bridge';
import { getScriptRuntime } from '@/api/script';

function Scripts() {
  const [scripts, setScripts] = createSignal([]);
  const [selectedScript, setSelectedScript] = createSignal(null);
  const [searchQuery, setSearchQuery] = createSignal('');
  const [showCreateDialog, setShowCreateDialog] = createSignal(false);
  const [scriptContent, setScriptContent] = createSignal('');
  const [isLoading, setIsLoading] = createSignal(false);
  const [selectedObjectScripts, setSelectedObjectScripts] = createSignal([]);
  const [editMode, setEditMode] = createSignal(false);
  const [scriptPauseStates, setScriptPauseStates] = createSignal({});
  const [refreshTrigger, setRefreshTrigger] = createSignal(0);
  
  const selection = () => editorStore.selection;
  const selectedEntity = () => selection().entity;

  onMount(async () => {
    await loadScripts();
  });

  createEffect(() => {
    const entityId = selectedEntity();
    if (entityId) {
      loadObjectScripts(entityId);
    } else {
      setSelectedObjectScripts([]);
    }
  });

  // Load script pause states reactively when scripts or entity changes
  createEffect(() => {
    const entityId = selectedEntity();
    const scripts = selectedObjectScripts();
    refreshTrigger(); // Track this signal for manual refreshes
    
    if (!entityId || scripts.length === 0) {
      setScriptPauseStates({});
      return;
    }

    const runtime = getScriptRuntime();
    const newStates = {};
    
    scripts.forEach(scriptPath => {
      try {
        newStates[scriptPath] = runtime.isScriptPaused(entityId, scriptPath);
      } catch (error) {
        newStates[scriptPath] = false;
      }
    });
    
    setScriptPauseStates(newStates);
  });

  const loadScripts = async () => {
    setIsLoading(true);
    try {
      const projectName = projects.getCurrentProject()?.name || 'demo';
      const response = await fetch(`http://localhost:3001/api/projects/${projectName}/scripts`);
      if (response.ok) {
        const data = await response.json();
        setScripts(data.scripts || []);
      }
    } catch (error) {
      console.error('Failed to load scripts:', error);
      setScripts([]);
    } finally {
      setIsLoading(false);
    }
  };

  const loadObjectScripts = (objectId) => {
    const objectProps = objectPropertiesActions.getObjectProperties(objectId);
    if (objectProps?.components?.scripting?.scriptFiles) {
      setSelectedObjectScripts(objectProps.components.scripting.scriptFiles);
    } else {
      setSelectedObjectScripts([]);
    }
  };

  const loadScriptContent = async (scriptPath) => {
    try {
      const projectName = projects.getCurrentProject()?.name || 'demo';
      const response = await fetch(`http://localhost:3001/api/projects/${projectName}/scripts/${scriptPath}`);
      if (response.ok) {
        const content = await response.text();
        setScriptContent(content);
      }
    } catch (error) {
      console.error('Failed to load script content:', error);
      setScriptContent('// Failed to load script');
    }
  };

  const handleCreateScript = async (scriptName) => {
    try {
      const projectName = projects.getCurrentProject()?.name || 'demo';
      const response = await fetch(`http://localhost:3001/api/projects/${projectName}/scripts`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ 
          name: scriptName,
          content: `// ${scriptName}.js
// Script API provides safe access to Babylon.js objects

export default class ${scriptName} {
  constructor(scene, api) {
    this.scene = scene;
    this.api = api; // Use this.api to interact with the object
  }

  // Called when the script is attached to an object
  onStart() {
    this.api.log('${scriptName} started!');
    
    // Example: Get object position
    const pos = this.api.getPosition();
    this.api.log('Object position:', pos);
  }

  // Called every frame
  onUpdate(deltaTime) {
    // Example: Rotate the object slowly
    // this.api.rotateBy(0, deltaTime * 0.5, 0);
    
    // Example: Change color over time
    // const time = this.api.getTime() / 1000;
    // const r = (Math.sin(time) + 1) / 2;
    // this.api.setColor(r, 0.5, 1 - r);
  }

  // Called when the script is removed from the object
  onDestroy() {
    this.api.log('${scriptName} destroyed');
  }
}
`
        })
      });
      
      if (response.ok) {
        await loadScripts();
        editorActions.addConsoleMessage(`Script "${scriptName}" created`, 'success');
      }
    } catch (error) {
      console.error('Failed to create script:', error);
      editorActions.addConsoleMessage(`Failed to create script: ${error.message}`, 'error');
    }
  };

  const handleDeleteScript = async (scriptPath) => {
    if (!confirm(`Delete script "${scriptPath}"?`)) return;
    
    try {
      const projectName = projects.getCurrentProject()?.name || 'demo';
      const response = await fetch(`http://localhost:3001/api/projects/${projectName}/scripts/${scriptPath}`, {
        method: 'DELETE'
      });
      
      if (response.ok) {
        await loadScripts();
        if (selectedScript() === scriptPath) {
          setSelectedScript(null);
          setScriptContent('');
          setEditMode(false);
        }
        editorActions.addConsoleMessage(`Script "${scriptPath}" deleted`, 'success');
      }
    } catch (error) {
      console.error('Failed to delete script:', error);
      editorActions.addConsoleMessage(`Failed to delete script: ${error.message}`, 'error');
    }
  };

  const handleSaveScript = async () => {
    if (!selectedScript()) return;
    
    try {
      const projectName = projects.getCurrentProject()?.name || 'demo';
      const response = await fetch(`http://localhost:3001/api/projects/${projectName}/scripts/${selectedScript()}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content: scriptContent() })
      });
      
      if (response.ok) {
        editorActions.addConsoleMessage(`Script "${selectedScript()}" saved`, 'success');
        setEditMode(false);
      }
    } catch (error) {
      console.error('Failed to save script:', error);
      editorActions.addConsoleMessage(`Failed to save script: ${error.message}`, 'error');
    }
  };

  const toggleScriptOnObject = async (scriptPath) => {
    const entityId = selectedEntity();
    if (!entityId) return;

    const currentScripts = selectedObjectScripts();
    const isAttached = currentScripts.includes(scriptPath);
    const runtime = getScriptRuntime();

    if (isAttached) {
      // Remove script using the runtime
      const success = runtime.detachScript(entityId, scriptPath);
      
      if (success) {
        const newScripts = currentScripts.filter(s => s !== scriptPath);
        objectPropertiesActions.updateObjectProperty(entityId, 'components.scripting.scriptFiles', newScripts);
        setSelectedObjectScripts(newScripts);
        editorActions.addConsoleMessage(`Removed script "${scriptPath}" from object`, 'info');
      } else {
        editorActions.addConsoleMessage(`Failed to remove script "${scriptPath}"`, 'error');
      }
    } else {
      // Attach script using the runtime
      const success = await runtime.attachScript(entityId, scriptPath);
      
      if (success) {
        const newScripts = [...currentScripts, scriptPath];
        objectPropertiesActions.updateObjectProperty(entityId, 'components.scripting.enabled', true);
        objectPropertiesActions.updateObjectProperty(entityId, 'components.scripting.scriptFiles', newScripts);
        setSelectedObjectScripts(newScripts);
        editorActions.addConsoleMessage(`Attached script "${scriptPath}" to object`, 'success');
      } else {
        editorActions.addConsoleMessage(`Failed to attach script "${scriptPath}"`, 'error');
      }
    }
  };

  const filteredScripts = () => {
    const query = searchQuery().toLowerCase();
    return scripts().filter(script => 
      script.name.toLowerCase().includes(query)
    );
  };

  return (
    <div className="flex flex-col h-full bg-slate-900">
      <div className="p-3 border-b border-slate-700">
        <div className="flex items-center gap-2">
          <input
            type="text"
            placeholder="Search scripts..."
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.target.value)}
            className="flex-1 px-2 py-1 text-xs bg-slate-800 border border-slate-600 rounded text-white placeholder-gray-400 focus:outline-none focus:border-blue-500"
          />
          <button
            onClick={() => setShowCreateDialog(true)}
            className="p-1.5 bg-blue-600 hover:bg-blue-700 rounded transition-colors"
            title="Create Script"
          >
            <IconPlus class="w-3.5 h-3.5 text-white" />
          </button>
        </div>
      </div>

      <div className="flex-1 flex overflow-hidden">
        <div className="w-full overflow-y-auto">
          <Show when={selectedEntity()}>
            <div className="p-3 bg-slate-800/50 border-b border-slate-700">
              <div className="text-xs text-gray-400 mb-2">Scripts attached to selected object:</div>
              <Show when={selectedObjectScripts().length === 0}>
                <div className="text-xs text-gray-500">No scripts attached</div>
              </Show>
              <For each={selectedObjectScripts()}>
                {(scriptPath) => {
                  const runtime = getScriptRuntime();
                  const isScriptPaused = () => scriptPauseStates()[scriptPath] || false;
                  
                  const toggleScriptPause = () => {
                    const entityId = selectedEntity();
                    if (!entityId) return;
                    
                    if (isScriptPaused()) {
                      runtime.resumeScript(entityId, scriptPath);
                      editorActions.addConsoleMessage(`Resumed script "${scriptPath}"`, 'success');
                    } else {
                      runtime.pauseScript(entityId, scriptPath);
                      editorActions.addConsoleMessage(`Paused script "${scriptPath}"`, 'info');
                    }
                    
                    // Trigger reactive update
                    setRefreshTrigger(prev => prev + 1);
                  };
                  
                  return (
                    <div className="flex items-center gap-2 py-1">
                      <input
                        type="checkbox"
                        checked={!isScriptPaused()}
                        onChange={toggleScriptPause}
                        className="toggle toggle-xs toggle-success"
                        title={isScriptPaused() ? "Resume script" : "Pause script"}
                      />
                      <IconFileText class="w-3 h-3 text-green-400" />
                      <span className={`text-xs ${isScriptPaused() ? 'text-gray-500' : 'text-gray-200'}`}>
                        {scriptPath}
                      </span>
                      {isScriptPaused() && (
                        <span className="text-xs text-orange-400">(paused)</span>
                      )}
                    </div>
                  );
                }}
              </For>
            </div>
          </Show>

          <div className="p-3">
            <div className="text-xs text-gray-400 mb-2">Available Scripts:</div>
            
            <Show when={isLoading()}>
              <div className="text-center text-gray-400 text-sm py-4">
                Loading scripts...
              </div>
            </Show>
            
            <Show when={!isLoading() && filteredScripts().length === 0}>
              <div className="text-center text-gray-400 text-sm py-4">
                No scripts found
              </div>
            </Show>

            <div className="space-y-1">
              <For each={filteredScripts()}>
                {(script) => {
                  const isSelected = () => selectedScript() === script.path;
                  const isAttached = () => selectedObjectScripts().includes(script.path);
                  
                  return (
                    <div
                      className={`group flex items-center gap-2 px-2 py-1.5 rounded hover:bg-slate-800 cursor-pointer transition-colors ${
                        isSelected() ? 'bg-slate-800 border-l-2 border-blue-500' : ''
                      }`}
                      onClick={() => {
                        setSelectedScript(script.path);
                        loadScriptContent(script.path);
                        setEditMode(false);
                      }}
                    >
                      <IconFileText class="w-3.5 h-3.5 text-gray-400 flex-shrink-0" />
                      <span className="flex-1 text-xs text-gray-200 truncate">
                        {script.name}
                      </span>
                      
                      <Show when={selectedEntity()}>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            toggleScriptOnObject(script.path);
                          }}
                          className={`p-0.5 rounded transition-colors ${
                            isAttached() 
                              ? 'bg-green-600 hover:bg-green-700' 
                              : 'bg-slate-700 hover:bg-slate-600'
                          }`}
                          title={isAttached() ? 'Remove from object' : 'Attach to object'}
                        >
                          <IconLink class="w-3 h-3 text-white" />
                        </button>
                      </Show>
                      
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setSelectedScript(script.path);
                          loadScriptContent(script.path);
                          setEditMode(true);
                        }}
                        className="p-0.5 opacity-0 group-hover:opacity-100 hover:bg-slate-600 rounded transition-all"
                        title="Edit Script"
                      >
                        <IconEdit class="w-3 h-3 text-gray-300" />
                      </button>
                      
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDeleteScript(script.path);
                        }}
                        className="p-0.5 opacity-0 group-hover:opacity-100 hover:bg-red-600 rounded transition-all"
                        title="Delete Script"
                      >
                        <IconTrash class="w-3 h-3 text-gray-300" />
                      </button>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>
        </div>
      </div>

      <Show when={editMode() && selectedScript()}>
        <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50">
          <div className="bg-slate-800 rounded-lg shadow-2xl w-full max-w-4xl h-[80vh] mx-4 flex flex-col">
            <div className="flex items-center justify-between p-4 border-b border-slate-700">
              <div className="flex items-center gap-2">
                <IconCode class="w-5 h-5 text-blue-400" />
                <span className="text-sm font-medium text-white">{selectedScript()}</span>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={handleSaveScript}
                  className="px-3 py-1 bg-blue-600 hover:bg-blue-700 text-white text-xs rounded transition-colors flex items-center gap-1"
                >
                  <IconDeviceFloppy class="w-3 h-3" />
                  Save
                </button>
                <button
                  onClick={() => setEditMode(false)}
                  className="p-1 hover:bg-slate-700 rounded transition-colors"
                >
                  <IconX class="w-4 h-4 text-gray-400" />
                </button>
              </div>
            </div>
            
            <div className="flex-1 relative">
              <textarea
                value={scriptContent()}
                onInput={(e) => setScriptContent(e.target.value)}
                className="absolute inset-0 w-full h-full p-4 bg-slate-950 text-gray-200 font-mono text-sm resize-none focus:outline-none"
                placeholder="// Script content..."
                spellcheck={false}
              />
            </div>
          </div>
        </div>
      </Show>

      <ScriptCreationDialog
        isOpen={showCreateDialog()}
        onClose={() => setShowCreateDialog(false)}
        onConfirm={handleCreateScript}
      />
    </div>
  );
}

export default Scripts;
