import { createStore } from 'solid-js/store';

const [closeConfirmationStore, setCloseConfirmationStore] = createStore({
  isOpen: false,
  projectName: '',
  changes: [],
  onSaveAndClose: null,
  onCloseWithoutSaving: null,
  onClose: null
});

export const closeConfirmationActions = {
  // Show the close confirmation overlay
  show: (options = {}) => {
    setCloseConfirmationStore({
      isOpen: true,
      projectName: options.projectName || '',
      changes: options.changes || [],
      onSaveAndClose: options.onSaveAndClose || null,
      onCloseWithoutSaving: options.onCloseWithoutSaving || null,
      onClose: options.onClose || (() => closeConfirmationActions.hide())
    });
  },

  // Hide the close confirmation overlay
  hide: () => {
    console.log('Hiding close confirmation overlay');
    setCloseConfirmationStore({
      isOpen: false,
      projectName: '',
      changes: [],
      onSaveAndClose: null,
      onCloseWithoutSaving: null,
      onClose: null
    });
  },

  // Check if overlay is currently open
  isOpen: () => closeConfirmationStore.isOpen
};

export { closeConfirmationStore };

// Make available globally for debugging
if (typeof window !== 'undefined') {
  window.closeConfirmationStore = closeConfirmationStore;
  window.closeConfirmationActions = closeConfirmationActions;
}