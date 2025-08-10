import { useState, useEffect, useCallback } from 'react';
import { projectManager } from '@/services/ProjectManager';

export function useAssetAPI() {
  const [isInitialized, setIsInitialized] = useState(true);

  useEffect(() => {
    // Removed Electron initialization
  }, []);

  const fetchFolderTree = useCallback(async (currentProject) => {
    const response = await fetch(`/api/projects/${currentProject.name}/assets/tree`);
    if (!response.ok) {
      throw new Error('Failed to fetch folder tree');
    }
    const data = await response.json();
    return data.tree;
  }, []);

  const fetchAssetCategories = useCallback(async (currentProject) => {
    const response = await fetch(`/api/projects/${currentProject.name}/assets/categories`);
    if (!response.ok) {
      throw new Error('Failed to fetch asset categories');
    }
    const data = await response.json();
    return data.categories;
  }, []);

  const fetchAssets = useCallback(async (currentProject, path = '') => {
    const response = await fetch(`/api/projects/${currentProject.name}/assets?folder=${encodeURIComponent(path)}`);
    if (!response.ok) {
      throw new Error('Failed to fetch assets');
    }
    const data = await response.json();
    return data.assets || [];
  }, []);

  const searchAssets = useCallback(async (currentProject, query) => {
    try {
      const response = await fetch(`/api/projects/${currentProject.name}/assets/search?q=${encodeURIComponent(query)}`);
      if (response.ok) {
        const data = await response.json();
        return data.results || [];
      }
    } catch (error) {
      console.warn('Server search not available:', error);
    }
    return [];
  }, []);

  const createFolder = useCallback(async (currentProject, folderName, parentPath = '') => {
    const response = await fetch(`/api/projects/${currentProject.name}/assets/folder`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        folderName: folderName.trim(),
        parentPath: parentPath
      })
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(errorData.error || 'Failed to create folder');
    }

    return await response.json();
  }, []);

  const moveAsset = useCallback(async (currentProject, sourcePath, targetPath) => {
    const response = await fetch(`/api/projects/${currentProject.name}/assets/move`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        sourcePath,
        targetPath
      })
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(errorData.error || 'Failed to move item');
    }

    return await response.json();
  }, []);

  const deleteAsset = useCallback(async (currentProject, assetPath) => {
    const response = await fetch(`/api/projects/${currentProject.name}/assets/${encodeURIComponent(assetPath)}`, {
      method: 'DELETE'
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(errorData.error || 'Failed to delete asset');
    }

    return await response.json();
  }, []);

  const addFileChangeListener = useCallback((callback) => {
    projectManager.addFileChangeListener(callback);
    return () => projectManager.removeFileChangeListener(callback);
  }, []);

  return {
    isInitialized,
    fetchFolderTree,
    fetchAssetCategories,
    fetchAssets,
    searchAssets,
    createFolder,
    moveAsset,
    deleteAsset,
    addFileChangeListener
  };
}