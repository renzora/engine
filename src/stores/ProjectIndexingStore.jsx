import { createSignal } from 'solid-js';

// Global project indexing state
const [indexingState, setIndexingState] = createSignal({
  isIndexing: false,
  projectName: '',
  progress: 0,
  stage: '',
  currentFile: '',
  filesProcessed: 0,
  totalFiles: 0,
  error: null,
  completed: false
});

// Actions to update indexing state
export const projectIndexingActions = {
  startIndexing: (projectName) => {
    setIndexingState({
      isIndexing: true,
      projectName: projectName,
      progress: 0,
      stage: 'Starting indexing...',
      currentFile: '',
      filesProcessed: 0,
      totalFiles: 0,
      error: null,
      completed: false
    });
  },

  updateProgress: (data) => {
    setIndexingState(prev => ({
      ...prev,
      progress: data.progress || prev.progress,
      stage: data.stage || data.current_stage || prev.stage,
      currentFile: data.currentFile || prev.currentFile,
      filesProcessed: data.filesProcessed !== undefined ? data.filesProcessed : prev.filesProcessed,
      totalFiles: data.totalFiles !== undefined ? data.totalFiles : prev.totalFiles
    }));
  },

  complete: (message = 'Indexing completed') => {
    setIndexingState(prev => ({
      ...prev,
      isIndexing: false,
      completed: true,
      stage: message,
      progress: 1
    }));
  },

  error: (errorMessage) => {
    setIndexingState(prev => ({
      ...prev,
      isIndexing: false,
      error: errorMessage,
      stage: 'Indexing failed'
    }));
  },

  reset: () => {
    setIndexingState({
      isIndexing: false,
      projectName: '',
      progress: 0,
      stage: '',
      currentFile: '',
      filesProcessed: 0,
      totalFiles: 0,
      error: null,
      completed: false
    });
  }
};

// Export reactive store
export { indexingState };