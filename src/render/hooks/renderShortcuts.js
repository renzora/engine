import { onMount, onCleanup } from 'solid-js';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';
import { renderStore, renderActions } from '../store.jsx';
import { editorStore } from '@/layout/stores/EditorStore.jsx';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';
import { Matrix } from '@babylonjs/core/Maths/math.vector.js';
import { CreateLines } from '@babylonjs/core/Meshes/Builders/linesBuilder.js';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color.js';

// Hook for registering render viewport keyboard shortcuts
// Export the transform trigger function for external use
let triggerTransform = null;

export function renderShortcuts(callbacks = {}) {
  let keysPressed = new Set();
  let movementInterval = null;
  let isRightClickHeld = false;
  let currentMousePos = { x: 0, y: 0 }; // Track current mouse position
  
  // Blender-style transform state with smooth movement
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
    savedHighlightedMeshes: [], // Store highlighted meshes before transform
    smoothMovement: {
      velocity: null, // Current movement velocity
      damping: 0.85, // Movement damping factor for smoothness
      threshold: 0.001 // Minimum velocity threshold
    }
  };
  
  let statusDiv = null;

  onMount(() => {
    // Expose transform trigger function globally
    window.triggerBlenderTransform = (mode = 'move') => {
      const fakeEvent = {
        preventDefault: () => {},
        stopPropagation: () => {},
        ctrlKey: false,
        altKey: false,
        shiftKey: false
      };
      return handleTransformShortcuts(fakeEvent, mode === 'move' ? 'g' : mode === 'rotate' ? 'r' : 's');
    };
    
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
        // Right click held - camera movement enabled
      }
    };

    const handleMouseUp = (e) => {
      if (e.button === 2) { // Right mouse button
        isRightClickHeld = false;
        // Right click released - camera movement disabled
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
      if (keysPressed.size > 0 && !keyboardShortcuts.isDisabled() && !keyboardShortcuts.isInputFocused() && isRightClickHeld) {
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
      
      // Copy/Paste/Duplicate
      'ctrl+c': () => callbacks.copy?.(),
      'ctrl+v': () => callbacks.paste?.(),
      'shift+d': () => callbacks.duplicate?.(),
      
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
      
      // Camera shortcuts
      'home': () => callbacks.resetCamera?.(),
      'numpad.': () => callbacks.resetCamera?.(),
      
      // Grid toggle
      'ctrl+g': () => callbacks.toggleGrid?.(),
      
      // Panel toggles
      'space': () => callbacks.toggleBottomPanel?.(),
      'p': () => callbacks.toggleRightPanel?.(),
      
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
      // Always track mouse position for transform start
      currentMousePos.x = event.clientX;
      currentMousePos.y = event.clientY;
      
      // Expose to global scope for duplicate function
      window.currentMousePos = currentMousePos;
      
      if (transformState.isActive) {
        // No virtual cursor updates needed anymore!
        
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
          
          // Use accumulated deltas instead of absolute cursor position
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
          // Transform cancelled with right click
          cancelTransform();
        }
      }
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('click', handleMouseClick);

    // Custom handler that also tracks key presses for movement
    const customHandler = (event) => {
      const key = event.key.toLowerCase();
      
      // Skip if any input is focused - this provides an additional safety check
      if (keyboardShortcuts.isInputFocused()) {
        console.log('🔇 RenderShortcuts: Skipping - input is focused');
        return;
      }
      
      // Debug all key presses
      if (event.ctrlKey && key === 's') {
        console.log('🔍 Ctrl+S detected in renderShortcuts!');
      }
      
      // Handle Blender-style transform shortcuts
      if (handleTransformShortcuts(event, key)) {
        return; // Transform shortcut handled
      }
      
      // First check if this is a gizmo shortcut without modifiers (these take priority)
      const gizmoKeys = ['g', 'r', 's'];
      if (gizmoKeys.includes(key) && !event.ctrlKey && !event.altKey && !event.shiftKey) {
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
      
      // Track key presses for movement (only if not a gizmo shortcut and no modifiers)
      if (['w', 'a', 's', 'd', 'e', 'q', 'shift', 'control'].includes(key) && !event.shiftKey && !event.ctrlKey && !event.altKey) {
        keysPressed.add(key);
        return; // Don't prevent default for movement keys
      }
      
      // Handle all other shortcuts (including modifier combinations)
      for (const [keyPattern, callback] of Object.entries(gameShortcuts)) {
        if (matchesKey(event, keyPattern)) {
          console.log(`🔍 Shortcut matched: ${keyPattern}`);
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
      
      // Use current tracked mouse position
      transformState.startMousePos = { x: currentMousePos.x, y: currentMousePos.y };
      transformState.accumulatedDelta = { x: 0, y: 0 };
      
      // Just hide the cursor - no virtual cursor needed
      document.body.style.cursor = 'none';
      
      // Request pointer lock for infinite mouse movement
      const canvas = document.querySelector('canvas');
      if (canvas && canvas.requestPointerLock) {
        canvas.requestPointerLock();
      }
      
      // Store current highlights and disable highlighting during transform
      const highlightLayer = renderStore.highlightLayer;
      transformState.savedHighlightedMeshes = [...renderStore.selectedMeshes];
      if (highlightLayer) {
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

  // Function to apply grid snapping to an object
  const applyGridSnapping = (object) => {
    if (!object) return;
    
    // Get grid cell size from editor store, fallback to 1.0 if not available
    const gridSettings = editorStore.settings?.grid;
    const gridSize = gridSettings?.cellSize || 1.0;
    
    // Snap position to grid
    object.position.x = Math.round(object.position.x / gridSize) * gridSize;
    object.position.y = Math.round(object.position.y / gridSize) * gridSize;
    object.position.z = Math.round(object.position.z / gridSize) * gridSize;
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
    
    // Distance and zoom-based sensitivity for natural movement
    let sensitivity = 0.01;
    if (camera && selectedObject) {
      const distance = Vector3.Distance(camera.position, selectedObject.position);
      // Scale sensitivity based on distance - farther = faster movement, closer = slower movement
      sensitivity = Math.max(0.001, Math.min(0.1, distance * 0.003));
      
      console.log(`Distance: ${distance.toFixed(2)}, Sensitivity: ${sensitivity.toFixed(4)}`);
    }
    
    switch (transformState.mode) {
      case 'move':
        if (transformState.axis) {
          // Constrained axis movement - project mouse movement onto world axis consistently
          if (camera) {
            // Get world axis vector
            let worldAxis;
            switch (transformState.axis) {
              case 'x':
                worldAxis = Vector3.Right(); // World X axis
                break;
              case 'y':
                worldAxis = Vector3.Up(); // World Y axis  
                break;
              case 'z':
                worldAxis = Vector3.Forward(); // World Z axis
                break;
            }
            
            // Get camera's screen space axes
            const cameraForward = camera.getDirection(Vector3.Forward()).normalize();
            const cameraRight = Vector3.Cross(Vector3.Up(), cameraForward).normalize();
            const cameraUp = Vector3.Cross(cameraRight, cameraForward).normalize();
            
            // Project world axis onto camera screen space to get consistent direction
            const screenRightComponent = Vector3.Dot(worldAxis, cameraRight);
            const screenUpComponent = Vector3.Dot(worldAxis, cameraUp);
            
            // Calculate movement based on screen projection with consistent behavior
            let axisMovement = 0;
            
            // Use combined mouse movement weighted by screen projection
            const rightContribution = (deltaX * screenRightComponent);
            const upContribution = (deltaY * screenUpComponent); // Don't invert Y here - let projection handle it
            
            axisMovement = (rightContribution + upContribution) * sensitivity;
            
            // Apply movement along pure world axis
            const movement = worldAxis.scale(axisMovement);
            selectedObject.position.copyFrom(transformState.originalTransform.position);
            selectedObject.position.addInPlace(movement);
            
            // Apply grid snapping if snap mode is enabled
            if (callbacks.checkSnapMode && callbacks.checkSnapMode()) {
              applyGridSnapping(selectedObject);
            }
            
            console.log(`🖱️ MOVE ${transformState.axis.toUpperCase()}: axisMovement=${axisMovement.toFixed(3)}, screenRight=${screenRightComponent.toFixed(3)}, screenUp=${screenUpComponent.toFixed(3)}`);
          }
        } else {
          // Free movement in camera screen space
          if (camera) {
            const cameraForward = camera.getDirection(Vector3.Forward()).normalize();
            const cameraRight = Vector3.Cross(Vector3.Up(), cameraForward).normalize();
            const cameraUp = Vector3.Cross(cameraRight, cameraForward).normalize();
            
            const rightMovement = deltaX * sensitivity;
            const upMovement = deltaY * sensitivity; // Remove Y inversion to test
            
            selectedObject.position.copyFrom(transformState.originalTransform.position);
            selectedObject.position.addInPlace(cameraRight.scale(rightMovement));
            selectedObject.position.addInPlace(cameraUp.scale(upMovement));
            
            // Apply grid snapping if snap mode is enabled
            if (callbacks.checkSnapMode && callbacks.checkSnapMode()) {
              applyGridSnapping(selectedObject);
            }
          }
        }
        break;
        
      case 'rotate':
        const rotationSensitivity = 0.02;
        if (transformState.axis) {
          // Constrained axis rotation - world-consistent (like Blender/Maya)
          let rotDelta;
          switch (transformState.axis) {
            case 'x':
              // X-axis rotation: horizontal mouse movement rotates around world X-axis
              rotDelta = deltaX * rotationSensitivity;
              selectedObject.rotation.x = transformState.originalTransform.rotation.x + rotDelta;
              break;
            case 'y':
              // Y-axis rotation: horizontal mouse movement rotates around world Y-axis
              rotDelta = deltaX * rotationSensitivity;
              selectedObject.rotation.y = transformState.originalTransform.rotation.y + rotDelta;
              break;
            case 'z':
              // Z-axis rotation: horizontal mouse movement rotates around world Z-axis
              rotDelta = deltaX * rotationSensitivity;
              selectedObject.rotation.z = transformState.originalTransform.rotation.z + rotDelta;
              break;
          }
        }
        break;
        
      case 'scale':
        const scaleSensitivity = 0.01;
        if (transformState.axis) {
          // Constrained axis scaling - world-consistent (like Blender/Maya)
          let scaleDelta;
          switch (transformState.axis) {
            case 'x':
              // X-axis scaling: horizontal mouse movement scales world X-axis
              scaleDelta = 1 + (deltaX * scaleSensitivity);
              selectedObject.scaling.x = transformState.originalTransform.scaling.x * scaleDelta;
              break;
            case 'y':
              // Y-axis scaling: vertical mouse movement scales world Y-axis (inverted)
              scaleDelta = 1 + (-deltaY * scaleSensitivity);
              selectedObject.scaling.y = transformState.originalTransform.scaling.y * scaleDelta;
              break;
            case 'z':
              // Z-axis scaling: vertical mouse movement scales world Z-axis (inverted)
              scaleDelta = 1 + (-deltaY * scaleSensitivity);
              selectedObject.scaling.z = transformState.originalTransform.scaling.z * scaleDelta;
              break;
          }
        } else {
          // Uniform scaling uses horizontal movement
          const scaleDelta = 1 + (deltaX * scaleSensitivity);
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


  const snapObjectToCursor = () => {
    const selectedObject = renderStore.selectedObject;
    const camera = renderStore.camera;
    const canvas = document.querySelector('canvas');
    
    if (!selectedObject || !camera || !canvas) return;
    
    try {
      const scene = renderStore.scene;
      const canvasRect = canvas.getBoundingClientRect();
      
      // Convert cursor screen position to canvas relative position
      const cursorX = currentMousePos.x - canvasRect.left;
      const cursorY = currentMousePos.y - canvasRect.top;
      
      // Use camera's unproject to get world position from screen position
      const pickInfo = scene.pick(cursorX, cursorY);
      if (pickInfo && pickInfo.pickedPoint) {
        // Use the picked point as the new object position
        selectedObject.position.copyFrom(pickInfo.pickedPoint);
        // Update original transform for relative movements
        transformState.originalTransform.position = selectedObject.position.clone();
      } else {
        // Fallback: project cursor to a plane in front of camera
        const distance = Vector3.Distance(camera.position, selectedObject.position);
        const ray = camera.getForwardRay(distance);
        const newPos = ray.origin.add(ray.direction.scale(distance));
        selectedObject.position.copyFrom(newPos);
        transformState.originalTransform.position = selectedObject.position.clone();
      }
      
      console.log(`📍 Snapped object to cursor position`);
    } catch (error) {
      console.warn('Could not snap object to cursor:', error);
    }
  };

  const resetTransformState = () => {
    transformState.isActive = false;
    transformState.mode = null;
    transformState.axis = null;
    transformState.numericInput = '';
    transformState.accumulatedDelta = null;
    transformState.smoothMovement.velocity = null; // Reset smooth movement
    statusDiv.style.display = 'none';
    
    // Restore real cursor
    document.body.style.cursor = '';
    
    // Remove axis line
    if (transformState.axisLine) {
      transformState.axisLine.dispose();
      transformState.axisLine = null;
    }
    
    
    // Re-enable highlights after transform using saved meshes
    const highlightLayer = renderStore.highlightLayer;
    if (highlightLayer && transformState.savedHighlightedMeshes && transformState.savedHighlightedMeshes.length > 0) {
      try {
        transformState.savedHighlightedMeshes.forEach(mesh => {
          if (mesh) {
            highlightLayer.addMesh(mesh, Color3.Yellow());
          }
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
      'Shift+D': 'Duplicate Selected Object',
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