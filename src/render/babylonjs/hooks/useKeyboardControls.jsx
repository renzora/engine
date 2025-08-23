import { createEffect, onCleanup, onMount } from 'solid-js'
import { Vector3 } from '@babylonjs/core/Maths/math.vector'
import { Animation } from '@babylonjs/core/Animations/animation'
import { Ray } from '@babylonjs/core/Culling/ray'
import { editorActions } from '@/layout/stores/EditorStore'
import { useGameEngineShortcuts } from '@/hooks/useGameEngineShortcuts'

export const useKeyboardControls = (sceneInstance, cameraController) => {
  let copiedObjectRef = null
  // Babylon modules are now directly imported
  
  // Use the centralized keyboard shortcuts system
  useGameEngineShortcuts({
    // Focus on selected object
    focusObject: () => {
      const scene = sceneInstance();
      if (scene && scene._gizmoManager?.attachedMesh && scene._camera) {
        const objectName = scene._gizmoManager.attachedMesh.name;
        focusOnObject(scene._gizmoManager.attachedMesh, scene._camera, scene);
        editorActions.addConsoleMessage(`Flying to ${objectName}`, 'info');
      } else {
        editorActions.addConsoleMessage('No object selected to focus on', 'warning');
      }
    },
    
    // Delete selected object  
    deleteObject: () => {
      const scene = sceneInstance();
      if (!scene) return;
      
      const attachedMesh = scene._gizmoManager?.attachedMesh;
      if (attachedMesh && attachedMesh.name !== '__grid_container__' && !attachedMesh.name.startsWith('__grid_')) {
        attachedMesh.dispose();
        scene._gizmoManager.attachToMesh(null);
        if (scene._highlightLayer) {
          scene._highlightLayer.removeAllMeshes();
        }
        editorActions.selectEntity(null);
        console.log('Deleted object:', attachedMesh.name);
      }
    },
    
    // Transform gizmos
    positionMode: () => {
      const scene = sceneInstance();
      if (scene && scene._gizmoManager?.attachedMesh) {
        scene._gizmoManager.positionGizmoEnabled = true;
        scene._gizmoManager.rotationGizmoEnabled = false;
        scene._gizmoManager.scaleGizmoEnabled = false;
        console.log('Switched to position gizmo');
      }
    },
    
    rotateMode: () => {
      const scene = sceneInstance();
      if (scene && scene._gizmoManager?.attachedMesh) {
        scene._gizmoManager.positionGizmoEnabled = false;
        scene._gizmoManager.rotationGizmoEnabled = true;
        scene._gizmoManager.scaleGizmoEnabled = false;
        console.log('Switched to rotation gizmo');
      }
    },
    
    scaleMode: () => {
      const scene = sceneInstance();
      if (scene && scene._gizmoManager?.attachedMesh) {
        scene._gizmoManager.positionGizmoEnabled = false;
        scene._gizmoManager.rotationGizmoEnabled = false;
        scene._gizmoManager.scaleGizmoEnabled = true;
        console.log('Switched to scale gizmo');
      }
    },
    
    // Copy/Paste
    copy: () => {
      const scene = sceneInstance();
      if (!scene) return;
      
      const attachedMesh = scene._gizmoManager?.attachedMesh;
      if (attachedMesh) {
        copiedObjectRef = {
          name: attachedMesh.name,
          position: attachedMesh.position.clone(),
          rotation: attachedMesh.rotation ? attachedMesh.rotation.clone() : null,
          scaling: attachedMesh.scaling ? attachedMesh.scaling.clone() : null,
          className: attachedMesh.getClassName(),
          babylonObject: attachedMesh
        };
        console.log('Copied object:', attachedMesh.name);
      }
    },
    
    paste: () => {
      const scene = sceneInstance();
      if (!scene || !copiedObjectRef) return;
      
      try {
        let newObject = null;
        
        if (copiedObjectRef.className === 'TransformNode') {
          newObject = copiedObjectRef.babylonObject.createInstance(copiedObjectRef.name + '_copy');
          if (!newObject) {
            newObject = copiedObjectRef.babylonObject.clone(copiedObjectRef.name + '_copy', null);
          }
        } else {
          newObject = copiedObjectRef.babylonObject.createInstance(copiedObjectRef.name + '_copy');
          if (!newObject) {
            newObject = copiedObjectRef.babylonObject.clone(copiedObjectRef.name + '_copy', null);
          }
        }
        
        if (newObject) {
          newObject.position = copiedObjectRef.position.add(new Vector3(2, 0, 2));
          if (copiedObjectRef.rotation && newObject.rotation) {
            newObject.rotation = copiedObjectRef.rotation.clone();
          }
          if (copiedObjectRef.scaling && newObject.scaling) {
            newObject.scaling = copiedObjectRef.scaling.clone();
          }
          console.log('Pasted object:', newObject.name);
        }
      } catch (error) {
        console.error('Failed to paste object:', error);
        editorActions.addConsoleMessage(`Failed to paste object: ${error.message}`, 'error');
      }
    },
    
    // Snap to ground
    snapToGround: () => {
      const scene = sceneInstance();
      if (!scene) return;
      
      const attachedMesh = scene._gizmoManager?.attachedMesh;
      if (attachedMesh && attachedMesh.name !== '__grid_container__' && !attachedMesh.name.startsWith('__grid_')) {
        snapObjectToGround(attachedMesh, scene);
      }
    }
  });
  
  const focusOnObject = async (targetObject, camera, scene) => {
    if (!targetObject || !camera) return
    
    let center, size
    
    if (targetObject.getClassName() === 'TransformNode') {
      const childMeshes = targetObject.getChildMeshes()
      
      if (childMeshes.length === 0) {
        console.warn('TransformNode has no child meshes to focus on')
        return
      }
      
      let min = new Vector3(Infinity, Infinity, Infinity)
      let max = new Vector3(-Infinity, -Infinity, -Infinity)
      
      childMeshes.forEach(childMesh => {
        childMesh.computeWorldMatrix(true)
        const boundingInfo = childMesh.getBoundingInfo()
        const meshMin = boundingInfo.boundingBox.minimumWorld
        const meshMax = boundingInfo.boundingBox.maximumWorld
        
        min = Vector3.Minimize(min, meshMin)
        max = Vector3.Maximize(max, meshMax)
      })
      
      center = min.add(max).scale(0.5)
      size = max.subtract(min)
    } else {
      targetObject.computeWorldMatrix(true)
      const boundingInfo = targetObject.getBoundingInfo()
      
      center = boundingInfo.boundingBox.centerWorld
      size = boundingInfo.boundingBox.maximumWorld.subtract(boundingInfo.boundingBox.minimumWorld)
    }
    
    const maxSize = Math.max(size.x, size.y, size.z)
    const distance = Math.max(maxSize * 3, 10)
    
    console.log(`Focusing on: ${targetObject.name}`)
    console.log('Center:', center)
    console.log('Size:', size)
    console.log('Distance:', distance)
    
    const currentForward = camera.getForwardRay().direction.normalize()
    const cameraPosition = center.subtract(currentForward.scale(distance))
    cameraPosition.y = Math.max(cameraPosition.y, center.y + distance * 0.3)
    
    console.log('Flying camera to position:', cameraPosition)
    
    Animation.CreateAndStartAnimation(
      'flyCameraPosition',
      camera,
      'position',
      60,
      15,
      camera.position.clone(),
      cameraPosition,
      Animation.ANIMATIONLOOPMODE_CONSTANT,
      null,
      () => {
        const lookDirection = center.subtract(camera.position).normalize()
        const targetRotation = Vector3.Zero()
        
        targetRotation.x = Math.asin(-lookDirection.y)
        targetRotation.y = Math.atan2(lookDirection.x, lookDirection.z)
        
        Animation.CreateAndStartAnimation(
          'flyCameraRotation',
          camera,
          'rotation',
          60,
          8,
          camera.rotation.clone(),
          targetRotation,
          Animation.ANIMATIONLOOPMODE_CONSTANT
        )
      }
    )
  }

  const snapObjectToGround = (targetObject, scene) => {
    if (!targetObject || !scene) return
    
    console.log('Snapping object to nearest surface:', targetObject.name)
    targetObject.computeWorldMatrix(true)
    
    let boundingInfo, objectBottom, objectCenter
    
    if (targetObject.getClassName() === 'TransformNode') {
      const childMeshes = targetObject.getChildMeshes()
      if (childMeshes.length > 0) {
        let minX = Infinity, minY = Infinity, minZ = Infinity
        let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity
        
        childMeshes.forEach(mesh => {
          mesh.computeWorldMatrix(true)
          const meshBounding = mesh.getBoundingInfo()
          const min = meshBounding.boundingBox.minimumWorld
          const max = meshBounding.boundingBox.maximumWorld
          
          minX = Math.min(minX, min.x)
          minY = Math.min(minY, min.y)
          minZ = Math.min(minZ, min.z)
          maxX = Math.max(maxX, max.x)
          maxY = Math.max(maxY, max.y)
          maxZ = Math.max(maxZ, max.z)
        })
        
        objectBottom = minY
        objectCenter = new Vector3((minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2)
      } else {
        objectBottom = targetObject.position.y
        objectCenter = targetObject.position.clone()
      }
    } else {
      boundingInfo = targetObject.getBoundingInfo()
      objectBottom = boundingInfo.boundingBox.minimumWorld.y
      objectCenter = boundingInfo.boundingBox.centerWorld
    }
    
    const rayDirections = [
      { dir: new Vector3(0, -1, 0), name: "down" },
      { dir: new Vector3(0, 1, 0), name: "up" },
      { dir: new Vector3(1, 0, 0), name: "right" },
      { dir: new Vector3(-1, 0, 0), name: "left" },
      { dir: new Vector3(0, 0, 1), name: "forward" },
      { dir: new Vector3(0, 0, -1), name: "back" }
    ]
    
    let closestHit = null
    let closestDistance = Infinity
    let hitDirection = null
    
    rayDirections.forEach(({ dir, name }) => {
      const ray = new Ray(objectCenter, dir)
      
      const hit = scene.pickWithRay(ray, (mesh) => {
        return mesh !== targetObject && 
               !mesh._isInternalMesh && 
               mesh.isVisible &&
               !mesh.name.startsWith('__') &&
               mesh.geometry
      })
      
      if (hit.hit && hit.distance < closestDistance) {
        closestDistance = hit.distance
        closestHit = hit
        hitDirection = name
        console.log(`Found ${name} surface at distance: ${hit.distance.toFixed(2)} on mesh: ${hit.pickedMesh?.name}`)
      }
    })
    
    if (closestHit && closestHit.pickedPoint) {
      const hitPoint = closestHit.pickedPoint
      
      switch (hitDirection) {
        case "down":
          const heightDifference = objectBottom - targetObject.position.y
          targetObject.position.y = hitPoint.y - heightDifference
          break
        case "up":
          const objectTop = boundingInfo.boundingBox.maximumWorld.y
          const topHeightDiff = objectTop - targetObject.position.y
          targetObject.position.y = hitPoint.y - topHeightDiff
          break
        case "right":
          const objectLeft = boundingInfo.boundingBox.minimumWorld.x
          const leftDiff = objectLeft - targetObject.position.x
          targetObject.position.x = hitPoint.x - leftDiff
          break
        case "left":
          const objectRight = boundingInfo.boundingBox.maximumWorld.x
          const rightDiff = objectRight - targetObject.position.x
          targetObject.position.x = hitPoint.x - rightDiff
          break
        case "forward":
          const objectBack = boundingInfo.boundingBox.minimumWorld.z
          const backDiff = objectBack - targetObject.position.z
          targetObject.position.z = hitPoint.z - backDiff
          break
        case "back":
          const objectFront = boundingInfo.boundingBox.maximumWorld.z
          const frontDiff = objectFront - targetObject.position.z
          targetObject.position.z = hitPoint.z - frontDiff
          break
      }
      
      console.log(`Snapped ${targetObject.name} to ${hitDirection} surface at distance: ${closestDistance.toFixed(2)}`)
      editorActions.addConsoleMessage(`Snapped ${targetObject.name} to ${hitDirection} surface`, 'success')
      // CLEAN SCENE: No store refresh needed
    } else {
      const heightDifference = objectBottom - targetObject.position.y
      targetObject.position.y = -heightDifference
      console.log(`Snapped ${targetObject.name} to default ground level`)
      editorActions.addConsoleMessage(`Snapped ${targetObject.name} to ground level`, 'success')
      // CLEAN SCENE: No store refresh needed
    }
  }
  
  // No longer needed - using centralized keyboard shortcuts
}