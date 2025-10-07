/**
 * Direct Project Loader
 * Loads projects directly without caching
 */

/**
 * Load a project directly
 * @param {Object} project - Project object with name and path
 * @param {Function} onProjectReady - Callback when project is ready to use
 * @returns {Promise<void>}
 */
export async function loadProjectDirect(project, onProjectReady) {
  try {
    // Immediately call onProjectReady to load the project
    if (onProjectReady) {
      onProjectReady(project);
    }
  } catch (error) {
    console.error('Failed to load project:', error);
    throw error;
  }
}