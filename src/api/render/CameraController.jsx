import { createEffect, onCleanup } from 'solid-js';
import { viewportStore } from '@/layout/stores/ViewportStore';
import { rendererAPI } from './RendererAPI';

export function useCameraController(canvas) {
  const cameraSettings = () => viewportStore.camera;
  let isLeftMouseDown = false;
  let isMiddleMouseDown = false;
  let isRightMouseDown = false;
  let lastMouseX = 0;
  let lastMouseY = 0;
  let keysPressed = new Set();
  let isDragging = false;
  let mouseDownPos = null;

  // WASD movement state
  const keys = {
    w: false, a: false, s: false, d: false,
    q: false, e: false
  };

  // Camera settings
  const cameraSpeed = () => cameraSettings()?.speed || 5;
  const mouseSensitivity = () => cameraSettings()?.mouseSensitivity || 0.002;
  const rotationSpeed = () => mouseSensitivity();
  const panSpeed = 0.01;
  const zoomSpeed = 0.3;
  const moveSpeed = () => 0.35 * (cameraSpeed() / 5);

  // Mouse event handlers
  const handleMouseDown = (event) => {
    if (!rendererAPI.getActiveRenderer()) return;

    event.preventDefault();
    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
    mouseDownPos = { x: event.clientX, y: event.clientY };
    isDragging = false;

    if (event.button === 0) isLeftMouseDown = true;
    if (event.button === 1) isMiddleMouseDown = true;
    if (event.button === 2) isRightMouseDown = true;
  };

  const handleMouseMove = async (event) => {
    if (!rendererAPI.getActiveRenderer()) return;

    const deltaX = event.clientX - lastMouseX;
    const deltaY = event.clientY - lastMouseY;

    if ((isLeftMouseDown || isMiddleMouseDown || isRightMouseDown) && mouseDownPos) {
      const totalDelta = Math.abs(event.clientX - mouseDownPos.x) + Math.abs(event.clientY - mouseDownPos.y);
      if (totalDelta > 5) {
        isDragging = true;
      }
    }

    const precisionMultiplier = keysPressed.has('Control') ? 0.2 : 1.0;

    if (isLeftMouseDown) {
      // Forward/backward movement (like Babylon.js left mouse)
      const moveDistance = -deltaY * 0.025 * precisionMultiplier;
      await rendererAPI.moveCamera('forward', moveDistance);
    } else if (isRightMouseDown) {
      // Look around (like Babylon.js right mouse)
      const rotateX = deltaY * rotationSpeed() * precisionMultiplier;
      const rotateY = deltaX * rotationSpeed() * precisionMultiplier;
      await rendererAPI.rotateCamera(rotateY, rotateX);
    } else if (isMiddleMouseDown) {
      // Pan (like Babylon.js middle mouse)
      const panX = -deltaX * panSpeed * precisionMultiplier;
      const panY = deltaY * panSpeed * precisionMultiplier;
      await rendererAPI.panCamera(panX, panY);
    }

    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
  };

  const handleMouseUp = (event) => {
    if (event.button === 0) isLeftMouseDown = false;
    if (event.button === 1) isMiddleMouseDown = false;
    if (event.button === 2) isRightMouseDown = false;
  };

  const handleWheel = async (event) => {
    if (!rendererAPI.getActiveRenderer()) return;

    event.preventDefault();
    const delta = event.deltaY * -0.01;
    const precisionMultiplier = keysPressed.has('Control') ? 0.2 : 1.0;
    
    let wheelSpeedMultiplier = 1.0;
    if (delta > 0) {
      wheelSpeedMultiplier = 0.6;
    } else {
      wheelSpeedMultiplier = 1.5;
    }
    
    const moveDistance = delta * zoomSpeed * wheelSpeedMultiplier * precisionMultiplier;
    await rendererAPI.moveCamera('forward', moveDistance);
  };

  // Keyboard event handlers
  const handleKeyDown = (event) => {
    const key = event.key.toLowerCase();
    keysPressed.add(key);
    
    switch(event.code) {
      case 'KeyW': keys.w = true; break;
      case 'KeyA': keys.a = true; break;
      case 'KeyS': keys.s = true; break;
      case 'KeyD': keys.d = true; break;
      case 'KeyQ': keys.q = true; break;
      case 'KeyE': keys.e = true; break;
    }
  };

  const handleKeyUp = (event) => {
    const key = event.key.toLowerCase();
    keysPressed.delete(key);
    
    switch(event.code) {
      case 'KeyW': keys.w = false; break;
      case 'KeyA': keys.a = false; break;
      case 'KeyS': keys.s = false; break;
      case 'KeyD': keys.d = false; break;
      case 'KeyQ': keys.q = false; break;
      case 'KeyE': keys.e = false; break;
    }
  };

  // WASD keyboard movement
  const handleKeyboardMovement = async () => {
    if (!rendererAPI.getActiveRenderer()) return;
    
    const speed = moveSpeed();
    const speedMultiplier = keysPressed.has('shift') ? 2.0 : 1.0;
    const finalSpeed = speed * speedMultiplier;
    
    if (keys.w) await rendererAPI.moveCamera('forward', finalSpeed);
    if (keys.s) await rendererAPI.moveCamera('backward', finalSpeed);
    if (keys.d) await rendererAPI.moveCamera('right', finalSpeed);
    if (keys.a) await rendererAPI.moveCamera('left', finalSpeed);
    if (keys.e) await rendererAPI.moveCamera('up', finalSpeed);
    if (keys.q) await rendererAPI.moveCamera('down', finalSpeed);
  };

  // Set up movement interval
  let movementInterval;
  createEffect(() => {
    if (rendererAPI.getActiveRenderer()) {
      movementInterval = setInterval(handleKeyboardMovement, 16); // ~60fps
    }
    
    onCleanup(() => {
      if (movementInterval) {
        clearInterval(movementInterval);
      }
    });
  });

  // Set up event listeners
  createEffect(() => {
    if (!canvas()) return;

    const handleContextMenu = (e) => e.preventDefault();
    canvas().addEventListener('contextmenu', handleContextMenu);
    canvas().addEventListener('mousedown', handleMouseDown);
    canvas().addEventListener('mousemove', handleMouseMove);
    canvas().addEventListener('mouseup', handleMouseUp);
    canvas().addEventListener('wheel', handleWheel, { passive: false });
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);

    onCleanup(() => {
      canvas().removeEventListener('contextmenu', handleContextMenu);
      canvas().removeEventListener('mousedown', handleMouseDown);
      canvas().removeEventListener('mousemove', handleMouseMove);
      canvas().removeEventListener('mouseup', handleMouseUp);
      canvas().removeEventListener('wheel', handleWheel);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
    });
  });

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