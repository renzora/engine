/**
 * Project Cache API
 * Handles project-level asset caching and processing
 */

import { bridgeFetch, parseJsonResponse } from './config.js';
import { getCurrentProject } from './projects.js';

/**
 * Validate project cache status
 */
export async function validateProjectCache(projectName) {
  if (!projectName) {
    const project = getCurrentProject();
    if (!project?.name) {
      throw new Error('No project specified and no current project set');
    }
    projectName = project.name;
  }

  const response = await bridgeFetch(`/projects/${encodeURIComponent(projectName)}/cache/validate`, {
    method: 'GET',
  });

  return parseJsonResponse(response);
}

/**
 * Process project cache (with optional progress callback)
 */
export async function processProjectCache(projectName, options = {}) {
  if (!projectName) {
    const project = getCurrentProject();
    if (!project?.name) {
      throw new Error('No project specified and no current project set');
    }
    projectName = project.name;
  }

  const {
    forceFullRebuild = false,
    fileTypes = ['all'],
    onProgress = null,
    onComplete = null,
    onError = null
  } = options;

  try {
    // Get cache status first to show detailed progress
    const cacheStatus = await validateProjectCache(projectName);
    const totalChanges = cacheStatus.changes_detected || 0;
    
    // Enhanced progress simulation with detailed stages
    if (onProgress) {
      onProgress({ 
        progress: 0.05, 
        current_stage: 'Validating project cache...',
        filesProcessed: 0,
        totalFiles: totalChanges
      });
    }

    const response = await bridgeFetch(`/projects/${encodeURIComponent(projectName)}/cache/process`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        force_full_rebuild: forceFullRebuild,
        file_types: fileTypes,
        stream_progress: true
      })
    });

    // For SSE streaming (future enhancement)
    if (response.headers.get('content-type')?.includes('text/event-stream')) {
      return handleProcessingStream(response, { onProgress, onComplete, onError });
    }

    // Handle as regular JSON response (fallback)
    const result = await parseJsonResponse(response);

    // Call completion callback
    if (result.success && onComplete) {
      onComplete(result);
    } else if (!result.success && onError) {
      onError(result.error || 'Cache processing failed');
    }

    return result;
  } catch (error) {
    if (onError) {
      onError(error.message || 'Network error during cache processing');
    }
    throw error;
  }
}

/**
 * Get cached assets for a project
 */
export async function getCachedAssets(projectName) {
  if (!projectName) {
    const project = getCurrentProject();
    if (!project?.name) {
      throw new Error('No project specified and no current project set');
    }
    projectName = project.name;
  }

  const response = await bridgeFetch(`/projects/${encodeURIComponent(projectName)}/assets`, {
    method: 'GET',
  });

  return parseJsonResponse(response);
}

/**
 * Check if project cache is valid (quick check)
 */
export async function isProjectCacheValid(projectName) {
  try {
    const validation = await validateProjectCache(projectName);
    return validation.cache_status === 'valid';
  } catch (error) {
    console.warn('Failed to validate project cache:', error);
    return false;
  }
}

/**
 * Get cache status summary
 */
export async function getProjectCacheStatus(projectName) {
  try {
    const validation = await validateProjectCache(projectName);
    return {
      isValid: validation.cache_status === 'valid',
      status: validation.cache_status,
      changesDetected: validation.changes_detected || 0,
      estimatedTime: validation.estimated_processing_time || 0,
      changeSummary: validation.change_summary || {
        new_files: 0,
        modified_files: 0,
        deleted_files: 0,
        moved_files: 0
      }
    };
  } catch (error) {
    console.warn('Failed to get cache status:', error);
    return {
      isValid: false,
      status: 'error',
      changesDetected: 0,
      estimatedTime: 0,
      changeSummary: { new_files: 0, modified_files: 0, deleted_files: 0, moved_files: 0 },
      error: error.message
    };
  }
}

/**
 * Process project if cache is invalid
 */
export async function ensureProjectCacheValid(projectName, options = {}) {
  const status = await getProjectCacheStatus(projectName);
  
  if (status.isValid && !options.forceRebuild) {
    // Cache is valid, return cached assets
    return await getCachedAssets(projectName);
  }

  // Cache needs processing
  const processResult = await processProjectCache(projectName, {
    forceFullRebuild: options.forceRebuild,
    onProgress: options.onProgress,
    onComplete: options.onComplete,
    onError: options.onError
  });

  if (processResult.success) {
    // Return cached assets after processing
    return await getCachedAssets(projectName);
  }

  throw new Error(`Cache processing failed: ${processResult.message || 'Unknown error'}`);
}

/**
 * Handle SSE processing stream
 */
async function handleProcessingStream(response, { onProgress, onComplete, onError }) {
  try {
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    
    while (true) {
      const { done, value } = await reader.read();
      
      if (done) {
        break;
      }
      
      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop(); // Keep incomplete line in buffer
      
      for (const line of lines) {
        if (line.startsWith('data: ')) {
          try {
            const data = JSON.parse(line.slice(6));
            
            if (data.type === 'progress' && onProgress) {
              onProgress({
                progress: data.progress,
                current_stage: data.stage,
                stage: data.stage,
                currentFile: data.current_file,
                filesProcessed: data.files_processed,
                totalFiles: data.total_files,
                operation: data.operation
              });
            } else if (data.type === 'complete' && onComplete) {
              const result = {
                success: data.success,
                processed_count: data.processed_count,
                message: data.message
              };
              onComplete(result);
              return result;
            } else if (data.type === 'error' && onError) {
              onError(data.error);
              throw new Error(data.error);
            }
          } catch (parseError) {
            console.warn('Failed to parse SSE data:', line, parseError);
          }
        }
      }
    }
    
    // If we reach here without completion, return a default result
    return { success: true, message: 'Processing completed' };
    
  } catch (error) {
    console.error('SSE processing error:', error);
    if (onError) {
      onError(error.message || 'Stream processing failed');
    }
    throw error;
  }
}

/**
 * Utility: Format cache status for UI display
 */
export function formatCacheStatus(status) {
  const statusLabels = {
    'valid': 'Up to date',
    'needs_update': 'Needs update',
    'needs_full_rebuild': 'Needs rebuild',
    'missing': 'Not cached',
    'error': 'Error'
  };

  return statusLabels[status] || status;
}

/**
 * Utility: Format estimated time for UI display
 */
export function formatEstimatedTime(seconds) {
  if (seconds < 60) {
    return `${seconds}s`;
  } else if (seconds < 3600) {
    const minutes = Math.ceil(seconds / 60);
    return `${minutes}m`;
  } else {
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.ceil((seconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
  }
}

/**
 * Utility: Get cache status color for UI
 */
export function getCacheStatusColor(status) {
  const colors = {
    'valid': 'text-green-600',
    'needs_update': 'text-yellow-600', 
    'needs_full_rebuild': 'text-orange-600',
    'missing': 'text-red-600',
    'error': 'text-red-600'
  };

  return colors[status] || 'text-gray-600';
}

/**
 * Get cached project asset tree structure
 */
export async function getProjectAssetTree(projectName) {
  if (!projectName) {
    const project = getCurrentProject();
    if (!project?.name) {
      throw new Error('No project specified and no current project set');
    }
    projectName = project.name;
  }

  const response = await bridgeFetch(`/projects/${encodeURIComponent(projectName)}/cache/tree`, {
    method: 'GET',
  });

  return parseJsonResponse(response);
}