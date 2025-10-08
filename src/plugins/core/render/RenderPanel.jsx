import { createSignal, createEffect, Show } from 'solid-js';
import { renderStore } from '@/render/store';
import { IconShadow, IconShield, IconEye } from '@tabler/icons-solidjs';

function RenderPanel(props) {
  const [shadowSettings, setShadowSettings] = createSignal({
    castShadows: false,
    receiveShadows: false
  });
  const [collisionEnabled, setCollisionEnabled] = createSignal(false);
  
  // Section collapse state
  const [sectionsOpen, setSectionsOpen] = createSignal({
    shadows: true,
    collision: true
  });
  
  const toggleSection = (section) => {
    setSectionsOpen(prev => ({
      ...prev,
      [section]: !prev[section]
    }));
  };
  
  const selectedObject = () => props.selectedObject;
  
  // Update settings when selected object changes
  createEffect(() => {
    const obj = selectedObject();
    if (obj) {
      // Check if object has shadow capabilities
      const castShadows = obj.material?.shadowGenerator !== undefined || obj.renderingGroupId === 1;
      const receiveShadows = obj.receiveShadows || false;
      
      setShadowSettings({
        castShadows,
        receiveShadows
      });
      
      // Check collision settings
      setCollisionEnabled(obj.checkCollisions || false);
    } else {
      // Reset for scene root or no selection
      setShadowSettings({
        castShadows: false,
        receiveShadows: false
      });
      setCollisionEnabled(false);
    }
  });
  
  const handleCastShadowsChange = (enabled) => {
    const obj = selectedObject();
    if (!obj) return;
    
    setShadowSettings(prev => ({ ...prev, castShadows: enabled }));
    
    // Apply shadow casting to the object
    if (enabled) {
      // Add to shadow casting render group
      obj.renderingGroupId = 1;
    } else {
      // Remove from shadow casting
      obj.renderingGroupId = 0;
    }
    
    // If there's a shadow generator in the scene, add/remove this mesh or its children
    const scene = renderStore.scene;
    if (scene && scene.lights) {
      scene.lights.forEach(light => {
        if (light.getShadowGenerator && light.getShadowGenerator()) {
          const shadowGenerator = light.getShadowGenerator();
          
          if (enabled) {
            // For containers with child meshes, add children selectively
            if (obj.getChildMeshes) {
              const childMeshes = obj.getChildMeshes();
              childMeshes.forEach(childMesh => {
                if (childMesh.getClassName && childMesh.getClassName() === 'Mesh') {
                  // Apply same filtering logic to avoid shadow artifacts
                  let shouldCastShadow = true;
                  
                  if (childMesh.getBoundingInfo) {
                    const size = childMesh.getBoundingInfo().boundingBox.extendSize;
                    const maxSize = Math.max(size.x, size.y, size.z);
                    if (maxSize < 0.05) shouldCastShadow = false;
                  }
                  
                  if (childMesh.material && childMesh.material.alpha < 0.1) {
                    shouldCastShadow = false;
                  }
                  
                  if (shouldCastShadow) {
                    shadowGenerator.addShadowCaster(childMesh);
                  }
                }
              });
            } else if (obj.getClassName && obj.getClassName() === 'Mesh') {
              // Direct mesh - add to shadow caster
              shadowGenerator.addShadowCaster(obj);
            }
          } else {
            // Remove from shadow casting
            if (obj.getChildMeshes) {
              const childMeshes = obj.getChildMeshes();
              childMeshes.forEach(childMesh => {
                if (childMesh.getClassName && childMesh.getClassName() === 'Mesh') {
                  shadowGenerator.removeShadowCaster(childMesh);
                }
              });
            } else if (obj.getClassName && obj.getClassName() === 'Mesh') {
              shadowGenerator.removeShadowCaster(obj);
            }
          }
        }
      });
    }
  };
  
  const handleReceiveShadowsChange = (enabled) => {
    const obj = selectedObject();
    if (!obj) return;
    
    setShadowSettings(prev => ({ ...prev, receiveShadows: enabled }));
    
    // Apply receive shadows to the object and all its child meshes
    if (obj.getChildMeshes) {
      const childMeshes = obj.getChildMeshes();
      childMeshes.forEach(childMesh => {
        if (childMesh.getClassName && childMesh.getClassName() === 'Mesh') {
          childMesh.receiveShadows = enabled;
        }
      });
    }
    
    // Also set on the container object itself
    obj.receiveShadows = enabled;
  };
  
  const handleCollisionChange = (enabled) => {
    const obj = selectedObject();
    if (!obj) return;
    
    setCollisionEnabled(enabled);
    obj.checkCollisions = enabled;
  };
  
  return (
    <div class="h-full flex flex-col">
      <div class="flex-1 p-2 space-y-2">
        <Show 
          when={selectedObject()}
          fallback={
            <div class="flex flex-col items-center justify-center h-full text-base-content/60 text-center">
              <IconEye class="w-8 h-8 mb-2 opacity-40" />
              <p class="text-sm">Select an object to configure render settings</p>
            </div>
          }
        >
          {/* Shadow Settings Section */}
          <div class="bg-base-100 border-base-300 border rounded-lg">
            <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().shadows ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
              <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('shadows')}>
                <IconShadow class="w-3 h-3" />
                Shadow Settings
              </div>
              <input
                type="checkbox"
                checked={sectionsOpen().shadows}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleSection('shadows');
                }}
                onClick={(e) => e.stopPropagation()}
                class="toggle toggle-primary toggle-xs"
              />
            </div>
            <Show when={sectionsOpen().shadows}>
              <div class="!p-2">
                <div class="space-y-0.5">
                  {/* Cast Shadows */}
                  <div class="form-control">
                    <div class="flex items-center justify-between">
                      <div class="flex flex-col">
                        <label class="text-xs font-medium text-base-content">Cast Shadows</label>
                        <span class="text-xs text-base-content/60">Object will cast shadows onto other surfaces</span>
                      </div>
                      <input 
                        type="checkbox" 
                        class="toggle toggle-primary toggle-sm" 
                        checked={shadowSettings().castShadows}
                        onChange={(e) => handleCastShadowsChange(e.target.checked)}
                      />
                    </div>
                  </div>
                  
                  {/* Receive Shadows */}
                  <div class="form-control">
                    <div class="flex items-center justify-between">
                      <div class="flex flex-col">
                        <label class="text-xs font-medium text-base-content">Receive Shadows</label>
                        <span class="text-xs text-base-content/60">Object will receive shadows from other objects</span>
                      </div>
                      <input 
                        type="checkbox" 
                        class="toggle toggle-primary toggle-sm" 
                        checked={shadowSettings().receiveShadows}
                        onChange={(e) => handleReceiveShadowsChange(e.target.checked)}
                      />
                    </div>
                  </div>
                </div>
              </div>
            </Show>
          </div>
          
          {/* Collision Settings Section */}
          <div class="bg-base-100 border-base-300 border rounded-lg">
            <div class={`!min-h-0 !py-1 !px-2 flex items-center justify-between font-medium text-xs border-b border-base-300/50 transition-colors ${ sectionsOpen().collision ? 'bg-primary/15 text-white rounded-t-lg' : 'hover:bg-base-200/50 rounded-t-lg' }`}>
              <div class="flex items-center gap-1.5 cursor-pointer" onClick={() => toggleSection('collision')}>
                <IconShield class="w-3 h-3" />
                Collision Settings
              </div>
              <input
                type="checkbox"
                checked={sectionsOpen().collision}
                onChange={(e) => {
                  e.stopPropagation();
                  toggleSection('collision');
                }}
                onClick={(e) => e.stopPropagation()}
                class="toggle toggle-primary toggle-xs"
              />
            </div>
            <Show when={sectionsOpen().collision}>
              <div class="!p-2">
                <div class="space-y-0.5">
                  {/* Object Collision */}
                  <div class="form-control">
                    <div class="flex items-center justify-between">
                      <div class="flex flex-col">
                        <label class="text-xs font-medium text-base-content">Object Collision</label>
                        <span class="text-xs text-base-content/60">Enable collision detection for this object</span>
                      </div>
                      <input 
                        type="checkbox" 
                        class="toggle toggle-primary toggle-sm" 
                        checked={collisionEnabled()}
                        onChange={(e) => handleCollisionChange(e.target.checked)}
                      />
                    </div>
                  </div>
                </div>
              </div>
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
}

export default RenderPanel;