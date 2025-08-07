import React, { useState, useEffect, useRef } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';
import { useSnapshot } from 'valtio';
import { globalStore, actions } from "@/store.js";
import { projectManager } from '@/plugins/projects/projectManager.js';
// Using simple on-demand loading instead of complex asset manager
import ContextMenu from '@/plugins/editor/components/ui/ContextMenu.jsx';
import ScriptCreationDialog from '@/plugins/editor/components/ui/ScriptCreationDialog.jsx';
import { useContextMenuActions } from '@/plugins/editor/components/actions/ContextMenuActions.jsx';
import { useAssetAPI } from '@/plugins/editor/hooks/useAssetAPI.js';

function AssetLibrary() {
  const [viewMode, setViewMode] = useState('folder'); // 'folder' or 'type'
  const [layoutMode, setLayoutMode] = useState('grid'); // 'grid' or 'list'
  const [currentPath, setCurrentPath] = useState('');
  const [selectedCategory, setSelectedCategory] = useState('3d-models');
  const [searchQuery, setSearchQuery] = useState('');
  const [hoveredItem, setHoveredItem] = useState(null);
  const [isResizing, setIsResizing] = useState(false);
  const [assets, setAssets] = useState([]);
  const [folderTree, setFolderTree] = useState(null);
  const [assetCategories, setAssetCategories] = useState(null);
  const [expandedFolders, setExpandedFolders] = useState(['']);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [loadedAssets, setLoadedAssets] = useState([]);
  const [preloadingAssets, setPreloadingAssets] = useState([]);
  const [failedAssets, setFailedAssets] = useState([]);
  const [showLoadingBar, setShowLoadingBar] = useState(true);
  const [isDragOver, setIsDragOver] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [contextMenu, setContextMenu] = useState(null);
  const [dragOverFolder, setDragOverFolder] = useState(null);
  const [dragOverTreeFolder, setDragOverTreeFolder] = useState(null);
  const [dragOverBreadcrumb, setDragOverBreadcrumb] = useState(null);
  const [isInternalDrag, setIsInternalDrag] = useState(false);
  const [showScriptDialog, setShowScriptDialog] = useState(false);
  const fileInputRef = useRef(null);
  const folderInputRef = useRef(null);
  
  // Using store-based cache instead of component-level cache
  
  const { ui, assets: assetCache } = useSnapshot(globalStore.editor);
  const { assetsLibraryWidth: treePanelWidth } = ui;
  const { setAssetsLibraryWidth: setTreePanelWidth } = actions.editor;
  
  // Get context menu actions
  const { handleCreateObject } = useContextMenuActions(actions.editor);

  // Asset API abstraction (Electron vs Server)
  const assetAPI = useAssetAPI();

  // Helper function to get file extension icon and color
  const getExtensionStyle = (extension) => {
    const ext = extension?.toLowerCase() || '';
    
    // 3D Models - Purple theme
    if (['.glb', '.gltf', '.obj', '.fbx'].includes(ext)) {
      return {
        icon: null, // No icon in badge for 3D models
        bgColor: 'bg-purple-600',
        hoverColor: 'hover:bg-purple-700',
        textColor: 'text-white'
      };
    }
    
    // Images/Textures - Green theme  
    if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext)) {
      return {
        icon: <Icons.Photo className="w-3 h-3" />,
        bgColor: 'bg-green-600', 
        hoverColor: 'hover:bg-green-700',
        textColor: 'text-white'
      };
    }
    
    // Audio - Orange theme
    if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) {
      return {
        icon: <Icons.Audio className="w-3 h-3" />,
        bgColor: 'bg-orange-600',
        hoverColor: 'hover:bg-orange-700', 
        textColor: 'text-white'
      };
    }
    
    // Scripts - Blue theme
    if (['.js', '.ts', '.jsx', '.tsx'].includes(ext)) {
      return {
        icon: null, // No icon in badge for scripts
        bgColor: 'bg-blue-600',
        hoverColor: 'hover:bg-blue-700',
        textColor: 'text-white'
      };
    }
    
    // Data files - Indigo theme
    if (['.json', '.xml', '.txt', '.md'].includes(ext)) {
      return {
        icon: <Icons.DocumentText className="w-3 h-3" />,
        bgColor: 'bg-indigo-600',
        hoverColor: 'hover:bg-indigo-700',
        textColor: 'text-white'
      };
    }
    
    // Default - Gray theme
    return {
      icon: <Icons.Document className="w-3 h-3" />,
      bgColor: 'bg-gray-600',
      hoverColor: 'hover:bg-gray-700',
      textColor: 'text-white'
    };
  };

  // Helper function to check if file is a script
  const isScriptFile = (extension) => {
    const ext = extension?.toLowerCase() || '';
    return ['.js', '.ts', '.jsx', '.tsx'].includes(ext);
  };

  // Helper function to check if file is a 3D model
  const is3DModelFile = (extension) => {
    const ext = extension?.toLowerCase() || '';
    return ['.glb', '.gltf', '.obj', '.fbx'].includes(ext);
  };

  // Cache management using store
  const clearCacheIfProjectChanged = (currentProject) => {
    actions.editor.setAssetsProject(currentProject.name);
  };

  // Cached folder tree fetcher
  const fetchFolderTree = async (currentProject) => {
    if (assetCache.folderTree && actions.editor.isCacheValid(assetCache.folderTreeTimestamp)) {
      // Using cached folder tree
      setFolderTree(assetCache.folderTree);
      return;
    }

    try {
      const tree = await assetAPI.fetchFolderTree(currentProject);
      actions.editor.setFolderTree(tree);
      setFolderTree(tree);
    } catch (err) {
      console.error('Error fetching folder tree:', err);
      setError(err.message);
    }
  };

  // Cached asset categories fetcher
  const fetchAssetCategories = async (currentProject) => {
    if (assetCache.categories && actions.editor.isCacheValid(assetCache.categoriesTimestamp)) {
      // Using cached asset categories
      setAssetCategories(assetCache.categories);
      
      // Set initial assets for the selected category
      const categoryAssets = assetCache.categories[selectedCategory]?.files || [];
      setAssets(categoryAssets);
      setLoading(false);
      return;
    }

    try {
      const categories = await assetAPI.fetchAssetCategories(currentProject);
      actions.editor.setAssetCategories(categories);
      setAssetCategories(categories);
      
      // Set initial assets for the selected category
      const categoryAssets = categories[selectedCategory]?.files || [];
      setAssets(categoryAssets);
      setLoading(false);
    } catch (err) {
      console.error('Error fetching asset categories:', err);
      setError(`Failed to load asset categories: ${err.message}`);
      setLoading(false);
    }
  };

  // Cached assets fetcher by path
  const fetchAssets = async (currentProject, path = currentPath) => {
    const cachedAssets = actions.editor.getAssetsForPath(path);
    if (cachedAssets) {
      // Using cached assets
      setAssets(cachedAssets);
      setLoading(false);
      return;
    }

    try {
      setLoading(true);
      setError(null);

      const newAssets = await assetAPI.fetchAssets(currentProject, path);
      
      // Cache the result in store
      actions.editor.setAssetsForPath(path, newAssets);
      
      setAssets(newAssets);
      setLoading(false);
    } catch (err) {
      console.error('Error fetching assets:', err);
      setError(err.message);
      setAssets([]);
      setLoading(false);
    }
  };

  // Fetch data based on view mode
  useEffect(() => {
    const currentProject = projectManager.getCurrentProject();
    
    if (!currentProject?.name) {
      // Set loading state and wait for project to be available
      setLoading(true);
      setError(null);
      
      // Add a loading listener to be notified when projects are loaded
      const handleProjectLoaded = ({ progress, operation, isLoading }) => {
        if (!isLoading && progress === 0) {
          // Loading finished, check if project is now available
          const newProject = projectManager.getCurrentProject();
          if (newProject?.name) {
            setError(null);
            if (viewMode === 'folder') {
              fetchFolderTree(newProject);
              fetchAssets(newProject);
            } else {
              fetchAssetCategories(newProject);
            }
          }
        }
      };
      
      projectManager.addLoadingListener(handleProjectLoaded);
      
      // Also try a simple retry mechanism as fallback
      let retryCount = 0;
      const maxRetries = 5;
      
      const retryProjectLoad = () => {
        const retryProject = projectManager.getCurrentProject();
        
        if (retryProject?.name) {
          setError(null);
          if (viewMode === 'folder') {
            fetchFolderTree(retryProject);
            fetchAssets(retryProject);
          } else {
            fetchAssetCategories(retryProject);
          }
        } else {
          retryCount++;
          if (retryCount < maxRetries) {
            setTimeout(retryProjectLoad, 500 * retryCount);
          } else {
            // Final check - if project manager says it's initialized but no project, show appropriate error
            if (projectManager.initialized) {
              setError('Project loading failed');
            } else {
              setError('Initializing project...');
            }
            setLoading(false);
          }
        }
      };
      
      // Start retry process after a brief delay
      setTimeout(retryProjectLoad, 200);
      
      // Clean up the loading listener
      return () => {
        projectManager.removeLoadingListener(handleProjectLoaded);
      };
    }

    clearCacheIfProjectChanged(currentProject);
    
    // Clear any previous errors when switching views
    setError(null);

    if (viewMode === 'folder') {
      fetchFolderTree(currentProject);
      fetchAssets(currentProject);
    } else {
      // Type view - always fetch categories first
      setLoading(true);
      fetchAssetCategories(currentProject);
    }

    // Handle real-time file changes
    const handleFileChange = (changeData) => {
      console.log('🔄 AssetLibrary: File change detected:', changeData);
      
      // Clear all asset cache when files change
      actions.editor.clearAllAssetCache();
      
      // Add a small delay to ensure file operations are complete
      setTimeout(() => {
        console.log('🔄 AssetLibrary: Refreshing asset data...');
        
        // Refresh data based on current view mode
        if (viewMode === 'folder') {
          fetchFolderTree(currentProject);
          fetchAssets(currentProject, currentPath);
        } else {
          fetchAssetCategories(currentProject);
        }
      }, 200);
    };

    // Add file change listener (uses appropriate API based on Electron/Browser)
    const removeListener = assetAPI.addFileChangeListener(handleFileChange);

    // Cleanup on unmount
    return removeListener;
  }, [currentPath, viewMode, selectedCategory]);

  // Handle currentPath changes for folder view
  useEffect(() => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject.name || viewMode !== 'folder') return;

    fetchAssets(currentProject, currentPath);
  }, [currentPath]);

  // Optimized asset loading using asset manager
  const queueAssetForLoading = (asset) => {
    // Simple on-demand loading - assets loaded by Babylon.js when needed
    // Asset available for loading
  };

  // Generate breadcrumb navigation (folder view only)
  const breadcrumbs = React.useMemo(() => {
    if (viewMode !== 'folder') return [];
    if (!currentPath) return [{ name: 'assets', path: '' }];
    
    const parts = currentPath.split('/');
    const crumbs = [{ name: 'assets', path: '' }];
    
    let currentBreadcrumbPath = '';
    for (const part of parts) {
      currentBreadcrumbPath = currentBreadcrumbPath ? `${currentBreadcrumbPath}/${part}` : part;
      crumbs.push({ name: part, path: currentBreadcrumbPath });
    }
    
    return crumbs;
  }, [currentPath, viewMode]);

  const getCategoryIcon = (categoryId) => {
    const iconMap = {
      '3d-models': Icons.Cube,
      'textures': Icons.Video,
      'audio': Icons.Audio,
      'scripts': Icons.Code,
      'data': Icons.FolderOpen,
      'misc': Icons.Folder
    };
    return iconMap[categoryId] || Icons.Folder;
  };

  // Generate asset type categories for type view
  const categoryList = React.useMemo(() => {
    if (!assetCategories) return [];
    
    return Object.entries(assetCategories).map(([id, data]) => ({
      id,
      label: data.name,
      count: data.files.length,
      icon: getCategoryIcon(id)
    }));
  }, [assetCategories]);

  // Global search across all assets
  const [globalSearchResults, setGlobalSearchResults] = useState([]);
  const [isSearching, setIsSearching] = useState(false);
  
  // Perform global search when search query changes
  useEffect(() => {
    if (!searchQuery.trim()) {
      setGlobalSearchResults([]);
      setIsSearching(false);
      return;
    }

    const performGlobalSearch = async () => {
      setIsSearching(true);
      const currentProject = projectManager.getCurrentProject();
      if (!currentProject.name) {
        setIsSearching(false);
        return;
      }

      try {
        const results = await assetAPI.searchAssets(currentProject, searchQuery);
        setGlobalSearchResults(results);
      } catch (error) {
        console.warn('Search API error, falling back to client-side search:', error);
        // Fallback: search through cached data
        performClientSideGlobalSearch();
      } finally {
        setIsSearching(false);
      }
    };

    const performClientSideGlobalSearch = () => {
      const searchResults = [];
      const searchLower = searchQuery.toLowerCase();
      
      // Search through cached assets by path
      cacheRef.current.assetsByPath.forEach((pathData, path) => {
        pathData.assets.forEach(asset => {
          if (asset.name.toLowerCase().includes(searchLower) || 
              asset.fileName?.toLowerCase().includes(searchLower)) {
            searchResults.push({
              ...asset,
              path: path ? `${path}/${asset.name}` : asset.name
            });
          }
        });
      });
      
      // Search through categories if available
      if (cacheRef.current.categories) {
        Object.values(cacheRef.current.categories).forEach(category => {
          category.files?.forEach(asset => {
            if (asset.name.toLowerCase().includes(searchLower) || 
                asset.fileName?.toLowerCase().includes(searchLower)) {
              // Avoid duplicates
              if (!searchResults.find(r => r.id === asset.id)) {
                searchResults.push(asset);
              }
            }
          });
        });
      }
      
      setGlobalSearchResults(searchResults);
    };

    // Debounce search to avoid too many requests
    const searchTimeout = setTimeout(performGlobalSearch, 300);
    return () => clearTimeout(searchTimeout);
  }, [searchQuery]);

  // Filter assets by search query - use global results if searching, otherwise current directory
  const filteredAssets = React.useMemo(() => {
    if (!searchQuery) return assets;
    
    // If we have global search results, use them
    if (globalSearchResults.length > 0) {
      return globalSearchResults;
    }
    
    // Fallback to local filtering of current directory
    return assets.filter(asset => {
      const matchesSearch = asset.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
                           asset.fileName.toLowerCase().includes(searchQuery.toLowerCase());
      return matchesSearch;
    });
  }, [assets, searchQuery, globalSearchResults]);

  // Get current category assets when in type view
  useEffect(() => {
    if (viewMode === 'type' && assetCategories) {
      const categoryAssets = assetCategories[selectedCategory]?.files || [];
      setAssets(categoryAssets);
      setLoading(false);
    }
  }, [viewMode, selectedCategory, assetCategories]);

  // Simple asset state management for UI feedback
  useEffect(() => {
    if (assets.length === 0) return;

    // Initialize with empty state for all assets - they'll be loaded on-demand
    const newLoadedAssets = [];
    const newFailedAssets = [];
    const newPreloadingAssets = [];
    
    // Update state with initial empty arrays
    setLoadedAssets(newLoadedAssets);
    setFailedAssets(newFailedAssets);
    setPreloadingAssets(newPreloadingAssets);
  }, [assets]);

  // Show loading bar only when actually uploading files
  useEffect(() => {
    setShowLoadingBar(isUploading);
  }, [isUploading]);

  // Auto-mark assets as loaded when they appear (simplified loading system)
  useEffect(() => {
    if (filteredAssets.length > 0) {
      // Mark all file assets as loaded by default (they're already available if they appear in the list)
      const newLoadedAssets = filteredAssets
        .filter(asset => asset.type === 'file')
        .map(asset => asset.id);
      
      setLoadedAssets(newLoadedAssets);
      setShowLoadingBar(false); // Hide loading bar when assets are loaded
    }
  }, [filteredAssets]);

  const handleResizeMouseDown = (e) => {
    setIsResizing(true);
    document.body.classList.add('dragging-horizontal');
    e.preventDefault();
  };

  const handleResizeMouseMove = (e) => {
    if (!isResizing) return;
    const newWidth = e.clientX;
    setTreePanelWidth(Math.max(200, Math.min(400, newWidth)));
  };

  const handleResizeMouseUp = () => {
    setIsResizing(false);
    document.body.classList.remove('dragging-horizontal');
  };

  React.useEffect(() => {
    if (isResizing) {
      document.addEventListener('mousemove', handleResizeMouseMove);
      document.addEventListener('mouseup', handleResizeMouseUp);
      return () => {
        document.removeEventListener('mousemove', handleResizeMouseMove);
        document.removeEventListener('mouseup', handleResizeMouseUp);
      };
    }
  }, [isResizing]);

  // Handle file upload
  const uploadFiles = async (files) => {
    setIsUploading(true);
    setError(null); // Clear any previous errors
    
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject.name) {
      console.error('No project loaded for file upload');
      setError('No project loaded for file upload');
      setIsUploading(false);
      return;
    }

    const uploadResults = [];

    try {
      console.log(`Starting upload of ${files.length} files...`);
      
      for (let i = 0; i < files.length; i++) {
        const file = files[i];
        console.log(`Uploading file ${i + 1}/${files.length}: ${file.name}`);
        
        const formData = new FormData();
        formData.append('file', file);
        
        // Upload to current folder path (don't force categorization)
        let targetFolderPath = currentPath; // Use current folder as default
        
        if (file.webkitRelativePath) {
          // This is a folder upload - preserve directory structure relative to current path
          const pathParts = file.webkitRelativePath.split('/');
          if (pathParts.length > 1) {
            // Remove the filename, keep the directory structure
            const relativePath = pathParts.slice(0, -1).join('/');
            targetFolderPath = currentPath ? `${currentPath}/${relativePath}` : relativePath;
          }
        }
        
        const headers = {};
        
        // Always send folder path header - empty string means root directory
        headers['X-Folder-Path'] = targetFolderPath;
        
        const response = await fetch(`/api/projects/${currentProject.name}/assets/upload`, {
          method: 'POST',
          body: formData,
          headers: headers
        });
        
        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(`Failed to upload ${file.name}: ${errorText}`);
        }
        
        const result = await response.json();
        console.log(`Successfully uploaded: ${result.filename}`);
        
        // File uploaded successfully - collect upload info for efficient refresh
        uploadResults.push({
          filename: result.filename,
          path: result.path,
          targetFolder: targetFolderPath
        });
      }
      
      console.log(`All ${files.length} files uploaded successfully. Refreshing cache...`);
      
      // Efficiently refresh only affected parts of the cache
      const affectedPaths = new Set();
      
      uploadResults.forEach(result => {
        // Files were uploaded to specific target folders
        affectedPaths.add(result.targetFolder || '');
      });
      
      // Only invalidate affected folder caches
      actions.editor.invalidateAssetPaths(Array.from(affectedPaths));
      
      // Always refresh categories cache if in type view (since files could be in any category)
      if (viewMode === 'type') {
        actions.editor.invalidateCategories();
        await fetchAssetCategories(currentProject);
      } else {
        // For folder view, refresh folder tree if new folders might have been created
        const needsFolderTreeRefresh = uploadResults.some(result => 
          result.targetFolder && result.targetFolder.includes('/') // New nested folders might have been created
        );
        
        if (needsFolderTreeRefresh) {
          actions.editor.invalidateFolderTree();
          await fetchFolderTree(currentProject);
        }
        
        // Always refresh current folder view since files were uploaded here
        await fetchAssets(currentProject, currentPath);
      }
      
      console.log('Cache refresh completed');
      
    } catch (error) {
      console.error('Upload error:', error);
      setError(`Upload failed: ${error.message}`);
    } finally {
      console.log('Upload process finished, clearing uploading state');
      setIsUploading(false);
    }
  };

  // Handle drag and drop events for external file uploads
  const handleDragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    // Only show upload overlay for external drags (not internal asset drags)
    if (!isInternalDrag && !isDragOver) {
      setIsDragOver(true);
    }
  };

  const handleDragEnter = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    // Only show upload overlay for external drags (not internal asset drags)
    if (!isInternalDrag) {
      setIsDragOver(true);
    }
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    e.stopPropagation();
    // Only hide drag over if we're leaving the asset grid area
    if (!e.currentTarget.contains(e.relatedTarget)) {
      setIsDragOver(false);
    }
  };

  const handleDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    
    // Only handle external file uploads, not internal asset drags
    if (!isInternalDrag) {
      const files = Array.from(e.dataTransfer.files);
      if (files.length > 0) {
        uploadFiles(files);
      }
    }
  };

  // Handle context menu
  const handleContextMenu = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    const contextMenuItems = [
      {
        label: 'Create Object',
        action: () => {},
        icon: <Icons.PlusCircle className="w-4 h-4" />,
        submenu: [
          { label: 'Cube', action: () => handleCreateObject('cube'), icon: <Icons.Cube className="w-4 h-4" /> },
          { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <Icons.Circle className="w-4 h-4" /> },
          { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <Icons.Rectangle className="w-4 h-4" /> },
          { label: 'Plane', action: () => handleCreateObject('plane'), icon: <Icons.Square2Stack className="w-4 h-4" /> },
          { separator: true },
          { label: 'Light', action: () => handleCreateObject('light'), icon: <Icons.LightBulb className="w-4 h-4" /> },
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <Icons.Video className="w-4 h-4" /> },
        ]
      },
      { separator: true },
      {
        label: 'Create Script',
        action: () => handleCreateScript(),
        icon: <Icons.FileText className="w-4 h-4" />
      },
      { separator: true },
      {
        label: 'Upload Files...',
        action: () => handleUploadClick(),
        icon: <Icons.Upload className="w-4 h-4" />,
        shortcut: 'Ctrl+U'
      },
      {
        label: 'Upload Folder...',
        action: () => handleUploadFolderClick(),
        icon: <Icons.FolderOpen className="w-4 h-4" />
      },
      { separator: true },
      {
        label: 'Camera',
        action: () => {},
        icon: <Icons.Video className="w-4 h-4" />,
        submenu: [
          { label: 'Frame All', action: () => handleFrameAll(), icon: <Icons.ArrowsPointingOut className="w-4 h-4" />, shortcut: 'F' },
          { label: 'Frame Selected', action: () => handleFocusSelected(), icon: <Icons.MagnifyingGlass className="w-4 h-4" />, shortcut: 'Shift+F' },
          { separator: true },
          { label: 'Reset View', action: () => handleResetView(), icon: <Icons.ArrowPath className="w-4 h-4" /> },
          { separator: true },
          { label: 'Top View', action: () => handleSetView('top'), icon: <Icons.ArrowUp className="w-4 h-4" />, shortcut: 'Numpad 7' },
          { label: 'Front View', action: () => handleSetView('front'), icon: <Icons.ArrowRight className="w-4 h-4" />, shortcut: 'Numpad 1' },
          { label: 'Right View', action: () => handleSetView('right'), icon: <Icons.ArrowDown className="w-4 h-4" />, shortcut: 'Numpad 3' },
        ]
      },
      {
        label: 'Refresh',
        action: () => window.location.reload(),
        icon: <Icons.ArrowPath className="w-4 h-4" />,
        shortcut: 'F5'
      },
      { separator: true },
      {
        label: 'New Folder',
        action: () => handleCreateFolder(),
        icon: <Icons.Folder className="w-4 h-4" />
      }
    ];
    
    setContextMenu({
      items: contextMenuItems,
      position: { x: e.clientX, y: e.clientY }
    });
  };

  // Handle file picker
  const handleUploadClick = () => {
    fileInputRef.current?.click();
  };

  const handleUploadFolderClick = () => {
    folderInputRef.current?.click();
  };

  const handleFileInputChange = (e) => {
    const files = Array.from(e.target.files);
    if (files.length > 0) {
      uploadFiles(files);
    }
    // Reset input so same file can be selected again
    e.target.value = '';
  };

  const handleFolderInputChange = (e) => {
    const files = Array.from(e.target.files);
    if (files.length > 0) {
      uploadFiles(files);
    }
    // Reset input so same folder can be selected again
    e.target.value = '';
  };

  // Close context menu when clicking elsewhere
  useEffect(() => {
    const handleClickOutside = () => {
      setContextMenu(null);
    };
    
    if (contextMenu) {
      document.addEventListener('click', handleClickOutside);
      return () => document.removeEventListener('click', handleClickOutside);
    }
  }, [contextMenu]);

  // Handle moving folders and files
  const handleMoveItem = async (sourcePath, targetFolderPath) => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject.name) {
      console.error('No project loaded for move operation');
      return;
    }

    const sourceFileName = sourcePath.split('/').pop();
    const targetPath = targetFolderPath ? `${targetFolderPath}/${sourceFileName}` : sourceFileName;

    try {
      await assetAPI.moveAsset(currentProject, sourcePath, targetPath);
      // Item moved successfully
      
      // The file watcher will automatically refresh the assets list
    } catch (error) {
      console.error('Error moving item:', error);
      setError(`Failed to move item: ${error.message}`);
    }
  };

  // Handle script creation
  const handleCreateScript = () => {
    setShowScriptDialog(true);
  };

  // Handle script creation with dialog
  const handleConfirmCreateScript = async (scriptName) => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject.name) {
      console.error('No project loaded for script creation');
      setError('No project loaded for script creation');
      return;
    }

    // Clean up the script name and add .js extension if not present
    let cleanScriptName = scriptName.trim();
    if (!cleanScriptName.endsWith('.js') && !cleanScriptName.endsWith('.ts') && 
        !cleanScriptName.endsWith('.jsx') && !cleanScriptName.endsWith('.tsx')) {
      cleanScriptName += '.js';
    }

    try {
      // Create the script content template
      const scriptContent = `// ${cleanScriptName}
// Created on ${new Date().toLocaleDateString()}

class Script {
  constructor(object) {
    this.object = object;
  }

  // Called when the script is first loaded
  init() {
    console.log('Script initialized for:', this.object.name);
  }

  // Called every frame
  update(deltaTime) {
    // Update logic here
  }

  // Called when the object is destroyed
  destroy() {
    console.log('Script destroyed for:', this.object.name);
  }
}

export default Script;
`;

      // Determine target path - use current folder in folder view, root in type view
      const targetPath = viewMode === 'folder' ? currentPath : '';
      const fullPath = targetPath ? `${targetPath}/${cleanScriptName}` : cleanScriptName;

      // Check if we're running in Electron or server environment
      if (window.electronAPI) {
        // Electron environment
        const result = await window.electronAPI.createScript({
          projectName: currentProject.name,
          scriptName: cleanScriptName,
          scriptContent: scriptContent,
          targetPath: targetPath
        });

        if (result.success) {
          console.log(`Script created successfully: ${result.filePath}`);
        } else {
          throw new Error(result.error || 'Failed to create script');
        }
      } else {
        // Server environment - use fetch API
        const response = await fetch(`/api/projects/${currentProject.name}/assets/create-script`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({
            scriptName: cleanScriptName,
            scriptContent: scriptContent,
            targetPath: targetPath
          })
        });

        if (!response.ok) {
          const errorText = await response.text();
          throw new Error(`Failed to create script: ${errorText}`);
        }

        const result = await response.json();
        console.log(`Script created successfully: ${result.filePath}`);
      }

      // Clear any previous errors
      setError(null);

      // Refresh the asset list to show the new script
      if (viewMode === 'folder') {
        // Invalidate the current folder cache and refresh
        actions.editor.invalidateAssetPaths([targetPath]);
        await fetchAssets(currentProject, currentPath);
        
        // Also refresh folder tree if we might have created a new folder
        if (targetPath.includes('/')) {
          actions.editor.invalidateFolderTree();
          await fetchFolderTree(currentProject);
        }
      } else {
        // Type view - invalidate categories and refresh
        actions.editor.invalidateCategories();
        await fetchAssetCategories(currentProject);
      }

    } catch (error) {
      console.error('Error creating script:', error);
      setError(`Failed to create script: ${error.message}`);
      throw error; // Re-throw so dialog can handle it
    }
  };

  // Handle folder creation
  const handleCreateFolder = async () => {
    const folderName = prompt('Enter folder name:');
    if (!folderName || !folderName.trim()) {
      return;
    }

    const currentProject = projectManager.getCurrentProject();
    if (!currentProject.name) {
      console.error('No project loaded for folder creation');
      return;
    }

    try {
      await assetAPI.createFolder(currentProject, folderName.trim(), viewMode === 'folder' ? currentPath : '');
      // Folder created successfully
      
      // The file watcher will automatically refresh the assets list
    } catch (error) {
      console.error('Error creating folder:', error);
      setError(`Failed to create folder: ${error.message}`);
    }
  };

  // Handle folder navigation
  const handleFolderClick = (folderPath) => {
    setCurrentPath(folderPath);
  };

  const handleFolderToggle = (folderPath) => {
    setExpandedFolders(prev => {
      if (prev.includes(folderPath)) {
        return prev.filter(path => path !== folderPath);
      } else {
        return [...prev, folderPath];
      }
    });
  };

  const handleBreadcrumbClick = (path) => {
    setCurrentPath(path);
  };

  // Handle double-click on folder items in the grid
  const handleAssetDoubleClick = (asset) => {
    if (asset.type === 'folder') {
      setCurrentPath(asset.path);
    }
  };

  // Render folder tree recursively
  const renderFolderTree = (node, depth = 0) => {
    if (!node) return null;

    const isExpanded = expandedFolders.includes(node.path);
    const isSelected = currentPath === node.path;
    const hasChildren = node.children && node.children.length > 0;
    
    return (
      <div key={node.path}>
        <div
          className={`flex items-center py-1 px-2 text-xs cursor-pointer transition-colors ${ 
            dragOverTreeFolder === node.path 
              ? 'bg-blue-600/30 border-2 border-blue-400 border-dashed rounded'
              : isSelected 
                ? 'bg-blue-600 text-white' 
                : 'text-gray-300 hover:bg-slate-700 hover:text-white'
          }`}
          style={{ paddingLeft: `${8 + depth * 12}px` }}
          onClick={() => handleFolderClick(node.path)}
          onDragOver={(e) => {
            if (isInternalDrag && viewMode === 'folder') {
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              setDragOverTreeFolder(node.path);
            }
          }}
          onDragEnter={(e) => {
            if (isInternalDrag && viewMode === 'folder') {
              e.preventDefault();
              setDragOverTreeFolder(node.path);
            }
          }}
          onDragLeave={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget)) {
              setDragOverTreeFolder(null);
            }
          }}
          onDrop={(e) => {
            e.preventDefault();
            if (isInternalDrag && viewMode === 'folder') {
              setDragOverTreeFolder(null);
              
              try {
                const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
                if (dragData.type === 'asset' && dragData.path !== node.path) {
                  // Don't allow dropping a folder into itself or its children
                  if (dragData.assetType === 'folder' && node.path.startsWith(dragData.path)) {
                    console.warn('Cannot move folder into itself or its children');
                    return;
                  }
                  handleMoveItem(dragData.path, node.path);
                }
              } catch (error) {
                console.error('Error parsing drag data in tree:', error);
              }
            }
          }}
        >
          {hasChildren && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleFolderToggle(node.path);
              }}
              className="mr-1 hover:bg-blue-500 rounded"
            >
              {isExpanded ? (
                <Icons.ChevronDown className="w-3 h-3" />
              ) : (
                <Icons.ChevronRight className="w-3 h-3" />
              )}
            </button>
          )}
          {!hasChildren && <div className="w-4 mr-1" />}
          <Icons.Folder className={`w-3 h-3 mr-2 ${
            isSelected ? 'text-white' : 'text-yellow-400'
          }`} />
          <span className="truncate">{node.name}</span>
          {node.files && node.files.length > 0 && (
            <span className={`ml-auto text-[10px] px-1.5 py-0.5 rounded-full ${
              isSelected 
                ? 'text-white bg-blue-500' 
                : 'text-gray-400 bg-slate-700'
            }`}>
              {node.files.length}
            </span>
          )}
        </div>
        
        {isExpanded && hasChildren && (
          <div>
            {node.children.map(child => renderFolderTree(child, depth + 1))}
          </div>
        )}
      </div>
    );
  };

  // Define view action handlers
  const handleFrameAll = () => {
    // TODO: Implement frame all functionality
  };

  const handleFocusSelected = () => {
    // TODO: Implement focus selected functionality
  };

  const handleResetView = () => {
    // TODO: Implement reset view functionality
  };

  const handleSetView = (view) => {
    // TODO: Implement set view functionality
  };


  return (
    <div className="h-full flex bg-slate-800 no-select">
      {/* Directory Tree Panel */}
      <div 
        className="bg-slate-900 border-r border-slate-700 flex flex-col relative"
        style={{ width: treePanelWidth }}
      >
        {/* Resize Handle */}
        <div
          className={`absolute right-0 top-0 bottom-0 w-0.5 resize-handle cursor-col-resize ${isResizing ? 'dragging' : ''}`}
          onMouseDown={handleResizeMouseDown}
        />
        {/* Fixed Header */}
        <div className="px-2 py-2 border-b border-slate-700">
          <div className="flex items-center justify-between mb-2">
            <div className="text-xs font-medium text-gray-300">Project Assets</div>
            <div className="flex items-center gap-2">
              <div className="flex bg-slate-800 rounded overflow-hidden">
                <button
                  onClick={() => setViewMode('folder')}
                  className={`px-2 py-1 text-xs transition-colors ${
                    viewMode === 'folder'
                      ? 'bg-blue-600 text-white'
                      : 'text-gray-400 hover:text-white hover:bg-slate-700'
                  }`}
                  title="Folder View"
                >
                  <Icons.Folder className="w-3 h-3" />
                </button>
                <button
                  onClick={() => setViewMode('type')}
                  className={`px-2 py-1 text-xs transition-colors ${
                    viewMode === 'type'
                      ? 'bg-blue-600 text-white'
                      : 'text-gray-400 hover:text-white hover:bg-slate-700'
                  }`}
                  title="Asset Type View"
                >
                  <Icons.Cube className="w-3 h-3" />
                </button>
              </div>
            </div>
          </div>
          <div className="relative">
            {isSearching ? (
              <div className="w-3 h-3 absolute left-2 top-1.5 animate-spin">
                <div className="w-3 h-3 border border-gray-400 border-t-blue-400 rounded-full"></div>
              </div>
            ) : (
              <Icons.MagnifyingGlass className="w-3 h-3 absolute left-2 top-1.5 text-gray-400" />
            )}
            <input
              type="text"
              placeholder={`Search all assets...`}
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full pl-6 pr-2 py-1 bg-slate-800 border border-slate-600 rounded text-xs text-white placeholder-gray-400 focus:outline-none focus:border-blue-500 transition-colors"
            />
          </div>
        </div>
        
        {/* Scrollable Content */}
        <div className="flex-1 overflow-y-auto scrollbar-thin">
          {viewMode === 'folder' ? (
            // Folder Tree View
            folderTree ? (
              <div className="py-1">
                {renderFolderTree(folderTree)}
              </div>
            ) : (
              <div className="p-4 text-center text-gray-500 text-xs">
                {error ? error : 'Loading directory tree...'}
              </div>
            )
          ) : (
            // Asset Type Categories View
            categoryList.length > 0 ? (
              <div className="space-y-0.5 p-1">
                {categoryList.map((category) => (
                  <button
                    key={category.id}
                    onClick={() => setSelectedCategory(category.id)}
                    className={`w-full flex items-center justify-between px-2 py-1.5 text-left text-xs rounded hover:bg-slate-800 transition-colors ${
                      selectedCategory === category.id 
                        ? 'bg-blue-600 text-white' 
                        : 'text-gray-300 hover:text-white'
                    }`}
                  >
                    <span className="flex items-center">
                      <category.icon className={`w-3 h-3 mr-2 ${
                        selectedCategory === category.id ? 'text-white' : 'text-gray-400'
                      }`} />
                      {category.label}
                    </span>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded-full ${
                      selectedCategory === category.id 
                        ? 'text-white bg-blue-500' 
                        : 'text-gray-400 bg-slate-700'
                    }`}>{category.count}</span>
                  </button>
                ))}
              </div>
            ) : (
              <div className="p-4 text-center text-gray-500 text-xs">
                {error ? error : 'Loading asset categories...'}
              </div>
            )
          )}
        </div>
      </div>
      
      {/* Asset Grid - More Compact */}
      <div 
        className={`flex-1 flex flex-col transition-all duration-200 relative ${
          isDragOver ? 'bg-blue-900/30 border-2 border-blue-400 border-dashed' : 'bg-slate-800'
        }`}
      >
        {/* Fixed Header with controls inline with directory tree */}
        <div className="bg-slate-800 flex-shrink-0 border-b border-slate-700">
          {/* Top row with breadcrumb and controls */}
          <div className="flex items-center justify-between px-3 py-2">
            {/* Breadcrumb Navigation - Top Left */}
            <div className="flex items-center text-xs">
              {viewMode === 'folder' && breadcrumbs.length > 0 ? (
                breadcrumbs.map((crumb, index) => (
                  <React.Fragment key={crumb.path}>
                    <button 
                      onClick={() => handleBreadcrumbClick(crumb.path)}
                      className={`px-2 py-1 rounded transition-colors ${
                        dragOverBreadcrumb === crumb.path
                          ? 'bg-blue-600/30 border border-blue-400 border-dashed text-blue-200'
                          : index === breadcrumbs.length - 1 
                            ? 'text-white font-medium hover:text-blue-400' 
                            : 'text-gray-400 hover:text-blue-400'
                      }`}
                      onDragOver={(e) => {
                        if (isInternalDrag) {
                          e.preventDefault();
                          e.dataTransfer.dropEffect = 'move';
                          setDragOverBreadcrumb(crumb.path);
                        }
                      }}
                      onDragEnter={(e) => {
                        if (isInternalDrag) {
                          e.preventDefault();
                          setDragOverBreadcrumb(crumb.path);
                        }
                      }}
                      onDragLeave={(e) => {
                        if (!e.currentTarget.contains(e.relatedTarget)) {
                          setDragOverBreadcrumb(null);
                        }
                      }}
                      onDrop={(e) => {
                        e.preventDefault();
                        if (isInternalDrag) {
                          setDragOverBreadcrumb(null);
                          
                          try {
                            const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
                            if (dragData.type === 'asset' && dragData.path !== crumb.path) {
                              // Don't allow dropping a folder into itself or its children
                              if (dragData.assetType === 'folder' && crumb.path.startsWith(dragData.path)) {
                                console.warn('Cannot move folder into itself or its children');
                                return;
                              }
                              handleMoveItem(dragData.path, crumb.path);
                            }
                          } catch (error) {
                            console.error('Error parsing drag data in breadcrumb:', error);
                          }
                        }
                      }}
                    >
                      {crumb.name}
                    </button>
                    {index < breadcrumbs.length - 1 && (
                      <Icons.ChevronRight className="w-3 h-3 mx-1 text-gray-600" />
                    )}
                  </React.Fragment>
                ))
              ) : (
                // Show category name when in type view mode
                <span className="text-gray-400 px-2 py-1">
                  {viewMode === 'type' && assetCategories && assetCategories[selectedCategory] 
                    ? assetCategories[selectedCategory].name 
                    : 'Assets'
                  }
                </span>
              )}
            </div>
            
            {/* Controls - Top Right */}
            <div className="flex items-center gap-3">
              <span className="text-xs text-gray-400">{filteredAssets.length} items</span>
              
              {isUploading && (
                /* Upload progress indicator */
                <div className="flex items-center gap-2 transition-all duration-300 opacity-100">
                  <div className="w-20 h-1.5 bg-gray-700 rounded-full overflow-hidden">
                    <div className="h-full bg-blue-500 rounded-full animate-pulse" style={{ width: '100%' }} />
                  </div>
                  <span className="text-xs text-gray-400">Uploading...</span>
                </div>
              )}
              
              {/* Grid/List Toggle */}
              <div className="flex bg-slate-700 rounded overflow-hidden">
                <button
                  onClick={() => setLayoutMode('grid')}
                  className={`px-2 py-1 text-xs transition-colors ${
                    layoutMode === 'grid'
                      ? 'bg-blue-600 text-white'
                      : 'text-gray-400 hover:text-white hover:bg-slate-600'
                  }`}
                  title="Grid View"
                >
                  <Icons.Square2Stack className="w-3 h-3" />
                </button>
                <button
                  onClick={() => setLayoutMode('list')}
                  className={`px-2 py-1 text-xs transition-colors ${
                    layoutMode === 'list'
                      ? 'bg-blue-600 text-white'
                      : 'text-gray-400 hover:text-white hover:bg-slate-600'
                  }`}
                  title="List View"
                >
                  <Icons.MenuBars className="w-3 h-3" />
                </button>
              </div>
              
              {/* Upload status indicator */}
              {isUploading ? (
                /* Uploading state */
                <div className="flex items-center gap-1.5 text-blue-400/80 bg-blue-400/10 px-2 py-1 rounded-md border border-blue-400/20">
                  <div className="w-2 h-2 bg-blue-400 rounded-full animate-spin" />
                  <span className="text-xs font-medium">Uploading...</span>
                </div>
              ) : filteredAssets.length > 0 ? (
                /* Normal state - show synced */
                <div className="flex items-center gap-1.5 text-green-400/80">
                  <div className="w-2 h-2 bg-green-400 rounded-full animate-pulse" />
                  <span className="text-xs font-medium">Synced</span>
                </div>
              ) : null}
            </div>
          </div>
        </div>
        
        {/* Scrollable Content area */}
        <div 
          className="flex-1 p-3 overflow-y-auto scrollbar-thin"
          onDragOver={handleDragOver}
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onContextMenu={handleContextMenu}
        >

        
        {loading && (
          <div className="text-center text-gray-400 mt-12">
            <p className="text-sm">Loading assets...</p>
          </div>
        )}
        
        {error && (
          <div className="text-center text-red-400 mt-12">
            <p className="text-sm">Error: {error}</p>
          </div>
        )}
        
        {isUploading && (
          <div className="text-center text-blue-400 mt-12">
            <div className="flex items-center justify-center gap-2">
              <div className="w-4 h-4 border-2 border-blue-400 border-t-transparent rounded-full animate-spin"></div>
              <p className="text-sm">Uploading files...</p>
            </div>
          </div>
        )}
        
        {isDragOver && (
          <div className="absolute inset-0 flex items-center justify-center bg-blue-900/20 backdrop-blur-sm z-10">
            <div className="text-center">
              <div className="w-16 h-16 mx-auto mb-4 border-2 border-blue-400 border-dashed rounded-lg flex items-center justify-center">
                <svg className="w-8 h-8 text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
              </div>
              <p className="text-lg font-medium text-blue-400">Drop files to upload</p>
              <p className="text-sm text-blue-300">Supports 3D models, textures, audio, and more</p>
            </div>
          </div>
        )}
        
        {!loading && !error && !isUploading && (
          <>
            {filteredAssets.length === 0 ? (
              /* Empty folder state - responsive vertical spacing */
              <div className="flex flex-col items-center justify-center min-h-[200px] h-[calc(100vh-500px)] max-h-[400px] py-4 sm:py-6 lg:py-8 text-center">
                <div className="w-16 h-16 sm:w-20 sm:h-20 mx-auto mb-4 sm:mb-6 border-2 border-gray-600 border-dashed rounded-xl flex items-center justify-center bg-gray-800/30">
                  <Icons.FolderOpen className="w-8 h-8 sm:w-10 sm:h-10 text-gray-500" />
                </div>
                
                <h3 className="text-base sm:text-lg font-medium text-gray-300 mb-2">
                  {viewMode === 'folder' 
                    ? 'Empty folder'
                    : `No ${assetCategories?.[selectedCategory]?.name?.toLowerCase() || 'assets'} found`
                  }
                </h3>
                
                <p className="text-sm text-gray-400 mb-4 sm:mb-6 max-w-sm px-4">
                  {viewMode === 'folder' 
                    ? 'This folder is empty. Add some assets to get started.'
                    : `No ${assetCategories?.[selectedCategory]?.name?.toLowerCase() || 'assets'} in this category yet.`
                  }
                </p>
                
                <div className="flex flex-col sm:flex-row gap-3 mb-3 sm:mb-4">
                  <button
                    onClick={() => fileInputRef.current?.click()}
                    className="flex items-center justify-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg transition-colors min-w-[120px]"
                  >
                    <Icons.Upload className="w-4 h-4" />
                    Upload Files
                  </button>
                  
                  <button
                    onClick={() => folderInputRef.current?.click()}
                    className="flex items-center justify-center gap-2 px-4 py-2 border border-gray-600 hover:border-gray-500 hover:bg-gray-800/50 text-gray-300 text-sm font-medium rounded-lg transition-colors min-w-[120px]"
                  >
                    <Icons.Folder className="w-4 h-4" />
                    Upload Folder
                  </button>
                </div>
                
                <p className="text-xs text-gray-500">
                  Or drag and drop files anywhere in this area
                </p>
              </div>
            ) : layoutMode === 'grid' ? (
              <div className="grid grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-10 gap-3">
                {filteredAssets.map((asset) => (
              <div
                key={asset.id}
                className={`group cursor-pointer transition-all duration-200 p-2 rounded hover:bg-slate-700/30 ${
                  dragOverFolder === asset.path ? 'bg-blue-600/30 border-2 border-blue-400 border-dashed' : ''
                }`}
                draggable={true}
                onMouseEnter={() => setHoveredItem(asset.id)}
                onMouseLeave={() => setHoveredItem(null)}
                onClick={(e) => {
                  if (asset.type === 'file') {
                    // If asset failed to load, retry on click
                    if (failedAssets.includes(asset.id)) {
                      e.preventDefault();
                      // Remove from failed set and retry with high priority
                      setFailedAssets(prev => {
                        return prev.filter(id => id !== asset.id);
                      });
                      queueAssetForLoading(asset);
                    }
                  }
                }}
                onDoubleClick={() => handleAssetDoubleClick(asset)}
                onDragStart={(e) => {
                  // Mark this as an internal drag (originating from within the page)
                  setIsInternalDrag(true);
                  
                  // Handle both files and folders
                  if (asset.type === 'file') {
                    
                    // Determine category based on file extension
                    const getAssetCategory = (extension) => {
                      const ext = extension?.toLowerCase() || '';
                      if (['.glb', '.gltf', '.obj', '.fbx'].includes(ext)) return '3d-models';
                      if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext)) return 'textures';
                      if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) return 'audio';
                      if (['.js', '.ts', '.py'].includes(ext)) return 'scripts';
                      return 'misc';
                    };
                    
                    const dragData = {
                      type: 'asset',
                      id: asset.id,
                      name: asset.name,
                      path: asset.path,
                      assetType: asset.type,
                      fileName: asset.fileName,
                      extension: asset.extension,
                      mimeType: asset.mimeType,
                      category: getAssetCategory(asset.extension)
                    };
                    
                    e.dataTransfer.setData('application/json', JSON.stringify(dragData));
                    e.dataTransfer.setData('text/plain', asset.name);
                    // Add a custom type to identify this as an asset drag (for viewport)
                    e.dataTransfer.setData('application/x-asset-drag', 'true');
                    
                    // Add download URL for dragging to desktop
                    const currentProject = projectManager.getCurrentProject();
                    const downloadUrl = `/api/projects/${currentProject.name}/assets/file/${encodeURIComponent(asset.path)}?download=true`;
                    e.dataTransfer.setData('text/uri-list', downloadUrl);
                    e.dataTransfer.setData('DownloadURL', `${asset.mimeType || 'application/octet-stream'}:${asset.name}:${downloadUrl}`);
                    
                    // Files can be copied to viewport or moved to folders
                    e.dataTransfer.effectAllowed = 'copyMove';
                    
                    // Create drag image for file
                    const dragImage = document.createElement('div');
                    const getFileIcon = (extension) => {
                      if (['.glb', '.gltf', '.obj', '.fbx'].includes(extension)) return '🧊';
                      if (['.jpg', '.jpeg', '.png', '.webp', '.bmp'].includes(extension)) return '🖼️';
                      if (['.mp3', '.wav', '.ogg', '.m4a'].includes(extension)) return '🎵';
                      if (['.js', '.ts', '.py'].includes(extension)) return '📄';
                      return '📦';
                    };
                    const icon = getFileIcon(asset.extension || '');
                    dragImage.innerHTML = `
                      <div style="
                        background: rgba(59, 130, 246, 0.9);
                        color: white;
                        padding: 8px 12px;
                        border-radius: 6px;
                        font-size: 12px;
                        font-weight: 500;
                        box-shadow: 0 4px 12px rgba(0,0,0,0.3);
                        backdrop-filter: blur(8px);
                        border: 1px solid rgba(255,255,255,0.2);
                      ">
                        ${icon} ${asset.name}
                      </div>
                    `;
                    dragImage.style.position = 'absolute';
                    dragImage.style.top = '-1000px';
                    document.body.appendChild(dragImage);
                    
                    e.dataTransfer.setDragImage(dragImage, 50, 20);
                    
                    // Clean up drag image after drag ends
                    setTimeout(() => {
                      document.body.removeChild(dragImage);
                    }, 0);
                    
                  } else if (asset.type === 'folder') {
                    
                    const dragData = {
                      type: 'asset',
                      id: asset.id,
                      name: asset.name,
                      path: asset.path,
                      assetType: asset.type
                    };
                    
                    e.dataTransfer.setData('application/json', JSON.stringify(dragData));
                    e.dataTransfer.setData('text/plain', asset.name);
                    
                    // Folders can only be moved to other folders
                    e.dataTransfer.effectAllowed = 'move';
                    
                    // Create drag image for folder
                    const dragImage = document.createElement('div');
                    dragImage.innerHTML = `
                      <div style="
                        background: rgba(251, 191, 36, 0.9);
                        color: black;
                        padding: 8px 12px;
                        border-radius: 6px;
                        font-size: 12px;
                        font-weight: 500;
                        box-shadow: 0 4px 12px rgba(0,0,0,0.3);
                        backdrop-filter: blur(8px);
                        border: 1px solid rgba(255,255,255,0.2);
                      ">
                        📁 ${asset.name}
                      </div>
                    `;
                    dragImage.style.position = 'absolute';
                    dragImage.style.top = '-1000px';
                    document.body.appendChild(dragImage);
                    
                    e.dataTransfer.setDragImage(dragImage, 50, 20);
                    
                    // Clean up drag image after drag ends
                    setTimeout(() => {
                      document.body.removeChild(dragImage);
                    }, 0);
                  }
                }}
                onDragEnd={() => {
                  // Reset all drag states when drag ends
                  setIsInternalDrag(false);
                  setDragOverFolder(null);
                  setDragOverTreeFolder(null);
                  setDragOverBreadcrumb(null);
                }}
                onDragOver={(e) => {
                  if (asset.type === 'folder' && viewMode === 'folder') {
                    e.preventDefault();
                    e.dataTransfer.dropEffect = 'move';
                    setDragOverFolder(asset.path);
                  }
                }}
                onDragLeave={(e) => {
                  if (asset.type === 'folder' && !e.currentTarget.contains(e.relatedTarget)) {
                    setDragOverFolder(null);
                  }
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  if (asset.type === 'folder' && viewMode === 'folder') {
                    setDragOverFolder(null);
                    
                    try {
                      const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
                      if (dragData.type === 'asset' && dragData.path !== asset.path) {
                        // Don't allow dropping a folder into itself or its children
                        if (dragData.assetType === 'folder' && asset.path.startsWith(dragData.path)) {
                          console.warn('Cannot move folder into itself or its children');
                          return;
                        }
                        handleMoveItem(dragData.path, asset.path);
                      }
                    } catch (error) {
                      console.error('Error parsing drag data:', error);
                    }
                  }
                }}
              >
                {/* Asset Item Container */}
                <div className="relative">
                  {/* Preview or folder icon */}
                  <div className="w-full h-16 mb-2 flex items-center justify-center relative">
                    {asset.type === 'folder' ? (
                      <Icons.Folder className="w-12 h-12 text-yellow-400 group-hover:scale-110 transition-all" />
                    ) : (
                      <div className={`w-14 h-14 bg-gray-700 rounded flex items-center justify-center transition-all group-hover:scale-110 ${
                          loadedAssets.includes(asset.id) 
                            ? 'opacity-100' 
                            : failedAssets.includes(asset.id) 
                              ? 'opacity-40 grayscale' 
                              : 'opacity-60'
                        }`}>
                        {isScriptFile(asset.extension) ? (
                          <Icons.Code className="w-8 h-8 text-blue-400" />
                        ) : is3DModelFile(asset.extension) ? (
                          <Icons.Cube className="w-8 h-8 text-purple-500" />
                        ) : (
                          <Icons.Cube className="w-8 h-8 text-gray-400" />
                        )}
                      </div>
                    )}
                    
                    {/* Extension Badge - Top right over the cube (files only) */}
                    {asset.type === 'file' && asset.extension && (() => {
                      const style = getExtensionStyle(asset.extension);
                      return (
                        <div className={`absolute top-0 right-0 ${style.bgColor} ${style.textColor} text-xs font-bold px-2 py-1 rounded-full text-center leading-none flex items-center transition-colors ${style.hoverColor} ${style.icon ? 'gap-1' : ''} shadow-sm`}>
                          {style.icon}
                          <span>{asset.extension.replace('.', '').toUpperCase()}</span>
                        </div>
                      );
                    })()}

                    {/* Loading/Status Indicator - Bottom right (files only) */}
                    {asset.type === 'file' && (
                      <div className="absolute -bottom-1 -right-1">
                        {preloadingAssets.includes(asset.id) ? (
                          // Loading spinner
                          <div className="w-6 h-6 bg-yellow-500 rounded-full flex items-center justify-center">
                            <div className="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                          </div>
                        ) : failedAssets.includes(asset.id) ? (
                          // Error cross
                          <div className="w-6 h-6 bg-red-500 rounded-full flex items-center justify-center" title={`Failed to load ${asset.name}`}>
                            <svg className="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M6 18L18 6M6 6l12 12" />
                            </svg>
                          </div>
                        ) : loadedAssets.includes(asset.id) ? (
                          // Success tick
                          <div className="w-6 h-6 bg-green-500 rounded-full flex items-center justify-center">
                            <svg className="w-3 h-3 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                            </svg>
                          </div>
                        ) : (
                          // Not loaded indicator
                          <div className="w-6 h-6 bg-gray-500 rounded-full flex items-center justify-center">
                            <div className="w-2 h-2 bg-gray-300 rounded-full"></div>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
                
                {/* Asset Name */}
                <div className="text-xs text-gray-300 group-hover:text-white transition-colors truncate text-center" title={asset.name}>
                  {asset.name}
                </div>
              </div>
                ))}
              </div>
            ) : (
              /* List View */
              <div className="space-y-0">
                {filteredAssets.map((asset, index) => (
                  <div
                    key={asset.id}
                    className={`group cursor-pointer transition-all duration-200 p-2 flex items-center gap-3 ${
                      dragOverFolder === asset.path 
                        ? 'bg-blue-600/30 border-2 border-blue-400 border-dashed rounded' 
                        : index % 2 === 0 
                          ? 'bg-slate-800/50 hover:bg-slate-700/50' 
                          : 'bg-slate-900/30 hover:bg-slate-700/50'
                    }`}
                    draggable={true}
                    onMouseEnter={() => setHoveredItem(asset.id)}
                    onMouseLeave={() => setHoveredItem(null)}
                    onClick={(e) => {
                      if (asset.type === 'file') {
                        // If asset failed to load, retry on click
                        if (failedAssets.includes(asset.id)) {
                          e.preventDefault();
                          // Remove from failed set and retry with high priority
                          setFailedAssets(prev => {
                            return prev.filter(id => id !== asset.id);
                          });
                          queueAssetForLoading(asset);
                        }
                      }
                    }}
                    onDoubleClick={() => handleAssetDoubleClick(asset)}
                    onDragStart={(e) => {
                      // Mark this as an internal drag (originating from within the page)
                      setIsInternalDrag(true);
                      
                      // Handle both files and folders
                      if (asset.type === 'file') {
                        
                        // Determine category based on file extension
                        const getAssetCategory = (extension) => {
                          const ext = extension?.toLowerCase() || '';
                          if (['.glb', '.gltf', '.obj', '.fbx'].includes(ext)) return '3d-models';
                          if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext)) return 'textures';
                          if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) return 'audio';
                          if (['.js', '.ts', '.py'].includes(ext)) return 'scripts';
                          return 'misc';
                        };
                        
                        const dragData = {
                          type: 'asset',
                          id: asset.id,
                          name: asset.name,
                          path: asset.path,
                          assetType: asset.type,
                          fileName: asset.fileName,
                          extension: asset.extension,
                          mimeType: asset.mimeType,
                          category: getAssetCategory(asset.extension)
                        };
                        
                        e.dataTransfer.setData('application/json', JSON.stringify(dragData));
                        e.dataTransfer.setData('text/plain', asset.name);
                        // Add a custom type to identify this as an asset drag (for viewport)
                        e.dataTransfer.setData('application/x-asset-drag', 'true');
                        
                        // Add download URL for dragging to desktop
                        const currentProject = projectManager.getCurrentProject();
                        const downloadUrl = `/api/projects/${currentProject.name}/assets/file/${encodeURIComponent(asset.path)}?download=true`;
                        e.dataTransfer.setData('text/uri-list', downloadUrl);
                        e.dataTransfer.setData('DownloadURL', `${asset.mimeType || 'application/octet-stream'}:${asset.name}:${downloadUrl}`);
                        
                        // Files can be copied to viewport or moved to folders
                        e.dataTransfer.effectAllowed = 'copyMove';
                        
                        // Create drag image for file
                        const dragImage = document.createElement('div');
                        const getFileIcon = (extension) => {
                          if (['.glb', '.gltf', '.obj', '.fbx'].includes(extension)) return '🧊';
                          if (['.jpg', '.jpeg', '.png', '.webp', '.bmp'].includes(extension)) return '🖼️';
                          if (['.mp3', '.wav', '.ogg', '.m4a'].includes(extension)) return '🎵';
                          if (['.js', '.ts', '.py'].includes(extension)) return '📄';
                          return '📦';
                        };
                        const icon = getFileIcon(asset.extension || '');
                        dragImage.innerHTML = `
                          <div style="
                            background: rgba(59, 130, 246, 0.9);
                            color: white;
                            padding: 8px 12px;
                            border-radius: 6px;
                            font-size: 12px;
                            font-weight: 500;
                            box-shadow: 0 4px 12px rgba(0,0,0,0.3);
                            backdrop-filter: blur(8px);
                            border: 1px solid rgba(255,255,255,0.2);
                          ">
                            ${icon} ${asset.name}
                          </div>
                        `;
                        dragImage.style.position = 'absolute';
                        dragImage.style.top = '-1000px';
                        document.body.appendChild(dragImage);
                        
                        e.dataTransfer.setDragImage(dragImage, 50, 20);
                        
                        // Clean up drag image after drag ends
                        setTimeout(() => {
                          document.body.removeChild(dragImage);
                        }, 0);
                        
                      } else if (asset.type === 'folder') {
                        
                        const dragData = {
                          type: 'asset',
                          id: asset.id,
                          name: asset.name,
                          path: asset.path,
                          assetType: asset.type
                        };
                        
                        e.dataTransfer.setData('application/json', JSON.stringify(dragData));
                        e.dataTransfer.setData('text/plain', asset.name);
                        
                        // Folders can only be moved to other folders
                        e.dataTransfer.effectAllowed = 'move';
                        
                        // Create drag image for folder
                        const dragImage = document.createElement('div');
                        dragImage.innerHTML = `
                          <div style="
                            background: rgba(251, 191, 36, 0.9);
                            color: black;
                            padding: 8px 12px;
                            border-radius: 6px;
                            font-size: 12px;
                            font-weight: 500;
                            box-shadow: 0 4px 12px rgba(0,0,0,0.3);
                            backdrop-filter: blur(8px);
                            border: 1px solid rgba(255,255,255,0.2);
                          ">
                            📁 ${asset.name}
                          </div>
                        `;
                        dragImage.style.position = 'absolute';
                        dragImage.style.top = '-1000px';
                        document.body.appendChild(dragImage);
                        
                        e.dataTransfer.setDragImage(dragImage, 50, 20);
                        
                        // Clean up drag image after drag ends
                        setTimeout(() => {
                          document.body.removeChild(dragImage);
                        }, 0);
                      }
                    }}
                    onDragEnd={() => {
                      // Reset all drag states when drag ends
                      setIsInternalDrag(false);
                      setDragOverFolder(null);
                      setDragOverTreeFolder(null);
                      setDragOverBreadcrumb(null);
                    }}
                    onDragOver={(e) => {
                      if (asset.type === 'folder' && viewMode === 'folder') {
                        e.preventDefault();
                        e.dataTransfer.dropEffect = 'move';
                        setDragOverFolder(asset.path);
                      }
                    }}
                    onDragLeave={(e) => {
                      if (asset.type === 'folder' && !e.currentTarget.contains(e.relatedTarget)) {
                        setDragOverFolder(null);
                      }
                    }}
                    onDrop={(e) => {
                      e.preventDefault();
                      if (asset.type === 'folder' && viewMode === 'folder') {
                        setDragOverFolder(null);
                        
                        try {
                          const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
                          if (dragData.type === 'asset' && dragData.path !== asset.path) {
                            // Don't allow dropping a folder into itself or its children
                            if (dragData.assetType === 'folder' && asset.path.startsWith(dragData.path)) {
                              console.warn('Cannot move folder into itself or its children');
                              return;
                            }
                            handleMoveItem(dragData.path, asset.path);
                          }
                        } catch (error) {
                          console.error('Error parsing drag data:', error);
                        }
                      }
                    }}
                  >
                    {/* Icon/Preview */}
                    <div className="w-8 h-8 flex items-center justify-center flex-shrink-0 relative">
                      {asset.type === 'folder' ? (
                        <Icons.Folder className="w-6 h-6 text-yellow-400" />
                      ) : (
                        <div className={`w-6 h-6 bg-gray-700 rounded flex items-center justify-center ${
                            loadedAssets.includes(asset.id) 
                              ? 'opacity-100' 
                              : failedAssets.includes(asset.id) 
                                ? 'opacity-40 grayscale' 
                                : 'opacity-60'
                          }`}>
                          {isScriptFile(asset.extension) ? (
                            <Icons.Code className="w-4 h-4 text-blue-400" />
                          ) : is3DModelFile(asset.extension) ? (
                            <Icons.Cube className="w-4 h-4 text-purple-500" />
                          ) : (
                            <Icons.Cube className="w-4 h-4 text-gray-400" />
                          )}
                        </div>
                      )}

                      {/* Status Indicator */}
                      {asset.type === 'file' && (
                        <div className="absolute -bottom-1 -right-1">
                          {preloadingAssets.includes(asset.id) ? (
                            <div className="w-3 h-3 bg-yellow-500 rounded-full flex items-center justify-center">
                              <div className="w-1.5 h-1.5 border border-white border-t-transparent rounded-full animate-spin"></div>
                            </div>
                          ) : failedAssets.includes(asset.id) ? (
                            <div className="w-3 h-3 bg-red-500 rounded-full flex items-center justify-center">
                              <svg className="w-2 h-2 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M6 18L18 6M6 6l12 12" />
                              </svg>
                            </div>
                          ) : loadedAssets.includes(asset.id) ? (
                            <div className="w-3 h-3 bg-green-500 rounded-full flex items-center justify-center">
                              <svg className="w-2 h-2 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
                              </svg>
                            </div>
                          ) : (
                            <div className="w-3 h-3 bg-gray-500 rounded-full flex items-center justify-center">
                              <div className="w-1 h-1 bg-gray-300 rounded-full"></div>
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                    
                    {/* Asset Details */}
                    <div className="flex-1 min-w-0">
                      <div className="text-sm text-gray-300 group-hover:text-white transition-colors truncate">
                        {asset.name}
                      </div>
                      {asset.type === 'file' && (
                        <div className="text-xs text-gray-500 truncate">
                          {asset.extension?.toUpperCase()} • {asset.size ? `${Math.round(asset.size / 1024)} KB` : 'Unknown size'}
                        </div>
                      )}
                    </div>

                    {/* Extension Badge */}
                    {asset.type === 'file' && asset.extension && (() => {
                      const style = getExtensionStyle(asset.extension);
                      return (
                        <div className="flex-shrink-0">
                          <div className={`${style.bgColor} ${style.textColor} text-xs font-bold px-2 py-1 rounded-full flex items-center transition-colors ${style.hoverColor} ${style.icon ? 'gap-1' : ''} shadow-sm`}>
                            {style.icon}
                            <span>{asset.extension.replace('.', '').toUpperCase()}</span>
                          </div>
                        </div>
                      );
                    })()}
                  </div>
                ))}
              </div>
            )}
          </>
        )}
        
        {!loading && !error && filteredAssets.length === 0 && searchQuery && (
          <div className="text-center text-gray-500 mt-12">
            <p className="text-sm">No assets found matching "{searchQuery}"</p>
            <p className="text-xs text-gray-600 mt-2">Try adjusting your search or upload new assets</p>
          </div>
        )}
        
        {/* Hidden file input for upload */}
        <input
          ref={fileInputRef}
          type="file"
          multiple
          accept=".glb,.gltf,.obj,.fbx,.jpg,.jpeg,.png,.bmp,.tga,.webp,.mp3,.wav,.ogg,.m4a,.js,.ts,.py,.json,.xml,.txt,.md"
          onChange={handleFileInputChange}
          style={{ display: 'none' }}
        />
        
        {/* Hidden folder input for upload */}
        <input
          ref={folderInputRef}
          type="file"
          webkitdirectory=""
          multiple
          onChange={handleFolderInputChange}
          style={{ display: 'none' }}
        />
        
        {/* Context Menu */}
        {contextMenu && (
          <ContextMenu
            items={contextMenu.items}
            position={contextMenu.position}
            onClose={() => setContextMenu(null)}
          />
        )}

        {/* Script Creation Dialog */}
        <ScriptCreationDialog
          isOpen={showScriptDialog}
          onClose={() => setShowScriptDialog(false)}
          onConfirm={handleConfirmCreateScript}
        />
        </div>
      </div>
    </div>
  );
}

export default AssetLibrary;