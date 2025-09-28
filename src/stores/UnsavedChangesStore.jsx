import { createStore } from 'solid-js/store';

const [unsavedChangesStore, setUnsavedChangesStore] = createStore({
  hasChanges: false,
  changes: [], // Array of change descriptions
  lastSaveTime: null,
  sceneModified: false,
  scriptsModified: false,
  objectsModified: false
});

export const unsavedChangesActions = {
  // Mark scene as modified
  markSceneModified: (description = 'Scene changes') => {
    setUnsavedChangesStore('sceneModified', true);
    setUnsavedChangesStore('hasChanges', true);
    unsavedChangesActions.addChange(description);
  },

  // Mark scripts as modified
  markScriptsModified: (description = 'Script changes') => {
    setUnsavedChangesStore('scriptsModified', true);
    setUnsavedChangesStore('hasChanges', true);
    unsavedChangesActions.addChange(description);
  },

  // Mark objects as modified
  markObjectsModified: (description = 'Object changes') => {
    setUnsavedChangesStore('objectsModified', true);
    setUnsavedChangesStore('hasChanges', true);
    unsavedChangesActions.addChange(description);
  },

  // Add a specific change description
  addChange: (description) => {
    const changes = unsavedChangesStore.changes;
    // Avoid duplicates
    if (!changes.includes(description)) {
      setUnsavedChangesStore('changes', [...changes, description]);
    }
  },

  // Clear all unsaved changes (called after successful save)
  clearChanges: () => {
    setUnsavedChangesStore({
      hasChanges: false,
      changes: [],
      lastSaveTime: Date.now(),
      sceneModified: false,
      scriptsModified: false,
      objectsModified: false
    });
  },

  // Clear specific type of changes
  clearSceneChanges: () => {
    setUnsavedChangesStore('sceneModified', false);
    unsavedChangesActions.removeChangesContaining('Scene');
    unsavedChangesActions.updateHasChanges();
  },

  clearScriptsChanges: () => {
    setUnsavedChangesStore('scriptsModified', false);
    unsavedChangesActions.removeChangesContaining('Script');
    unsavedChangesActions.updateHasChanges();
  },

  clearObjectsChanges: () => {
    setUnsavedChangesStore('objectsModified', false);
    unsavedChangesActions.removeChangesContaining('Object');
    unsavedChangesActions.updateHasChanges();
  },

  // Helper to remove changes containing specific text
  removeChangesContaining: (text) => {
    const filteredChanges = unsavedChangesStore.changes.filter(
      change => !change.toLowerCase().includes(text.toLowerCase())
    );
    setUnsavedChangesStore('changes', filteredChanges);
  },

  // Update hasChanges based on individual flags
  updateHasChanges: () => {
    const hasAnyChanges = unsavedChangesStore.sceneModified || 
                         unsavedChangesStore.scriptsModified || 
                         unsavedChangesStore.objectsModified;
    setUnsavedChangesStore('hasChanges', hasAnyChanges);
  },

  // Get formatted time since last save
  getTimeSinceLastSave: () => {
    if (!unsavedChangesStore.lastSaveTime) return null;
    
    const now = Date.now();
    const diff = now - unsavedChangesStore.lastSaveTime;
    const minutes = Math.floor(diff / 60000);
    const seconds = Math.floor((diff % 60000) / 1000);
    
    if (minutes > 0) {
      return `${minutes}m ${seconds}s ago`;
    } else {
      return `${seconds}s ago`;
    }
  },

  // Force set changes state (for manual control)
  setHasChanges: (hasChanges) => {
    setUnsavedChangesStore('hasChanges', hasChanges);
  }
};

export { unsavedChangesStore };

// Make available globally for debugging
if (typeof window !== 'undefined') {
  window.unsavedChangesStore = unsavedChangesStore;
  window.unsavedChangesActions = unsavedChangesActions;
}