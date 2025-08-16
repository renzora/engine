import { createEffect, onCleanup } from 'solid-js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { viewportStore } from '@/plugins/editor/stores/ViewportStore';

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
  // Babylon modules are now directly imported
  
  const cameraSpeed = () => cameraSettings()?.speed || 5;
  const mouseSensitivity = () => cameraSettings()?.mouseSensitivity || 0.002;
  const rotationSpeed = () => mouseSensitivity();
  const panSpeed = 0.01;
  const zoomSpeed = 0.3;
  const moveSpeed = () => 0.35 * (cameraSpeed() / 5);
  const fovSpeed = 0.02;
  const fovSpringSpeed = 0.15;
  const fovSpringDamping = 0.8;

  const applyKeyboardMovement = () => {
    if (!camera()) return;

    const forward = camera().getForwardRay().direction.normalize();
    const right = Vector3.Cross(Vector3.Up(), forward).normalize()
    const fovSpeedMultiplier = camera().fov / (Math.PI / 4);
    const precisionMultiplier = keysPressed.has('control') ? 0.2 : 1.0;
    const adjustedMoveSpeed = moveSpeed() * fovSpeedMultiplier * precisionMultiplier;
    let moveVector = Vector3.Zero();

    if (keysPressed.has('w')) moveVector = moveVector.add(forward.scale(adjustedMoveSpeed));
    if (keysPressed.has('s')) moveVector = moveVector.add(forward.scale(-adjustedMoveSpeed));
    if (keysPressed.has('a')) moveVector = moveVector.add(right.scale(-adjustedMoveSpeed));
    if (keysPressed.has('d')) moveVector = moveVector.add(right.scale(adjustedMoveSpeed));
    if (keysPressed.has('e')) moveVector = moveVector.add(Vector3.Up().scale(adjustedMoveSpeed));
    if (keysPressed.has('q')) moveVector = moveVector.add(Vector3.Up().scale(-adjustedMoveSpeed));

    if (keysPressed.has('c')) {
      const newFov = Math.min(Math.PI * 0.8, targetFov + fovSpeed);
      if (newFov !== targetFov) {
        targetFov = newFov;
        camera().fov = targetFov;
        currentFov = targetFov;
      }
    }
    if (keysPressed.has('z')) {
      const newFov = Math.max(0.1, targetFov - fovSpeed);
      if (newFov !== targetFov) {
        targetFov = newFov;
        camera().fov = targetFov;
        currentFov = targetFov;
      }
    }

    if (moveVector.length() > 0) {
      camera().position = camera().position.add(moveVector);
    }
  };

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

  const handleKeyDown = (event) => {
    keysPressed.add(event.key.toLowerCase());
  };

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
    
    const handleContextMenu = (e) => e.preventDefault();
    canvas().addEventListener('contextmenu', handleContextMenu);
    canvas().addEventListener('pointerdown', handleMouseDown);
    canvas().addEventListener('pointermove', handleMouseMove);
    canvas().addEventListener('pointerup', handleMouseUp);
    canvas().addEventListener('wheel', handleWheel, { passive: false });
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    onCleanup(() => {
      console.log('Camera controller: Cleaning up event listeners from canvas:', canvas());
      canvas().removeEventListener('contextmenu', handleContextMenu);
      canvas().removeEventListener('pointerdown', handleMouseDown);
      canvas().removeEventListener('pointermove', handleMouseMove);
      canvas().removeEventListener('pointerup', handleMouseUp);
      canvas().removeEventListener('wheel', handleWheel);
      window.removeEventListener('keydown', handleKeyDown);
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

  createEffect(() => {
    const interval = setInterval(() => {
      if (isLeftMouseDown || isMiddleMouseDown || isRightMouseDown) {
        applyKeyboardMovement();
      }
    }, 16);

    onCleanup(() => clearInterval(interval));
  });

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