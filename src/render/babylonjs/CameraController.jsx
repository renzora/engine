import { createEffect, onCleanup } from 'solid-js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { viewportStore } from '@/layout/stores/ViewportStore';
import { useGameEngineShortcuts } from '@/hooks/useGameEngineShortcuts';

export function useCameraController(camera, canvas, scene) {
  const cameraSettings = () => viewportStore.camera;
  let isLeftMouseDown = false;
  let isMiddleMouseDown = false;
  let isRightMouseDown = false;
  let lastMouseX = 0;
  let lastMouseY = 0;
  let keysPressed = new Set();
  let isDragging = false;
  let mouseDownPos = null;
  let defaultFov = Math.PI / 4;
  let currentFov = Math.PI / 4;
  let targetFov = Math.PI / 4;
  let fovAnimationId = null;

  // Integrate camera movement with centralized shortcuts
  useGameEngineShortcuts({
    // Camera movement
    moveForward: (speedMultiplier = 1.0) => {
      if (!camera()) return;
      const speed = moveSpeed() * speedMultiplier;
      const forward = camera().getForwardRay().direction.normalize();
      camera().position = camera().position.add(forward.scale(speed));
    },
    
    moveBackward: (speedMultiplier = 1.0) => {
      if (!camera()) return;
      const speed = moveSpeed() * speedMultiplier;
      const forward = camera().getForwardRay().direction.normalize();
      camera().position = camera().position.add(forward.scale(-speed));
    },
    
    moveLeft: (speedMultiplier = 1.0) => {
      if (!camera()) return;
      const speed = moveSpeed() * speedMultiplier;
      const forward = camera().getForwardRay().direction.normalize();
      const right = Vector3.Cross(Vector3.Up(), forward).normalize();
      camera().position = camera().position.add(right.scale(-speed));
    },
    
    moveRight: (speedMultiplier = 1.0) => {
      if (!camera()) return;
      const speed = moveSpeed() * speedMultiplier;
      const forward = camera().getForwardRay().direction.normalize();
      const right = Vector3.Cross(Vector3.Up(), forward).normalize();
      camera().position = camera().position.add(right.scale(speed));
    },
    
    moveUp: (speedMultiplier = 1.0) => {
      if (!camera()) return;
      const speed = moveSpeed() * speedMultiplier;
      camera().position = camera().position.add(Vector3.Up().scale(speed));
    },
    
    moveDown: (speedMultiplier = 1.0) => {
      if (!camera()) return;
      const speed = moveSpeed() * speedMultiplier;
      camera().position = camera().position.add(Vector3.Up().scale(-speed));
    }
  });
  
  const cameraSpeed = () => cameraSettings()?.speed || 5;
  const mouseSensitivity = () => cameraSettings()?.mouseSensitivity || 0.002;
  const rotationSpeed = () => mouseSensitivity();
  const panSpeed = 0.01;
  const zoomSpeed = 0.3;
  const moveSpeed = () => 0.35 * (cameraSpeed() / 5);
  const fovSpeed = 0.02;
  const fovSpringSpeed = 0.15;
  const fovSpringDamping = 0.8;

  // Camera movement now handled by centralized shortcuts

  const shouldAllowCameraMovement = (event) => {
    if (!scene() || !scene()._gizmoManager) return true;
    
    const gizmoManager = scene()._gizmoManager;
    
    if (!gizmoManager.attachedMesh) return true;
    
    if (!gizmoManager.positionGizmoEnabled && 
        !gizmoManager.rotationGizmoEnabled && 
        !gizmoManager.scaleGizmoEnabled) {
      return true;
    }
    
    if (event.button === 1 || event.button === 2) return true;
    
    return event.target === canvas();
  };

  const handleMouseDown = (event) => {
    if (!camera() || !canvas()) return;
    
    if (!shouldAllowCameraMovement(event)) {
      return;
    }

    event.preventDefault();
    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
    mouseDownPos = { x: event.clientX, y: event.clientY };
    isDragging = false;

    if (event.button === 0) isLeftMouseDown = true;
    if (event.button === 1) isMiddleMouseDown = true;
    if (event.button === 2) isRightMouseDown = true;

    canvas().setPointerCapture(event.pointerId);
  };

  const handleMouseMove = (event) => {
    if (!camera()) return;
    
    if (scene() && scene()._gizmoManager && scene()._gizmoManager.attachedMesh && 
        (scene()._gizmoManager.positionGizmoEnabled || scene()._gizmoManager.rotationGizmoEnabled || scene()._gizmoManager.scaleGizmoEnabled) &&
        isLeftMouseDown) {
      return;
    }

    const deltaX = event.clientX - lastMouseX;
    const deltaY = event.clientY - lastMouseY;

    if ((isLeftMouseDown || isMiddleMouseDown || isRightMouseDown) && mouseDownPos) {
        const totalDelta = Math.abs(event.clientX - mouseDownPos.x) + Math.abs(event.clientY - mouseDownPos.y);
        if (totalDelta > 5) {
          isDragging = true;
        }
    }

    const precisionMultiplier = keysPressed.has('control') ? 0.2 : 1.0;

    if (isLeftMouseDown) {
      const forward = camera().getForwardRay().direction;
      const horizontalForward = new Vector3(forward.x, 0, forward.z).normalize();
      const fovSpeedMultiplier = camera().fov / (Math.PI / 4);
      const moveDirection = horizontalForward.scale(-deltaY * 0.025 * fovSpeedMultiplier * precisionMultiplier);
      camera().position = camera().position.add(moveDirection);
    } else if (isRightMouseDown) {
      camera().rotation.y += deltaX * rotationSpeed() * precisionMultiplier;
      camera().rotation.x += deltaY * rotationSpeed() * precisionMultiplier;
      camera().rotation.x = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, camera().rotation.x));
    } else if (isMiddleMouseDown) {
        const forward = camera().getForwardRay().direction.normalize();
        const right = Vector3.Cross(Vector3.Up(), forward).normalize();
        const up = Vector3.Cross(right, forward).normalize();
        const panVector = right.scale(-deltaX * panSpeed * precisionMultiplier)
          .add(up.scale(deltaY * panSpeed * precisionMultiplier));
        camera().position = camera().position.add(panVector);
    }

    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
  };

  const handleMouseUp = (event) => {
    if (!canvas()) return;

    if (event.button === 0) isLeftMouseDown = false;
    if (event.button === 1) isMiddleMouseDown = false;
    if (event.button === 2) isRightMouseDown = false;

    canvas().releasePointerCapture(event.pointerId);
  };

  const handleWheel = (event) => {
    if (!camera()) return;

    event.preventDefault();
    const delta = event.deltaY * -0.01;
    const forward = camera().getForwardRay().direction.normalize();
    const fovSpeedMultiplier = camera().fov / (Math.PI / 4);
    const precisionMultiplier = keysPressed.has('control') ? 0.2 : 1.0;
    
    let wheelSpeedMultiplier = 1.0;
    if (delta > 0) {
      wheelSpeedMultiplier = 0.6;
    } else {
      wheelSpeedMultiplier = 1.5;
    }
    
    camera().position = camera().position.add(forward.scale(delta * zoomSpeed * fovSpeedMultiplier * wheelSpeedMultiplier * precisionMultiplier));
  };

  // Key handling now done by centralized shortcuts

  const animateFovSpringBack = () => {
    if (!camera()) return;

    const animate = () => {
      const diff = defaultFov - currentFov;
      const velocity = diff * fovSpringSpeed;
      currentFov += velocity * fovSpringDamping;
      camera().fov = currentFov;
      
      if (Math.abs(diff) > 0.001) {
        fovAnimationId = requestAnimationFrame(animate);
      } else {
        camera().fov = defaultFov;
        currentFov = defaultFov;
        targetFov = defaultFov;
        fovAnimationId = null;
      }
    };
    
    if (fovAnimationId) {
      cancelAnimationFrame(fovAnimationId);
    }
    
    animate();
  };

  // Key up handling simplified - only for FOV zoom
  const handleKeyUp = (event) => {
    const key = event.key.toLowerCase();
    keysPressed.delete(key);
    
    if ((key === 'c' || key === 'z') && !keysPressed.has('c') && !keysPressed.has('z')) {
      animateFovSpringBack();
    }
  };

  createEffect(() => {
    if (!canvas()) return;

    console.log('Camera controller: Setting up event listeners on canvas:', canvas());
    
    if (camera()) {
      try {
        if (typeof camera().attachControls === 'function') {
          camera().attachControls(canvas());
          console.log('🎮 UniversalCamera controls attached to canvas');
        }
      } catch (error) {
        console.warn('🎮 Could not attach camera controls:', error);
      }
    }
    
    const handleContextMenu = (e) => e.preventDefault();
    canvas().addEventListener('contextmenu', handleContextMenu);
    canvas().addEventListener('pointerdown', handleMouseDown);
    canvas().addEventListener('pointermove', handleMouseMove);
    canvas().addEventListener('pointerup', handleMouseUp);
    canvas().addEventListener('wheel', handleWheel, { passive: false });
    window.addEventListener('keyup', handleKeyUp);

    onCleanup(() => {
      console.log('Camera controller: Cleaning up event listeners from canvas:', canvas());
      
      if (camera()) {
        try {
          if (typeof camera().detachControls === 'function') {
            camera().detachControls();
            console.log('🎮 Camera controls detached');
          }
        } catch (error) {
          console.warn('🎮 Could not detach camera controls:', error);
        }
      }
      
      canvas().removeEventListener('contextmenu', handleContextMenu);
      canvas().removeEventListener('pointerdown', handleMouseDown);
      canvas().removeEventListener('pointermove', handleMouseMove);
      canvas().removeEventListener('pointerup', handleMouseUp);
      canvas().removeEventListener('wheel', handleWheel);
      window.removeEventListener('keyup', handleKeyUp);
      
      if (fovAnimationId) {
        cancelAnimationFrame(fovAnimationId);
      }
    });
  });

  createEffect(() => {
    if (camera()) {
      const initialFov = camera().fov || Math.PI / 4;
      defaultFov = initialFov;
      currentFov = initialFov;
      targetFov = initialFov;
    }
  });

  // Camera movement interval removed - now handled by centralized shortcuts

  const handleKeyboardMovement = () => {
  };

  const getIsDragging = () => isDragging;
  const getMouseDownPos = () => mouseDownPos;
  const getKeysPressed = () => keysPressed;
  const resetDragState = () => {
    mouseDownPos = null;
    isDragging = false;
  };

  return {
    handleKeyboardMovement,
    getIsDragging,
    getMouseDownPos,
    getKeysPressed,
    resetDragState
  };
}