import { createSignal, createMemo, onCleanup, onMount, createEffect, For, Show, Switch, Match, createComponent } from 'solid-js';
import { CodeSlash, X, Reset } from '@/ui/icons';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { objectPropertiesActions, objectPropertiesStore } from '@/layout/stores/ViewportStore';
import { renderStore } from '@/render/store';
import { CollapsibleSection } from '@/ui';
import { getScriptRuntime } from '@/api/script';
import { bridgeService } from '@/plugins/core/bridge';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';

function ObjectProperties() {
  //console.log('🏗️ ObjectProperties component created');
  const { selection } = editorStore;
  const { updateObjectProperty } = objectPropertiesActions;
  const [isDragOverScript, setIsDragOverScript] = createSignal(false);
  const [isResettingProperties, setIsResettingProperties] = createSignal(false);
  const [scriptSearchTerm, setScriptSearchTerm] = createSignal('');
  const [searchResults, setSearchResults] = createSignal([]);
  const [isSearching, setIsSearching] = createSignal(false);
  const [showSearchResults, setShowSearchResults] = createSignal(false);
  
  // Individual signals for each property type
  //console.log('🏗️ Creating signals');
  const [positionSignal, setPositionSignal] = createSignal([0, 0, 0]);
  const [rotationSignal, setRotationSignal] = createSignal([0, 0, 0]);
  const [scaleSignal, setScaleSignal] = createSignal([1, 1, 1]);
  const [scriptPropertiesSignal, setScriptPropertiesSignal] = createSignal({});
  const [scriptMetadataVersion, setScriptMetadataVersion] = createSignal(0);
  let previousScriptSections = {};

  // Cache for RenScript directory structure
  let scriptCache = null;
  let cacheTimestamp = 0;
  const CACHE_DURATION = 30000; // 30 seconds

  // Function to load and cache all RenScript files
  const loadScriptCache = async () => {
    try {
      //console.log('📂 Loading RenScript cache...');
      //console.log('📂 Requesting directory: renscripts');
      const directories = await bridgeService.listDirectory('renscripts');
      //console.log('📂 Found directories:', directories);
      const allScripts = [];
      
      for (const dir of directories) {
        //console.log('📂 Processing directory:', dir);
        /*console.log('📂 Directory properties:', { 
          name: dir.name, 
          isDirectory: dir.isDirectory, 
          is_directory: dir.is_directory,
          type: typeof dir.isDirectory,
          keys: Object.keys(dir)
        });*/
        if (dir.isDirectory || dir.is_directory) {
          try {
            //console.log(`📂 Reading subdirectory: renscripts/${dir.name}`);
            const dirContents = await bridgeService.listDirectory(`renscripts/${dir.name}`);
            //console.log(`📂 Contents of ${dir.name}:`, dirContents);
            const scriptFiles = dirContents.filter(file => file.name.endsWith('.ren'));
            //console.log(`📂 Script files in ${dir.name}:`, scriptFiles);
            
            scriptFiles.forEach(script => {
              const scriptEntry = {
                name: script.name.replace('.ren', ''),
                path: `renscripts/${dir.name}/${script.name}`,
                directory: dir.name,
                searchableText: `${dir.name} ${script.name}`.toLowerCase()
              };
              //console.log('📂 Adding script:', scriptEntry);
              allScripts.push(scriptEntry);
            });
          } catch (dirError) {
            console.warn(`❌ Error reading directory ${dir.name}:`, dirError);
          }
        } else {
          //console.log('📂 Skipping non-directory:', dir);
        }
      }
      
      scriptCache = allScripts;
      cacheTimestamp = Date.now();
      //console.log(`📂 RenScript cache loaded: ${allScripts.length} scripts`);
      return allScripts;
    } catch (error) {
      console.warn('Error loading script cache:', error);
      return [];
    }
  };

  // Dynamic script search using cached data
  const searchScripts = async (searchTerm) => {
    //console.log('🔍 searchScripts called with term:', searchTerm);
    
    if (!searchTerm || searchTerm.length < 1) {
      //console.log('🔍 Empty search term, clearing results');
      setSearchResults([]);
      setShowSearchResults(false);
      return;
    }
    
    //console.log('🔍 Starting search for:', searchTerm);
    setIsSearching(true);
    try {
      // Check if cache is valid
      const now = Date.now();
      const cacheExpired = !scriptCache || (now - cacheTimestamp) > CACHE_DURATION;
      /*console.log('🔍 Cache status:', { 
        hasCache: !!scriptCache, 
        cacheTimestamp, 
        now, 
        expired: cacheExpired,
        cacheAge: now - cacheTimestamp 
      });*/
      
      if (cacheExpired) {
        //console.log('📂 Cache expired or missing, reloading...');
        await loadScriptCache();
        //console.log('📂 Cache reload complete. New cache:', scriptCache);
      } else {
        //console.log('📂 Using cached RenScript data, cache size:', scriptCache?.length);
      }
      
      if (!scriptCache || scriptCache.length === 0) {
        console.warn('⚠️ No scripts in cache after loading!');
        setSearchResults([]);
        setShowSearchResults(false);
        return;
      }
      
      // Filter cached scripts based on search term
      const searchTermLower = searchTerm.toLowerCase();
      //console.log('🔍 Filtering scripts with term:', searchTermLower);
      //console.log('🔍 Available scripts:', scriptCache.map(s => s.searchableText));
      
      const matchingScripts = scriptCache.filter(script => {
        const matches = script.searchableText.includes(searchTermLower);
        //console.log(`🔍 Script "${script.name}" (${script.searchableText}) matches "${searchTermLower}":`, matches);
        return matches;
      });
      
      //console.log('🔍 Final matching scripts:', matchingScripts);
      setSearchResults(matchingScripts);
      setShowSearchResults(matchingScripts.length > 0);
      //console.log('📜 Script search results:', matchingScripts);
    } catch (error) {
      console.warn('❌ Error searching scripts:', error);
      setSearchResults([]);
      setShowSearchResults(false);
    } finally {
      setIsSearching(false);
      //console.log('🔍 Search complete');
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
    //console.log('🔍 getSelectedBabylonObject called, selection.entity:', selection.entity);
    if (!selection.entity || !renderStore.scene) {
      //console.log('🔍 No entity or scene, returning null');
      return null;
    }
    
    // Find the Babylon object by ID
    const allObjects = [...renderStore.scene.meshes, ...renderStore.scene.transformNodes, ...renderStore.scene.lights, ...renderStore.scene.cameras];
    const found = allObjects.find(obj => (obj.uniqueId || obj.name) === selection.entity);
    //console.log('🔍 Found babylon object:', found?.name || 'null');
    return found;
  };

  // Unified property sync: Set up render loop observer for selected object
  let renderObserver = null;
  let currentObservedEntity = null;
  
  // Separate the entity tracking from signal updates
  createEffect(() => {
    //console.log('🔄 createEffect triggered for entity selection change');
    const entityId = selection.entity;
    //console.log('🔄 Entity changed to:', entityId);
    
    // Only proceed if entity actually changed
    if (currentObservedEntity === entityId) {
      //console.log('🔄 Same entity, skipping observer setup');
      return;
    }
    
    currentObservedEntity = entityId;
    
    // Clean up previous observer
    if (renderObserver && renderStore.scene) {
      //console.log('🔄 Cleaning up previous render observer');
      renderStore.scene.unregisterBeforeRender(renderObserver);
      renderObserver = null;
    }
    
    if (!entityId || !renderStore.scene) {
      //console.log('🔄 No entity or scene, skipping observer setup');
      return;
    }
    
    // Get babylon object without triggering reactivity
    const allObjects = [...renderStore.scene.meshes, ...renderStore.scene.transformNodes, ...renderStore.scene.lights, ...renderStore.scene.cameras];
    const babylonObject = allObjects.find(obj => (obj.uniqueId || obj.name) === entityId);
    
    if (!babylonObject) {
      //console.log('🔄 No babylon object found for entity:', entityId);
      return;
    }
    
    //console.log('🔄 Setting up observer for babylon object:', babylonObject.name);
    
    
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
    
    // Capture original transform values when object is first selected
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
    
    // Register render loop observer for live updates
    renderObserver = () => {
      //console.log('🔄 Render observer called');
      // Skip sync during reset operations
      if (isResettingProperties()) {
        //console.log('🔄 Skipping sync - reset in progress');
        return;
      }
      
      // Update individual transform signals
      if (babylonObject.position) {
        const newPosition = [babylonObject.position.x, babylonObject.position.y, babylonObject.position.z];
        const currentPosition = positionSignal();
        if (newPosition.some((val, i) => val !== currentPosition[i])) {
          //console.log('🔄 Position changed, updating signal:', newPosition);
          setPositionSignal(newPosition);
        }
      }
      
      if (babylonObject.rotation) {
        const newRotation = [babylonObject.rotation.x, babylonObject.rotation.y, babylonObject.rotation.z];
        const currentRotation = rotationSignal();
        if (newRotation.some((val, i) => val !== currentRotation[i])) {
          //console.log('🔄 Rotation changed, updating signal:', newRotation);
          setRotationSignal(newRotation);
        }
      }
      
      if (babylonObject.scaling) {
        const newScale = [babylonObject.scaling.x, babylonObject.scaling.y, babylonObject.scaling.z];
        const currentScale = scaleSignal();
        if (newScale.some((val, i) => val !== currentScale[i])) {
          //console.log('🔄 Scale changed, updating signal:', newScale);
          setScaleSignal(newScale);
        }
      } else if (babylonObject.scale) {
        const newScale = [babylonObject.scale.x, babylonObject.scale.y, babylonObject.scale.z];
        const currentScale = scaleSignal();
        if (newScale.some((val, i) => val !== currentScale[i])) {
          //console.log('🔄 Scale (scale prop) changed, updating signal:', newScale);
          setScaleSignal(newScale);
        }
      }
      
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
        //console.log('🔄 Script properties changed, updating signal:', scriptProperties);
        setScriptPropertiesSignal(scriptProperties);
        
        // Check if script metadata changed when properties change (indicates script reload)
        //console.log('🔄 Checking script metadata changes');
        const runtime = getScriptRuntime();
        const scripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
        const scriptInstances = scripts.map(s => s.instance).filter(Boolean);
        //console.log('🔄 Found script instances:', scriptInstances.length);
        
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
          //console.log('🔄 Script metadata sections changed!');
          //console.log('🔄 Previous sections:', previousSectionsStr);
          //console.log('🔄 Current sections:', currentSectionsStr);
          previousScriptSections = currentScriptSections;
          setScriptMetadataVersion(prev => {
            //console.log('🔄 Incrementing script metadata version from', prev, 'to', prev + 1);
            return prev + 1;
          });
          //console.log('🔄 Script metadata changed - section structure updated');
        }
      }
    };
    
    //console.log('🔄 Registering render observer');
    renderStore.scene.registerBeforeRender(renderObserver);
    
    
    // Initial sync
    //console.log('🔄 Running initial sync');
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
      
      //console.log(`📝 Storing default: ${prop.name} = ${prop.defaultValue}`);
    });
    
    //console.log('📝 Script defaults added to babylon metadata:', babylonObject.metadata.scriptProperties);
  };

  // Helper function to completely remove script properties when script is detached
  const removeScriptProperties = (babylonObject, scriptInstance) => {
    if (!babylonObject?.metadata || !scriptInstance?._scriptAPI) return;
    
    const scriptAPI = scriptInstance._scriptAPI;
    const scriptProperties = scriptAPI.getScriptProperties?.() || [];
    
    //console.log('🗑️ Removing script properties from Babylon metadata:', scriptProperties.map(p => p.name));
    
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
      
      //console.log(`🗑️ Completely removed script property: ${prop.name}`);
    });
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
    
    //console.log('🔄 Main reset button: Resetting all object properties');
    
    // Temporarily disable live sync to prevent it from overriding reset values
    setIsResettingProperties(true);
    
    try {
      const originalProperties = babylonObject.metadata.originalProperties;
      //console.log('🔄 Resetting to original values:', originalProperties);
      
      // Reset transform properties
      if (originalProperties.position && babylonObject.position) {
        babylonObject.position.set(originalProperties.position[0], originalProperties.position[1], originalProperties.position[2]);
        //console.log('✅ Reset position to:', originalProperties.position);
      }
      if (originalProperties.rotation && babylonObject.rotation) {
        babylonObject.rotation.set(originalProperties.rotation[0], originalProperties.rotation[1], originalProperties.rotation[2]);
        //console.log('✅ Reset rotation to:', originalProperties.rotation);
      }
      if (originalProperties.scale) {
        if (babylonObject.scaling) {
          babylonObject.scaling.set(originalProperties.scale[0], originalProperties.scale[1], originalProperties.scale[2]);
        } else if (babylonObject.scale) {
          babylonObject.scale.set(originalProperties.scale[0], originalProperties.scale[1], originalProperties.scale[2]);
        }
        //console.log('✅ Reset scale to:', originalProperties.scale);
      }
      
      // Reset script properties
      const runtime = getScriptRuntime();
      Object.keys(originalProperties).forEach(propName => {
        // Skip transform properties (already handled above)
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
        
        //console.log(`✅ Reset script property ${propName} to original value: ${originalValue}`);
      });
      
      //console.log('✅ Main reset completed - all properties reset to original values');
      
    } finally {
      // Re-enable live sync after a short delay
      setTimeout(() => {
        setIsResettingProperties(false);
        //console.log('🔄 Live sync re-enabled after reset');
      }, 100);
    }
  };

  const isNodeControlled = (propertyPath) => {
    const objectProps = objectPropertiesStore.objects[selection.entity];
    return objectProps?.nodeBindings && objectProps.nodeBindings[propertyPath];
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
      
      //console.log(`🔧 Reset ${transformType} to`, defaultValue);
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
    <div className="mb-3">
      <div className="flex items-center justify-between mb-1">
        <label className="block text-xs text-base-content/60">{label}</label>
        <button
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            //console.log('🔄 Individual transform reset clicked for:', propertyPath);
            
            const babylonObject = getSelectedBabylonObject();
            if (!babylonObject?.metadata?.originalProperties) return;
            
            // Temporarily disable live sync during individual reset
            setIsResettingProperties(true);
            
            const transformType = propertyPath.split('.')[1];
            const originalValue = babylonObject.metadata.originalProperties[transformType];
            //console.log(`🔄 Resetting ${transformType} to:`, originalValue);
            
            if (originalValue) {
              resetSingleProperty(propertyPath, originalValue);
            }
            
            // Re-enable live sync after a short delay
            setTimeout(() => {
              setIsResettingProperties(false);
              //console.log('🔄 Live sync re-enabled after individual transform reset');
            }, 50);
          }}
          className="p-0.5 rounded hover:bg-base-300/50 text-base-content/40 hover:text-base-content/60 transition-all duration-150"
          title={`Reset ${label.toLowerCase()}`}
        >
          <Reset className="w-3 h-3" />
        </button>
      </div>
      <div className="grid grid-cols-3 gap-1">
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
                  //console.log(`🔧 Transform input focused: ${propertyPath}[${index()}]`);
                  keyboardShortcuts.disable();
                }}
                onBlur={() => {
                  keyboardShortcuts.enable();
                }}
                onMouseDown={() => {}}
                onChange={(e) => {
                  const newValue = parseFloat(e.target.value) || 0;
                  //console.log(`🔧 Transform input changed: ${propertyPath}[${index()}] = ${newValue}`);
                  
                  const babylonObject = getSelectedBabylonObject();
                  if (!babylonObject) {
                    console.warn('🔧 No babylon object found for transform update');
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
                    //console.log(`🔧 Updated position[${axisIndex}] to ${newValue}, new position:`, newPosition);
                  } else if (transformType === 'rotation' && babylonObject.rotation) {
                    if (axisIndex === 0) babylonObject.rotation.x = newValue;
                    else if (axisIndex === 1) babylonObject.rotation.y = newValue;
                    else if (axisIndex === 2) babylonObject.rotation.z = newValue;
                    
                    // Update rotation signal immediately for instant UI response
                    const newRotation = [babylonObject.rotation.x, babylonObject.rotation.y, babylonObject.rotation.z];
                    setRotationSignal(newRotation);
                    //console.log(`🔧 Updated rotation[${axisIndex}] to ${newValue}, new rotation:`, newRotation);
                  } else if (transformType === 'scale' && babylonObject.scaling) {
                    if (axisIndex === 0) babylonObject.scaling.x = newValue;
                    else if (axisIndex === 1) babylonObject.scaling.y = newValue;
                    else if (axisIndex === 2) babylonObject.scaling.z = newValue;
                    
                    // Update scale signal immediately for instant UI response
                    const newScale = [babylonObject.scaling.x, babylonObject.scaling.y, babylonObject.scaling.z];
                    setScaleSignal(newScale);
                    //console.log(`🔧 Updated scaling[${axisIndex}] to ${newValue}, new scaling:`, newScale);
                  }
                }}
                className={`w-full text-xs p-1.5 pl-7 pr-1.5 rounded text-center focus:outline-none focus:ring-1 focus:ring-primary ${
                  isNodeControlled(`${propertyPath}.${index()}`) 
                    ? 'border-primary bg-primary/20 text-primary' 
                    : 'border-base-300 bg-secondary/10 text-base-content'
                } border`}
                disabled={isNodeControlled(`${propertyPath}.${index()}`)}
              />
            </div>
          )}
        </For>
      </div>
      <Show when={isNodeControlled(propertyPath)}>
        <div className="text-xs text-primary mt-1">Controlled by node</div>
      </Show>
    </div>
    );
  };

  // Extract script property input as a separate component to avoid reactive issues
  const ScriptPropertyInput = (props) => {
    const { property, babylonObject } = props;
    
    //console.log(`🔍 ScriptPropertyInput CREATED for: ${property.name}`);
    //console.log(`🔍 ScriptPropertyInput props:`, property);
    
    // Use a simple function instead of createMemo to avoid circular dependencies
    const getCurrentValue = () => {
      //console.log(`🔍 getCurrentValue CALLED for: ${property.name}`);
      //console.log(`🔍 babylonObject metadata:`, babylonObject?.metadata);
      if (!babylonObject?.metadata?.scriptProperties) {
        //console.log(`🔍 No script properties in metadata, using default:`, property.defaultValue);
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
        //console.log(`🎛️ UI Script property change: ${propertyName} = ${newValue}`);
        //console.log(`🎛️ Property change call stack:`);
        console.trace();
        
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
        //console.log(`🎛️ Updating babylon object metadata for:`, babylonObject.name);
        
        if (!babylonObject.metadata) babylonObject.metadata = {};
        if (!babylonObject.metadata.scriptProperties) babylonObject.metadata.scriptProperties = {};
        
        //console.log(`🎛️ Setting ${propertyName} = ${newValue} in babylon metadata`);
        babylonObject.metadata.scriptProperties[propertyName] = newValue;
        //console.log(`🎛️ Babylon metadata after update:`, babylonObject.metadata.scriptProperties);
        
        // Also update through ScriptAPI for script instance synchronization
        //console.log(`🎛️ Updating script instances for entity:`, selection.entity);
        const runtime = getScriptRuntime();
        const scripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
        //console.log(`🎛️ Found ${scripts.length} scripts to update`);
        scripts.forEach(script => {
          const instance = script.instance;
          if (instance?._scriptAPI?.setScriptProperty) {
            try {
              //console.log(`🎛️ Calling setScriptProperty on instance for ${propertyName}`);
              instance._scriptAPI.setScriptProperty(propertyName, newValue);
              //console.log(`🎛️ Successfully updated script instance property`);
            } catch (scriptError) {
              console.error('Error updating script instance:', scriptError);
            }
          } else {
            //console.log(`🎛️ No setScriptProperty method available on script instance`);
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
              //console.log('🔄 Individual script property reset clicked for:', property.name);
              
              const babylonObject = getSelectedBabylonObject();
              if (!babylonObject?.metadata?.originalProperties) return;
              
              // Temporarily disable live sync during individual reset
              setIsResettingProperties(true);
              
              const originalValue = babylonObject.metadata.originalProperties[property.name] ?? property.defaultValue;
              //console.log(`🔄 Resetting script property ${property.name} from ${getCurrentValue()} to:`, originalValue);
              
              handlePropertyChange(property.name, originalValue);
              
              // Re-enable live sync after a short delay
              setTimeout(() => {
                setIsResettingProperties(false);
                //console.log('🔄 Live sync re-enabled after individual script property reset');
              }, 50);
            }}
            className="p-0.5 rounded hover:bg-base-300/50 text-base-content/40 hover:text-base-content/60 transition-all duration-150"
            title={`Reset ${property.name.replace(/_/g, ' ')} to default`}
          >
            <Reset className="w-3 h-3" />
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
                      //console.log('🎨 Dropped material file:', data.path, 'onto property:', property.name);
                      handlePropertyChange(property.name, data.path);
                    }
                    // Check if it's a texture file (common texture formats)
                    else if (/\.(jpg|jpeg|png|webp|tga|bmp|dds|hdr|ktx)$/i.test(data.name)) {
                      //console.log('🖼️ Dropped texture file:', data.path, 'onto property:', property.name);
                      handlePropertyChange(property.name, data.path);
                    }
                  }
                } catch (err) {
                  console.warn('Invalid drop data for string property:', droppedData);
                }
              }}
            >
              <Show when={getCurrentValue() && getCurrentValue().trim() !== ''} fallback={
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
            <div className="space-y-3">
              <div className="flex items-center gap-3">
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

  //console.log('🎨 ObjectProperties render cycle started');
  //console.log('🎨 Current selection.entity:', selection.entity);
  //console.log('🎨 Position signal:', positionSignal());
  //console.log('🎨 Rotation signal:', rotationSignal());
  //console.log('🎨 Scale signal:', scaleSignal());
  //console.log('🎨 Script properties signal:', Object.keys(scriptPropertiesSignal()));
  
  return (
    <div className="flex flex-col h-full" data-object-properties>
      {/* Header with object name and reset button */}
      <Show when={selection.entity}>
        <div 
          className="flex items-center justify-between p-2 border-b border-base-content/10 bg-base-200/50 cursor-row-resize hover:bg-base-200/70 transition-colors duration-150 select-none"
          onMouseDown={(e) => {
            // Don't trigger resize if clicking on the reset button
            if (e.target.closest('button')) {
              return;
            }
            
            e.preventDefault();
            e.stopPropagation();
            //console.log('🔧 Object Properties header drag started');
            
            let startY = e.clientY;
            let startHeight = parseInt(getComputedStyle(document.querySelector('[data-object-properties]')).height);
            
            const handleMouseMove = (moveEvent) => {
              const deltaY = startY - moveEvent.clientY; // Inverted because we want to resize from top
              const newHeight = Math.max(200, Math.min(800, startHeight + deltaY));
              
              // Update the height via EditorStore action
              editorActions.setScenePropertiesHeight(newHeight);
            };
            
            const handleMouseUp = () => {
              document.removeEventListener('mousemove', handleMouseMove);
              document.removeEventListener('mouseup', handleMouseUp);
              //console.log('🔧 Object Properties header drag ended');
            };
            
            document.addEventListener('mousemove', handleMouseMove);
            document.addEventListener('mouseup', handleMouseUp);
          }}
        >
          <div className="text-sm font-medium text-base-content/80 hover:text-base-content/50 transition-colors duration-150">
            {getSelectedBabylonObject()?.name || 'Unknown Object'} Properties
          </div>
          <button
            onMouseDown={() => {}}
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              //console.log('🔄 Main reset button clicked');
              resetToDefaults();
            }}
            className="p-1.5 rounded hover:bg-base-300/50 text-base-content/60 hover:text-base-content transition-all duration-150 active:scale-95 pointer-events-auto"
            title="Reset to defaults"
          >
            <Reset className="w-4 h-4" />
          </button>
        </div>
      </Show>
      
      {/* Scrollable content */}
      <div className="overflow-y-auto flex-1" style="scrollbar-width: thin; scrollbar-color: rgba(255,255,255,0.3) rgba(0,0,0,0.1);"
        onWheel={(e) => e.stopPropagation()}>
        {(() => {
        let objectProps = objectPropertiesStore.objects[selection.entity];
        
        if (!objectProps && selection.entity) {
          objectPropertiesActions.ensureDefaultComponents(selection.entity);
          objectProps = objectPropertiesStore.objects[selection.entity];
        }
        
        if (!objectProps) {
          return (
            <div className="p-4 text-base-content/50 text-sm">
              No object selected.
            </div>
          );
        }
        
        return (
          <div className="space-y-0">
            <CollapsibleSection title="Scripts" defaultOpen={true} index={0}>
              <div>
                {/* Script Search Box */}
                <div className="mb-3 relative">
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
                      // Delay hiding to allow clicking on dropdown items
                      setTimeout(() => setShowSearchResults(false), 150);
                    }}
                    className="input input-sm w-full text-sm rounded-none bg-gradient-to-b from-base-300/80 to-base-300 border-0 focus:outline-none"
                  />
                  
                  {/* Available Scripts Dropdown */}
                  <Show when={showSearchResults() && scriptSearchTerm() && searchResults().length > 0}>
                    <div className="absolute top-full left-0 right-0 z-50 max-h-32 overflow-y-auto bg-base-100 border border-base-300 rounded shadow-lg">
                      <For each={searchResults()}>
                        {(script) => (
                          <div 
                            className="p-2 hover:bg-base-200 cursor-pointer border-b border-base-300/50 last:border-b-0"
                            onClick={async () => {
                              if (!selection.entity) return;
                              
                              //console.log('🔧 Adding script from search:', script.path, 'to', selection.entity);
                              
                              const currentScripts = objectProps.scripts || [];
                              if (!currentScripts.find(s => s.path === script.path)) {
                                // Use script runtime to attach and start the script
                                const runtime = getScriptRuntime();
                                const success = await runtime.attachScript(selection.entity, script.path);
                                
                                if (success) {
                                  // Get script instance and add its defaults to Babylon metadata
                                  const scriptInstance = runtime.getScriptInstance(selection.entity, script.path);
                                  const babylonObject = getSelectedBabylonObject();
                                  
                                  if (scriptInstance && babylonObject) {
                                    // Add script defaults to originalProperties in Babylon metadata
                                    addScriptDefaults(babylonObject, scriptInstance);
                                  }
                                  
                                  const metadata = scriptInstance?._scriptProperties || [];
                                  const defaultValues = {};
                                  
                                  //console.log('📝 Script metadata:', metadata);
                                  
                                  // Extract default values from metadata (for UI compatibility)
                                  if (Array.isArray(metadata)) {
                                    metadata.forEach(prop => {
                                      defaultValues[prop.name] = prop.defaultValue;
                                      //console.log(`📝 Storing default: ${prop.name} = ${prop.defaultValue}`);
                                    });
                                  }
                                  
                                  const newScripts = [...currentScripts, { 
                                    path: script.path, 
                                    name: script.name + '.ren', // Add .ren extension back for display
                                    enabled: true,
                                    metadata: metadata,
                                    defaultValues: defaultValues,
                                    properties: defaultValues
                                  }];
                                  updateObjectProperty(selection.entity, 'scripts', newScripts);
                                  
                                  // Force immediate script properties signal update
                                  if (babylonObject?.metadata?.scriptProperties) {
                                    setScriptPropertiesSignal({ ...babylonObject.metadata.scriptProperties });
                                    //console.log('🔄 Forced script properties signal update after attachment');
                                  }
                                  
                                  editorActions.addConsoleMessage(`Script "${script.name}" attached and started`, 'success');
                                  setScriptSearchTerm(''); // Clear search
                                  setShowSearchResults(false); // Hide dropdown
                                } else {
                                  editorActions.addConsoleMessage(`Cannot attach "${script.name}" - check console for details`, 'error');
                                }
                              } else {
                                editorActions.addConsoleMessage(`Script "${script.name}" already attached`, 'warning');
                              }
                            }}
                          >
                            <div className="flex items-center gap-2">
                              <CodeSlash className="w-4 h-4 text-secondary" />
                              <div className="flex flex-col">
                                <span className="text-sm font-medium">{script.name}</span>
                                <span className="text-xs text-base-content/50">{script.directory}/</span>
                              </div>
                            </div>
                          </div>
                        )}
                      </For>
                    </div>
                  </Show>
                </div>
                
                {/* Attached Scripts List - Outside the drop zone */}
                <Show when={objectProps.scripts && objectProps.scripts.length > 0}>
                  <div>
                    <For each={objectProps.scripts}>
                      {(script, index) => (
                        <div className="flex items-center justify-between bg-base-200 border border-base-300 px-2 py-1.5 shadow-sm">
                          <div className="flex items-center gap-2 min-w-0 flex-1">
                            <input
                              id={`script-enable-${index()}-${selection.entity || 'unknown'}`}
                              type="checkbox"
                              checked={script.enabled !== false}
                              onChange={async (e) => {
                                const runtime = getScriptRuntime();
                                const entityId = selection.entity;
                                
                                if (e.target.checked) {
                                  // Enable script - attach it to runtime
                                  const success = await runtime.attachScript(entityId, script.path);
                                  if (success) {
                                    // Update the enabled state in the UI
                                    const updatedScripts = objectProps.scripts.map((s, i) => 
                                      i === index() ? { ...s, enabled: true } : s
                                    );
                                    updateObjectProperty(entityId, 'scripts', updatedScripts);
                                    editorActions.addConsoleMessage(`Enabled script "${script.name}"`, 'success');
                                  } else {
                                    editorActions.addConsoleMessage(`Cannot enable "${script.name}" - check console for details`, 'error');
                                  }
                                } else {
                                  // Disable script - detach from runtime
                                  runtime.detachScript(entityId, script.path);
                                  // Update the enabled state in the UI
                                  const updatedScripts = objectProps.scripts.map((s, i) => 
                                    i === index() ? { ...s, enabled: false } : s
                                  );
                                  updateObjectProperty(entityId, 'scripts', updatedScripts);
                                  editorActions.addConsoleMessage(`Disabled script "${script.name}"`, 'info');
                                }
                              }}
                              className="toggle toggle-xs toggle-success flex-shrink-0"
                              title={script.enabled !== false ? "Disable script" : "Enable script"}
                            />
                            <div className="flex flex-col min-w-0 flex-1">
                              <span className={`text-sm font-medium truncate ${
                                script.enabled !== false ? 'text-base-content' : 'text-base-content/40'
                              }`} title={script.name}>{script.name}</span>
                              {script.enabled === false && (
                                <span className="text-xs text-warning">Disabled</span>
                              )}
                            </div>
                          </div>
                          <button
                            onClick={() => {
                              //console.log('🔧 Removing script:', script.path, 'from', selection.entity);
                              
                              // Get script instance before detaching to clean up properties
                              const runtime = getScriptRuntime();
                              const scriptInstance = runtime.getScriptInstance(selection.entity, script.path);
                              const babylonObject = getSelectedBabylonObject();
                              
                              // Remove script properties from Babylon metadata
                              if (scriptInstance && babylonObject) {
                                removeScriptProperties(babylonObject, scriptInstance);
                              }
                              
                              // Detach from runtime
                              runtime.detachScript(selection.entity, script.path);
                              
                              // Update UI
                              const updatedScripts = objectProps.scripts.filter((_, i) => i !== index());
                              updateObjectProperty(selection.entity, 'scripts', updatedScripts);
                              
                              // Force immediate cleanup of script properties signals
                              if (babylonObject?.metadata?.scriptProperties) {
                                // Clear any properties that belonged to this script
                                const remainingScripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
                                
                                // Rebuild script properties from remaining scripts only
                                const remainingProperties = {};
                                remainingScripts.forEach(remainingScript => {
                                  const scriptAPI = remainingScript.instance?._scriptAPI;
                                  if (scriptAPI && scriptAPI.getScriptProperties) {
                                    const props = scriptAPI.getScriptProperties();
                                    props.forEach(prop => {
                                      remainingProperties[prop.name] = babylonObject.metadata.scriptProperties[prop.name];
                                    });
                                  }
                                });
                                
                                babylonObject.metadata.scriptProperties = remainingProperties;
                                setScriptPropertiesSignal(remainingProperties);
                                //console.log('🗑️ Forced script properties signal update after removal');
                              }
                              
                              editorActions.addConsoleMessage(`Script "${script.name}" removed`, 'info');
                            }}
                            className="p-1.5 hover:bg-base-300 rounded transition-colors"
                          >
                            <X className="w-4 h-4 text-base-content/60 hover:text-error" />
                          </button>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>

                {/* Drop Zone */}
                <div 
                  data-drop-zone="scripts"
                  className={`min-h-[60px] bg-base-200/30 text-center ${isDragOverScript() ? 'animate-pulse brightness-110' : ''}`}
                  onDragOver={(e) => {
                    e.preventDefault();
                    // Check if it's a script file being dragged
                    const types = Array.from(e.dataTransfer.types);
                    if (types.includes('text/plain')) {
                      // Assume it's valid for now - we'll validate on drop
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
                            //console.log('🔧 Attaching script via drag and drop:', data.path, 'to', selection.entity);
                            
                            // Use script runtime to attach and start the script
                            const runtime = getScriptRuntime();
                            const success = await runtime.attachScript(selection.entity, data.path);
                            
                            if (success) {
                              // Get script instance and add its defaults to Babylon metadata
                              const scriptInstance = runtime.getScriptInstance(selection.entity, data.path);
                              const babylonObject = getSelectedBabylonObject();
                              
                              if (scriptInstance && babylonObject) {
                                // Add script defaults to originalProperties in Babylon metadata
                                addScriptDefaults(babylonObject, scriptInstance);
                              }
                              
                              const metadata = scriptInstance?._scriptProperties || [];
                              const defaultValues = {};
                              
                              //console.log('📝 Script metadata:', metadata);
                              
                              // Extract default values from metadata (for UI compatibility)
                              if (Array.isArray(metadata)) {
                                metadata.forEach(prop => {
                                  defaultValues[prop.name] = prop.defaultValue;
                                  //console.log(`📝 Storing default: ${prop.name} = ${prop.defaultValue}`);
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
                              
                              // Force signal update to ensure properties are displayed
                              if (babylonObject?.metadata?.scriptProperties) {
                                setScriptPropertiesSignal({ ...babylonObject.metadata.scriptProperties });
                                //console.log('🔧 ObjectProperties: Forced signal update after drag-and-drop script attachment');
                              }
                              
                              editorActions.addConsoleMessage(`Script "${data.name}" attached and started`, 'success');
                            } else {
                              // The error message from ScriptManager will be more specific about type mismatches
                              editorActions.addConsoleMessage(`Cannot attach "${data.name}" - check console for details`, 'error');
                            }
                          } else {
                            //console.log('🔧 Script already attached:', data.path);
                          }
                        }
                      }
                    } catch (err) {
                      console.warn('Invalid drop data:', droppedData);
                    }
                  }}
                >
                  <div className="flex flex-col items-center gap-2 p-4">
                    <CodeSlash className="w-5 h-5 text-base-content/40" />
                    <div className="text-base-content/60 text-sm">drop scripts here</div>
                    <div className="text-xs text-base-content/40">.ren, .js, .jsx, .ts, .tsx</div>
                  </div>
                </div>
              </div>
            </CollapsibleSection>

            <Show when={positionSignal().length > 0 || rotationSignal().length > 0 || scaleSignal().length > 0}>
              <CollapsibleSection title="Transform" defaultOpen={true} index={1}>
                <div className="p-4 bg-base-100/50">
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
              </CollapsibleSection>
            </Show>

            {/* Script Properties Sections - Read from script properties signal */}
            {(() => {
              const scriptProps = scriptPropertiesSignal();
              const scriptPropsKeys = Object.keys(scriptProps);
              //console.log('🔍 SCRIPT PROPERTIES SECTION - scriptPropertiesSignal():', scriptProps);
              //console.log('🔍 SCRIPT PROPERTIES SECTION - keys:', scriptPropsKeys);
              //console.log('🔍 SCRIPT PROPERTIES SECTION - keys.length:', scriptPropsKeys.length);
              //console.log('🔍 SCRIPT PROPERTIES SECTION - Show condition result:', scriptPropsKeys.length > 0);
              return null; // Temporary return to see if this logging appears
            })()}
            <Show when={Object.keys(scriptPropertiesSignal()).length > 0}>
              {(() => {
                
                // Force reactivity by accessing the metadata version signal
                const metadataVersion = scriptMetadataVersion();
                
                const babylonObject = getSelectedBabylonObject();
                if (!babylonObject) {
                  return null;
                }
                
                //console.log(`🔍 Getting script instances for metadata version: ${metadataVersion}`);
                const runtime = getScriptRuntime();
                //console.log(`🔍 Runtime available:`, !!runtime);
                //console.log(`🔍 ScriptManager available:`, !!runtime?.scriptManager);
                //console.log(`🔍 getScriptsForObject method available:`, !!runtime?.scriptManager?.getScriptsForObject);
                const scripts = runtime?.scriptManager?.getScriptsForObject?.(selection.entity) || [];
                //console.log(`🔍 Raw scripts for entity ${selection.entity}:`, scripts);
                const scriptInstances = scripts.map(s => s.instance).filter(Boolean);
                //console.log(`🔍 Found ${scriptInstances.length} script instances`);
                //console.log(`🔍 Script instances:`, scriptInstances.map(s => ({path: s._scriptPath, hasAPI: !!s._scriptAPI})));
                
                
                
                return (
                  <For each={scriptInstances}>
                    {(scriptInstance) => {
                      
                      const scriptAPI = scriptInstance._scriptAPI;
                      if (!scriptAPI) {
                        return null;
                      }
                      
                      //console.log(`🔍 Getting properties by section for script:`, scriptInstance._scriptPath);
                      const propertiesBySection = scriptAPI.getScriptPropertiesBySection?.() || {};
                      //console.log(`🔍 Properties by section:`, propertiesBySection);
                      
                      return (
                        <Show when={Object.keys(propertiesBySection).length > 0}>
                          <For each={Object.entries(propertiesBySection)}>
                            {([sectionName, properties]) => {
                              //console.log(`🔍 Rendering section: ${sectionName} with ${properties.length} properties`);
                        //console.log(`🔍 Section properties:`, properties);
                              
                              return (
                                <CollapsibleSection
                                  title={sectionName}
                                  icon={<CodeSlash className="w-4 h-4 text-secondary" />}
                                  defaultExpanded={true}
                                >
                                  <div className="space-y-6 p-4 bg-base-100/50">
                                    <For each={properties}>
                                      {(property) => {
                                        //console.log(`🔍 About to render property: ${property.name}`);
                                        //console.log(`🔍 Property details:`, property);
                                        //console.log(`🔍 BabylonObject for property:`, babylonObject?.name);
                                        return <ScriptPropertyInput property={property} babylonObject={babylonObject} />;
                                      }}
                                    </For>
                                  </div>
                                </CollapsibleSection>
                              );
                            }}
                          </For>
                        </Show>
                      );
                    }}
                  </For>
                );
              })()}
            </Show>

            <Show when={positionSignal().length === 0 && rotationSignal().length === 0 && scaleSignal().length === 0 && Object.keys(scriptPropertiesSignal()).length === 0}>
              <div className="p-4 text-center">
                <div className="text-base-content/50 text-sm mb-2">
                  No properties available
                </div>
                <div className="text-base-content/40 text-xs">
                  Select an object to view its properties
                </div>
              </div>
            </Show>
          </div>
        );
      })()}
      </div>
    </div>
  );
}

export default ObjectProperties;