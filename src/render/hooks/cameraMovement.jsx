import { createEffect, onCleanup } from 'solid-js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { viewportStore } from '@/layout/stores/ViewportStore';
import { renderStore } from '../store.jsx';

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
  let isDisabled = false;
  
  const cameraSpeed = () => cameraSettings()?.speed || 5;
  const mouseSensitivity = () => cameraSettings()?.mouseSensitivity || 0.002;
  const rotationSpeed = () => mouseSensitivity();
  const panSpeed = 0.01;
  const zoomSpeed = 0.3;
  const moveSpeed = () => 0.3 * (cameraSpeed() / 5);
  const fovSpeed = 0.02;
  const fovSpringSpeed = 0.15;
  const fovSpringDamping = 0.8;

  let movementInterval = null;
  
  // Velocity for smooth stopping (like Unreal Engine)
  let velocity = new Vector3(0, 0, 0);
  const cameraFriction = () => cameraSettings()?.friction || 0.93;

  const shouldAllowCameraMovement = (event) => {
    // If gizmo is being dragged, block all camera movement
    if (renderStore.isGizmoDragging) {
      return false;
    }
    
    // Check render store gizmo manager
    const gizmoManager = renderStore.gizmoManager;
    if (gizmoManager) {
      // Block camera movement when rotation or scale gizmos are active
      if (gizmoManager.rotationGizmoEnabled || gizmoManager.scaleGizmoEnabled) {
        return false;
      }
      
      // Allow camera movement for position gizmo (move gizmo)
      if (gizmoManager.positionGizmoEnabled) {
        // For position gizmo, only block left-click (to allow gizmo interaction)
        // Allow middle-click and right-click for camera movement
        if (event && (event.button === 1 || event.button === 2)) {
          return true; // Allow middle/right click even with position gizmo
        }
        // Block left-click when position gizmo is active
        if (event && event.button === 0) {
          return false;
        }
      }
    }
    
    return true;
  };

  const handlePointerDown = (event) => {
    if (!camera() || !canvas()) return;
    
    // Handle pointer down for camera movement
    
    if (!shouldAllowCameraMovement(event)) {
      return;
    }

    // Only prevent default and handle the event if we should allow camera movement
    event.preventDefault();
    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
    mouseDownPos = { x: event.clientX, y: event.clientY };
    isDragging = false;

    if (event.button === 0) {
      isLeftMouseDown = true;
    }
    if (event.button === 1) {
      isMiddleMouseDown = true;
    }
    if (event.button === 2) {
      isRightMouseDown = true;
    }

  };

  const handleMouseMove = (event) => {
    if (!camera() || isDisabled) return;
    
    // Check if camera movement should be blocked
    if (!shouldAllowCameraMovement(event)) {
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
      // Vertical drag = forward/backward movement (existing behavior)
      const forward = camera().getForwardRay().direction;
      const horizontalForward = new Vector3(forward.x, 0, forward.z).normalize();
      const fovSpeedMultiplier = camera().fov / (Math.PI / 4);
      const speedMultiplier = cameraSpeed() / 5; // Use camera speed setting
      const moveDirection = horizontalForward.scale(-deltaY * 0.025 * fovSpeedMultiplier * speedMultiplier * precisionMultiplier);
      camera().position = camera().position.add(moveDirection);
      
      // Horizontal drag = turn/rotate camera (added) - natural feel
      camera().rotation.y += deltaX * rotationSpeed() * precisionMultiplier;
    } else if (isRightMouseDown) {
      if (keysPressed.has('shift')) {
        // Shift + Right-click = Pan/strafe movement
        const forward = camera().getForwardRay().direction.normalize();
        const right = Vector3.Cross(Vector3.Up(), forward).normalize();
        const up = Vector3.Cross(right, forward).normalize();
        const speedMultiplier = cameraSpeed() / 5;
        const panVector = right.scale(-deltaX * panSpeed * speedMultiplier * precisionMultiplier)
          .add(up.scale(deltaY * panSpeed * speedMultiplier * precisionMultiplier));
        camera().position = camera().position.add(panVector);
        // Shift + Right-click panning
      } else {
        // Regular right-click = Free-look
        camera().rotation.y += deltaX * rotationSpeed() * precisionMultiplier; // Natural horizontal rotation
        camera().rotation.x += deltaY * rotationSpeed() * precisionMultiplier; // Natural vertical rotation
        camera().rotation.x = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, camera().rotation.x));
      }
    } else if (isMiddleMouseDown) {
        // Middle-click orbit around target (selected object or intelligently calculated point)
        let target;
        
        // First try to use selected object position
        const selectedObject = renderStore.selectedObject;
        if (selectedObject && selectedObject.position) {
          target = selectedObject.position;
        } else {
          // If no selection, orbit around a point in front of the camera
          // Use camera's current look direction and find a point at a reasonable distance
          const cameraForward = camera().getForwardRay().direction.normalize();
          const orbitDistance = 10; // Default orbit distance
          target = camera().position.add(cameraForward.scale(orbitDistance));
        }
        
        const distance = Vector3.Distance(camera().position, target);
        
        // Convert camera position to spherical coordinates relative to target
        const offset = camera().position.subtract(target);
        
        // Apply rotation based on mouse delta - natural feel
        const phi = Math.atan2(offset.x, offset.z) + deltaX * rotationSpeed() * precisionMultiplier;
        const theta = Math.acos(offset.y / distance) - deltaY * rotationSpeed() * precisionMultiplier;
        
        // Clamp theta to prevent flipping
        const clampedTheta = Math.max(0.1, Math.min(Math.PI - 0.1, theta));
        
        // Convert back to cartesian and set new position
        const newPosition = new Vector3(
          target.x + distance * Math.sin(clampedTheta) * Math.sin(phi),
          target.y + distance * Math.cos(clampedTheta),
          target.z + distance * Math.sin(clampedTheta) * Math.cos(phi)
        );
        
        camera().position = newPosition;
        camera().setTarget(target);
        
        // Middle-click orbiting around target
    }

    lastMouseX = event.clientX;
    lastMouseY = event.clientY;
  };

  const handleMouseUp = (event) => {
    if (!canvas()) return;

    if (event.button === 0) isLeftMouseDown = false;
    if (event.button === 1) isMiddleMouseDown = false;
    if (event.button === 2) isRightMouseDown = false;

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

  // Keyboard movement functions - simple and direct like before
  const moveForward = (speedMultiplier = 1.0) => {
    if (!camera()) return;
    const speed = moveSpeed() * speedMultiplier;
    const forward = camera().getDirection(Vector3.Forward()).normalize();
    camera().position = camera().position.add(forward.scale(speed));
  };
  
  const moveBackward = (speedMultiplier = 1.0) => {
    if (!camera()) return;
    const speed = moveSpeed() * speedMultiplier;
    const forward = camera().getDirection(Vector3.Forward()).normalize();
    camera().position = camera().position.add(forward.scale(-speed));
  };
  
  const moveLeft = (speedMultiplier = 1.0) => {
    if (!camera()) return;
    const speed = moveSpeed() * speedMultiplier;
    const forward = camera().getDirection(Vector3.Forward()).normalize();
    const right = Vector3.Cross(Vector3.Up(), forward).normalize();
    camera().position = camera().position.add(right.scale(-speed));
  };
  
  const moveRight = (speedMultiplier = 1.0) => {
    if (!camera()) return;
    const speed = moveSpeed() * speedMultiplier;
    const forward = camera().getDirection(Vector3.Forward()).normalize();
    const right = Vector3.Cross(Vector3.Up(), forward).normalize();
    camera().position = camera().position.add(right.scale(speed));
  };
  
  const moveUp = (speedMultiplier = 1.0) => {
    if (!camera()) return;
    const speed = moveSpeed() * speedMultiplier;
    camera().position = camera().position.add(Vector3.Up().scale(speed));
  };
  
  const moveDown = (speedMultiplier = 1.0) => {
    if (!camera()) return;
    const speed = moveSpeed() * speedMultiplier;
    camera().position = camera().position.add(Vector3.Up().scale(-speed));
  };

  // Arrow key movement functions based on camera type
  const handleArrowKeyMovement = (finalSpeed) => {
    if (!camera()) return;
    
    const cameraType = cameraSettings()?.type || 'universal';
    const cameraMode = cameraSettings()?.mode || 'orbit';
    
    // Get camera directions
    const forward = camera().getDirection(Vector3.Forward()).normalize();
    const right = Vector3.Cross(Vector3.Up(), forward).normalize();
    const up = Vector3.Up();
    
    // Apply movement based on camera type and mode
    if (cameraType === 'top' || cameraMode === 'top') {
      // Top view: arrows move in world X/Z plane
      if (keys.arrowUp) velocity = velocity.add(new Vector3(0, 0, finalSpeed));    // Move forward in world Z
      if (keys.arrowDown) velocity = velocity.add(new Vector3(0, 0, -finalSpeed));  // Move backward in world Z
      if (keys.arrowLeft) velocity = velocity.add(new Vector3(-finalSpeed, 0, 0));  // Move left in world X
      if (keys.arrowRight) velocity = velocity.add(new Vector3(finalSpeed, 0, 0));  // Move right in world X
    } else if (cameraType === 'front' || cameraMode === 'front') {
      // Front view: arrows move in world X/Y plane
      if (keys.arrowUp) velocity = velocity.add(new Vector3(0, finalSpeed, 0));     // Move up in world Y
      if (keys.arrowDown) velocity = velocity.add(new Vector3(0, -finalSpeed, 0));  // Move down in world Y
      if (keys.arrowLeft) velocity = velocity.add(new Vector3(-finalSpeed, 0, 0));  // Move left in world X
      if (keys.arrowRight) velocity = velocity.add(new Vector3(finalSpeed, 0, 0));  // Move right in world X
    } else if (cameraType === 'side' || cameraMode === 'side') {
      // Side view: arrows move in world Y/Z plane
      if (keys.arrowUp) velocity = velocity.add(new Vector3(0, finalSpeed, 0));     // Move up in world Y
      if (keys.arrowDown) velocity = velocity.add(new Vector3(0, -finalSpeed, 0));  // Move down in world Y
      if (keys.arrowLeft) velocity = velocity.add(new Vector3(0, 0, -finalSpeed));  // Move left in world Z
      if (keys.arrowRight) velocity = velocity.add(new Vector3(0, 0, finalSpeed));  // Move right in world Z
    } else {
      // Universal/orbit/perspective camera: arrows move relative to camera orientation
      if (keys.arrowUp) velocity = velocity.add(forward.scale(finalSpeed));         // Move forward
      if (keys.arrowDown) velocity = velocity.add(forward.scale(-finalSpeed));      // Move backward
      if (keys.arrowLeft) velocity = velocity.add(right.scale(-finalSpeed));        // Move left
      if (keys.arrowRight) velocity = velocity.add(right.scale(finalSpeed));        // Move right
    }
  };

  // WASD and arrow key movement state
  const keys = {
    w: false, a: false, s: false, d: false,
    q: false, e: false,
    arrowUp: false, arrowDown: false, arrowLeft: false, arrowRight: false
  };

  // Key handling for WASD and arrow keys
  const handleKeyDown = (event) => {
    if (isDisabled) return;
    const key = event.key.toLowerCase();
    keysPressed.add(key);
    
    switch(event.code) {
      case 'KeyW': keys.w = true; break;
      case 'KeyA': keys.a = true; break;
      case 'KeyS': keys.s = true; break;
      case 'KeyD': keys.d = true; break;
      case 'KeyQ': keys.q = true; break;
      case 'KeyE': keys.e = true; break;
      case 'ArrowUp': keys.arrowUp = true; break;
      case 'ArrowDown': keys.arrowDown = true; break;
      case 'ArrowLeft': keys.arrowLeft = true; break;
      case 'ArrowRight': keys.arrowRight = true; break;
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
      case 'ArrowUp': keys.arrowUp = false; break;
      case 'ArrowDown': keys.arrowDown = false; break;
      case 'ArrowLeft': keys.arrowLeft = false; break;
      case 'ArrowRight': keys.arrowRight = false; break;
    }
    
    if ((key === 'c' || key === 'z') && !keysPressed.has('c') && !keysPressed.has('z')) {
      animateFovSpringBack();
    }
  };

  // Movement loop with velocity and smooth stopping
  const handleKeyboardMovement = () => {
    if (!camera() || isDisabled) return;
    
    // Check if camera movement should be blocked (for rotation/scale gizmos)
    const gizmoManager = renderStore.gizmoManager;
    if (gizmoManager && (gizmoManager.rotationGizmoEnabled || gizmoManager.scaleGizmoEnabled)) {
      // Apply friction to stop movement when rotation/scale gizmos are active
      velocity = velocity.scale(0.8);
      if (velocity.length() < 0.001) {
        velocity = new Vector3(0, 0, 0);
      }
      return;
    }
    
    // Block keyboard movement if gizmo is being dragged
    if (renderStore.isGizmoDragging) {
      // Apply friction to stop movement
      velocity = velocity.scale(0.8);
      if (velocity.length() < 0.001) {
        velocity = new Vector3(0, 0, 0);
      }
      return;
    }
    
    // Only allow WASD movement when right-click is held (like Unreal Engine)
    if (!isRightMouseDown) {
      // Apply friction when not moving - convert 1-5 scale to retention factor
      const momentumLevel = cameraFriction(); // 1 to 5 (1=quick stop, 5=smooth drift)
      // Convert to retention factor: 1->0.01, 2->0.7, 3->0.82, 4->0.9, 5->0.95
      let retentionFactor;
      if (momentumLevel === 1) retentionFactor = 0.01;
      else if (momentumLevel === 2) retentionFactor = 0.7;
      else if (momentumLevel === 3) retentionFactor = 0.82;
      else if (momentumLevel === 4) retentionFactor = 0.9;
      else retentionFactor = 0.95;
      velocity = velocity.scale(retentionFactor);
      
      // Stop tiny movements
      if (velocity.length() < 0.001) {
        velocity = new Vector3(0, 0, 0);
      }
    } else {
      // Add velocity based on key input
      const speed = moveSpeed();
      const speedMultiplier = keysPressed.has('shift') ? 2.0 : (keysPressed.has('control') ? 0.2 : 1.0);
      const finalSpeed = speed * speedMultiplier * 0.3; // Scale down for velocity accumulation
      
      const forward = camera().getDirection(Vector3.Forward()).normalize();
      const right = Vector3.Cross(Vector3.Up(), forward).normalize();
      const up = Vector3.Up();
      
      // Add velocity instead of direct movement for WASD keys
      if (keys.w) velocity = velocity.add(forward.scale(finalSpeed));   // Forward (natural)
      if (keys.s) velocity = velocity.add(forward.scale(-finalSpeed));  // Backward
      if (keys.d) velocity = velocity.add(right.scale(finalSpeed));     // Right
      if (keys.a) velocity = velocity.add(right.scale(-finalSpeed));    // Left
      if (keys.e) velocity = velocity.add(up.scale(finalSpeed));        // Up
      if (keys.q) velocity = velocity.add(up.scale(-finalSpeed));       // Down
      
      // Apply some friction even when moving to prevent infinite acceleration
      velocity = velocity.scale(0.95);
    }
    
    // Handle arrow keys independently (they work without right-click)
    if (keys.arrowUp || keys.arrowDown || keys.arrowLeft || keys.arrowRight) {
      const speedMultiplier = keysPressed.has('shift') ? 2.0 : (keysPressed.has('control') ? 0.2 : 1.0);
      const finalSpeed = moveSpeed() * speedMultiplier * 0.3;
      handleArrowKeyMovement(finalSpeed);
    }
    
    // Apply velocity to camera position
    if (velocity.length() > 0.001) {
      camera().position = camera().position.add(velocity);
    }
  };

  createEffect(() => {
    if (!canvas()) return;

    // Set up camera event listeners
    
    if (camera()) {
      try {
        // IMPORTANT: Don't attach Babylon's native controls - we handle everything manually
        if (typeof camera().detachControls === 'function') {
          camera().detachControls();
          // Disabled native camera controls - using custom system
        }
      } catch (error) {
        console.warn('Could not detach camera controls:', error);
      }
    }
    
    const handleContextMenu = (e) => {
      e.preventDefault();
    };
    
    canvas().addEventListener('contextmenu', handleContextMenu);
    canvas().addEventListener('pointerdown', handlePointerDown, { capture: true }); // Use pointer events instead
    canvas().addEventListener('pointermove', handleMouseMove, { capture: true }); // Rename later
    canvas().addEventListener('pointerup', handleMouseUp, { capture: true }); // Rename later  
    canvas().addEventListener('wheel', handleWheel, { passive: false, capture: true });
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    
    // Start movement loop like your old working code
    if (movementInterval) {
      clearInterval(movementInterval);
    }
    movementInterval = setInterval(handleKeyboardMovement, 16); // ~60fps

    onCleanup(() => {
      // Clean up camera event listeners
      if (camera()) {
        try {
          if (typeof camera().detachControls === 'function') {
            camera().detachControls();
            // Camera controls detached
          }
        } catch (error) {
          console.warn('Could not detach camera controls:', error);
        }
      }
      
      canvas().removeEventListener('contextmenu', handleContextMenu);
      canvas().removeEventListener('pointerdown', handlePointerDown);
      canvas().removeEventListener('pointermove', handleMouseMove);
      canvas().removeEventListener('pointerup', handleMouseUp);
      canvas().removeEventListener('wheel', handleWheel);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
      
      // Stop movement loop
      if (movementInterval) {
        clearInterval(movementInterval);
        movementInterval = null;
      }
      
      if (fovAnimationId) {
        cancelAnimationFrame(fovAnimationId);
        fovAnimationId = null;
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


  const getIsDragging = () => isDragging;
  const getMouseDownPos = () => mouseDownPos;
  const getKeysPressed = () => keysPressed;
  const resetDragState = () => {
    mouseDownPos = null;
    isDragging = false;
  };

  // Expose disable method globally
  if (canvas()) {
    canvas()._cameraMovementController = {
      disable: () => { isDisabled = true; },
      enable: () => { isDisabled = false; }
    };
  }

  return {
    handleKeyboardMovement,
    getIsDragging,
    getMouseDownPos,
    getKeysPressed,
    resetDragState,
    disable: () => { isDisabled = true; },
    enable: () => { isDisabled = false; }
  };
}