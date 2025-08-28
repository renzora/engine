import { onMount, onCleanup } from 'solid-js';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';

// Hook for registering render viewport keyboard shortcuts
export function renderShortcuts(callbacks = {}) {
  let keysPressed = new Set();
  let movementInterval = null;
  let isRightClickHeld = false;

  onMount(() => {
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

    // Custom handler that also tracks key presses for movement
    const customHandler = (event) => {
      const key = event.key.toLowerCase();
      
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
      unregister();
    });
  });
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