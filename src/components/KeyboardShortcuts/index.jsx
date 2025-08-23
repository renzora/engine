import { onMount, onCleanup } from 'solid-js';

let isDisabled = false;
let activeHandlers = [];

const KeyboardShortcuts = () => {
  onMount(() => {
    const handleKeyDown = (event) => {
      // Skip if shortcuts are disabled (e.g., when Monaco Editor is focused)
      if (isDisabled) return;

      // Run all active keyboard handlers
      activeHandlers.forEach(handler => {
        try {
          handler(event);
        } catch (error) {
          console.error('Keyboard shortcut handler error:', error);
        }
      });
    };

    document.addEventListener('keydown', handleKeyDown);
    console.log('[KeyboardShortcuts] Global keyboard shortcut system initialized');

    onCleanup(() => {
      document.removeEventListener('keydown', handleKeyDown);
      console.log('[KeyboardShortcuts] Global keyboard shortcut system cleaned up');
    });
  });

  return null; // This component doesn't render anything
};

// Public API for managing keyboard shortcuts
export const keyboardShortcuts = {
  // Disable all shortcuts (useful when text input is focused)
  disable() {
    isDisabled = true;
    console.log('[KeyboardShortcuts] Shortcuts disabled');
  },

  // Re-enable shortcuts
  enable() {
    isDisabled = false;
    console.log('[KeyboardShortcuts] Shortcuts enabled');
  },

  // Register a keyboard shortcut handler
  register(handler) {
    if (typeof handler !== 'function') {
      console.error('[KeyboardShortcuts] Handler must be a function');
      return;
    }
    
    activeHandlers.push(handler);
    console.log('[KeyboardShortcuts] Handler registered, total handlers:', activeHandlers.length);
    
    // Return unregister function
    return () => {
      const index = activeHandlers.indexOf(handler);
      if (index > -1) {
        activeHandlers.splice(index, 1);
        console.log('[KeyboardShortcuts] Handler unregistered, total handlers:', activeHandlers.length);
      }
    };
  },

  // Helper to create common shortcut patterns
  createHandler(shortcuts) {
    return (event) => {
      for (const [key, callback] of Object.entries(shortcuts)) {
        if (matchesKey(event, key)) {
          event.preventDefault();
          event.stopPropagation();
          callback(event);
          break;
        }
      }
    };
  },

  // Get current state
  isDisabled() {
    return isDisabled;
  },

  // Get number of active handlers
  getHandlerCount() {
    return activeHandlers.length;
  }
};

// Helper function to match key combinations
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

export default KeyboardShortcuts;