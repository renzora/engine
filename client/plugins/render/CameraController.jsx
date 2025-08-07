import { useRef, useCallback, useEffect } from 'react';
import { useSnapshot } from 'valtio';
import { globalStore } from '@/store.js';
import * as BABYLON from '@babylonjs/core';

export function useCameraController(camera, canvas, scene) {
  const { camera: cameraSettings } = useSnapshot(globalStore.editor);
  const isLeftMouseDown = useRef(false);
  const isMiddleMouseDown = useRef(false);
  const isRightMouseDown = useRef(false);
  const lastMouseX = useRef(0);
  const lastMouseY = useRef(0);
  const keysPressed = useRef(new Set());
  const isDragging = useRef(false);
  const mouseDownPos = useRef(null);
  const defaultFov = useRef(Math.PI / 4); // Store default FOV (45 degrees)
  const currentFov = useRef(Math.PI / 4);
  const targetFov = useRef(Math.PI / 4);
  const fovAnimationId = useRef(null);

  // Use dynamic camera settings from global store
  const cameraSpeed = cameraSettings.speed || 5;
  const mouseSensitivity = cameraSettings.mouseSensitivity || 0.002;
  
  const rotationSpeed = mouseSensitivity;
  const panSpeed = 0.01;
  const zoomSpeed = 0.3;
  const moveSpeed = 0.35 * (cameraSpeed / 5); // Scale movement speed based on camera speed setting
  const fovSpeed = 0.02;
  const fovSpringSpeed = 0.15; // Controls how fast FOV springs back
  const fovSpringDamping = 0.8; // Controls the damping of the spring

  const applyKeyboardMovement = useCallback(() => {
    if (!camera) return;

    const forward = camera.getForwardRay().direction.normalize();
    const right = BABYLON.Vector3.Cross(BABYLON.Vector3.Up(), forward).normalize()

    // Scale movement speed based on FOV (higher FOV = faster movement)
    const fovSpeedMultiplier = camera.fov / (Math.PI / 4); // Normalize to default FOV of 45 degrees
    
    // Apply precision mode when Ctrl is held (slower movement)
    const precisionMultiplier = keysPressed.current.has('control') ? 0.2 : 1.0;
    const adjustedMoveSpeed = moveSpeed * fovSpeedMultiplier * precisionMultiplier;

    let moveVector = BABYLON.Vector3.Zero();

    if (keysPressed.current.has('w')) moveVector = moveVector.add(forward.scale(adjustedMoveSpeed));
    if (keysPressed.current.has('s')) moveVector = moveVector.add(forward.scale(-adjustedMoveSpeed));
    if (keysPressed.current.has('a')) moveVector = moveVector.add(right.scale(-adjustedMoveSpeed));
    if (keysPressed.current.has('d')) moveVector = moveVector.add(right.scale(adjustedMoveSpeed));
    if (keysPressed.current.has('e')) moveVector = moveVector.add(BABYLON.Vector3.Up().scale(adjustedMoveSpeed));
    if (keysPressed.current.has('q')) moveVector = moveVector.add(BABYLON.Vector3.Up().scale(-adjustedMoveSpeed));

    // FOV adjustment
    if (keysPressed.current.has('c')) {
      const newFov = Math.min(Math.PI * 0.8, targetFov.current + fovSpeed);
      if (newFov !== targetFov.current) {
        targetFov.current = newFov;
        camera.fov = targetFov.current;
        currentFov.current = targetFov.current;
      }
    }
    if (keysPressed.current.has('z')) {
      const newFov = Math.max(0.1, targetFov.current - fovSpeed);
      if (newFov !== targetFov.current) {
        targetFov.current = newFov;
        camera.fov = targetFov.current;
        currentFov.current = targetFov.current;
      }
    }

    if (moveVector.length() > 0) {
      camera.position = camera.position.add(moveVector);
    }
  }, [camera, moveSpeed, fovSpeed]);

  const shouldAllowCameraMovement = useCallback((event) => {
    if (!scene || !scene._gizmoManager) return true;
    
    const gizmoManager = scene._gizmoManager;
    
    // If no object is selected, always allow camera movement
    if (!gizmoManager.attachedMesh) return true;
    
    // If no gizmos are enabled, allow camera movement
    if (!gizmoManager.positionGizmoEnabled && 
        !gizmoManager.rotationGizmoEnabled && 
        !gizmoManager.scaleGizmoEnabled) {
      return true;
    }
    
    // For right-click and middle-click, always allow camera movement
    if (event.button === 1 || event.button === 2) return true;
    
    // For left-click, we need to check if we're clicking on empty space vs gizmo
    // This is a simple approach: if the event target is the canvas, it's likely empty space
    return event.target === canvas;
  }, [scene, canvas]);


  const handleMouseDown = useCallback((event) => {
    if (!camera || !canvas) return;
    
    // Only handle camera movement if allowed
    if (!shouldAllowCameraMovement(event)) {
      return;
    }

    event.preventDefault();
    lastMouseX.current = event.clientX;
    lastMouseY.current = event.clientY;
    mouseDownPos.current = { x: event.clientX, y: event.clientY };
    isDragging.current = false;

    if (event.button === 0) isLeftMouseDown.current = true;
    if (event.button === 1) isMiddleMouseDown.current = true;
    if (event.button === 2) isRightMouseDown.current = true;

    canvas.setPointerCapture(event.pointerId);
  }, [camera, canvas, shouldAllowCameraMovement]);

  const handleMouseMove = useCallback((event) => {
    if (!camera) return;
    
    // Don't handle mouse move if we're potentially dragging a gizmo (simple check)
    if (scene && scene._gizmoManager && scene._gizmoManager.attachedMesh && 
        (scene._gizmoManager.positionGizmoEnabled || scene._gizmoManager.rotationGizmoEnabled || scene._gizmoManager.scaleGizmoEnabled) &&
        isLeftMouseDown.current) {
      return;
    }

    const deltaX = event.clientX - lastMouseX.current;
    const deltaY = event.clientY - lastMouseY.current;

    if ((isLeftMouseDown.current || isMiddleMouseDown.current || isRightMouseDown.current) && mouseDownPos.current) {
        const totalDelta = Math.abs(event.clientX - mouseDownPos.current.x) + Math.abs(event.clientY - mouseDownPos.current.y);
        if (totalDelta > 5) { // 5 pixel threshold to start drag
          isDragging.current = true;
        }
    }

    // Apply precision mode when Ctrl is held (slower movement)
    const precisionMultiplier = keysPressed.current.has('control') ? 0.2 : 1.0;

    if (isLeftMouseDown.current) {
      // Normal gliding movement with FOV-scaled speed
      const forward = camera.getForwardRay().direction;
      const horizontalForward = new BABYLON.Vector3(forward.x, 0, forward.z).normalize();
      const fovSpeedMultiplier = camera.fov / (Math.PI / 4); // Normalize to default FOV of 45 degrees
      const moveDirection = horizontalForward.scale(-deltaY * 0.025 * fovSpeedMultiplier * precisionMultiplier);
      camera.position = camera.position.add(moveDirection);
    } else if (isRightMouseDown.current) {
      camera.rotation.y += deltaX * rotationSpeed * precisionMultiplier;
      camera.rotation.x += deltaY * rotationSpeed * precisionMultiplier;
      camera.rotation.x = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, camera.rotation.x));
    } else if (isMiddleMouseDown.current) {
        const forward = camera.getForwardRay().direction.normalize();
        const right = BABYLON.Vector3.Cross(BABYLON.Vector3.Up(), forward).normalize();
        const up = BABYLON.Vector3.Cross(right, forward).normalize();
        const panVector = right.scale(-deltaX * panSpeed * precisionMultiplier)
          .add(up.scale(deltaY * panSpeed * precisionMultiplier));
        camera.position = camera.position.add(panVector);
    }

    lastMouseX.current = event.clientX;
    lastMouseY.current = event.clientY;
  }, [camera, rotationSpeed, scene]);

  const handleMouseUp = useCallback((event) => {
    if (!canvas) return;

    if (event.button === 0) isLeftMouseDown.current = false;
    if (event.button === 1) isMiddleMouseDown.current = false;
    if (event.button === 2) isRightMouseDown.current = false;

    canvas.releasePointerCapture(event.pointerId);
  }, [canvas]);

  const handleWheel = useCallback((event) => {
    if (!camera) return;

    event.preventDefault();
    const delta = event.deltaY * -0.01;
    const forward = camera.getForwardRay().direction.normalize();
    const fovSpeedMultiplier = camera.fov / (Math.PI / 4); // Normalize to default FOV of 45 degrees
    
    // Apply precision mode when Ctrl is held (slower movement)
    const precisionMultiplier = keysPressed.current.has('control') ? 0.2 : 1.0;
    
    // Adjust wheel speed based on direction: faster when zooming out (scroll down), slower when zooming in (scroll up)
    let wheelSpeedMultiplier = 1.0;
    if (delta > 0) {
      // Scrolling up (zooming in) - slower
      wheelSpeedMultiplier = 0.6;
    } else {
      // Scrolling down (zooming out) - faster
      wheelSpeedMultiplier = 1.5;
    }
    
    camera.position = camera.position.add(forward.scale(delta * zoomSpeed * fovSpeedMultiplier * wheelSpeedMultiplier * precisionMultiplier));
  }, [camera, zoomSpeed]);

  const handleKeyDown = useCallback((event) => {
    keysPressed.current.add(event.key.toLowerCase());
  }, []);

  const animateFovSpringBack = useCallback(() => {
    if (!camera) return;

    const animate = () => {
      const diff = defaultFov.current - currentFov.current;
      const velocity = diff * fovSpringSpeed;
      currentFov.current += velocity * fovSpringDamping;
      
      // Apply the new FOV to the camera
      camera.fov = currentFov.current;
      
      // Continue animation if we haven't reached the target yet
      if (Math.abs(diff) > 0.001) {
        fovAnimationId.current = requestAnimationFrame(animate);
      } else {
        // Snap to exact default value when close enough
        camera.fov = defaultFov.current;
        currentFov.current = defaultFov.current;
        targetFov.current = defaultFov.current;
        fovAnimationId.current = null;
      }
    };
    
    // Cancel any existing animation
    if (fovAnimationId.current) {
      cancelAnimationFrame(fovAnimationId.current);
    }
    
    animate();
  }, [camera, fovSpringSpeed, fovSpringDamping]);

  const handleKeyUp = useCallback((event) => {
    const key = event.key.toLowerCase();
    keysPressed.current.delete(key);
    
    // Start spring-back animation when FOV keys are released
    if ((key === 'c' || key === 'z') && !keysPressed.current.has('c') && !keysPressed.current.has('z')) {
      animateFovSpringBack();
    }
  }, [animateFovSpringBack]);

  useEffect(() => {
    if (!canvas) return;

    console.log('Camera controller: Setting up event listeners on canvas:', canvas);
    
    const handleContextMenu = (e) => e.preventDefault();
    canvas.addEventListener('contextmenu', handleContextMenu);
    canvas.addEventListener('pointerdown', handleMouseDown);
    canvas.addEventListener('pointermove', handleMouseMove);
    canvas.addEventListener('pointerup', handleMouseUp);
    canvas.addEventListener('wheel', handleWheel, { passive: false });
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    return () => {
      console.log('Camera controller: Cleaning up event listeners from canvas:', canvas);
      canvas.removeEventListener('contextmenu', handleContextMenu);
      canvas.removeEventListener('pointerdown', handleMouseDown);
      canvas.removeEventListener('pointermove', handleMouseMove);
      canvas.removeEventListener('pointerup', handleMouseUp);
      canvas.removeEventListener('wheel', handleWheel);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
      
      // Clean up FOV animation
      if (fovAnimationId.current) {
        cancelAnimationFrame(fovAnimationId.current);
      }
    };
  }, [canvas, handleMouseDown, handleMouseMove, handleMouseUp, handleWheel, handleKeyDown, handleKeyUp]);

  // Initialize FOV refs when camera is available
  useEffect(() => {
    if (camera) {
      const initialFov = camera.fov || Math.PI / 4;
      defaultFov.current = initialFov;
      currentFov.current = initialFov;
      targetFov.current = initialFov;
    }
  }, [camera]);

  useEffect(() => {
    const interval = setInterval(() => {
      if (isLeftMouseDown.current || isMiddleMouseDown.current || isRightMouseDown.current) {
        applyKeyboardMovement();
      }
    }, 16); // roughly 60fps

    return () => clearInterval(interval);
  }, [applyKeyboardMovement]);

  const handleKeyboardMovement = useCallback(() => {
    // This function is kept for compatibility with index.jsx, but the movement is handled by the useEffect with setInterval.
  }, []);

  const getIsDragging = useCallback(() => isDragging.current, []);
  const getMouseDownPos = useCallback(() => mouseDownPos.current, []);
  const getKeysPressed = useCallback(() => keysPressed.current, []);
  const resetDragState = useCallback(() => {
    mouseDownPos.current = null;
    isDragging.current = false;
  }, []);

  return {
    handleKeyboardMovement,
    getIsDragging,
    getMouseDownPos,
    getKeysPressed,
    resetDragState
  };
}