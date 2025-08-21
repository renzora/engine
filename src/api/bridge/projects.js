/**
 * Project Management API
 * Handles project CRUD operations and project state
 */

import { createSignal } from 'solid-js';
import { bridgeFetch, parseJsonResponse } from './config.js';

// Project state management
const [currentProject, setCurrentProjectSignal] = createSignal(null);

/**
 * Get the current active project
 */
export function getCurrentProject() {
  return currentProject();
}

/**
 * Set the current active project
 */
export function setCurrentProject(project) {
  setCurrentProjectSignal(project);
}

/**
 * Get all projects
 */
export async function getProjects() {
  const response = await bridgeFetch('/projects');
  return parseJsonResponse(response);
}

/**
 * Create a new project
 */
export async function createProject(name, template = 'basic') {
  const response = await bridgeFetch('/projects', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, template })
  });
  return parseJsonResponse(response);
}

/**
 * Construct project-relative path
 */
export function getProjectPath(filePath = '', projectName = null) {
  const project = projectName || currentProject()?.name;
  if (!project) {
    throw new Error('No current project set');
  }
  
  if (filePath) {
    return `projects/${project}/${filePath}`;
  }
  return `projects/${project}`;
}

/**
 * Construct project asset path
 */
export function getProjectAssetPath(assetPath = '', projectName = null) {
  const project = projectName || currentProject()?.name;
  if (!project) {
    throw new Error('No current project set');
  }
  
  if (assetPath) {
    return `projects/${project}/assets/${assetPath}`;
  }
  return `projects/${project}/assets`;
}