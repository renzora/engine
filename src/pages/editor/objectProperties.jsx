import { createSignal, createMemo, onCleanup, onMount, createEffect, For, Show, Switch, Match, createComponent } from 'solid-js';
import { IconX, IconRotateClockwise, IconSettings, IconArrowsMove } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { objectPropertiesActions, objectPropertiesStore } from '@/layout/stores/ViewportStore';
import { renderStore } from '@/render/store';
import { CollapsibleSection } from '@/ui';
import { bridgeService } from '@/plugins/core/bridge';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';

function ObjectProperties() {
  //console.log('🏗️ ObjectProperties component created');
  const { selection } = editorStore;
  const { updateObjectProperty } = objectPropertiesActions;
  const [isResettingProperties, setIsResettingProperties] = createSignal(false);
  
  // Individual signals for each property type
  //console.log('🏗️ Creating signals');
  const [positionSignal, setPositionSignal] = createSignal([0, 0, 0]);
  const [rotationSignal, setRotationSignal] = createSignal([0, 0, 0]);
  const [scaleSignal, setScaleSignal] = createSignal([1, 1, 1]);

  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    transform: true
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };


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
    };
    
    //console.log('🔄 Registering render observer');
    renderStore.scene.registerBeforeRender(renderObserver);
    
    
    // Initial sync
    //console.log('🔄 Running initial sync');
    renderObserver();
  });
  


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
      
      //console.log('✅ Transform reset completed');
      
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
    <div className="mb-2">
      <div className="flex items-center justify-between mb-0.5">
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
                className={`w-full text-xs p-1 pl-6 pr-1 rounded text-center focus:outline-none focus:ring-1 focus:ring-primary ${
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
        <div className="text-xs text-primary mt-0.5">Controlled by node</div>
      </Show>
    </div>
    );
  };


  //console.log('🎨 ObjectProperties render cycle started');
  //console.log('🎨 Current selection.entity:', selection.entity);
  //console.log('🎨 Position signal:', positionSignal());
  //console.log('🎨 Rotation signal:', rotationSignal());
  //console.log('🎨 Scale signal:', scaleSignal());
  
  return (
    <div class="h-full flex flex-col">
      {/* Content */}
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
                  Select an object to view its properties
                </div>
              </div>
            );
          }
          
          return (
            <>

              {/* Transform */}
              <Show when={positionSignal().length > 0 || rotationSignal().length > 0 || scaleSignal().length > 0}>
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


              {/* Reset Transform Button */}
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
                    Reset Transform
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

export default ObjectProperties;