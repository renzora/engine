import { onMount, onCleanup } from 'solid-js';
import { keyboardShortcuts } from '@/components/KeyboardShortcuts';

// Hook for registering game engine keyboard shortcuts
export function useGameEngineShortcuts(callbacks = {}) {
  let keysPressed = new Set();
  let movementInterval = null;

  onMount(() => {
    // Start continuous movement handling
    movementInterval = setInterval(() => {
      if (keysPressed.size > 0 && !keyboardShortcuts.isDisabled()) {
        applyMovement();
      }
    }, 16); // 60fps

    const applyMovement = () => {
      const speedMultiplier = keysPressed.has('shift') ? 3.0 : keysPressed.has('control') ? 0.3 : 1.0;
      
      if (keysPressed.has('w')) callbacks.moveForward?.(speedMultiplier);
      if (keysPressed.has('s')) callbacks.moveBackward?.(speedMultiplier);
      if (keysPressed.has('a')) callbacks.moveLeft?.(speedMultiplier);
      if (keysPressed.has('d')) callbacks.moveRight?.(speedMultiplier);
      if (keysPressed.has('e')) callbacks.moveUp?.(speedMultiplier);
      if (keysPressed.has('q')) callbacks.moveDown?.(speedMultiplier);
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
      
      // Track key presses for movement
      if (['w', 'a', 's', 'd', 'e', 'q', 'shift', 'control'].includes(key)) {
        keysPressed.add(key);
        return; // Don't prevent default for movement keys
      }
      
      // Handle other shortcuts
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
export function getGameEngineShortcuts() {
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