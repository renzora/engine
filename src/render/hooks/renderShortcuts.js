import { onMount, onCleanup } from 'solid-js';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';
import { renderStore, renderActions } from '../store.jsx';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { CreateLines } from '@babylonjs/core/Meshes/Builders/linesBuilder.js';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';

// Hook for registering render viewport keyboard shortcuts
export function renderShortcuts(callbacks = {}) {
  let keysPressed = new Set();
  let movementInterval = null;
  let isRightClickHeld = false;
  
  // Blender-style transform state
  let transformState = {
    mode: null, // 'move', 'rotate', 'scale'
    axis: null, // 'x', 'y', 'z', null for free
    numericInput: '',
    isActive: false,
    startMousePos: { x: 0, y: 0 },
    originalTransform: null,
    isFreeTransform: false,
    axisLine: null, // Babylon.js line mesh for axis visualization
    virtualCursor: null, // Virtual cursor element
    savedHighlightedMeshes: [] // Store highlighted meshes before transform
  };
  
  let statusDiv = null;

  onMount(() => {
    // Create status div for showing transform mode
    statusDiv = document.createElement('div');
    statusDiv.style.cssText = `
      position: fixed;
      top: 10px;
      left: 50%;
      transform: translateX(-50%);
      background: rgba(0, 0, 0, 0.8);
      color: white;
      padding: 8px 16px;
      border-radius: 4px;
      font-family: monospace;
      font-size: 14px;
      z-index: 1000;
      display: none;
      pointer-events: none;
    `;
    document.body.appendChild(statusDiv);
    
    // Create virtual cursor div with axis arrows
    const virtualCursor = document.createElement('div');
    virtualCursor.style.cssText = `
      position: fixed;
      width: 80px;
      height: 80px;
      pointer-events: none;
      z-index: 10000;
      display: none;
      transform: translate(-50%, -50%);
    `;
    
    virtualCursor.innerHTML = `
      <div style="
        position: absolute;
        top: 50%;
        left: 50%;
        width: 20px;
        height: 20px;
        border: 2px solid white;
        border-radius: 50%;
        background: rgba(0, 0, 0, 0.5);
        transform: translate(-50%, -50%);
        box-shadow: 0 0 10px rgba(0, 0, 0, 0.5);
      "></div>
    `;
    
    document.body.appendChild(virtualCursor);
    transformState.virtualCursor = virtualCursor;
    
    // Track right mouse button state
    const handleMouseDown = (e) => {
      if (e.button === 2) { // Right mouse button
        isRightClickHeld = true;
        console.log('🖱️ Right click held - camera movement enabled');
      }
    };

    const handleMouseUp = (e) => {
      if (e.button === 2) { // Right mouse button
        isRightClickHeld = false;
        console.log('🖱️ Right click released - camera movement disabled');
      }
    };

    // Prevent context menu when right-clicking
    const handleContextMenu = (e) => {
      e.preventDefault();
    };

    // Add mouse event listeners
    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('mouseup', handleMouseUp);
    document.addEventListener('contextmenu', handleContextMenu);

    // Start continuous movement handling
    movementInterval = setInterval(() => {
      if (keysPressed.size > 0 && !keyboardShortcuts.isDisabled() && isRightClickHeld) {
        applyMovement();
      }
    }, 16); // 60fps

    const applyMovement = () => {
      const speedMultiplier = keysPressed.has('shift') ? 3.0 : keysPressed.has('control') ? 0.3 : 1.0;
      
      if (keysPressed.has('w')) {
        console.log('🎮 W pressed - moving forward');
        callbacks.moveForward?.(speedMultiplier);
      }
      if (keysPressed.has('s')) {
        console.log('🎮 S pressed - moving backward');
        callbacks.moveBackward?.(speedMultiplier);
      }
      if (keysPressed.has('a')) {
        console.log('🎮 A pressed - moving left');
        callbacks.moveLeft?.(speedMultiplier);
      }
      if (keysPressed.has('d')) {
        console.log('🎮 D pressed - moving right');
        callbacks.moveRight?.(speedMultiplier);
      }
      if (keysPressed.has('e')) {
        console.log('🎮 E pressed - moving up');
        callbacks.moveUp?.(speedMultiplier);
      }
      if (keysPressed.has('q')) {
        console.log('🎮 Q pressed - moving down');
        callbacks.moveDown?.(speedMultiplier);
      }
    };

    // Define all game engine shortcuts
    const gameShortcuts = {
      // Focus on object
      'f': () => callbacks.focusObject?.(),
      
      // Focus on mouse point
      'shift+f': () => callbacks.focusOnMousePoint?.(),
      
      // Delete object
      'delete': () => callbacks.deleteObject?.(),
      
      // Transform gizmos
      'g': () => callbacks.positionMode?.(),  // Position/Move gizmo
      'r': () => callbacks.rotateMode?.(),    // Rotation gizmo
      's': () => callbacks.scaleMode?.(),     // Scale gizmo
      
      // Copy/Paste
      'ctrl+c': () => callbacks.copy?.(),
      'ctrl+v': () => callbacks.paste?.(),
      
      // Snap to ground
      'end': () => callbacks.snapToGround?.(),
      
      // File operations (common shortcuts)
      'ctrl+s': () => callbacks.save?.(),
      'ctrl+o': () => callbacks.open?.(),
      'ctrl+n': () => callbacks.newScene?.(),
      'ctrl+z': () => callbacks.undo?.(),
      'ctrl+y': () => callbacks.redo?.(),
      
      // View shortcuts
      '1': () => callbacks.frontView?.(),
      '2': () => callbacks.sideView?.(),
      '3': () => callbacks.topView?.(),
      '7': () => callbacks.perspectiveView?.(),
      
      // Grid toggle
      'ctrl+g': () => callbacks.toggleGrid?.(),
      
      // Selection
      'ctrl+a': () => callbacks.selectAll?.(),
      'alt+a': () => callbacks.deselectAll?.(),
      
      // Viewport shortcuts
      'alt+1': () => callbacks.viewport1?.(),
      'alt+2': () => callbacks.viewport2?.(),
      'alt+3': () => callbacks.viewport3?.(),
      'alt+4': () => callbacks.viewport4?.(),
      
      // Function keys
      'f1': () => callbacks.help?.(),
      'f5': () => callbacks.refresh?.(),
      'f11': () => callbacks.fullscreen?.(),
    };

    // Add mouse event handlers for transforms
    const handleMouseMove = (event) => {
      if (transformState.isActive) {
        // Update virtual cursor position
        if (transformState.virtualCursor && transformState.virtualCursor.style.display === 'block') {
          if (document.pointerLockElement) {
            // Get canvas bounds for proper viewport wrapping
            const canvas = document.querySelector('canvas');
            if (canvas) {
              const canvasRect = canvas.getBoundingClientRect();
              
              // Move virtual cursor based on movement deltas with viewport wrapping
              const currentLeft = parseInt(transformState.virtualCursor.style.left) || (canvasRect.left + canvasRect.width / 2);
              const currentTop = parseInt(transformState.virtualCursor.style.top) || (canvasRect.top + canvasRect.height / 2);
              
              let newLeft = currentLeft + (event.movementX || 0);
              let newTop = currentTop + (event.movementY || 0);
              
              // Wrap around canvas viewport edges
              const margin = 20; // Small margin from edges
              if (newLeft < canvasRect.left + margin) {
                newLeft = canvasRect.right - margin;
              } else if (newLeft > canvasRect.right - margin) {
                newLeft = canvasRect.left + margin;
              }
              
              if (newTop < canvasRect.top + margin) {
                newTop = canvasRect.bottom - margin;
              } else if (newTop > canvasRect.bottom - margin) {
                newTop = canvasRect.top + margin;
              }
              
              transformState.virtualCursor.style.left = newLeft + 'px';
              transformState.virtualCursor.style.top = newTop + 'px';
            }
          } else {
            transformState.virtualCursor.style.left = event.clientX + 'px';
            transformState.virtualCursor.style.top = event.clientY + 'px';
          }
        }
        
        if (document.pointerLockElement) {
          // Use movementX/Y when pointer is locked
          const deltaX = event.movementX;
          const deltaY = event.movementY;
          
          // Accumulate movement for smooth transforms
          if (!transformState.accumulatedDelta) {
            transformState.accumulatedDelta = { x: 0, y: 0 };
          }
          
          transformState.accumulatedDelta.x += deltaX;
          transformState.accumulatedDelta.y += deltaY;
          
          applyFreeTransform(transformState.accumulatedDelta.x, transformState.accumulatedDelta.y);
        } else {
          // Fallback to regular mouse movement
          const deltaX = event.clientX - transformState.startMousePos.x;
          const deltaY = event.clientY - transformState.startMousePos.y;
          applyFreeTransform(deltaX, deltaY);
        }
      }
    };

    const handleMouseClick = (event) => {
      if (transformState.isActive) {
        event.preventDefault();
        event.stopPropagation();
        
        if (event.button === 0) { // Left click - confirm
          console.log('✅ Transform confirmed with left click');
          resetTransformState();
        } else if (event.button === 2) { // Right click - cancel
          console.log('❌ Transform cancelled with right click');
          cancelTransform();
        }
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('click', handleMouseClick);

    // Custom handler that also tracks key presses for movement
    const customHandler = (event) => {
      const key = event.key.toLowerCase();
      
      // Disable Ctrl+F (browser find)
      if (event.ctrlKey && key === 'f') {
        event.preventDefault();
        event.stopPropagation();
        console.log('🚫 Ctrl+F disabled in render viewport');
        return;
      }
      
      // Handle Blender-style transform shortcuts
      if (handleTransformShortcuts(event, key)) {
        return; // Transform shortcut handled
      }
      
      // First check if this is a gizmo shortcut without modifiers (these take priority)
      const gizmoKeys = ['g', 'r', 's'];
      if (gizmoKeys.includes(key) && !event.ctrlKey && !event.altKey && !event.shiftKey && !transformState.isActive) {
        // Handle gizmo shortcuts first
        for (const [keyPattern, callback] of Object.entries(gameShortcuts)) {
          if (matchesKey(event, keyPattern)) {
            event.preventDefault();
            event.stopPropagation();
            callback(event);
            console.log(`🛠️ Gizmo shortcut triggered: ${key.toUpperCase()}`);
            return; // Exit early - don't add to movement keys
          }
        }
      }
      
      // Track key presses for movement (only if not a gizmo shortcut)
      if (['w', 'a', 's', 'd', 'e', 'q', 'shift', 'control'].includes(key)) {
        keysPressed.add(key);
        return; // Don't prevent default for movement keys
      }
      
      // Handle other shortcuts (non-gizmo)
      for (const [keyPattern, callback] of Object.entries(gameShortcuts)) {
        if (matchesKey(event, keyPattern)) {
          event.preventDefault();
          event.stopPropagation();
          callback(event);
          break;
        }
      }
    };

    // Track key releases for movement
    const handleKeyUp = (event) => {
      const key = event.key.toLowerCase();
      keysPressed.delete(key);
    };

    // Register handlers
    const unregister = keyboardShortcuts.register(customHandler);
    document.addEventListener('keyup', handleKeyUp);

    console.log('[GameEngineShortcuts] Game engine shortcuts registered');

    onCleanup(() => {
      if (movementInterval) {
        clearInterval(movementInterval);
      }
      document.removeEventListener('keyup', handleKeyUp);
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('mouseup', handleMouseUp);
      document.removeEventListener('contextmenu', handleContextMenu);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('click', handleMouseClick);
      unregister();
      
      // Clean up status div and virtual cursor
      if (statusDiv && statusDiv.parentNode) {
        document.body.removeChild(statusDiv);
      }
      if (transformState.virtualCursor && transformState.virtualCursor.parentNode) {
        document.body.removeChild(transformState.virtualCursor);
      }
    });
  });

  // Blender-style transform functions
  const handleTransformShortcuts = (event, key) => {
    // Start transform modes - but not when right mouse button is held for camera movement
    if (!transformState.isActive && ['g', 'r', 's'].includes(key) && !event.ctrlKey && !event.altKey && !event.shiftKey && !isRightClickHeld) {
      const selectedObject = renderStore.selectedObject;
      if (!selectedObject) return false;

      event.preventDefault();
      event.stopPropagation();
      
      transformState.isActive = true;
      transformState.numericInput = '';
      transformState.axis = null;
      transformState.isFreeTransform = true;
      transformState.startMousePos = { x: event.clientX || 0, y: event.clientY || 0 };
      transformState.accumulatedDelta = { x: 0, y: 0 };
      
      // Request pointer lock for infinite mouse movement
      const canvas = document.querySelector('canvas');
      if (canvas && canvas.requestPointerLock) {
        canvas.requestPointerLock();
      }
      
      // Show virtual cursor and hide real cursor
      if (transformState.virtualCursor) {
        transformState.virtualCursor.style.display = 'block';
        transformState.virtualCursor.style.left = event.clientX + 'px';
        transformState.virtualCursor.style.top = event.clientY + 'px';
      }
      document.body.style.cursor = 'none';
      
      // Store current highlights and disable highlighting during transform
      const highlightLayer = renderStore.highlightLayer;
      transformState.savedHighlightedMeshes = [];
      if (highlightLayer) {
        // Store currently highlighted meshes
        if (selectedObject.getChildMeshes) {
          const childMeshes = selectedObject.getChildMeshes();
          childMeshes.forEach(childMesh => {
            if (childMesh.getClassName() === 'Mesh') {
              transformState.savedHighlightedMeshes.push(childMesh);
            }
          });
        } else {
          transformState.savedHighlightedMeshes.push(selectedObject);
        }
        highlightLayer.removeAllMeshes();
      }
      
      // Store original transform for cancellation
      transformState.originalTransform = {
        position: selectedObject.position.clone(),
        rotation: selectedObject.rotation.clone(),
        scaling: selectedObject.scaling.clone()
      };
      
      switch (key) {
        case 'g':
          transformState.mode = 'move';
          break;
        case 'r':
          transformState.mode = 'rotate';
          break;
        case 's':
          transformState.mode = 'scale';
          break;
      }
      
      updateTransformStatus();
      return true;
    }

    // Handle transform mode input
    if (transformState.isActive) {
      event.preventDefault();
      event.stopPropagation();
      console.log('Transform mode key:', key, 'event.key:', event.key);
      
      // Axis selection - constrains to single axis
      if (['x', 'y', 'z'].includes(key)) {
        transformState.axis = key;
        transformState.isFreeTransform = false; // Switch to constrained mode
        createAxisLine(key);
        updateTransformStatus();
        return true;
      }
      
      // Numeric input - applies immediately
      if (/[0-9.-]/.test(key)) {
        transformState.numericInput += key;
        const value = parseFloat(transformState.numericInput);
        applyNumericTransform(value);
        resetTransformState();
        return true;
      }
      
      // Backspace to edit numeric input
      if (key === 'backspace') {
        transformState.numericInput = transformState.numericInput.slice(0, -1);
        updateTransformStatus();
        return true;
      }
      
      
      // Escape to cancel
      if (key === 'escape') {
        resetTransformState();
        return true;
      }
      
      return true; // Consume all other keys while in transform mode
    }
    
    return false; // Not a transform shortcut
  };

  const updateTransformStatus = () => {
    if (!transformState.isActive) {
      statusDiv.style.display = 'none';
      return;
    }

    // Only show status for numeric input, hide for visual modes (free transform or axis lines)
    if (!transformState.numericInput) {
      statusDiv.style.display = 'none';
      return;
    }

    let text = '';
    switch (transformState.mode) {
      case 'move':
        text = 'Move';
        break;
      case 'rotate':
        text = 'Rotate';
        break;
      case 'scale':
        text = 'Scale';
        break;
    }

    if (transformState.axis) {
      text += ` ${transformState.axis.toUpperCase()}`;
    }

    text += `: ${transformState.numericInput}`;

    statusDiv.textContent = text;
    statusDiv.style.display = 'block';
  };

  const applyNumericTransform = (value) => {
    const selectedObject = renderStore.selectedObject;
    if (!selectedObject || !transformState.mode) return;

    const axis = transformState.axis;

    switch (transformState.mode) {
      case 'move':
        applyMove(selectedObject, axis, value);
        break;
      case 'rotate':
        applyRotate(selectedObject, axis, value);
        break;
      case 'scale':
        applyScale(selectedObject, axis, value);
        break;
    }
  };

  const applyFreeTransform = (deltaX, deltaY) => {
    const selectedObject = renderStore.selectedObject;
    if (!selectedObject || !transformState.originalTransform) return;

    const camera = renderStore.camera;
    
    // Calculate distance-based sensitivity for zoom-responsive movement
    let sensitivity = 0.01;
    if (camera && selectedObject) {
      const distance = Vector3.Distance(camera.position, selectedObject.position);
      // Scale sensitivity based on camera distance (closer = slower, farther = faster)
      sensitivity = Math.max(0.001, distance * 0.002);
    }
    
    switch (transformState.mode) {
      case 'move':
        if (transformState.axis) {
          // Constrained axis movement - project mouse movement onto world axes relative to camera
          if (camera) {
            const forward = camera.getForwardRay().direction;
            const right = Vector3.Cross(forward, Vector3.Up()).normalize();
            const up = Vector3.Cross(right, forward).normalize();
            
            // Calculate movement based on camera orientation
            let axisVector, mouseDelta;
            
            switch (transformState.axis) {
              case 'x':
                axisVector = Vector3.Right(); // World X axis
                // Project camera right vector onto world X to determine direction
                const xDot = Vector3.Dot(right, axisVector);
                mouseDelta = deltaX * (xDot > 0 ? 1 : -1) * sensitivity;
                selectedObject.position.x = transformState.originalTransform.position.x + mouseDelta;
                break;
              case 'y':
                axisVector = Vector3.Up(); // World Y axis
                // Y axis is always up regardless of camera rotation
                mouseDelta = -deltaY * sensitivity; // Invert for natural movement
                selectedObject.position.y = transformState.originalTransform.position.y + mouseDelta;
                break;
              case 'z':
                axisVector = Vector3.Forward(); // World Z axis
                // Project camera forward vector onto world Z to determine direction
                const zDot = Vector3.Dot(forward, axisVector);
                mouseDelta = deltaY * (zDot > 0 ? -1 : 1) * sensitivity;
                selectedObject.position.z = transformState.originalTransform.position.z + mouseDelta;
                break;
            }
          }
        } else {
          // Free movement in screen space
          if (camera) {
            const forward = camera.getForwardRay().direction;
            const right = Vector3.Cross(forward, Vector3.Up()).normalize();
            const up = Vector3.Cross(right, forward).normalize();
            
            const moveAmount = deltaX * sensitivity;
            const upAmount = -deltaY * sensitivity; // Invert Y for natural movement
            
            selectedObject.position.copyFrom(transformState.originalTransform.position);
            selectedObject.position.addInPlace(right.scale(moveAmount));
            selectedObject.position.addInPlace(up.scale(upAmount));
          }
        }
        break;
        
      case 'rotate':
        const rotationSensitivity = 0.02;
        if (transformState.axis) {
          const rotDelta = deltaX * rotationSensitivity;
          switch (transformState.axis) {
            case 'x':
              selectedObject.rotation.x = transformState.originalTransform.rotation.x + rotDelta;
              break;
            case 'y':
              selectedObject.rotation.y = transformState.originalTransform.rotation.y + rotDelta;
              break;
            case 'z':
              selectedObject.rotation.z = transformState.originalTransform.rotation.z + rotDelta;
              break;
          }
        }
        break;
        
      case 'scale':
        const scaleSensitivity = 0.01;
        const scaleDelta = 1 + (deltaX * scaleSensitivity);
        if (transformState.axis) {
          switch (transformState.axis) {
            case 'x':
              selectedObject.scaling.x = transformState.originalTransform.scaling.x * scaleDelta;
              break;
            case 'y':
              selectedObject.scaling.y = transformState.originalTransform.scaling.y * scaleDelta;
              break;
            case 'z':
              selectedObject.scaling.z = transformState.originalTransform.scaling.z * scaleDelta;
              break;
          }
        } else {
          // Uniform scaling
          selectedObject.scaling.copyFrom(transformState.originalTransform.scaling);
          selectedObject.scaling.scaleInPlace(scaleDelta);
        }
        break;
    }
  };

  const cancelTransform = () => {
    const selectedObject = renderStore.selectedObject;
    if (selectedObject && transformState.originalTransform) {
      // Restore original transform
      selectedObject.position.copyFrom(transformState.originalTransform.position);
      selectedObject.rotation.copyFrom(transformState.originalTransform.rotation);
      selectedObject.scaling.copyFrom(transformState.originalTransform.scaling);
    }
    resetTransformState();
  };

  const applyMove = (object, axis, value) => {
    if (!axis) {
      return;
    }

    const moveVector = new Vector3(0, 0, 0);
    switch (axis) {
      case 'x':
        moveVector.x = value;
        break;
      case 'y':
        moveVector.y = value;
        break;
      case 'z':
        moveVector.z = value;
        break;
    }

    object.position.addInPlace(moveVector);
    console.log(`Moved object ${value} units on ${axis.toUpperCase()} axis`);
  };

  const applyRotate = (object, axis, value) => {
    if (!axis) {
      return;
    }

    const radians = (value * Math.PI) / 180;
    
    switch (axis) {
      case 'x':
        object.rotation.x += radians;
        break;
      case 'y':
        object.rotation.y += radians;
        break;
      case 'z':
        object.rotation.z += radians;
        break;
    }

    console.log(`Rotated object ${value}° on ${axis.toUpperCase()} axis`);
  };

  const applyScale = (object, axis, value) => {
    if (!axis) {
      if (value !== 0) {
        object.scaling.scaleInPlace(value);
        console.log(`Scaled object uniformly by ${value}`);
      }
      return;
    }

    switch (axis) {
      case 'x':
        object.scaling.x *= value || 1;
        break;
      case 'y':
        object.scaling.y *= value || 1;
        break;
      case 'z':
        object.scaling.z *= value || 1;
        break;
    }

    console.log(`Scaled object by ${value} on ${axis.toUpperCase()} axis`);
  };


  const createAxisLine = (axis) => {
    const selectedObject = renderStore.selectedObject;
    const scene = renderStore.scene;
    
    if (!selectedObject || !scene) return;
    
    // Remove existing axis line
    if (transformState.axisLine) {
      transformState.axisLine.dispose();
      transformState.axisLine = null;
    }
    
    
    // Get object center position and offset it slightly above the object
    const center = selectedObject.position.clone();
    const offset = new Vector3(0, 0.5, 0); // Lift line above object
    const lineCenter = center.add(offset);
    const lineLength = 1000; // Very long line to span entire viewport
    
    // Define axis direction and color
    let direction, color;
    switch (axis) {
      case 'x':
        direction = new Vector3(lineLength, 0, 0);
        color = Color3.Red();
        break;
      case 'y':
        direction = new Vector3(0, lineLength, 0);
        color = Color3.Green();
        break;
      case 'z':
        direction = new Vector3(0, 0, lineLength);
        color = Color3.Blue();
        break;
    }
    
    // Create line points (from far negative to far positive) centered above object
    const points = [
      lineCenter.subtract(direction),
      lineCenter.add(direction)
    ];
    
    // Create the line mesh
    transformState.axisLine = CreateLines(`transformAxis_${axis}`, { points }, scene);
    transformState.axisLine.color = color;
    transformState.axisLine.isPickable = false;
    transformState.axisLine.alwaysSelectAsActiveMesh = false;
    transformState.axisLine.renderingGroupId = 2; // Render on top
    transformState.axisLine.alpha = 1; // Ensure full opacity
    transformState.axisLine.useAlphaFromDiffuseTexture = false;
    transformState.axisLine.doNotSyncBoundingInfo = true; // Don't affect bounding calculations
    
    console.log(`Created ${axis.toUpperCase()} axis line`);
  };

  const resetTransformState = () => {
    transformState.isActive = false;
    transformState.mode = null;
    transformState.axis = null;
    transformState.numericInput = '';
    transformState.accumulatedDelta = null;
    statusDiv.style.display = 'none';
    
    // Hide virtual cursor and restore real cursor
    if (transformState.virtualCursor) {
      transformState.virtualCursor.style.display = 'none';
    }
    document.body.style.cursor = '';
    
    // Remove axis line
    if (transformState.axisLine) {
      transformState.axisLine.dispose();
      transformState.axisLine = null;
    }
    
    
    // Re-enable highlighting after transform using saved meshes
    const highlightLayer = renderStore.highlightLayer;
    if (highlightLayer && transformState.savedHighlightedMeshes.length > 0) {
      try {
        transformState.savedHighlightedMeshes.forEach(mesh => {
          highlightLayer.addMesh(mesh, Color3.Yellow());
        });
      } catch (error) {
        console.warn('Could not restore highlight after transform:', error);
      }
    }
    transformState.savedHighlightedMeshes = [];
    
    // Exit pointer lock
    if (document.pointerLockElement) {
      document.exitPointerLock();
    }
  };

}

// Helper function to match key combinations (copied from KeyboardShortcuts)
function matchesKey(event, keyPattern) {
  const parts = keyPattern.toLowerCase().split('+');
  const key = parts.pop(); // Last part is the actual key
  
  // Check modifiers
  const needsCtrl = parts.includes('ctrl') || parts.includes('cmd');
  const needsAlt = parts.includes('alt');
  const needsShift = parts.includes('shift');
  
  const hasCtrl = event.ctrlKey || event.metaKey;
  const hasAlt = event.altKey;
  const hasShift = event.shiftKey;
  
  // Check if modifiers match
  if (needsCtrl !== hasCtrl || needsAlt !== hasAlt || needsShift !== hasShift) {
    return false;
  }
  
  // Check the actual key
  const eventKey = event.key.toLowerCase();
  const eventCode = event.code?.toLowerCase();
  
  return eventKey === key || eventCode === key || eventCode === `key${key}`;
}

// Utility to get all defined shortcuts for documentation
export function getRenderShortcuts() {
  return {
    'Movement': {
      'W': 'Move Forward',
      'A': 'Move Left', 
      'S': 'Move Backward',
      'D': 'Move Right',
      'Q': 'Move Up',
      'E': 'Move Down',
      'Shift+W/A/S/D': 'Fast Movement'
    },
    'Camera': {
      '1': 'Front View',
      '2': 'Side View', 
      '3': 'Top View',
      '7': 'Perspective View',
      'F': 'Focus Object'
    },
    'Tools': {
      'R': 'Rotate Mode',
      'T': 'Translate Mode',
      'Y': 'Scale Mode',
      'Space': 'Select Tool',
      'G': 'Toggle Grid'
    },
    'File': {
      'Ctrl+S': 'Save',
      'Ctrl+O': 'Open',
      'Ctrl+N': 'New Scene'
    },
    'Edit': {
      'Ctrl+Z': 'Undo',
      'Ctrl+Y': 'Redo',
      'Ctrl+C': 'Copy',
      'Ctrl+V': 'Paste',
      'Ctrl+X': 'Cut',
      'Ctrl+D': 'Duplicate',
      'Delete': 'Delete Object'
    },
    'Selection': {
      'Ctrl+A': 'Select All',
      'Alt+A': 'Deselect All',
      'Ctrl+I': 'Invert Selection'
    },
    'Playback': {
      'Ctrl+Space': 'Play/Pause',
      'Ctrl+Shift+Space': 'Stop',
      'Ctrl+Left': 'Previous Frame',
      'Ctrl+Right': 'Next Frame'
    }
  };
}