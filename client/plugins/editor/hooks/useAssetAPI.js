import { useState, useEffect, useCallback } from 'react';
import { projectManager } from '@/plugins/projects/projectManager.js';

export function useAssetAPI() {
  const [isElectron, setIsElectron] = useState(false);
  const [isInitialized, setIsInitialized] = useState(false);

  useEffect(() => {
    const electronCheck = window.electronAPI?.isElectron || false;
    setIsElectron(electronCheck);
    
    if (electronCheck && window.fileSystemAPI) {
      const currentProject = projectManager.getCurrentProject();
      if (currentProject?.path) {
        window.fileSystemAPI.setProjectPath(currentProject.path)
          .then(() => {
            console.log('File system API initialized for project:', currentProject.path);
            setIsInitialized(true);
          })
          .catch(error => {
            console.error('Failed to initialize file system API:', error);
            setIsInitialized(true);
          });
      } else {
        console.log('No project path available for file system API, using server fallback');
        setIsInitialized(true);
      }
    } else {
      setIsInitialized(true);
    }
  }, []);

  const fetchFolderTree = useCallback(async (currentProject) => {
    if (isElectron && window.fileSystemAPI) {
      try {
        const result = await window.fileSystemAPI.getProjectAssetsTree();
        return result.tree;
      } catch (error) {
        console.error('Electron file system error, falling back to server:', error);
      }
    }
    
    const response = await fetch(`/api/projects/${currentProject.name}/assets/tree`);
    if (!response.ok) {
      throw new Error('Failed to fetch folder tree');
    }
    const data = await response.json();
    return data.tree;
  }, [isElectron]);

  const fetchAssetCategories = useCallback(async (currentProject) => {
    if (isElectron && window.fileSystemAPI) {
      try {
        const result = await window.fileSystemAPI.getAssetCategories();
        return result.categories;
      } catch (error) {
        console.error('Electron file system error, falling back to server:', error);
      }
    }
    
    const response = await fetch(`/api/projects/${currentProject.name}/assets/categories`);
    if (!response.ok) {
      throw new Error('Failed to fetch asset categories');
    }
    const data = await response.json();
    return data.categories;
  }, [isElectron]);

  const fetchAssets = useCallback(async (currentProject, path = '') => {
    if (isElectron && window.fileSystemAPI) {
      try {
        const result = await window.fileSystemAPI.getAssetsInFolder(path);
        return result.assets;
      } catch (error) {
        console.error('Electron file system error, falling back to server:', error);
      }
    }

    const response = await fetch(`/api/projects/${currentProject.name}/assets?folder=${encodeURIComponent(path)}`);
    if (!response.ok) {
      throw new Error('Failed to fetch assets');
    }
    const data = await response.json();
    return data.assets || [];
  }, [isElectron]);

  const searchAssets = useCallback(async (currentProject, query) => {
    if (isElectron && window.fileSystemAPI) {
      try {
        const result = await window.fileSystemAPI.searchAssets(query);
        return result.results;
      } catch (error) {
        console.error('Electron file system error, falling back to server:', error);
      }
    }
    
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
  }, [isElectron]);

  const createFolder = useCallback(async (currentProject, folderName, parentPath = '') => {
    if (isElectron && window.fileSystemAPI) {
      try {
        const result = await window.fileSystemAPI.createFolder(folderName, parentPath);
        return result;
      } catch (error) {
        console.error('Electron file system error, falling back to server:', error);
      }
    }
    
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
  }, [isElectron]);

  const moveAsset = useCallback(async (currentProject, sourcePath, targetPath) => {
    if (isElectron && window.fileSystemAPI) {
      try {
        const result = await window.fileSystemAPI.moveAsset(sourcePath, targetPath);
        return result;
      } catch (error) {
        console.error('Electron file system error, falling back to server:', error);
      }
    }
    
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
  }, [isElectron]);

  const deleteAsset = useCallback(async (currentProject, assetPath) => {
    if (isElectron && window.fileSystemAPI) {
      try {
        const result = await window.fileSystemAPI.deleteAsset(assetPath);
        return result;
      } catch (error) {
        console.error('Electron file system error, falling back to server:', error);
      }
    }
    
    const response = await fetch(`/api/projects/${currentProject.name}/assets/${encodeURIComponent(assetPath)}`, {
      method: 'DELETE'
    });

    if (!response.ok) {
      const errorData = await response.json();
      throw new Error(errorData.error || 'Failed to delete asset');
    }

    return await response.json();
  }, [isElectron]);

  const addFileChangeListener = useCallback((callback) => {
    if (isElectron && window.fileSystemAPI) {
      window.fileSystemAPI.onFileChanged(callback);
      return () => window.fileSystemAPI.removeFileChangeListener();
    }
    
    projectManager.addFileChangeListener(callback);
    return () => projectManager.removeFileChangeListener(callback);
  }, [isElectron]);

  return {
    isElectron,
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