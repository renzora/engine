/**
 * Project Management API
 * Handles project CRUD operations and project state
 */

import { createSignal } from 'solid-js';
import { bridgeFetch, parseJsonResponse } from './config.js';
import { bridgeService } from '@/plugins/core/bridge';

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
export async function createProject(name, template = 'basic', settings = null) {
  const response = await bridgeFetch('/projects', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, template, settings })
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

/**
 * Update current scene in project.json
 */
export async function updateProjectCurrentScene(sceneName, projectName = null) {
  const project = projectName || currentProject()?.name;
  if (!project) {
    throw new Error('No current project set');
  }
  
  try {
    const projectPath = `projects/${project}/project.json`;
    const projectContent = await bridgeService.readFile(projectPath);
    const projectData = JSON.parse(projectContent);
    
    projectData.currentScene = sceneName;
    projectData.last_modified = new Date().toISOString();
    
    await bridgeService.writeFile(projectPath, JSON.stringify(projectData, null, 2));
    console.log('✅ Updated project currentScene to:', sceneName);
    
    return { success: true };
  } catch (error) {
    console.error('❌ Failed to update project currentScene:', error);
    return { success: false, error: error.message };
  }
}

/**
 * Get current scene from project.json
 */
export async function getProjectCurrentScene(projectName = null) {
  const project = projectName || currentProject()?.name;
  if (!project) {
    throw new Error('No current project set');
  }
  
  try {
    const projectPath = `projects/${project}/project.json`;
    const projectContent = await bridgeService.readFile(projectPath);
    const projectData = JSON.parse(projectContent);
    
    return projectData.currentScene || 'main';
  } catch (error) {
    console.error('❌ Failed to get project currentScene:', error);
    return 'main'; // fallback
  }
}