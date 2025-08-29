import { onMount, onCleanup } from 'solid-js';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';
import { renderStore, renderActions } from '../store.jsx';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';

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
    isFreeTransform: false
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
      if (transformState.isActive && transformState.isFreeTransform) {
        const deltaX = event.clientX - transformState.startMousePos.x;
        const deltaY = event.clientY - transformState.startMousePos.y;
        applyFreeTransform(deltaX, deltaY);
      }
    };

    const handleMouseClick = (event) => {
      if (transformState.isActive) {
        if (event.button === 0) { // Left click - confirm
          event.preventDefault();
          event.stopPropagation();
          resetTransformState();
        } else if (event.button === 2) { // Right click - cancel
          event.preventDefault();
          event.stopPropagation();
          cancelTransform();
        }
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mousedown', handleMouseClick);

    // Custom handler that also tracks key presses for movement
    const customHandler = (event) => {
      const key = event.key.toLowerCase();
      
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
      document.removeEventListener('mousedown', handleMouseClick);
      unregister();
      
      // Clean up status div
      if (statusDiv && statusDiv.parentNode) {
        document.body.removeChild(statusDiv);
      }
    });
  });

  // Blender-style transform functions
  const handleTransformShortcuts = (event, key) => {
    // Start transform modes
    if (!transformState.isActive && ['g', 'r', 's'].includes(key) && !event.ctrlKey && !event.altKey && !event.shiftKey) {
      const selectedObject = renderStore.selectedObject;
      if (!selectedObject) return false;

      event.preventDefault();
      event.stopPropagation();
      
      transformState.isActive = true;
      transformState.numericInput = '';
      transformState.axis = null;
      transformState.isFreeTransform = true;
      transformState.startMousePos = { x: event.clientX || 0, y: event.clientY || 0 };
      
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

    if (transformState.numericInput) {
      text += `: ${transformState.numericInput}`;
    } else if (transformState.isFreeTransform) {
      text += ' (Free - move mouse, click to confirm, right-click to cancel)';
    } else {
      text += ' (Constrained - move mouse, click to confirm, right-click to cancel)';
    }

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

    const sensitivity = 0.01;
    
    switch (transformState.mode) {
      case 'move':
        if (transformState.axis) {
          // Constrained axis movement
          const delta = deltaX * sensitivity;
          switch (transformState.axis) {
            case 'x':
              selectedObject.position.x = transformState.originalTransform.position.x + delta;
              break;
            case 'y':
              selectedObject.position.y = transformState.originalTransform.position.y + delta;
              break;
            case 'z':
              selectedObject.position.z = transformState.originalTransform.position.z + delta;
              break;
          }
        } else {
          // Free movement in screen space
          const camera = renderStore.camera;
          if (camera) {
            const forward = camera.getForwardRay().direction;
            const right = Vector3.Cross(forward, Vector3.Up()).normalize();
            const up = Vector3.Cross(right, forward).normalize();
            
            const moveAmount = deltaX * sensitivity;
            const upAmount = -deltaY * sensitivity;
            
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

  const resetTransformState = () => {
    transformState.isActive = false;
    transformState.mode = null;
    transformState.axis = null;
    transformState.numericInput = '';
    statusDiv.style.display = 'none';
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