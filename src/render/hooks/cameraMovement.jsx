import { createEffect, onCleanup } from 'solid-js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { viewportStore } from '@/layout/stores/ViewportStore';
import { renderStore } from '../store.jsx';
import { editorActions } from '@/layout/stores/EditorStore.jsx';

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
    // Check centralized camera controls first - pass the button number
    if (event && event.button === 0 && !editorActions.canPanCamera(0)) {
      return false; // Left-click panning disabled
    }
    if (event && event.button === 1 && !editorActions.canOrbitCamera(1)) {
      return false; // Middle-click orbit disabled
    }
    if (event && event.button === 2) {
      // Right-click can be either pan or orbit depending on modifier keys
      // For now, allow right-click if either panning or orbiting is enabled
      if (!editorActions.canPanCamera(2) && !editorActions.canOrbitCamera(2)) {
        return false;
      }
    }
    
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
    
    if (!shouldAllowCameraMovement(event)) {
      return;
    }

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

  // Helper function to detect if camera is in an orthographic view position
  const isOrthographicView = () => {
    if (!camera()) return false;
    
    const position = camera().position;
    const rotation = camera().rotation;
    
    // Check if camera is aligned with world axes (within tolerance)
    const tolerance = 0.1; // radians
    
    // Top view: looking down (pitch ~-90°, any yaw)
    if (Math.abs(rotation.x - (-Math.PI / 2)) < tolerance) {
      return { type: 'top', axis: 'y', direction: -1 };
    }
    
    // Bottom view: looking up (pitch ~90°, any yaw)
    if (Math.abs(rotation.x - (Math.PI / 2)) < tolerance) {
      return { type: 'bottom', axis: 'y', direction: 1 };
    }
    
    // Front view: looking along -Z axis (pitch ~0°, yaw ~0°)
    if (Math.abs(rotation.x) < tolerance && Math.abs(rotation.y) < tolerance) {
      return { type: 'front', axis: 'z', direction: -1 };
    }
    
    // Back view: looking along +Z axis (pitch ~0°, yaw ~180°)
    if (Math.abs(rotation.x) < tolerance && Math.abs(Math.abs(rotation.y) - Math.PI) < tolerance) {
      return { type: 'back', axis: 'z', direction: 1 };
    }
    
    // Right view: looking along -X axis (pitch ~0°, yaw ~90°)
    if (Math.abs(rotation.x) < tolerance && Math.abs(rotation.y - (Math.PI / 2)) < tolerance) {
      return { type: 'right', axis: 'x', direction: -1 };
    }
    
    // Left view: looking along +X axis (pitch ~0°, yaw ~-90°)
    if (Math.abs(rotation.x) < tolerance && Math.abs(rotation.y - (-Math.PI / 2)) < tolerance) {
      return { type: 'left', axis: 'x', direction: 1 };
    }
    
    return false;
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
    const speedMultiplier = cameraSpeed() / 5;
    const orthographicView = isOrthographicView();

    if (isLeftMouseDown) {
      if (orthographicView) {
        // Orthographic view panning - move in world space
        let panVector = new Vector3(0, 0, 0);
        const panAmount = 0.025 * speedMultiplier * precisionMultiplier;
        
        switch (orthographicView.type) {
          case 'top':
            // Top view: X/Z plane movement
            panVector = new Vector3(deltaX * panAmount, 0, deltaY * panAmount);
            break;
          case 'bottom':
            // Bottom view: X/Z plane movement 
            panVector = new Vector3(deltaX * panAmount, 0, -deltaY * panAmount);
            break;
          case 'front':
          case 'back':
            // Front/back view: X/Y plane movement
            panVector = new Vector3(-deltaX * panAmount, deltaY * panAmount, 0);
            break;
          case 'right':
          case 'left':
            // Right/left view: Z/Y plane movement  
            panVector = new Vector3(0, deltaY * panAmount, -deltaX * panAmount);
            break;
        }
        
        camera().position = camera().position.add(panVector);
      } else {
        // Perspective view - original behavior
        const forward = camera().getForwardRay().direction;
        const horizontalForward = new Vector3(forward.x, 0, forward.z).normalize();
        const fovSpeedMultiplier = camera().fov / (Math.PI / 4);
        const moveDirection = horizontalForward.scale(-deltaY * 0.025 * fovSpeedMultiplier * speedMultiplier * precisionMultiplier);
        camera().position = camera().position.add(moveDirection);
        
        // Horizontal drag = turn/rotate camera (added) - natural feel
        camera().rotation.y += deltaX * rotationSpeed() * precisionMultiplier;
      }
    } else if (isRightMouseDown) {
      if (keysPressed.has('shift')) {
        // Shift + Right-click = Pan/strafe movement
        if (orthographicView) {
          // Orthographic view panning - same as left-click for consistency
          let panVector = new Vector3(0, 0, 0);
          const panAmount = panSpeed * speedMultiplier * precisionMultiplier;
          
          switch (orthographicView.type) {
            case 'top':
              panVector = new Vector3(deltaX * panAmount, 0, deltaY * panAmount);
              break;
            case 'bottom':
              panVector = new Vector3(deltaX * panAmount, 0, -deltaY * panAmount);
              break;
            case 'front':
            case 'back':
              panVector = new Vector3(-deltaX * panAmount, deltaY * panAmount, 0);
              break;
            case 'right':
            case 'left':
              panVector = new Vector3(0, deltaY * panAmount, -deltaX * panAmount);
              break;
          }
          
          camera().position = camera().position.add(panVector);
        } else {
          // Perspective view - camera-relative panning
          const forward = camera().getForwardRay().direction.normalize();
          const right = Vector3.Cross(Vector3.Up(), forward).normalize();
          const up = Vector3.Cross(right, forward).normalize();
          const panVector = right.scale(-deltaX * panSpeed * speedMultiplier * precisionMultiplier)
            .add(up.scale(deltaY * panSpeed * speedMultiplier * precisionMultiplier));
          camera().position = camera().position.add(panVector);
        }
      } else {
        // Regular right-click = Free-look (disabled only for top/bottom views)
        if (!orthographicView || (orthographicView.type !== 'top' && orthographicView.type !== 'bottom')) {
          camera().rotation.y += deltaX * rotationSpeed() * precisionMultiplier;
          camera().rotation.x += deltaY * rotationSpeed() * precisionMultiplier;
          camera().rotation.x = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, camera().rotation.x));
        }
      }
    } else if (isMiddleMouseDown) {
      // Middle-click orbit around target (for all camera views)
      let target;
      
      const selectedObject = renderStore.selectedObject;
      if (selectedObject && selectedObject.position) {
        target = selectedObject.position;
      } else {
        const cameraForward = camera().getForwardRay().direction.normalize();
        const orbitDistance = 10;
        target = camera().position.add(cameraForward.scale(orbitDistance));
      }
      
      const distance = Vector3.Distance(camera().position, target);
      const offset = camera().position.subtract(target);
      
      const phi = Math.atan2(offset.x, offset.z) + deltaX * rotationSpeed() * precisionMultiplier;
      const theta = Math.acos(offset.y / distance) - deltaY * rotationSpeed() * precisionMultiplier;
      const clampedTheta = Math.max(0.1, Math.min(Math.PI - 0.1, theta));
      
      const newPosition = new Vector3(
        target.x + distance * Math.sin(clampedTheta) * Math.sin(phi),
        target.y + distance * Math.cos(clampedTheta),
        target.z + distance * Math.sin(clampedTheta) * Math.cos(phi)
      );
      
      camera().position = newPosition;
      camera().setTarget(target);
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
    
    // Check if zoom is enabled through centralized controls
    if (!editorActions.canZoomCamera()) {
      return;
    }

    event.preventDefault();
    const delta = event.deltaY * -0.01;
    const precisionMultiplier = keysPressed.has('control') ? 0.2 : 1.0;
    const orthographicView = isOrthographicView();
    
    let wheelSpeedMultiplier = 1.0;
    if (delta > 0) {
      wheelSpeedMultiplier = 0.6;
    } else {
      wheelSpeedMultiplier = 1.5;
    }
    
    if (orthographicView) {
      // Orthographic view: zoom by moving along the view direction
      let zoomVector = new Vector3(0, 0, 0);
      const zoomAmount = delta * zoomSpeed * wheelSpeedMultiplier * precisionMultiplier;
      
      switch (orthographicView.type) {
        case 'top':
          zoomVector = new Vector3(0, zoomAmount, 0);
          break;
        case 'bottom':
          zoomVector = new Vector3(0, -zoomAmount, 0);
          break;
        case 'front':
          zoomVector = new Vector3(0, 0, zoomAmount);
          break;
        case 'back':
          zoomVector = new Vector3(0, 0, -zoomAmount);
          break;
        case 'right':
          zoomVector = new Vector3(zoomAmount, 0, 0);
          break;
        case 'left':
          zoomVector = new Vector3(-zoomAmount, 0, 0);
          break;
      }
      
      camera().position = camera().position.add(zoomVector);
    } else {
      // Perspective view: zoom along camera forward direction
      const forward = camera().getForwardRay().direction.normalize();
      const fovSpeedMultiplier = camera().fov / (Math.PI / 4);
      camera().position = camera().position.add(forward.scale(delta * zoomSpeed * fovSpeedMultiplier * wheelSpeedMultiplier * precisionMultiplier));
    }
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

  // Arrow key movement functions based on camera view
  const handleArrowKeyMovement = (finalSpeed) => {
    if (!camera()) return;
    
    const orthographicView = isOrthographicView();
    
    if (orthographicView) {
      // Orthographic view: arrows move in world space
      switch (orthographicView.type) {
        case 'top':
          // Top view: arrows move in world X/Z plane
          if (keys.arrowUp) velocity = velocity.add(new Vector3(0, 0, finalSpeed));
          if (keys.arrowDown) velocity = velocity.add(new Vector3(0, 0, -finalSpeed));
          if (keys.arrowLeft) velocity = velocity.add(new Vector3(finalSpeed, 0, 0));
          if (keys.arrowRight) velocity = velocity.add(new Vector3(-finalSpeed, 0, 0));
          break;
        case 'bottom':
          // Bottom view: arrows move in world X/Z plane
          if (keys.arrowUp) velocity = velocity.add(new Vector3(0, 0, -finalSpeed));
          if (keys.arrowDown) velocity = velocity.add(new Vector3(0, 0, finalSpeed));
          if (keys.arrowLeft) velocity = velocity.add(new Vector3(finalSpeed, 0, 0));
          if (keys.arrowRight) velocity = velocity.add(new Vector3(-finalSpeed, 0, 0));
          break;
        case 'front':
        case 'back':
          // Front/back view: arrows move in world X/Y plane
          if (keys.arrowUp) velocity = velocity.add(new Vector3(0, finalSpeed, 0));
          if (keys.arrowDown) velocity = velocity.add(new Vector3(0, -finalSpeed, 0));
          if (keys.arrowLeft) velocity = velocity.add(new Vector3(-finalSpeed, 0, 0));
          if (keys.arrowRight) velocity = velocity.add(new Vector3(finalSpeed, 0, 0));
          break;
        case 'right':
        case 'left':
          // Right/left view: arrows move in world Y/Z plane
          if (keys.arrowUp) velocity = velocity.add(new Vector3(0, finalSpeed, 0));
          if (keys.arrowDown) velocity = velocity.add(new Vector3(0, -finalSpeed, 0));
          if (keys.arrowLeft) velocity = velocity.add(new Vector3(0, 0, -finalSpeed));
          if (keys.arrowRight) velocity = velocity.add(new Vector3(0, 0, finalSpeed));
          break;
      }
    } else {
      // Perspective camera: arrows move relative to camera orientation
      const forward = camera().getDirection(Vector3.Forward()).normalize();
      const right = Vector3.Cross(Vector3.Up(), forward).normalize();
      
      if (keys.arrowUp) velocity = velocity.add(forward.scale(finalSpeed));
      if (keys.arrowDown) velocity = velocity.add(forward.scale(-finalSpeed));
      if (keys.arrowLeft) velocity = velocity.add(right.scale(-finalSpeed));
      if (keys.arrowRight) velocity = velocity.add(right.scale(finalSpeed));
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
    canvas().addEventListener('pointerdown', handlePointerDown, { capture: true });
    canvas().addEventListener('pointermove', handleMouseMove, { capture: true });
    canvas().addEventListener('pointerup', handleMouseUp, { capture: true });
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