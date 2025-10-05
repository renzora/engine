/**
 * Direct Project Loader
 * Loads projects directly without overlay, shows indexing progress in status bar
 */

import { getProjectCacheStatus, ensureProjectCacheValid } from './projectCache.js';
import { projectIndexingActions } from '@/stores/ProjectIndexingStore.jsx';

/**
 * Load a project directly, handling cache indexing in background
 * @param {Object} project - Project object with name and path
 * @param {Function} onProjectReady - Callback when project is ready to use
 * @returns {Promise<void>}
 */
export async function loadProjectDirect(project, onProjectReady) {
  try {
    // Immediately call onProjectReady to load the project UI
    // This allows the user to start working while indexing happens in background
    if (onProjectReady) {
      onProjectReady(project);
    }

    // Check if cache needs processing
    const status = await getProjectCacheStatus(project.name);
    
    if (status.isValid) {
      // Cache is valid, no indexing needed
      return;
    }

    // Start background indexing
    projectIndexingActions.startIndexing(project.name);

    try {
      await ensureProjectCacheValid(project.name, {
        onProgress: (progressData) => {
          projectIndexingActions.updateProgress({
            progress: progressData.progress || 0,
            stage: progressData.current_stage || progressData.stage || 'Processing...',
            currentFile: progressData.currentFile,
            filesProcessed: progressData.filesProcessed,
            totalFiles: progressData.totalFiles
          });
        },
        onComplete: (result) => {
          projectIndexingActions.complete('Project indexed successfully');
          // Auto-hide after 2 seconds
          setTimeout(() => {
            projectIndexingActions.reset();
          }, 2000);
        },
        onError: (error) => {
          console.error('Background indexing error:', error);
          projectIndexingActions.error('Indexing failed - some features may be limited');
          // Auto-hide error after 5 seconds
          setTimeout(() => {
            projectIndexingActions.reset();
          }, 5000);
        }
      });
    } catch (error) {
      console.error('Failed to ensure cache valid:', error);
      projectIndexingActions.error('Indexing failed - some features may be limited');
      // Auto-hide error after 5 seconds
      setTimeout(() => {
        projectIndexingActions.reset();
      }, 5000);
    }
  } catch (error) {
    console.error('Failed to load project:', error);
    projectIndexingActions.error('Failed to load project');
    // Auto-hide error after 5 seconds
    setTimeout(() => {
      projectIndexingActions.reset();
    }, 5000);
    throw error;
  }
}

/**
 * Check if a project needs indexing without starting the process
 * @param {string} projectName - Name of the project
 * @returns {Promise<boolean>} - True if indexing is needed
 */
export async function projectNeedsIndexing(projectName) {
  try {
    const status = await getProjectCacheStatus(projectName);
    return !status.isValid;
  } catch (error) {
    console.warn('Failed to check project cache status:', error);
    return true; // Assume it needs indexing if we can't check
  }
}