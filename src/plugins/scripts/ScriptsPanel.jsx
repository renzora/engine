import { createSignal, createMemo, onCleanup, onMount, createEffect, For, Show, Switch, Match, createComponent } from 'solid-js';
import { IconCode, IconX, IconRotateClockwise, IconPlayerPlay, IconPlayerPause, IconArrowsMove } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { objectPropertiesActions, objectPropertiesStore } from '@/layout/stores/ViewportStore';
import { renderStore } from '@/render/store';
import { getScriptRuntime } from '@/api/script';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';

export default function ScriptsPanel() {
  const { selection } = editorStore;
  const { updateObjectProperty } = objectPropertiesActions;
  const [isDragOverScript, setIsDragOverScript] = createSignal(false);
  const [isResettingProperties, setIsResettingProperties] = createSignal(false);
  const [scriptSearchTerm, setScriptSearchTerm] = createSignal('');
  const [searchResults, setSearchResults] = createSignal([]);
  const [isSearching, setIsSearching] = createSignal(false);
  const [showSearchResults, setShowSearchResults] = createSignal(false);
  
  // Individual signals for script properties
  const [scriptPropertiesSignal, setScriptPropertiesSignal] = createSignal({});
  const [scriptMetadataVersion, setScriptMetadataVersion] = createSignal(0);
  const [pausedScripts, setPausedScripts] = createSignal(new Set());
  let previousScriptSections = {};

  // Individual signals for transform properties
  const [positionSignal, setPositionSignal] = createSignal([0, 0, 0]);
  const [rotationSignal, setRotationSignal] = createSignal([0, 0, 0]);
  const [scaleSignal, setScaleSignal] = createSignal([1, 1, 1]);

  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    scripts: true,
    transform: true
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };

  // Simple cache to avoid repeated API calls
  let scriptCache = null;
  let cacheTimestamp = 0;
  const CACHE_DURATION = 30000; // 30 seconds

  // Function to load scripts from bridge API
  const loadScriptCache = async () => {
    try {
      const response = await fetch('http://localhost:3001/renscripts');
      if (response.ok) {
        const scripts = await response.json();
        scriptCache = scripts;
        cacheTimestamp = Date.now();
        return scripts;
      } else {
        console.warn('Failed to fetch scripts from bridge:', response.status);
        return [];
      }
    } catch (error) {
      console.warn('Error loading script cache from bridge:', error);
      return [];
    }
  };

  // Dynamic script search using bridge API
  const searchScripts = async (searchTerm) => {
    if (!searchTerm || searchTerm.length < 1) {
      setSearchResults([]);
      setShowSearchResults(false);
      return;
    }
    
    setIsSearching(true);
    try {
      // Use bridge API for search
      const response = await fetch(`http://localhost:3001/renscripts/search?q=${encodeURIComponent(searchTerm)}`);
      if (response.ok) {
        const results = await response.json();
        setSearchResults(results);
        setShowSearchResults(results.length > 0);
      } else {
        console.warn('Failed to search scripts via bridge:', response.status);
        setSearchResults([]);
        setShowSearchResults(false);
      }
    } catch (error) {
      console.warn('❌ Error searching scripts via bridge:', error);
      setSearchResults([]);
      setShowSearchResults(false);
    } finally {
      setIsSearching(false);
    }
  };

  // Debounced search effect
  createEffect(() => {
    const term = scriptSearchTerm();
    const timeoutId = setTimeout(() => {
      searchScripts(term);
    }, 300); // 300ms debounce
    
    onCleanup(() => clearTimeout(timeoutId));
  });

  // Get the selected Babylon object directly
  const getSelectedBabylonObject = () => {
    if (!selection.entity || !renderStore.scene) {
      return null;
    }
    
    // Handle scene-root as special case
    if (selection.entity === 'scene-root') {
      return renderStore.scene;
    }
    
    // Find the Babylon object by ID
    const allObjects = [...renderStore.scene.meshes, ...renderStore.scene.transformNodes, ...renderStore.scene.lights, ...renderStore.scene.cameras];
    const found = allObjects.find(obj => (obj.uniqueId || obj.name) === selection.entity);
    return found;
  };

  // Unified property sync: Set up render loop observer for selected object
  let renderObserver = null;
  let currentObservedEntity = null;
  
  // Separate the entity tracking from signal updates
  createEffect(() => {
    const entityId = selection.entity;
    
    // Only proceed if entity actually changed
    if (currentObservedEntity === entityId) {
      return;
    }
    
    currentObservedEntity = entityId;
    
    // Clean up previous observer
    if (renderObserver && renderStore.scene) {
      renderStore.scene.unregisterBeforeRender(renderObserver);
      renderObserver = null;
    }
    
    // Reset paused scripts when changing entities and sync with actual state
    const runtime = getScriptRuntime();
    const newPausedSet = new Set();
    
    if (runtime?.scriptManager && entityId) {
      const scripts = runtime.scriptManager.getScriptsForObject(entityId) || [];
      scripts.forEach(script => {
        const scriptPath = script.path;
        if (runtime.scriptManager.isScriptPaused(entityId, scriptPath)) {
          const scriptKey = `${entityId}:${scriptPath}`;
          newPausedSet.add(scriptKey);
        }
      });
    }
    
    setPausedScripts(newPausedSet);
    
    if (!entityId || !renderStore.scene) {
      return;
    }
    
    // Get babylon object without triggering reactivity
    let babylonObject;
    if (entityId === 'scene-root') {
      babylonObject = renderStore.scene;
    } else {
      const allObjects = [...renderStore.scene.meshes, ...renderStore.scene.transformNodes, ...renderStore.scene.lights, ...renderStore.scene.cameras];
      babylonObject = allObjects.find(obj => (obj.uniqueId || obj.name) === entityId);
    }
    
    if (!babylonObject) {
      return;
    }
    
    // Initialize metadata structure if it doesn't exist
    if (!babylonObject.metadata) {
      babylonObject.metadata = {};
    }
    if (!babylonObject.metadata.properties) {
      babylonObject.metadata.properties = {};
    }
    if (!babylonObject.metadata.originalProperties) {
      babylonObject.metadata.originalProperties = {};
    }
    
    // Capture original transform values when object is first selected (skip for scene-root)
    if (entityId !== 'scene-root') {
      if (!babylonObject.metadata.originalProperties.position && babylonObject.position) {
        babylonObject.metadata.originalProperties.position = [babylonObject.position.x, babylonObject.position.y, babylonObject.position.z];
      }
      if (!babylonObject.metadata.originalProperties.rotation && babylonObject.rotation) {
        babylonObject.metadata.originalProperties.rotation = [babylonObject.rotation.x, babylonObject.rotation.y, babylonObject.rotation.z];
      }
      if (!babylonObject.metadata.originalProperties.scale) {
        if (babylonObject.scaling) {
          babylonObject.metadata.originalProperties.scale = [babylonObject.scaling.x, babylonObject.scaling.y, babylonObject.scaling.z];
        } else if (babylonObject.scale) {
          babylonObject.metadata.originalProperties.scale = [babylonObject.scale.x, babylonObject.scale.y, babylonObject.scale.z];
        } else {
          babylonObject.metadata.originalProperties.scale = [1, 1, 1];
        }
      }
    }
    
    // Register render loop observer for live updates
    renderObserver = () => {
      // Skip sync during reset operations
      if (isResettingProperties()) {
        return;
      }
      
      // Skip transform sync for scene-root (scenes don't have position/rotation/scale)
      if (entityId !== 'scene-root') {
        // Update transform signals
        if (babylonObject.position) {
        const newPosition = [babylonObject.position.x, babylonObject.position.y, babylonObject.position.z];
        const currentPosition = positionSignal();
        if (newPosition.some((val, i) => val !== currentPosition[i])) {
          setPositionSignal(newPosition);
        }
      }
      
      if (babylonObject.rotation) {
        const newRotation = [babylonObject.rotation.x, babylonObject.rotation.y, babylonObject.rotation.z];
        const currentRotation = rotationSignal();
        if (newRotation.some((val, i) => val !== currentRotation[i])) {
          setRotationSignal(newRotation);
        }
      }
      
      if (babylonObject.scaling) {
        const newScale = [babylonObject.scaling.x, babylonObject.scaling.y, babylonObject.scaling.z];
        const currentScale = scaleSignal();
        if (newScale.some((val, i) => val !== currentScale[i])) {
          setScaleSignal(newScale);
        }
      } else if (babylonObject.scale) {
        const newScale = [babylonObject.scale.x, babylonObject.scale.y, babylonObject.scale.z];
        const currentScale = scaleSignal();
        if (newScale.some((val, i) => val !== currentScale[i])) {
          setScaleSignal(newScale);
        }
      }
      } // End of transform sync for non-scene objects

      // Update script properties signal
      const scriptProperties = babylonObject.metadata?.scriptProperties || {};
      const currentScriptProps = scriptPropertiesSignal();
      let scriptPropsChanged = false;
      
      // First check if the number of properties changed
      const scriptKeys = Object.keys(scriptProperties);
      const currentKeys = Object.keys(currentScriptProps);
      
      if (scriptKeys.length !== currentKeys.length) {
        scriptPropsChanged = true;
      } else {
        // Check if script properties changed
        for (const key of scriptKeys) {
          if (scriptProperties[key] !== currentScriptProps[key]) {
            scriptPropsChanged = true;
            break;
          }
        }
      }
      
      if (scriptPropsChanged) {
        setScriptPropertiesSignal(scriptProperties);
        
        // Check if script metadata changed when properties change (indicates script reload)
        const runtime = getScriptRuntime();
        const scripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
        const scriptInstances = scripts.map(s => s.instance).filter(Boolean);
        
        // Build current sections structure
        const currentScriptSections = {};
        for (const scriptInstance of scriptInstances) {
          const scriptAPI = scriptInstance._scriptAPI;
          if (scriptAPI) {
            const scriptPath = scriptInstance._scriptPath || 'unknown';
            currentScriptSections[scriptPath] = scriptAPI.getScriptPropertiesBySection?.() || {};
          }
        }
        
        // Compare with previous sections
        const currentSectionsStr = JSON.stringify(currentScriptSections);
        const previousSectionsStr = JSON.stringify(previousScriptSections);
        
        if (currentSectionsStr !== previousSectionsStr) {
          previousScriptSections = currentScriptSections;
          setScriptMetadataVersion(prev => prev + 1);
        }
      }
    };
    
    renderStore.scene.registerBeforeRender(renderObserver);
    
    // Initial sync
    renderObserver();
  });

  // Helper function to add script defaults to originalProperties when script is attached
  const addScriptDefaults = (babylonObject, scriptInstance) => {
    if (!babylonObject?.metadata || !scriptInstance?._scriptAPI) return;
    
    // Initialize metadata structure
    if (!babylonObject.metadata.originalProperties) babylonObject.metadata.originalProperties = {};
    if (!babylonObject.metadata.scriptProperties) babylonObject.metadata.scriptProperties = {};
    
    const scriptAPI = scriptInstance._scriptAPI;
    const scriptProperties = scriptAPI.getScriptProperties?.() || [];
    
    scriptProperties.forEach(prop => {
      // Always add/overwrite to originalProperties (for reset functionality)
      babylonObject.metadata.originalProperties[prop.name] = prop.defaultValue;
      
      // Always add/overwrite to scriptProperties (for UI display)
      babylonObject.metadata.scriptProperties[prop.name] = prop.defaultValue;
    });
  };

  // Helper function to completely remove script properties when script is detached
  const removeScriptProperties = (babylonObject, scriptInstance) => {
    if (!babylonObject?.metadata || !scriptInstance?._scriptAPI) return;
    
    const scriptAPI = scriptInstance._scriptAPI;
    const scriptProperties = scriptAPI.getScriptProperties?.() || [];
    
    scriptProperties.forEach(prop => {
      // Remove from originalProperties (defaults)
      if (babylonObject.metadata.originalProperties) {
        delete babylonObject.metadata.originalProperties[prop.name];
      }
      
      // Remove from current properties
      if (babylonObject.metadata.properties) {
        delete babylonObject.metadata.properties[prop.name];
      }
      
      // Remove from scriptProperties
      if (babylonObject.metadata.scriptProperties) {
        delete babylonObject.metadata.scriptProperties[prop.name];
      }
    });
  };

  // Individual script pause/resume functions
  const toggleScriptPause = (scriptPath) => {
    if (!selection.entity) return;
    
    const runtime = getScriptRuntime();
    const scriptKey = `${selection.entity}:${scriptPath}`;
    const currentPaused = pausedScripts();
    const newPaused = new Set(currentPaused);
    
    if (currentPaused.has(scriptKey)) {
      // Resume script
      runtime.resumeScript(selection.entity, scriptPath);
      newPaused.delete(scriptKey);
      editorActions.addConsoleMessage(`Resumed script: ${scriptPath.split('/').pop()}`, 'info');
    } else {
      // Pause script
      runtime.pauseScript(selection.entity, scriptPath);
      newPaused.add(scriptKey);
      editorActions.addConsoleMessage(`Paused script: ${scriptPath.split('/').pop()}`, 'info');
    }
    
    setPausedScripts(newPaused);
  };

  const isScriptPaused = (scriptPath) => {
    const scriptKey = `${selection.entity}:${scriptPath}`;
    return pausedScripts().has(scriptKey);
  };

  // Cleanup on component unmount
  onCleanup(() => {
    if (renderObserver && renderStore.scene) {
      renderStore.scene.unregisterBeforeRender(renderObserver);
    }
  });

  const resetToDefaults = async () => {
    if (!selection.entity) return;
    
    const babylonObject = getSelectedBabylonObject();
    if (!babylonObject || !babylonObject.metadata?.originalProperties) return;
    
    // Temporarily disable live sync to prevent it from overriding reset values
    setIsResettingProperties(true);
    
    try {
      const originalProperties = babylonObject.metadata.originalProperties;
      
      // Reset transform properties
      if (originalProperties.position && babylonObject.position) {
        babylonObject.position.set(originalProperties.position[0], originalProperties.position[1], originalProperties.position[2]);
      }
      if (originalProperties.rotation && babylonObject.rotation) {
        babylonObject.rotation.set(originalProperties.rotation[0], originalProperties.rotation[1], originalProperties.rotation[2]);
      }
      if (originalProperties.scale) {
        if (babylonObject.scaling) {
          babylonObject.scaling.set(originalProperties.scale[0], originalProperties.scale[1], originalProperties.scale[2]);
        } else if (babylonObject.scale) {
          babylonObject.scale.set(originalProperties.scale[0], originalProperties.scale[1], originalProperties.scale[2]);
        }
      }
      
      // Reset script properties
      const runtime = getScriptRuntime();
      Object.keys(originalProperties).forEach(propName => {
        // Skip transform properties (handled above)
        if (['position', 'rotation', 'scale'].includes(propName)) return;
        
        const originalValue = originalProperties[propName];
        
        // Update Babylon metadata
        if (!babylonObject.metadata.scriptProperties) babylonObject.metadata.scriptProperties = {};
        babylonObject.metadata.scriptProperties[propName] = originalValue;
        
        // Update script instances
        const scripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
        scripts.forEach(script => {
          const instance = script.instance;
          if (instance._scriptAPI?.setScriptProperty) {
            instance._scriptAPI.setScriptProperty(propName, originalValue);
            instance[propName] = originalValue;
          }
        });
      });
      
    } finally {
      // Re-enable live sync after a short delay
      setTimeout(() => {
        setIsResettingProperties(false);
      }, 100);
    }
  };

  const resetSingleProperty = (propertyPath, defaultValue) => {
    const babylonObject = getSelectedBabylonObject();
    if (!babylonObject) return;
    
    // Update transform properties directly on Babylon object
    if (propertyPath.startsWith('transform.')) {
      const transformType = propertyPath.split('.')[1]; // position, rotation, or scale
      
      if (transformType === 'position' && babylonObject.position) {
        babylonObject.position.set(defaultValue[0], defaultValue[1], defaultValue[2]);
      } else if (transformType === 'rotation' && babylonObject.rotation) {
        babylonObject.rotation.set(defaultValue[0], defaultValue[1], defaultValue[2]);
      } else if (transformType === 'scale' && babylonObject.scaling) {
        babylonObject.scaling.set(defaultValue[0], defaultValue[1], defaultValue[2]);
      }
    }
  };

  const renderVector3Input = (label, propertyPath) => {
    const transformType = propertyPath.split('.')[1]; // position, rotation, or scale
    
    // Get the appropriate signal based on transform type
    const getSignalValue = () => {
      switch (transformType) {
        case 'position': return positionSignal;
        case 'rotation': return rotationSignal;
        case 'scale': return scaleSignal;
        default: return () => [0, 0, 0];
      }
    };
    
    const value = getSignalValue();
    
    return (
    <div className="mb-2">
      <div className="flex items-center justify-between mb-0.5">
        <label className="block text-xs text-base-content/60">{label}</label>
        <button
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            
            const babylonObject = getSelectedBabylonObject();
            if (!babylonObject?.metadata?.originalProperties) return;
            
            // Temporarily disable live sync during individual reset
            setIsResettingProperties(true);
            
            const transformType = propertyPath.split('.')[1];
            const originalValue = babylonObject.metadata.originalProperties[transformType];
            
            if (originalValue) {
              resetSingleProperty(propertyPath, originalValue);
            }
            
            // Re-enable live sync after a short delay
            setTimeout(() => {
              setIsResettingProperties(false);
            }, 50);
          }}
          className="p-0.5 rounded hover:bg-base-300/50 text-base-content/40 hover:text-base-content/60 transition-all duration-150"
          title={`Reset ${label.toLowerCase()}`}
        >
          <IconRotateClockwise className="w-3 h-3" />
        </button>
      </div>
      <div className="grid grid-cols-3 gap-0.5">
        <For each={['X', 'Y', 'Z']}>
          {(axis, index) => (
            <div className="relative">
              <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-base-content/70 pointer-events-none font-medium border-t border-l border-b border-r border-base-300 bg-base-200 rounded-l">
                {axis}
              </span>
              <input
                id={`transform-${propertyPath.split('.')[1]}-${axis.toLowerCase()}-${selection.entity || 'unknown'}`}
                type="number"
                step="0.1"
                value={value()[index()] || 0}
                onFocus={() => {
                  keyboardShortcuts.disable();
                }}
                onBlur={() => {
                  keyboardShortcuts.enable();
                }}
                onMouseDown={() => {}}
                onChange={(e) => {
                  const newValue = parseFloat(e.target.value) || 0;
                  
                  const babylonObject = getSelectedBabylonObject();
                  if (!babylonObject) {
                    console.warn('No babylon object found for transform update');
                    return;
                  }
                  
                  // Update Babylon object directly
                  const transformType = propertyPath.split('.')[1];
                  const axisIndex = index();
                  
                  if (transformType === 'position' && babylonObject.position) {
                    if (axisIndex === 0) babylonObject.position.x = newValue;
                    else if (axisIndex === 1) babylonObject.position.y = newValue;
                    else if (axisIndex === 2) babylonObject.position.z = newValue;
                    
                    // Update position signal immediately for instant UI response
                    const newPosition = [babylonObject.position.x, babylonObject.position.y, babylonObject.position.z];
                    setPositionSignal(newPosition);
                  } else if (transformType === 'rotation' && babylonObject.rotation) {
                    if (axisIndex === 0) babylonObject.rotation.x = newValue;
                    else if (axisIndex === 1) babylonObject.rotation.y = newValue;
                    else if (axisIndex === 2) babylonObject.rotation.z = newValue;
                    
                    // Update rotation signal immediately for instant UI response
                    const newRotation = [babylonObject.rotation.x, babylonObject.rotation.y, babylonObject.rotation.z];
                    setRotationSignal(newRotation);
                  } else if (transformType === 'scale' && babylonObject.scaling) {
                    if (axisIndex === 0) babylonObject.scaling.x = newValue;
                    else if (axisIndex === 1) babylonObject.scaling.y = newValue;
                    else if (axisIndex === 2) babylonObject.scaling.z = newValue;
                    
                    // Update scale signal immediately for instant UI response
                    const newScale = [babylonObject.scaling.x, babylonObject.scaling.y, babylonObject.scaling.z];
                    setScaleSignal(newScale);
                  }
                }}
                className={`w-full text-xs p-1 pl-6 pr-1 rounded text-center focus:outline-none focus:ring-1 focus:ring-primary border-base-300 bg-secondary/10 text-base-content border`}
              />
            </div>
          )}
        </For>
      </div>
    </div>
    );
  };

  // Extract script property input as a separate component to avoid reactive issues
  const ScriptPropertyInput = (props) => {
    const { property, babylonObject } = props;
    
    // Use a simple function instead of createMemo to avoid circular dependencies
    const getCurrentValue = () => {
      // Force reactivity by accessing the signal
      const currentProps = scriptPropertiesSignal();
      
      if (!babylonObject?.metadata?.scriptProperties) {
        return property.defaultValue !== undefined ? property.defaultValue : getDefaultValueForType(property.type);
      }
      const val = babylonObject.metadata.scriptProperties[property.name];
      // For booleans, don't use || operator as false is a valid value
      if (property.type === 'boolean') {
        return val !== undefined ? val : property.defaultValue;
      }
      // For strings, don't fall back to 0
      if (property.type === 'string') {
        return val !== undefined && val !== null ? val : (property.defaultValue || '');
      }
      return val !== undefined && val !== null ? val : (property.defaultValue !== undefined ? property.defaultValue : 0);
    };

    const getDefaultValueForType = (type) => {
      switch (type) {
        case 'string': return '';
        case 'boolean': return false;
        case 'number':
        case 'float':
        case 'range': return 0;
        default: return '';
      }
    };

    const handlePropertyChange = (propertyName, newValue) => {
      try {
        // Validate the property change
        if (!propertyName || propertyName.trim() === '') {
          console.error('Invalid property name:', propertyName);
          return;
        }

        // Update Babylon object metadata directly FIRST
        if (!babylonObject) {
          console.error('No babylon object available for property change');
          return;
        }
        
        if (!babylonObject.metadata) babylonObject.metadata = {};
        if (!babylonObject.metadata.scriptProperties) babylonObject.metadata.scriptProperties = {};
        
        babylonObject.metadata.scriptProperties[propertyName] = newValue;
        
        // Trigger signal update for reactivity
        setScriptPropertiesSignal({ ...babylonObject.metadata.scriptProperties });
        
        // Also update through ScriptAPI for script instance synchronization
        const runtime = getScriptRuntime();
        const scripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
        scripts.forEach(script => {
          const instance = script.instance;
          if (instance?._scriptAPI?.setScriptProperty) {
            try {
              instance._scriptAPI.setScriptProperty(propertyName, newValue);
            } catch (scriptError) {
              console.error('Error updating script instance:', scriptError);
            }
          }
        });
      } catch (error) {
        console.error('Error in handlePropertyChange:', error);
      }
    };

    return (
      <div className="form-control">
        <label className="label pb-2">
          <div className="flex items-center gap-2">
            <span className="label-text text-sm font-medium capitalize">
              {property.name.replace(/_/g, ' ')}
            </span>
            <Show when={property.description && property.description !== 'null'}>
              <div className="tooltip tooltip-left" data-tip={property.description.replace(/"/g, '')}>
                <span className="text-xs text-base-content/50 cursor-help">?</span>
              </div>
            </Show>
          </div>
          <button
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              
              const babylonObject = getSelectedBabylonObject();
              if (!babylonObject?.metadata?.originalProperties) return;
              
              // Temporarily disable live sync during individual reset
              setIsResettingProperties(true);
              
              const originalValue = babylonObject.metadata.originalProperties[property.name] ?? property.defaultValue;
              handlePropertyChange(property.name, originalValue);
              
              // Re-enable live sync after a short delay
              setTimeout(() => {
                setIsResettingProperties(false);
              }, 50);
            }}
            className="p-0.5 rounded hover:bg-base-300/50 text-base-content/40 hover:text-base-content/60 transition-all duration-150"
            title={`Reset ${property.name.replace(/_/g, ' ')} to default`}
          >
            <IconRotateClockwise className="w-3 h-3" />
          </button>
        </label>
        
        <Switch>
          <Match when={property.type === 'number' || property.type === 'float'}>
            <Show 
              when={property.min !== undefined && property.max !== undefined}
              fallback={
                <input
                  id={`script-${property.name}-number-${selection.entity || 'unknown'}`}
                  type="number"
                  value={getCurrentValue()}
                  step={property.type === 'float' ? '0.1' : '1'}
                  min={property.min}
                  max={property.max}
                  onFocus={() => keyboardShortcuts.disable()}
                  onBlur={() => keyboardShortcuts.enable()}
                  onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value) || 0)}
                  className="input input-bordered input-sm w-full text-sm"
                  placeholder="0"
                />
              }
            >
              <div className="flex items-center gap-2 w-full">
                <input
                  id={`script-${property.name}-range-${selection.entity || 'unknown'}`}
                  type="range"
                  min={property.min}
                  max={property.max}
                  step={property.type === 'float' ? '0.1' : '1'}
                  value={getCurrentValue()}
                  onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value))}
                  className="range range-primary range-xs flex-1"
                />
                <input
                  id={`script-${property.name}-number-small-${selection.entity || 'unknown'}`}
                  type="number"
                  value={getCurrentValue()}
                  step={property.type === 'float' ? '0.1' : '1'}
                  min={property.min}
                  max={property.max}
                  onFocus={() => keyboardShortcuts.disable()}
                  onBlur={() => keyboardShortcuts.enable()}
                  onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value) || 0)}
                  className="input input-bordered input-xs w-16 text-xs text-center"
                />
              </div>
            </Show>
          </Match>
          
          <Match when={property.type === 'boolean'}>
            <div className="flex items-center justify-between">
              <div className="form-control">
                <label className="label cursor-pointer justify-start gap-3">
                  <input
                    id={`script-${property.name}-toggle-${selection.entity || 'unknown'}`}
                    type="checkbox"
                    checked={!!getCurrentValue()}
                    onChange={(e) => handlePropertyChange(property.name, e.target.checked)}
                    className="toggle toggle-secondary toggle-sm"
                  />
                  <span className="label-text text-sm">
                    {getCurrentValue() ? 'Enabled' : 'Disabled'}
                  </span>
                </label>
              </div>
              <div className={`badge badge-sm ${getCurrentValue() ? 'badge-success' : 'badge-ghost'}`}>
                {getCurrentValue() ? 'ON' : 'OFF'}
              </div>
            </div>
          </Match>
          
          <Match when={property.type === 'string'}>
            <div 
              className="relative"
              onDragOver={(e) => {
                e.preventDefault();
                e.stopPropagation();
                // Check if dragging a material/texture file
                const types = Array.from(e.dataTransfer.types);
                if (types.includes('text/plain')) {
                  e.currentTarget.classList.add('ring-2', 'ring-primary');
                }
              }}
              onDragLeave={(e) => {
                e.currentTarget.classList.remove('ring-2', 'ring-primary');
              }}
              onDrop={(e) => {
                e.preventDefault();
                e.stopPropagation();
                e.currentTarget.classList.remove('ring-2', 'ring-primary');
                
                const droppedData = e.dataTransfer.getData('text/plain');
                try {
                  const data = JSON.parse(droppedData);
                  if (data.type === 'asset') {
                    // Check if it's a material file (.jsx)
                    if (data.name.endsWith('.jsx')) {
                      handlePropertyChange(property.name, data.path);
                    }
                    // Check if it's a texture file (common texture formats)
                    else if (/\.(jpg|jpeg|png|webp|tga|bmp|dds|hdr|exr|ktx)$/i.test(data.name)) {
                      handlePropertyChange(property.name, data.path);
                    }
                  }
                } catch (err) {
                  console.warn('Invalid drop data for string property:', droppedData);
                }
              }}
            >
              <Show when={getCurrentValue() && String(getCurrentValue()).trim() !== ''} fallback={
                <input
                  id={`script-${property.name}-text-${selection.entity || 'unknown'}`}
                  type="text"
                  value={getCurrentValue() || ''}
                  onFocus={() => keyboardShortcuts.disable()}
                  onBlur={() => keyboardShortcuts.enable()}
                  onChange={(e) => handlePropertyChange(property.name, e.target.value)}
                  className="input input-bordered input-sm w-full text-sm"
                  placeholder={property.defaultValue?.replace(/"/g, '') || 'Drag material/texture files here...'}
                />
              }>
                <div className="flex items-center gap-2 p-2 bg-base-200 rounded-lg border border-base-300">
                  <div className="flex items-center gap-2 flex-1">
                    <div className="w-8 h-8 bg-primary/20 rounded flex items-center justify-center flex-shrink-0">
                      {getCurrentValue().endsWith('.jsx') ? (
                        <div className="w-4 h-4 bg-primary rounded-sm"></div>
                      ) : (
                        <div className="w-4 h-4 bg-secondary rounded-sm"></div>
                      )}
                    </div>
                    <div className="flex flex-col flex-1 min-w-0">
                      <span className="text-sm font-medium text-base-content truncate">
                        {getCurrentValue().split('/').pop()}
                      </span>
                      <span className="text-xs text-base-content/60 truncate">
                        {getCurrentValue().endsWith('.jsx') ? 'Material' : 'Texture'}
                      </span>
                    </div>
                  </div>
                  <button
                    className="btn btn-ghost btn-xs btn-circle text-error hover:bg-error/20"
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      handlePropertyChange(property.name, '');
                    }}
                    title="Remove material/texture"
                  >
                    ×
                  </button>
                </div>
              </Show>
            </div>
          </Match>
          
          <Match when={property.type === 'range'}>
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <input
                  id={`script-${property.name}-slider-${selection.entity || 'unknown'}`}
                  type="range"
                  value={getCurrentValue()}
                  min={property.min || 0}
                  max={property.max || 100}
                  step={0.01}
                  onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value))}
                  className="range range-secondary range-sm flex-1"
                />
                <div className="badge badge-secondary badge-outline font-mono text-xs min-w-[4rem]">
                  {parseFloat(getCurrentValue()).toFixed(2)}
                </div>
              </div>
              <div className="flex justify-between text-xs text-base-content/60">
                <span className="badge badge-ghost badge-xs">{property.min || 0}</span>
                <span className="badge badge-ghost badge-xs">{property.max || 100}</span>
              </div>
            </div>
          </Match>
          
          <Match when={property.type === 'select'}>
            <select
              id={`script-${property.name}-select-${selection.entity || 'unknown'}`}
              value={getCurrentValue() || property.defaultValue}
              onChange={(e) => handlePropertyChange(property.name, e.target.value)}
              className="select select-bordered select-sm w-full text-sm"
            >
              <Show when={property.options && Array.isArray(property.options)}>
                <For each={property.options}>
                  {(option) => (
                    <option value={option}>{option}</option>
                  )}
                </For>
              </Show>
            </select>
          </Match>

          <Match when={true}>
            <input
              type="text"
              value={getCurrentValue() || ''}
              onFocus={() => keyboardShortcuts.disable()}
              onBlur={() => keyboardShortcuts.enable()}
              onChange={(e) => handlePropertyChange(property.name, e.target.value)}
              className="input input-bordered input-sm w-full text-sm"
              placeholder="Enter value"
            />
          </Match>
        </Switch>
      </div>
    );
  };

  return (
    <div class="h-full flex flex-col">
      <div class="flex-1 p-2 space-y-2">
        {(() => {
          let objectProps = objectPropertiesStore.objects[selection.entity];
          
          if (!objectProps && selection.entity) {
            objectPropertiesActions.ensureDefaultComponents(selection.entity);
            objectProps = objectPropertiesStore.objects[selection.entity];
          }
          
          if (!objectProps) {
            return (
              <div class="p-4 text-center">
                <div class="text-base-content/50 text-xs mb-1">
                  No object selected
                </div>
                <div class="text-base-content/40 text-xs">
                  Select an object to attach scripts
                </div>
              </div>
            );
          }
          
          return (
            <>
              {/* Transform - Hide for scene-root */}
              <Show when={(positionSignal().length > 0 || rotationSignal().length > 0 || scaleSignal().length > 0) && selection.entity !== 'scene-root'}>
                <div class="bg-base-100 border-base-300 border rounded-lg">
                  <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().transform ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
                    <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('transform')}>
                      <IconArrowsMove class="w-3 h-3" />
                      Transform
                    </div>
                    <input
                      type="checkbox"
                      checked={sectionsOpen().transform}
                      onChange={(e) => {
                        e.stopPropagation();
                        toggleSection('transform');
                      }}
                      onClick={(e) => e.stopPropagation()}
                      class="toggle toggle-primary toggle-xs"
                    />
                  </div>
                  <Show when={sectionsOpen().transform}>
                    <div class="!p-2">
                      <div class="space-y-0.5">
                        <Show when={positionSignal().length > 0}>
                          {renderVector3Input('Position', 'transform.position')}
                        </Show>
                        <Show when={rotationSignal().length > 0}>
                          {renderVector3Input('Rotation', 'transform.rotation')}
                        </Show>
                        <Show when={scaleSignal().length > 0}>
                          {renderVector3Input('Scale', 'transform.scale')}
                        </Show>
                      </div>
                    </div>
                  </Show>
                </div>
              </Show>

              {/* Scripts */}
              <div class="bg-base-100 border-base-300 border rounded-lg">
                <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().scripts ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
                  <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('scripts')}>
                    <IconCode class="w-3 h-3" />
                    Scripts
                  </div>
                  <input
                    type="checkbox"
                    checked={sectionsOpen().scripts}
                    onChange={(e) => {
                      e.stopPropagation();
                      toggleSection('scripts');
                    }}
                    onClick={(e) => e.stopPropagation()}
                    class="toggle toggle-primary toggle-xs"
                  />
                </div>
                <Show when={sectionsOpen().scripts}>
                  <div class="!p-2">
                  <div class="space-y-0.5">
                    {/* Script Search Box */}
                    <div class="relative">
                      <input
                        type="text"
                        placeholder="Search scripts..."
                        value={scriptSearchTerm()}
                        onInput={(e) => {
                          setScriptSearchTerm(e.target.value);
                          if (e.target.value.length > 0) {
                            setShowSearchResults(true);
                          }
                        }}
                        onFocus={() => {
                          keyboardShortcuts.disable();
                          if (scriptSearchTerm().length > 0 && searchResults().length > 0) {
                            setShowSearchResults(true);
                          }
                        }}
                        onBlur={() => {
                          keyboardShortcuts.enable();
                          setTimeout(() => setShowSearchResults(false), 150);
                        }}
                        class="w-full text-sm rounded bg-neutral/40 border-0 focus:outline-none focus:ring-0 p-1"
                      />
                      
                      {/* Available Scripts Dropdown */}
                      <Show when={showSearchResults() && scriptSearchTerm() && searchResults().length > 0}>
                        <div class="absolute top-full left-0 right-0 z-50 max-h-32 overflow-y-auto bg-base-100 border border-base-300 rounded shadow-lg">
                          <For each={searchResults()}>
                            {(script) => (
                              <div 
                                class="p-2 hover:bg-base-200 cursor-pointer border-b border-base-300/50 last:border-b-0"
                                onClick={async () => {
                                  if (!selection.entity) return;
                                  
                                  const currentScripts = objectProps.scripts || [];
                                  if (!currentScripts.find(s => s.path === script.path)) {
                                    const runtime = getScriptRuntime();
                                    const success = await runtime.attachScript(selection.entity, script.path);
                                    
                                    if (success) {
                                      const scriptInstance = runtime.getScriptInstance(selection.entity, script.path);
                                      const babylonObject = getSelectedBabylonObject();
                                      
                                      if (scriptInstance && babylonObject) {
                                        addScriptDefaults(babylonObject, scriptInstance);
                                      }
                                      
                                      const metadata = scriptInstance?._scriptProperties || [];
                                      const defaultValues = {};
                                      
                                      if (Array.isArray(metadata)) {
                                        metadata.forEach(prop => {
                                          defaultValues[prop.name] = prop.defaultValue;
                                        });
                                      }
                                      
                                      const newScripts = [...currentScripts, { 
                                        path: script.path, 
                                        name: script.name + '.ren',
                                        enabled: true,
                                        metadata: metadata,
                                        defaultValues: defaultValues,
                                        properties: defaultValues
                                      }];
                                      updateObjectProperty(selection.entity, 'scripts', newScripts);
                                      
                                      if (babylonObject?.metadata?.scriptProperties) {
                                        setScriptPropertiesSignal({ ...babylonObject.metadata.scriptProperties });
                                      }
                                      
                                      editorActions.addConsoleMessage(`Script "${script.name}" attached and started`, 'success');
                                      setScriptSearchTerm('');
                                      setShowSearchResults(false);
                                    } else {
                                      editorActions.addConsoleMessage(`Cannot attach "${script.name}" - check console for details`, 'error');
                                    }
                                  } else {
                                    editorActions.addConsoleMessage(`Script "${script.name}" already attached`, 'warning');
                                  }
                                }}
                              >
                                <div class="flex items-center gap-2">
                                  <IconCode class="w-4 h-4 text-secondary" />
                                  <div class="flex flex-col">
                                    <span class="text-sm font-medium">{script.name}</span>
                                    <span class="text-xs text-base-content/50">{script.full_path || script.directory}</span>
                                  </div>
                                </div>
                              </div>
                            )}
                          </For>
                        </div>
                      </Show>
                    </div>
                    
                    {/* Drop Zone */}
                    <div 
                      data-drop-zone="scripts"
                      class={`min-h-[60px] text-center border-2 border-dashed border-base-300 rounded-lg ${isDragOverScript() ? 'animate-pulse brightness-110' : ''}`}
                      onDragOver={(e) => {
                        e.preventDefault();
                        const types = Array.from(e.dataTransfer.types);
                        if (types.includes('text/plain')) {
                          e.currentTarget.classList.add('bg-success/20');
                          e.currentTarget.classList.remove('bg-error/20');
                          setIsDragOverScript(true);
                        } else {
                          e.currentTarget.classList.add('bg-error/20');
                          e.currentTarget.classList.remove('bg-success/20');
                          setIsDragOverScript(true);
                        }
                      }}
                      onDragLeave={(e) => {
                        e.currentTarget.classList.remove('bg-success/20', 'bg-error/20');
                        setIsDragOverScript(false);
                      }}
                      onDrop={async (e) => {
                        e.preventDefault();
                        e.currentTarget.classList.remove('bg-success/20', 'bg-error/20');
                        setIsDragOverScript(false);
                        
                        const droppedData = e.dataTransfer.getData('text/plain');
                        try {
                          const data = JSON.parse(droppedData);
                          if (data.type === 'asset' && data.fileType === 'script') {
                            const validExtensions = ['.js', '.jsx', '.ts', '.tsx', '.ren'];
                            const fileExt = data.name.substring(data.name.lastIndexOf('.')).toLowerCase();
                            
                            if (validExtensions.includes(fileExt)) {
                              if (!objectProps.scripts) {
                                objectPropertiesActions.addPropertySection(selection.entity, 'scripts', []);
                              }
                              
                              const currentScripts = objectProps.scripts || [];
                              if (!currentScripts.find(s => s.path === data.path)) {
                                const runtime = getScriptRuntime();
                                const success = await runtime.attachScript(selection.entity, data.path);
                                
                                if (success) {
                                  const scriptInstance = runtime.getScriptInstance(selection.entity, data.path);
                                  const babylonObject = getSelectedBabylonObject();
                                  
                                  if (scriptInstance && babylonObject) {
                                    addScriptDefaults(babylonObject, scriptInstance);
                                  }
                                  
                                  const metadata = scriptInstance?._scriptProperties || [];
                                  const defaultValues = {};
                                  
                                  if (Array.isArray(metadata)) {
                                    metadata.forEach(prop => {
                                      defaultValues[prop.name] = prop.defaultValue;
                                    });
                                  }
                                  
                                  const newScripts = [...currentScripts, { 
                                    path: data.path, 
                                    name: data.name,
                                    enabled: true,
                                    metadata: metadata,
                                    defaultValues: defaultValues,
                                    properties: defaultValues
                                  }];
                                  updateObjectProperty(selection.entity, 'scripts', newScripts);
                                  
                                  if (babylonObject?.metadata?.scriptProperties) {
                                    setScriptPropertiesSignal({ ...babylonObject.metadata.scriptProperties });
                                  }
                                  
                                  editorActions.addConsoleMessage(`Script "${data.name}" attached and started`, 'success');
                                } else {
                                  editorActions.addConsoleMessage(`Cannot attach "${data.name}" - check console for details`, 'error');
                                }
                              }
                            }
                          }
                        } catch (err) {
                          console.warn('Invalid drop data:', droppedData);
                        }
                      }}
                    >
                      <div class="flex flex-col items-center gap-2 p-4">
                        <IconCode class="w-5 h-5 text-base-content/40" />
                        <div class="text-base-content/60 text-sm">drop scripts here</div>
                        <div class="text-xs text-base-content/40">.ren, .js, .jsx, .ts, .tsx</div>
                      </div>
                    </div>
                  </div>
                  </div>
                </Show>
              </div>

              {/* Script Properties */}
              <Show when={Object.keys(scriptPropertiesSignal()).length > 0}>
                {(() => {
                  const metadataVersion = scriptMetadataVersion();
                  const babylonObject = getSelectedBabylonObject();
                  if (!babylonObject) {
                    return null;
                  }
                  
                  const runtime = getScriptRuntime();
                  const scripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
                  const scriptInstances = scripts.map(s => s.instance).filter(Boolean);
                  
                  return (
                    <For each={scriptInstances}>
                      {(scriptInstance) => {
                        const scriptAPI = scriptInstance._scriptAPI;
                        if (!scriptAPI) {
                          return null;
                        }
                        
                        const propertiesBySection = scriptAPI.getScriptPropertiesBySection?.() || {};
                        
                        return (
                          <Show when={Object.keys(propertiesBySection).length > 0}>
                            <For each={Object.entries(propertiesBySection)}>
                              {([sectionName, properties]) => (
                                <div class="bg-base-100 border-base-300 border rounded-lg">
                                  <div class="!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 bg-primary/15 text-white rounded-t-lg">
                                    <div class="flex items-center gap-1.5">
                                      <IconCode class="w-3 h-3" />
                                      {sectionName}
                                    </div>
                                    <div class="flex items-center gap-1">
                                      <button
                                        onClick={(e) => {
                                          e.preventDefault();
                                          e.stopPropagation();
                                          toggleScriptPause(scriptInstance._scriptPath);
                                        }}
                                        class={`btn btn-xs btn-circle ${
                                          isScriptPaused(scriptInstance._scriptPath)
                                            ? 'btn-success hover:btn-success'
                                            : 'btn-warning hover:btn-warning'
                                        }`}
                                        title={`${isScriptPaused(scriptInstance._scriptPath) ? 'Resume' : 'Pause'} script`}
                                      >
                                        {isScriptPaused(scriptInstance._scriptPath) ? 
                                          <IconPlayerPlay class="w-3 h-3" /> : 
                                          <IconPlayerPause class="w-3 h-3" />
                                        }
                                      </button>
                                      <button
                                        onClick={async (e) => {
                                          e.preventDefault();
                                          e.stopPropagation();
                                          
                                          if (!selection.entity) return;
                                          
                                          const scriptPath = scriptInstance._scriptPath;
                                          const scriptName = scriptPath.split('/').pop();
                                          
                                          // Detach script from runtime
                                          const runtime = getScriptRuntime();
                                          const success = await runtime.detachScript(selection.entity, scriptPath);
                                          
                                          if (success) {
                                            // Remove script properties from babylon object
                                            const babylonObject = getSelectedBabylonObject();
                                            if (babylonObject && scriptInstance) {
                                              removeScriptProperties(babylonObject, scriptInstance);
                                            }
                                            
                                            // Update object properties to remove script from UI
                                            const currentScripts = objectProps.scripts || [];
                                            const newScripts = currentScripts.filter(s => s.path !== scriptPath);
                                            updateObjectProperty(selection.entity, 'scripts', newScripts);
                                            
                                            // Clear the paused state for this script
                                            const scriptKey = `${selection.entity}:${scriptPath}`;
                                            const newPaused = new Set(pausedScripts());
                                            newPaused.delete(scriptKey);
                                            setPausedScripts(newPaused);
                                            
                                            // Update script properties signal
                                            if (babylonObject?.metadata?.scriptProperties) {
                                              setScriptPropertiesSignal({ ...babylonObject.metadata.scriptProperties });
                                            }
                                            
                                            editorActions.addConsoleMessage(`Script "${scriptName}" detached`, 'info');
                                          } else {
                                            editorActions.addConsoleMessage(`Failed to detach script "${scriptName}"`, 'error');
                                          }
                                        }}
                                        class="btn btn-xs btn-circle btn-error hover:btn-error"
                                        title="Detach script"
                                      >
                                        <IconX class="w-3 h-3" />
                                      </button>
                                    </div>
                                  </div>
                                  <div class="!p-2">
                                    <div class="space-y-0.5">
                                      <For each={properties}>
                                        {(property) => (
                                          <ScriptPropertyInput property={property} babylonObject={babylonObject} />
                                        )}
                                      </For>
                                    </div>
                                  </div>
                                </div>
                              )}
                            </For>
                          </Show>
                        );
                      }}
                    </For>
                  );
                })()}
              </Show>

              {/* Reset All Properties Button */}
              <Show when={selection.entity}>
                <div class="p-1">
                  <button 
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      resetToDefaults();
                    }}
                    class="btn btn-outline btn-error btn-xs w-full"
                  >
                    Reset All Properties
                  </button>
                </div>
              </Show>
            </>
          );
        })()}
      </div>
    </div>
  );
}