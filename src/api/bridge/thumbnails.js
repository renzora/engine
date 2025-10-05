/**
 * Thumbnail Generation API
 * Handles 3D model thumbnail generation
 */

import { bridgeFetch, parseJsonResponse } from './config.js';
import { getCurrentProject } from './projects.js';

/**
 * Generate thumbnail for a 3D model asset
 */
export async function generateThumbnail(assetPath, size = 512) {
  const project = getCurrentProject();
  if (!project?.name) {
    throw new Error('No current project set');
  }

  const response = await bridgeFetch('/thumbnail', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      project_name: project.name,
      asset_path: assetPath,
      size: size
    })
  });

  return parseJsonResponse(response);
}

/**
 * Generate thumbnails for all compatible assets in a project in batch
 */
export async function generateBatchThumbnails() {
  const project = getCurrentProject();
  if (!project?.name) {
    throw new Error('No current project set');
  }

  const response = await bridgeFetch('/thumbnails/batch', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      project_name: project.name
    })
  });

  return parseJsonResponse(response);
}