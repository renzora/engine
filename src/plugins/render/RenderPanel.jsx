import { createSignal, createEffect, Show } from 'solid-js';
import { renderStore, renderActions } from '@/render/store';
import { IconShadow, IconShield, IconEye } from '@tabler/icons-solidjs';

function RenderPanel(props) {
  const [shadowSettings, setShadowSettings] = createSignal({
    castShadows: false,
    receiveShadows: false
  });
  const [collisionEnabled, setCollisionEnabled] = createSignal(false);
  
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
    <div class="h-full overflow-y-auto p-4 space-y-6 bg-base-100">
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
        <div class="space-y-4">
          <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
            <IconShadow class="w-4 h-4 text-primary" />
            <h3 class="text-sm font-medium text-base-content">Shadow Settings</h3>
          </div>
          
          {/* Cast Shadows */}
          <div class="flex items-center justify-between">
            <div class="flex flex-col">
              <label class="text-xs font-medium text-base-content">Cast Shadows</label>
              <span class="text-xs text-base-content/60">Object will cast shadows onto other surfaces</span>
            </div>
            <div class="form-control">
              <label class="cursor-pointer label">
                <input 
                  type="checkbox" 
                  class="toggle toggle-primary toggle-sm" 
                  checked={shadowSettings().castShadows}
                  onChange={(e) => handleCastShadowsChange(e.target.checked)}
                />
              </label>
            </div>
          </div>
          
          {/* Receive Shadows */}
          <div class="flex items-center justify-between">
            <div class="flex flex-col">
              <label class="text-xs font-medium text-base-content">Receive Shadows</label>
              <span class="text-xs text-base-content/60">Object will receive shadows from other objects</span>
            </div>
            <div class="form-control">
              <label class="cursor-pointer label">
                <input 
                  type="checkbox" 
                  class="toggle toggle-primary toggle-sm" 
                  checked={shadowSettings().receiveShadows}
                  onChange={(e) => handleReceiveShadowsChange(e.target.checked)}
                />
              </label>
            </div>
          </div>
        </div>
        
        {/* Collision Settings Section */}
        <div class="space-y-4">
          <div class="flex items-center space-x-2 pb-2 border-b border-base-300">
            <IconShield class="w-4 h-4 text-primary" />
            <h3 class="text-sm font-medium text-base-content">Collision Settings</h3>
          </div>
          
          {/* Object Collision */}
          <div class="flex items-center justify-between">
            <div class="flex flex-col">
              <label class="text-xs font-medium text-base-content">Object Collision</label>
              <span class="text-xs text-base-content/60">Enable collision detection for this object</span>
            </div>
            <div class="form-control">
              <label class="cursor-pointer label">
                <input 
                  type="checkbox" 
                  class="toggle toggle-primary toggle-sm" 
                  checked={collisionEnabled()}
                  onChange={(e) => handleCollisionChange(e.target.checked)}
                />
              </label>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}

export default RenderPanel;