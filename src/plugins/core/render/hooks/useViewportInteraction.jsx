import { PointerEventTypes } from '@babylonjs/core/Events/pointerEvents'
import { sceneActions } from '../store'
import { editorActions } from '@/plugins/editor/stores/EditorStore'

export const useViewportInteraction = (sceneInstance, cameraController) => {
  
  const setupPointerEvents = async (scene) => {
    
    scene.onPointerObservable.add((pointerInfo) => {
      switch (pointerInfo.type) {
        case PointerEventTypes.POINTERDOWN:
          break;
        case PointerEventTypes.POINTERMOVE:
          break;
          
        case PointerEventTypes.POINTERUP:
          const isDragging = cameraController.getIsDragging();
          const mouseDownPos = cameraController.getMouseDownPos();
          const keysPressed = cameraController.getKeysPressed();
          
          if (!isDragging && mouseDownPos && !keysPressed.size) {
            const pickInfo = pointerInfo.pickInfo;
            
            if (pickInfo?.hit) {
              let targetMesh = pickInfo.pickedMesh;
              
              if (targetMesh && targetMesh._isInternalMesh) {
                let parent = targetMesh.parent;
                while (parent && (parent._isInternalMesh || parent._isInternalNode)) {
                  parent = parent.parent;
                }
                if (parent) {
                  targetMesh = parent;
                }
              }
              
              if (targetMesh) {
                const objectId = targetMesh.uniqueId || targetMesh.name;
                console.log('🎯 3D Viewport - Selecting object:', targetMesh.name, 'ID:', objectId);
                sceneActions.selectObject(objectId);
                // Sync with editor store
                editorActions.selectEntity(objectId);
              } else {
                console.log('🎯 3D Viewport - Clearing selection (no valid target)');
                sceneActions.selectObject(null);
                // Sync with editor store
                editorActions.selectEntity(null);
              }
            } else {
              console.log('🎯 3D Viewport - Clearing selection (no hit)');
              sceneActions.selectObject(null);
              // Sync with editor store
              editorActions.selectEntity(null);
            }
          }
          
          cameraController.resetDragState();
          break;
      }
    });
  }
  
  return {
    setupPointerEvents
  }
}