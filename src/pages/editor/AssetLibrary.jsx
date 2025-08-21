import { createSignal, createEffect, onMount, onCleanup, Show, For, createMemo, batch } from 'solid-js';
import { Photo, Wave, FileText, File, Cube, Video, Code, FolderOpen, Folder, Plus, Circle, Rectangle, Grid, Lightbulb, Upload, X, Check, Search, Menu, ChevronRight, Refresh } from '@/ui/icons';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { assetsStore, assetsActions } from '@/layout/stores/AssetStore';
import { createContextMenuActions } from '@/ui/ContextMenuActions.jsx';
import ContextMenu from '@/ui/ContextMenu.jsx';
import ScriptCreationDialog from './ScriptCreationDialog.jsx';
import { getCurrentProject, setCurrentProject, getProjects } from '@/api/bridge/projects';
import { getFileUrl, writeFile, writeBinaryFile, readFile, readBinaryFile, deleteFile, listDirectory } from '@/api/bridge/files';
import { generateThumbnail } from '@/api/bridge/thumbnails';
import { modelThumbnailGenerator } from '@/render/babylonjs/utils/modelThumbnailGenerator';
import { scriptEditorActions } from '@/layout/stores/ScriptEditorStore.js';

const getProjectManager = () => {
  return {
    getCurrentProject,
    setCurrentProject
  };
};

function AssetLibrary({ onContextMenu }) {
  const viewMode = () => assetsStore.viewMode;
  const setViewMode = (mode) => assetsActions.setViewMode(mode);
  const [layoutMode, setLayoutMode] = createSignal('grid');
  const [currentPath, setCurrentPath] = createSignal('');
  const selectedCategory = () => assetsStore.selectedCategory;
  const setSelectedCategory = (category) => assetsActions.setSelectedCategory(category);
  const [searchQuery, setSearchQuery] = createSignal('');
  const [hoveredItem, setHoveredItem] = createSignal(null);
  const [isResizing, setIsResizing] = createSignal(false);
  const [assets, setAssets] = createSignal([]);
  const [folderTree, setFolderTree] = createSignal(null);
  const [currentProject, setCurrentProject] = createSignal(null);
  const [assetCategories, setAssetCategories] = createSignal(null);
  const expandedFolders = () => assetsStore.expandedFolders;
  const toggleFolderExpansion = (folderPath) => assetsActions.toggleFolderExpansion(folderPath);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal(null);
  const [loadedAssets, setLoadedAssets] = createSignal([]);
  const [preloadingAssets, setPreloadingAssets] = createSignal([]);
  const [failedAssets, setFailedAssets] = createSignal([]);
  const [showLoadingBar, setShowLoadingBar] = createSignal(false);
  const [isDragOver, setIsDragOver] = createSignal(false);
  const [isUploading, setIsUploading] = createSignal(false);
  const [contextMenu, setContextMenu] = createSignal(null);
  const [dragOverFolder, setDragOverFolder] = createSignal(null);
  const [dragOverTreeFolder, setDragOverTreeFolder] = createSignal(null);
  const [dragOverBreadcrumb, setDragOverBreadcrumb] = createSignal(null);
  const [isInternalDrag, setIsInternalDrag] = createSignal(false);
  const [showScriptDialog, setShowScriptDialog] = createSignal(false);
  const [selectedAssets, setSelectedAssets] = createSignal(new Set());
  const [lastSelectedAsset, setLastSelectedAsset] = createSignal(null);
  const [isSelecting, setIsSelecting] = createSignal(false);
  const [selectionStart, setSelectionStart] = createSignal(null);
  const [selectionEnd, setSelectionEnd] = createSignal(null);
  const [selectionRect, setSelectionRect] = createSignal(null);
  const [globalSearchResults, setGlobalSearchResults] = createSignal([]);
  const [isSearching, setIsSearching] = createSignal(false);
  
  let fileInputRef;
  let assetGridRef;

  const toggleAssetSelection = (asset, ctrlKey = false, shiftKey = false) => {
    const currentSelected = selectedAssets();
    const newSelected = new Set(currentSelected);
    
    if (shiftKey && lastSelectedAsset()) {
      const currentAssets = filteredAssets();
      const lastIndex = currentAssets.findIndex(a => a.id === lastSelectedAsset().id);
      const currentIndex = currentAssets.findIndex(a => a.id === asset.id);
      
      if (lastIndex !== -1 && currentIndex !== -1) {
        const start = Math.min(lastIndex, currentIndex);
        const end = Math.max(lastIndex, currentIndex);
        
        for (let i = start; i <= end; i++) {
          newSelected.add(currentAssets[i].id);
        }
      }
    } else if (ctrlKey) {
      if (newSelected.has(asset.id)) {
        newSelected.delete(asset.id);
      } else {
        newSelected.add(asset.id);
        setLastSelectedAsset(asset);
      }
    } else {
      newSelected.clear();
      newSelected.add(asset.id);
      setLastSelectedAsset(asset);
    }
    
    setSelectedAssets(newSelected);
  };

  const clearSelection = () => {
    setSelectedAssets(new Set());
    setLastSelectedAsset(null);
  };

  const isAssetSelected = (assetId) => {
    return selectedAssets().has(assetId);
  };

  const startDragSelection = (e) => {
    const target = e.target;
    
    const isInteractiveElement = target.closest('button, input, a, select, textarea');
    if (isInteractiveElement) {
      return;
    }
    
    const isAssetElement = target.closest('[data-asset-id]');
    if (isAssetElement) {
      return;
    }
    
    const isDraggableElement = target.closest('[draggable="true"]');
    if (isDraggableElement) {
      return;
    }
    
    const rect = e.currentTarget.getBoundingClientRect();
    const startPos = {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top
    };
    
    setSelectionStart(startPos);
    setSelectionEnd(startPos);
    
    if (!e.ctrlKey && !e.metaKey) {
      clearSelection();
    }
  };

  let mainContentRef;

  const updateDragSelection = (e) => {
    if (!isSelecting() || !mainContentRef) return;
    
    const containerRect = mainContentRef.getBoundingClientRect();
    let currentPos = {
      x: e.clientX - containerRect.left,
      y: e.clientY - containerRect.top
    };
    
    const containerPadding = 12;
    currentPos.x = Math.max(0, Math.min(currentPos.x, containerRect.width));
    currentPos.y = Math.max(0, Math.min(currentPos.y, containerRect.height));
  
    const scrollTop = mainContentRef.scrollTop;
    const scrollHeight = mainContentRef.scrollHeight;
    const clientHeight = mainContentRef.clientHeight;
    const maxContentY = scrollHeight - containerPadding;
    const adjustedY = currentPos.y + scrollTop;
    const constrainedY = Math.max(0, Math.min(adjustedY, maxContentY));
  
    currentPos.y = constrainedY - scrollTop;
    
    setSelectionEnd(currentPos);
    
    const start = selectionStart();
    let selectionBox = {
      x: Math.min(start.x, currentPos.x),
      y: Math.min(start.y, currentPos.y),
      width: Math.abs(currentPos.x - start.x),
      height: Math.abs(currentPos.y - start.y)
    };
    
    selectionBox.x = Math.max(0, selectionBox.x);
    selectionBox.y = Math.max(0, selectionBox.y);
    selectionBox.width = Math.min(selectionBox.width, containerRect.width - selectionBox.x);
    selectionBox.height = Math.min(selectionBox.height, containerRect.height - selectionBox.y);
    
    setSelectionRect(selectionBox);
    
    const assetElements = mainContentRef.querySelectorAll('[data-asset-id]');
    const newSelected = new Set(e.ctrlKey || e.metaKey ? selectedAssets() : []);
    
    assetElements?.forEach(element => {
      const elementRect = element.getBoundingClientRect();
      
      const relativeRect = {
        x: elementRect.left - containerRect.left,
        y: elementRect.top - containerRect.top,
        width: elementRect.width,
        height: elementRect.height
      };
      
      if (selectionBox.x < relativeRect.x + relativeRect.width &&
          selectionBox.x + selectionBox.width > relativeRect.x &&
          selectionBox.y < relativeRect.y + relativeRect.height &&
          selectionBox.y + selectionBox.height > relativeRect.y) {
        
        const assetId = element.getAttribute('data-asset-id');
        newSelected.add(assetId);
      }
    });
    
    setSelectedAssets(newSelected);
  };

  const endDragSelection = () => {
    setIsSelecting(false);
    setSelectionRect(null);
    setSelectionStart(null);
  };

  const handleKeyDown = (e) => {
    if (e.key === 'Escape') {
      clearSelection();
      return;
    }
    
    if (e.key === 'a' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      const allAssets = filteredAssets();
      const allAssetIds = new Set(allAssets.map(a => a.id));
      setSelectedAssets(allAssetIds);
      if (allAssets.length > 0) {
        setLastSelectedAsset(allAssets[allAssets.length - 1]);
      }
      return;
    }
    
    if (e.key === 'Delete' && selectedAssets().size > 0) {
      console.log('Delete key pressed with', selectedAssets().size, 'selected assets');
    }
  };

  const handleGlobalMouseMove = (e) => {
    if (selectionStart() && !isSelecting()) {
      const startPos = selectionStart();
      const deltaX = Math.abs(e.clientX - (startPos.x + (mainContentRef?.getBoundingClientRect().left || 0)));
      const deltaY = Math.abs(e.clientY - (startPos.y + (mainContentRef?.getBoundingClientRect().top || 0)));
      
      if (deltaX > 3 || deltaY > 3) {
        setIsSelecting(true);
        e.preventDefault();
      }
    }
    
    updateDragSelection(e);
  };

  const handleGlobalMouseUp = (e) => {
    if (isSelecting()) {
      endDragSelection();
    } else if (selectionStart()) {
      setSelectionStart(null);
    }
  };

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('mousemove', handleGlobalMouseMove);
    document.addEventListener('mouseup', handleGlobalMouseUp);
  });

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown);
    document.removeEventListener('mousemove', handleGlobalMouseMove);
    document.removeEventListener('mouseup', handleGlobalMouseUp);
  });
  let folderInputRef;
  
  const ui = () => editorStore.ui;
  const assetCache = () => assetsStore;
  const treePanelWidth = () => ui().assetsLibraryWidth && ui().assetsLibraryWidth > 150 ? ui().assetsLibraryWidth : 150;
  const { setAssetsLibraryWidth: setTreePanelWidth } = editorActions;
  const contextMenuActions = createContextMenuActions(editorActions);
  const { handleCreateObject } = contextMenuActions;
  
  // Bridge API functions directly
  const isInitialized = () => true; // Always initialized since we have direct bridge API
  
  const buildTreeFromAssets = (assets, projectName = null) => {
    if (!assets || !Array.isArray(assets)) {
      console.warn('buildTreeFromAssets: Invalid assets data:', assets);
      return null;
    }

    console.log('🦀 buildTreeFromAssets input:', assets, 'projectName:', projectName);
    const folders = assets.filter(asset => asset.is_directory);
    const files = assets.filter(asset => !asset.is_directory);
    console.log('🦀 Filtered folders:', folders);
    console.log('🦀 Filtered files:', files);

    let processedFolders = folders;
    let processedFiles = files;
    
    if (projectName) {
      processedFolders = folders.map(folder => {
        let adjustedPath = folder.path;
        const projectPrefix1 = `projects/${projectName}/`;
        const projectPrefix2 = `projects\\\\${projectName}\\\\`;
        const projectRoot1 = `projects/${projectName}`;
        const projectRoot2 = `projects\\\\${projectName}`;
        
        if (adjustedPath.startsWith(projectPrefix1)) {
          adjustedPath = adjustedPath.replace(projectPrefix1, '');
        } else if (adjustedPath.startsWith(projectPrefix2)) {
          adjustedPath = adjustedPath.replace(projectPrefix2, '');
        } else if (adjustedPath === projectRoot1 || adjustedPath === projectRoot2) {
          return null;
        }
        
        adjustedPath = adjustedPath.replace(/\\\\/g, '/');
        
        return {
          ...folder,
          path: adjustedPath
        };
      }).filter(Boolean);

      processedFiles = files.map(file => {
        let adjustedPath = file.path;
        const projectPrefix1 = `projects/${projectName}/`;
        const projectPrefix2 = `projects\\\\${projectName}\\\\`;
        
        if (adjustedPath.startsWith(projectPrefix1)) {
          adjustedPath = adjustedPath.replace(projectPrefix1, '');
        } else if (adjustedPath.startsWith(projectPrefix2)) {
          adjustedPath = adjustedPath.replace(projectPrefix2, '');
        }
        
        adjustedPath = adjustedPath.replace(/\\\\/g, '/');
        
        return {
          ...file,
          path: adjustedPath
        };
      });
    }

    const buildTree = (parentPath = '') => {
      console.log('🦀 buildTree called with parentPath:', parentPath);
      console.log('🦀 processedFolders:', processedFolders);
      
      const rootFolders = processedFolders.filter(folder => {
        const folderDepth = folder.path.split('/').length;
        const parentDepth = parentPath ? parentPath.split('/').length : 0;
        
        console.log('🦀 Checking folder:', folder.path, 'depth:', folderDepth, 'parentDepth:', parentDepth);
        
        if (parentPath) {
          const result = folder.path.startsWith(parentPath + '/') && folderDepth === parentDepth + 1;
          console.log('🦀 Parent path filter result:', result);
          return result;
        } else {
          const result = folderDepth === 1 || !folder.path.includes('/');
          console.log('🦀 Root filter result:', result, 'folderDepth === 1:', folderDepth === 1, '!includes(/):', !folder.path.includes('/'));
          return result;
        }
      });
      
      console.log('🦀 rootFolders found:', rootFolders);

      return rootFolders.map(folder => {
        const folderFiles = processedFiles.filter(file => {
          const filePath = file.path.substring(0, file.path.lastIndexOf('/')) || '';
          return filePath === folder.path;
        });

        const children = buildTree(folder.path);

        return {
          name: folder.name,
          path: folder.path,
          type: 'folder',
          children: children,
          files: folderFiles
        };
      });
    };

    const finalTree = buildTree();
    console.log('🦀 Final tree result:', finalTree);
    
    // Fallback: if buildTree returns empty, create a simple flat tree
    if (!finalTree || finalTree.length === 0) {
      console.log('🦀 BuildTree returned empty, creating simple flat tree');
      console.log('🦀 processedFolders:', processedFolders);
      const simpleTree = processedFolders.map(folder => ({
        name: folder.name,
        path: folder.path,
        type: 'folder',
        children: [],
        files: []
      }));
      console.log('🦀 Simple tree created:', simpleTree);
      return simpleTree;
    }
    
    return finalTree;
  };

  const fetchFolderTree = async (currentProject) => {
    console.log('🦀 Using bridge API for folder tree, project:', currentProject.name);
    
    try {
      const projects = await getProjects();
      console.log('🦀 All projects from bridge:', projects);
      const currentProjectData = projects.find(p => p.name === currentProject.name);
      console.log('🦀 Current project data:', currentProjectData);
      
      if (currentProjectData && currentProjectData.files && currentProjectData.files.length > 0) {
        const tree = buildTreeFromAssets(currentProjectData.files, currentProject.name);
        console.log('🦀 Built tree from project files:', tree);
        return tree;
      }
      
      console.log('🦀 Falling back to listing project assets directory directly');
      const projectFiles = await listDirectory(`projects/${currentProject.name}`);
      console.log('🦀 Project files from direct listing:', projectFiles);
      
      // Build nested tree structure recursively
      const buildNestedTree = async (items, basePath = '') => {
        const tree = [];
        
        for (const item of items.filter(i => i.is_directory)) {
          const folderPath = basePath ? `${basePath}/${item.path}` : item.path;
          
          try {
            // Get contents of this folder
            const subItems = await listDirectory(`projects/${currentProject.name}/${item.path}`);
            const children = await buildNestedTree(subItems, basePath);
            const files = subItems.filter(subItem => !subItem.is_directory);
            
            tree.push({
              name: item.name,
              path: item.path,
              type: 'folder',
              children: children,
              files: files
            });
          } catch (err) {
            console.warn('🦀 Could not read folder:', item.path, err);
            // Add folder without children if we can't read it
            tree.push({
              name: item.name,
              path: item.path,
              type: 'folder',
              children: [],
              files: []
            });
          }
        }
        
        return tree;
      };
      
      const nestedTree = await buildNestedTree(projectFiles);
      console.log('🦀 Created nested tree:', nestedTree);
      return nestedTree;
      
    } catch (error) {
      console.error('🦀 Bridge API failed:', error);
      return [];
    }
  };

  const fetchAssetCategories = async (currentProject) => {
    console.log('🦀 Generating asset categories from bridge data');
    
    try {
      const allAssets = await listDirectory(`projects/${currentProject.name}`);
      console.log('🦀 RAW ASSETS FROM BRIDGE:', allAssets);
      console.log('🦀 TOTAL ASSET COUNT:', allAssets.length);
      
      const categories = {
        '3d-models': {
          name: '3D Models',
          extensions: ['.glb', '.gltf', '.obj', '.fbx', '.dae'],
          assets: []
        },
        'textures': {
          name: 'Textures',
          extensions: ['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga', '.hdr', '.exr'],
          assets: []
        },
        'audio': {
          name: 'Audio',
          extensions: ['.mp3', '.wav', '.ogg', '.flac', '.aac'],
          assets: []
        },
        'video': {
          name: 'Video',
          extensions: ['.mp4', '.webm', '.avi', '.mov', '.mkv'],
          assets: []
        },
        'scripts': {
          name: 'Scripts',
          extensions: ['.js', '.ts', '.jsx', '.tsx', '.json'],
          assets: []
        },
        'documents': {
          name: 'Documents',
          extensions: ['.txt', '.md', '.pdf', '.doc', '.docx'],
          assets: []
        },
        'other': {
          name: 'Other',
          extensions: [],
          assets: []
        }
      };
      
      allAssets.forEach(asset => {
        console.log('🦀 PROCESSING ASSET:', asset.name, 'is_directory:', asset.is_directory);
        
        if (asset.is_directory) {
          console.log('🦀 SKIPPING FOLDER:', asset.name);
          return;
        }
        
        const extension = asset.name.toLowerCase().match(/\.[^.]+$/)?.[0] || '';
        console.log('🦀 FILE EXTENSION:', extension, 'for', asset.name);
        let categorized = false;
        
        for (const [key, category] of Object.entries(categories)) {
          if (key === 'other') continue;
          
          if (category.extensions.includes(extension)) {
            console.log('🦀 CATEGORIZING FILE TO:', key, asset.name);
            category.assets.push({
              id: asset.path,
              name: asset.name,
              path: asset.path,
              type: 'file',
              extension,
              size: asset.size || 0
            });
            categorized = true;
            break;
          }
        }
        
        if (!categorized) {
          console.log('🦀 ADDING FILE TO OTHER:', asset.name);
          categories.other.assets.push({
            id: asset.path,
            name: asset.name,
            path: asset.path,
            type: 'file',
            extension,
            size: asset.size || 0
          });
        }
      });
      
      console.log('🦀 Generated categories:', Object.keys(categories).map(key => 
        `${key}: ${categories[key].assets.length} assets`
      ));
      
      return categories;
      
    } catch (error) {
      console.warn('🦀 Bridge failed for categories, using fallback:', error);
      
      return {
        '3d-models': { name: '3D Models', extensions: ['.glb', '.gltf', '.obj'], assets: [] },
        'textures': { name: 'Textures', extensions: ['.jpg', '.png', '.webp'], assets: [] },
        'audio': { name: 'Audio', extensions: ['.mp3', '.wav', '.ogg'], assets: [] },
        'scripts': { name: 'Scripts', extensions: ['.js', '.ts', '.json'], assets: [] },
        'other': { name: 'Other', extensions: [], assets: [] }
      };
    }
  };

  const fetchAssets = async (currentProject, path = '') => {
    console.log('🦀 Using bridge API for assets in path:', path);
    
    try {
      const dirPath = path 
        ? `projects/${currentProject.name}/${path}` 
        : `projects/${currentProject.name}`;
      
      console.log('🦀 Requesting directory:', dirPath);
      const rawAssets = await listDirectory(dirPath);
      console.log('🦀 Got raw assets from bridge API:', rawAssets.length, rawAssets);
      
      const assets = rawAssets.map(asset => {
        console.log(`🦀 Processing asset: ${asset.name}, is_directory: ${asset.is_directory}, type: ${typeof asset.is_directory}`);
        const hasExtension = asset.name.includes('.') && !asset.is_directory;
        const convertedAsset = {
          id: asset.path,
          name: asset.name,
          path: path ? `${path}/${asset.name}` : asset.name,
          type: asset.is_directory ? 'folder' : 'file',
          extension: hasExtension ? '.' + asset.name.split('.').pop() : null,
          size: asset.size || 0,
          fileName: asset.name
        };
        console.log(`🦀 Converted to: type=${convertedAsset.type}`);
        return convertedAsset;
      });
      
      console.log('🦀 Converted assets:', assets);
      return assets;
      
    } catch (error) {
      console.error('🦀 Bridge API failed:', error);
      return [];
    }
  };

  const searchAssets = async (currentProject, query) => {
    console.log('🦀 Using bridge API for asset search');
    
    try {
      const allAssets = await listDirectory(`projects/${currentProject.name}`);
      const searchLower = query.toLowerCase();
      
      const results = allAssets.filter(asset => 
        asset.name.toLowerCase().includes(searchLower)
      );
      
      console.log('🦀 Found', results.length, 'assets matching search');
      return results;
      
    } catch (error) {
      console.error('🦀 Bridge API search failed:', error);
      return [];
    }
  };

  const createFolder = async (currentProject, folderName, parentPath = '') => {
    try {
      const folderPath = parentPath 
        ? `projects/${currentProject.name}/assets/${parentPath}/${folderName.trim()}`
        : `projects/${currentProject.name}/assets/${folderName.trim()}`;
      
      await writeFile(`${folderPath}/.gitkeep`, '');
      console.log('🦀 Created folder:', folderPath);
      return { success: true, path: folderPath };
    } catch (error) {
      throw new Error(`Failed to create folder: ${error.message}`);
    }
  };

  const isBinaryFile = (fileName) => {
    const extension = fileName.toLowerCase().match(/\.[^.]+$/)?.[0] || '';
    const binaryExtensions = [
      '.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.tga', '.tiff', '.ico', '.svg',
      '.mp3', '.wav', '.ogg', '.m4a', '.aac', '.flac',
      '.mp4', '.avi', '.mov', '.mkv', '.webm', '.wmv',
      '.glb', '.gltf', '.obj', '.fbx', '.dae', '.3ds', '.blend', '.max', '.ma', '.mb', '.stl', '.ply', '.x3d',
      '.zip', '.rar', '.7z', '.tar', '.gz',
      '.pdf', '.doc', '.docx', '.xls', '.xlsx', '.ppt', '.pptx',
      '.exe', '.dll', '.so', '.dylib'
    ];
    return binaryExtensions.includes(extension);
  };

  const moveAsset = async (currentProject, sourcePath, targetPath) => {
    try {
      const fullSourcePath = `projects/${currentProject.name}/assets/${sourcePath}`;
      const fullTargetPath = `projects/${currentProject.name}/assets/${targetPath}`;
      
      console.log('🦀 Moving asset:', sourcePath, '->', targetPath);
      console.log('🦀 Full source path:', fullSourcePath);
      console.log('🦀 Full target path:', fullTargetPath);
      
      const sourceFileName = sourcePath.split('/').pop();
      
      if (isBinaryFile(sourceFileName)) {
        console.log('🦀 Moving binary file:', sourceFileName);
        const base64Content = await readBinaryFile(fullSourcePath);
        await writeBinaryFile(fullTargetPath, base64Content);
      } else {
        console.log('🦀 Moving text file:', sourceFileName);
        const content = await readFile(fullSourcePath);
        await writeFile(fullTargetPath, content);
      }
      
      await deleteFile(fullSourcePath);
      console.log('🦀 Successfully moved asset:', sourcePath, '->', targetPath);
      return { success: true, sourcePath, targetPath };
    } catch (error) {
      throw new Error(`Failed to move item: ${error.message}`);
    }
  };

  const deleteAsset = async (currentProject, assetPath) => {
    try {
      const fullAssetPath = `projects/${currentProject.name}/assets/${assetPath}`;
      await deleteFile(fullAssetPath);
      console.log('🦀 Deleted asset:', assetPath);
      return { success: true, path: assetPath };
    } catch (error) {
      throw new Error(`Failed to delete asset: ${error.message}`);
    }
  };

  const addFileChangeListener = (callback) => {
    const handleProjectSelect = (event) => callback(event.detail);
    document.addEventListener('engine:project-selected', handleProjectSelect);
    return () => document.removeEventListener('engine:project-selected', handleProjectSelect);
  };
  
  const projectManager = getProjectManager();

  const getExtensionStyle = (extension) => {
    const ext = extension?.toLowerCase() || '';
    
    if (['.glb', '.gltf', '.obj', '.fbx'].includes(ext)) {
      return {
        icon: null,
        bgColor: 'bg-secondary',
        hoverColor: 'hover:bg-secondary/80',
        textColor: 'text-white'
      };
    }
    
    if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext)) {
      return {
        icon: <Photo class="w-3 h-3" />,
        bgColor: 'bg-success', 
        hoverColor: 'hover:bg-success/80',
        textColor: 'text-white'
      };
    }
    
    if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) {
      return {
        icon: <Wave class="w-3 h-3" />,
        bgColor: 'bg-warning',
        hoverColor: 'hover:bg-warning/80', 
        textColor: 'text-white'
      };
    }
    
    if (['.js', '.ts', '.jsx', '.tsx'].includes(ext)) {
      return {
        icon: null,
        bgColor: 'bg-primary',
        hoverColor: 'hover:bg-primary/80',
        textColor: 'text-white'
      };
    }
    
    if (['.json', '.xml', '.txt', '.md'].includes(ext)) {
      return {
        icon: <FileText class="w-3 h-3" />,
        bgColor: 'bg-info',
        hoverColor: 'hover:bg-info/80',
        textColor: 'text-white'
      };
    }
    
    return {
      icon: <File class="w-3 h-3" />,
      bgColor: 'bg-neutral',
      hoverColor: 'hover:bg-neutral/80',
      textColor: 'text-white'
    };
  };

  const isScriptFile = (extension) => {
    const ext = extension?.toLowerCase() || '';
    return ['.js', '.ts', '.jsx', '.tsx'].includes(ext);
  };

  const is3DModelFile = (extension) => {
    const ext = extension?.toLowerCase() || '';
    return ['.glb', '.gltf', '.obj', '.fbx'].includes(ext);
  };

  const isImageFile = (extension) => {
    const ext = extension?.toLowerCase() || '';
    return ['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext);
  };

  const getAssetThumbnailUrl = (asset) => {
    if (!asset || asset.type !== 'file' || !isImageFile(asset.extension)) {
      return null;
    }
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) return null;
    
    const assetPath = asset.path || asset.name;
    return getFileUrl(`projects/${currentProject.name}/assets/${assetPath}`);
  };

  const ModelThumbnail = ({ asset, size = 'w-full h-full' }) => {
    const [thumbnailUrl, setThumbnailUrl] = createSignal(null);
    const [isLoading, setIsLoading] = createSignal(true);
    const [error, setError] = createSignal(false);

    createEffect(async () => {
      const currentProject = projectManager.getCurrentProject();
      if (!currentProject?.name || !is3DModelFile(asset.extension)) return;

      try {
        setIsLoading(true);
        
        const assetPath = asset.name || asset.path;
        console.log(`🎯 Requesting thumbnail for: assets/${assetPath} (original path: ${asset.path})`);
        
        const result = await generateThumbnail(assetPath, 512);
        
        if (result.success && result.thumbnail_data) {
          setThumbnailUrl(result.thumbnail_data);
          setError(false);
          console.log(`Thumbnail ${result.cached ? 'loaded from cache' : 'generated'} for ${asset.name}`);
        } else {
          throw new Error(result.error || 'Failed to generate thumbnail');
        }
      } catch (err) {
        console.error('Failed to get model thumbnail:', err);
        setError(true);
      } finally {
        setIsLoading(false);
      }
    });

    return (
      <div class={`${size} bg-base-300 rounded overflow-hidden transition-all group-hover:scale-110 relative`}>
        <Show when={!error()} fallback={
          <div class="w-full h-full flex items-center justify-center">
            <Cube class="w-8 h-8 text-secondary" />
          </div>
        }>
          <Show when={thumbnailUrl()} fallback={
            <div class="w-full h-full flex items-center justify-center">
              <Show when={isLoading()} fallback={
                <Cube class="w-8 h-8 text-secondary" />
              }>
                <div class="w-4 h-4 border-2 border-secondary border-t-transparent rounded-full animate-spin"></div>
              </Show>
            </div>
          }>
            <img 
              src={thumbnailUrl()}
              alt={asset.name}
              class="w-full h-full object-cover"
            />
          </Show>
        </Show>
      </div>
    );
  };

  const ImageThumbnail = ({ asset, size = 'w-full h-full' }) => {
    const [imageLoaded, setImageLoaded] = createSignal(false);
    const [imageError, setImageError] = createSignal(false);
    const thumbnailUrl = getAssetThumbnailUrl(asset);
    
    if (!thumbnailUrl) {
      return (
        <div class={`${size} bg-base-300 rounded flex items-center justify-center transition-all group-hover:scale-110`}>
          <Photo class="w-6 h-6 text-success" />
        </div>
      );
    }
    
    return (
      <div class={`${size} bg-base-300 rounded overflow-hidden transition-all group-hover:scale-110 relative`}>
        <Show when={!imageError()} fallback={
          <div class="w-full h-full flex items-center justify-center">
            <Photo class="w-6 h-6 text-success" />
          </div>
        }>
          <img 
            src={thumbnailUrl}
            alt={asset.name}
            class={`w-full h-full object-cover transition-opacity duration-200 ${
              imageLoaded() ? 'opacity-100' : 'opacity-0'
            }`}
            onLoad={() => setImageLoaded(true)}
            onError={() => {
              setImageError(true);
              setImageLoaded(false);
            }}
          />
          <Show when={!imageLoaded() && !imageError()}>
            <div class="absolute inset-0 flex items-center justify-center bg-base-300">
              <div class="w-3 h-3 border border-base-content/40 border-t-primary rounded-full animate-spin"></div>
            </div>
          </Show>
        </Show>
      </div>
    );
  };

  const clearCacheIfProjectChanged = (currentProject) => {
    if (currentProject?.name) {
      assetsActions.setAssetsProject(currentProject.name);
    }
  };

  const fetchFolderTreeWithCache = async (currentProject) => {
    const cache = assetCache();
    if (cache.folderTree && assetsActions.isCacheValid(cache.folderTreeTimestamp)) {
      setFolderTree(cache.folderTree);
      return;
    }

    try {
      const tree = await fetchFolderTree(currentProject);
      console.log('🦀 Setting folder tree:', tree);
      assetsActions.setFolderTree(tree);
      setFolderTree(tree);
      console.log('🦀 folderTree signal is now:', folderTree());
    } catch (err) {
      console.error('Error fetching folder tree:', err);
      setError(err.message);
    }
  };

  const ensureProjectLoaded = async () => {
    let project = projectManager.getCurrentProject();
    if (!project) {
      try {
        const allProjects = await getProjects();
        if (allProjects.length > 0) {
          const preferredProject = allProjects[0];
          const projectData = {
            name: preferredProject.name,
            path: preferredProject.path || `projects/${preferredProject.name}`,
            loaded: new Date()
          };
          setCurrentProject(projectData);
          project = projectData;
          console.log('🦀 Auto-loaded project:', project);
        }
      } catch (err) {
        console.warn('Failed to auto-load project:', err);
      }
    }
    setCurrentProject(project);
    return project;
  };

  const fetchAssetCategoriesWithCache = async (currentProject) => {
    const cache = assetCache();
    if (cache.categories && assetsActions.isCacheValid(cache.categoriesTimestamp)) {
      setAssetCategories(cache.categories);
      const categoryAssets = cache.categories[selectedCategory()]?.assets || cache.categories[selectedCategory()]?.files || [];
      setAssets(categoryAssets);
      setLoading(false);
      return;
    }

    try {
      const categories = await fetchAssetCategories(currentProject);
      assetsActions.setAssetCategories(categories);
      setAssetCategories(categories);
      const categoryAssets = categories[selectedCategory()]?.assets || categories[selectedCategory()]?.files || [];
      setAssets(categoryAssets);
      setLoading(false);
    } catch (err) {
      console.error('Error fetching asset categories:', err);
      setError(`Failed to load asset categories: ${err.message}`);
      setLoading(false);
    }
  };

  const fetchAssetsWithCache = async (currentProject, path = '') => {
    const cachedAssets = assetsActions.getAssetsForPath(path || currentPath());
    if (cachedAssets) {
      setAssets(cachedAssets);
      setLoading(false);
      return;
    }

    try {
      setLoading(true);
      setError(null);
      const newAssets = await fetchAssets(currentProject, path || currentPath());
      console.log('Fetched assets:', newAssets);
      
      if (newAssets && Array.isArray(newAssets)) {
        assetsActions.setAssetsForPath(path || currentPath(), newAssets);
        setAssets(newAssets);
      } else {
        console.warn('Invalid assets response:', newAssets);
        setAssets([]);
      }
      
      setLoading(false);
    } catch (err) {
      console.error('Error fetching assets:', err);
      setError(err.message);
      setAssets([]);
      setLoading(false);
    }
  };

  const handleFileChange = async (changeData) => {
    console.log('🔄 AssetLibrary: File change detected:', changeData);
    
    // Ignore temporary files and system files that shouldn't trigger refresh
    if (changeData.message) {
      const message = changeData.message.toLowerCase();
      if (message.includes('.tmp.') || message.includes('%') || message.includes('~')) {
        console.log('🔄 AssetLibrary: Ignoring temporary/system file change');
        return;
      }
    }
    
    console.log('🔄 AssetLibrary: Refreshing asset data...');
    assetsActions.clearAllAssetCache();
    
    const currentProject = await ensureProjectLoaded();
    if (!currentProject?.name) {
      console.log('🔄 AssetLibrary: No project to refresh');
      return;
    }
    
    if (viewMode() === 'folder') {
      fetchFolderTreeWithCache(currentProject);
      fetchAssetsWithCache(currentProject, currentPath());
    } else {
      fetchAssetCategoriesWithCache(currentProject);
    }
  };

  onMount(async () => {
    setTreePanelWidth(200);
    
    let currentProject = await ensureProjectLoaded();
    
    if (!currentProject?.name) {
      console.log('🦀 No current project available');
      return;
    }
    
    console.log('🦀 AssetLibrary mounting with project:', currentProject?.name || 'undefined');

    clearCacheIfProjectChanged(currentProject);
    setError(null);

    if (viewMode() === 'folder') {
      fetchFolderTreeWithCache(currentProject);
      fetchAssetsWithCache(currentProject);
    } else {
      setLoading(true);
      fetchAssetCategoriesWithCache(currentProject);
    }

    const handleWebSocketMessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message.type === 'file-change') {
          console.log('🔄 AssetLibrary: External file change detected:', message.data);
          handleFileChange(message.data);
        }
      } catch (error) {

      }
    };

    // File watching with SSE stream
    if (typeof window !== 'undefined') {
      console.log('🔄 AssetLibrary: Setting up SSE file watching');
      
      let eventSource = null;
      let reconnectTimer = null;
      let reconnectAttempts = 0;
      const maxReconnectAttempts = 5;
      const reconnectDelay = 2000;
      
      const connectSSE = () => {
        try {
          eventSource = new EventSource('http://localhost:3001/file-changes/stream');
          
          eventSource.onopen = () => {
            console.log('📡 AssetLibrary: SSE connection opened');
            reconnectAttempts = 0;
          };
          
          eventSource.onmessage = (event) => {
            try {
              const data = JSON.parse(event.data);
              console.log('📡 AssetLibrary: SSE message received:', data);
              
              if (data.type === 'file_change') {
                console.log('📡 AssetLibrary: File change detected:', data.message);
                handleFileChange({ source: 'sse', message: data.message });
              } else if (data.type === 'connected') {
                console.log('📡 AssetLibrary: SSE connected successfully');
              } else if (data.type === 'heartbeat') {
                // Heartbeat received - connection is alive
              }
            } catch (error) {
              console.error('📡 AssetLibrary: Failed to parse SSE message:', error);
            }
          };
          
          eventSource.onerror = (error) => {
            console.error('📡 AssetLibrary: SSE connection error:', error);
            eventSource.close();
            
            if (reconnectAttempts < maxReconnectAttempts) {
              reconnectAttempts++;
              console.log(`📡 AssetLibrary: Reconnecting SSE... attempt ${reconnectAttempts}/${maxReconnectAttempts}`);
              reconnectTimer = setTimeout(connectSSE, reconnectDelay);
            } else {
              console.error('📡 AssetLibrary: Max reconnection attempts reached, falling back to manual refresh');
            }
          };
        } catch (error) {
          console.error('📡 AssetLibrary: Failed to create SSE connection:', error);
        }
      };
      
      // Start SSE connection
      connectSSE();
      
      // Manual refresh with F5 key as fallback
      const handleKeyPress = (e) => {
        if (e.key === 'F5' || (e.ctrlKey && e.key === 'r')) {
          e.preventDefault();
          console.log('🔄 AssetLibrary: Manual refresh triggered');
          handleFileChange({ source: 'manual' });
        }
      };
      
      document.addEventListener('keydown', handleKeyPress);
      
      const removeListener = addFileChangeListener(handleFileChange);
      
      onCleanup(() => {
        if (eventSource) {
          eventSource.close();
        }
        if (reconnectTimer) {
          clearTimeout(reconnectTimer);
        }
        document.removeEventListener('keydown', handleKeyPress);
        removeListener();
      });
    } else {
      const removeListener = addFileChangeListener(handleFileChange);
      onCleanup(() => {
        removeListener();
      });
    }
  });

  createEffect(async () => {
    console.log('🦀 Effect triggered - currentPath:', currentPath(), 'viewMode:', viewMode());
    
    if (viewMode() !== 'folder') {
      console.log('🦀 Skipping fetch - not folder view');
      return;
    }

    const currentProject = await ensureProjectLoaded();
    console.log('🦀 Project loaded:', currentProject?.name);
    
    if (!currentProject?.name) {
      console.log('🦀 Skipping fetch - no project available');
      return;
    }

    console.log('🦀 Fetching assets for path:', currentPath());
    fetchAssetsWithCache(currentProject, currentPath());
  });

  createEffect(() => {
    if (!searchQuery().trim()) {
      setGlobalSearchResults([]);
      setIsSearching(false);
      return;
    }

    const performGlobalSearch = async () => {
      setIsSearching(true);
      const currentProject = projectManager.getCurrentProject();
      if (!currentProject?.name) {
        setIsSearching(false);
        return;
      }

      try {
        const results = await searchAssets(currentProject, searchQuery());
        setGlobalSearchResults(results);
      } catch (error) {
        console.warn('Search API error, falling back to client-side search:', error);
        performClientSideGlobalSearch();
      } finally {
        setIsSearching(false);
      }
    };

    const performClientSideGlobalSearch = () => {
      const searchResults = [];
      const searchLower = searchQuery().toLowerCase();
      const cache = assetCache();
      
      Object.entries(cache.assetsByPath).forEach(([path, pathData]) => {
        if (pathData?.assets) {
          pathData.assets.forEach(asset => {
            if (asset.name.toLowerCase().includes(searchLower) || 
                asset.fileName?.toLowerCase().includes(searchLower)) {
              searchResults.push({
                ...asset,
                path: path ? `${path}/${asset.name}` : asset.name
              });
            }
          });
        }
      });
      
      setGlobalSearchResults(searchResults);
    };

    const searchTimeout = setTimeout(performGlobalSearch, 300);
    onCleanup(() => clearTimeout(searchTimeout));
  });

  const breadcrumbs = createMemo(() => {
    if (viewMode() !== 'folder') return [];
    
    const project = currentProject();
    if (!project?.name) {
      return [];
    }
    
    const parts = currentPath() ? currentPath().split('/') : [];
    const crumbs = [{ name: project.name, path: '' }];
    
    let currentBreadcrumbPath = '';
    for (const part of parts) {
      currentBreadcrumbPath = currentBreadcrumbPath ? `${currentBreadcrumbPath}/${part}` : part;
      crumbs.push({ name: part, path: currentBreadcrumbPath });
    }
    
    return crumbs;
  });

  const getCategoryIcon = (categoryId) => {
    const iconMap = {
      '3d-models': Cube,
      'textures': Video,
      'audio': Wave,
      'scripts': Code,
      'data': FolderOpen,
      'misc': Folder
    };
    return iconMap[categoryId] || Folder;
  };

  const categoryList = createMemo(() => {
    const categories = assetCategories();
    if (!categories) return [];
    
    return Object.entries(categories).map(([id, data]) => ({
      id,
      label: data.name,
      count: data.assets ? data.assets.length : (data.files ? data.files.length : 0),
      icon: getCategoryIcon(id)
    }));
  });

  const filteredAssets = createMemo(() => {
    const fileAssets = assets().filter(asset => asset.type === 'file');
    
    if (!searchQuery()) return fileAssets;
    
    if (globalSearchResults().length > 0) {
      return globalSearchResults().filter(asset => asset.type === 'file');
    }
    
    return fileAssets.filter(asset => {
      const matchesSearch = asset.name.toLowerCase().includes(searchQuery().toLowerCase()) ||
                           asset.fileName?.toLowerCase().includes(searchQuery().toLowerCase());
      return matchesSearch;
    });
  });

  createEffect(() => {
    if (viewMode() === 'type') {
      const currentProject = projectManager.getCurrentProject();
      if (!currentProject?.name) return;
      
      if (!assetCategories()) {
        setLoading(true);
        fetchAssetCategoriesWithCache(currentProject);
      } else {
        const categoryAssets = assetCategories()[selectedCategory()]?.assets || assetCategories()[selectedCategory()]?.files || [];
        setAssets(categoryAssets);
        setLoading(false);
      }
    }
  });

  createEffect(() => {
    if (viewMode() === 'type' && assetCategories()) {
      const categoryAssets = assetCategories()[selectedCategory()]?.assets || assetCategories()[selectedCategory()]?.files || [];
      setAssets(categoryAssets);
      setLoading(false);
    }
  });

  const handleResizeMouseDown = (e) => {
    setIsResizing(true);
    document.body.classList.add('dragging-horizontal');
    e.preventDefault();
  };

  const handleResizeMouseMove = (e) => {
    if (!isResizing()) return;
    const newWidth = e.clientX;
    setTreePanelWidth(Math.max(150, Math.min(400, newWidth)));
  };

  const handleResizeMouseUp = () => {
    setIsResizing(false);
    document.body.classList.remove('dragging-horizontal');
  };

  createEffect(() => {
    if (isResizing()) {
      document.addEventListener('mousemove', handleResizeMouseMove);
      document.addEventListener('mouseup', handleResizeMouseUp);
      
      onCleanup(() => {
        document.removeEventListener('mousemove', handleResizeMouseMove);
        document.removeEventListener('mouseup', handleResizeMouseUp);
      });
    }
  });


  const uploadFiles = async (files) => {
    setIsUploading(true);
    setError(null);
    
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) {
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
        let targetFolderPath = currentPath();
        
        if (file.webkitRelativePath) {
          const pathParts = file.webkitRelativePath.split('/');
          if (pathParts.length > 1) {
            const relativePath = pathParts.slice(0, -1).join('/');
            targetFolderPath = currentPath() ? `${currentPath()}/${relativePath}` : relativePath;
          }
        }
        
        const headers = {};
        headers['X-Folder-Path'] = targetFolderPath;
        
        const targetPath = targetFolderPath ? `projects/${currentProject.name}/assets/${targetFolderPath}/${file.name}` : `projects/${currentProject.name}/assets/${file.name}`;
        
        const isTextFile = file.type.startsWith('text/') || 
                          file.name.match(/\.(js|jsx|ts|tsx|json|xml|txt|md|css|html|yml|yaml|csv|log|ini|conf|cfg|properties)$/i);
        
        const isBinaryFile = file.type.startsWith('image/') ||
                            file.type.startsWith('audio/') ||
                            file.type.startsWith('video/') ||
                            file.type.startsWith('application/octet-stream') ||
                            file.name.match(/\.(png|jpg|jpeg|gif|webp|bmp|tga|tiff|ico|svg|mp3|wav|ogg|m4a|aac|flac|mp4|avi|mov|mkv|webm|wmv|glb|gltf|obj|fbx|dae|3ds|blend|max|ma|mb|stl|ply|x3d)$/i);
        
        if (isTextFile) {
          console.log(`Uploading text file: ${file.name}`);
          const textContent = await new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(reader.result);
            reader.onerror = () => reject(reader.error);
            reader.readAsText(file);
          });
          await writeFile(targetPath, textContent);
        } else if (isBinaryFile) {
          console.log(`Uploading binary file: ${file.name} (type: ${file.type})`);
          const base64Content = await new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => {
              const result = reader.result;
              if (typeof result === 'string' && result.includes(',')) {
                resolve(result.split(',')[1]);
              } else {
                reject(new Error('Invalid base64 data'));
              }
            };
            reader.onerror = () => reject(reader.error);
            reader.readAsDataURL(file);
          });
          await writeBinaryFile(targetPath, base64Content);
        } else {
          console.log(`Uploading unknown file type as binary: ${file.name} (type: ${file.type})`);
          const base64Content = await new Promise((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => {
              const result = reader.result;
              if (typeof result === 'string' && result.includes(',')) {
                resolve(result.split(',')[1]);
              } else {
                reject(new Error('Invalid base64 data'));
              }
            };
            reader.onerror = () => reject(reader.error);
            reader.readAsDataURL(file);
          });
          await writeBinaryFile(targetPath, base64Content);
        }
        
        const result = {
          filename: file.name,
          path: targetPath
        };
        console.log(`Successfully uploaded: ${result.filename}`);
        
        uploadResults.push({
          filename: result.filename,
          path: result.path,
          targetFolder: targetFolderPath
        });
      }
      
      console.log(`All ${files.length} files uploaded successfully. Refreshing cache...`);
      const affectedPaths = new Set();
      
      uploadResults.forEach(result => {
        affectedPaths.add(result.targetFolder || '');
      });
      
      assetsActions.invalidateAssetPaths(Array.from(affectedPaths));
      
      if (viewMode() === 'type') {
        assetsActions.invalidateCategories();
        await fetchAssetCategoriesWithCache(currentProject);
      } else {
        const needsFolderTreeRefresh = uploadResults.some(result => 
          result.targetFolder && result.targetFolder.includes('/')
        );
        
        if (needsFolderTreeRefresh) {
          assetsActions.invalidateFolderTree();
          await fetchFolderTreeWithCache(currentProject);
        }
        
        await fetchAssetsWithCache(currentProject, currentPath());
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

  const handleDragOver = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    if (!isInternalDrag() && !isDragOver()) {
      setIsDragOver(true);
    }
  };

  const handleDragEnter = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    if (!isInternalDrag()) {
      setIsDragOver(true);
    }
  };

  const handleDragLeave = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (!e.currentTarget.contains(e.relatedTarget)) {
      setIsDragOver(false);
    }
  };

  const handleDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
    
    console.log('Drop event triggered:', {
      isInternalDrag: isInternalDrag(),
      filesLength: e.dataTransfer.files.length,
      files: Array.from(e.dataTransfer.files).map(f => f.name)
    });
    
    if (!isInternalDrag()) {
      const files = Array.from(e.dataTransfer.files);
      if (files.length > 0) {
        console.log('Starting upload for files:', files.map(f => f.name));
        uploadFiles(files);
      } else {
        console.log('No files found in drop event');
      }
    } else {
      console.log('Internal drag detected, skipping file upload');
    }
  };

  const handleContextMenu = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    const contextMenuItems = [
      {
        label: 'Create Object',
        action: () => {},
        icon: <Plus class="w-4 h-4" />,
        submenu: [
          { label: 'Cube', action: () => handleCreateObject('cube'), icon: <Cube class="w-4 h-4" /> },
          { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <Circle class="w-4 h-4" /> },
          { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <Rectangle class="w-4 h-4" /> },
          { label: 'Plane', action: () => handleCreateObject('plane'), icon: <Grid class="w-4 h-4" /> },
          { separator: true },
          { label: 'Light', action: () => handleCreateObject('light'), icon: <Lightbulb class="w-4 h-4" /> },
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <Video class="w-4 h-4" /> },
        ]
      },
      { separator: true },
      {
        label: 'Create Script',
        action: () => handleCreateScript(),
        icon: <FileText class="w-4 h-4" />
      },
      { separator: true },
      {
        label: 'Upload Files...',
        action: () => handleUploadClick(),
        icon: <Upload class="w-4 h-4" />,
        shortcut: 'Ctrl+U'
      },
      {
        label: 'Upload Folder...',
        action: () => handleUploadFolderClick(),
        icon: <FolderOpen class="w-4 h-4" />
      },
      { separator: true },
      {
        label: 'New Folder',
        action: () => handleCreateFolder(),
        icon: <Folder class="w-4 h-4" />
      }
    ];
    
    setContextMenu({
      items: contextMenuItems,
      position: { x: e.clientX, y: e.clientY }
    });
  };

  const handleUploadClick = () => {
    fileInputRef?.click();
  };

  const handleUploadFolderClick = () => {
    folderInputRef?.click();
  };

  const handleFileInputChange = (e) => {
    const files = Array.from(e.target.files);
    if (files.length > 0) {
      uploadFiles(files);
    }
    e.target.value = '';
  };

  const handleFolderInputChange = (e) => {
    const files = Array.from(e.target.files);
    if (files.length > 0) {
      uploadFiles(files);
    }
    e.target.value = '';
  };

  const handleMoveItem = async (sourcePath, targetFolderPath) => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) {
      console.error('No project loaded for move operation');
      return;
    }

    const sourceFileName = sourcePath.split('/').pop();
    const targetPath = targetFolderPath ? `${targetFolderPath}/${sourceFileName}` : sourceFileName;

    try {
      await moveAsset(currentProject, sourcePath, targetPath);
      assetsActions.invalidateAssetPaths([sourcePath.split('/').slice(0, -1).join('/'), targetFolderPath]);
      await fetchAssetsWithCache(currentProject, currentPath());
    } catch (error) {
      console.error('Error moving item:', error);
      setError(`Failed to move item: ${error.message}`);
    }
  };

  const handleMoveMultipleItems = async (assets, targetFolderPath) => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) {
      console.error('No project loaded for move operation');
      return;
    }

    const sourceDirectories = new Set();
    let successCount = 0;
    let failCount = 0;

    console.log(`🚚 Moving ${assets.length} assets to ${targetFolderPath || 'root'}`);

    for (const asset of assets) {
      const sourcePath = asset.path;
      const sourceFileName = sourcePath.split('/').pop();
      const targetPath = targetFolderPath ? `${targetFolderPath}/${sourceFileName}` : sourceFileName;

      try {
        await moveAsset(currentProject, sourcePath, targetPath);
        sourceDirectories.add(sourcePath.split('/').slice(0, -1).join('/'));
        successCount++;
        console.log(`✅ Moved: ${asset.name}`);
      } catch (error) {
        console.error(`❌ Failed to move ${asset.name}:`, error);
        failCount++;
      }
    }

    const affectedPaths = Array.from(sourceDirectories).concat(targetFolderPath ? [targetFolderPath] : []);
    assetsActions.invalidateAssetPaths(affectedPaths);
    
    await fetchAssetsWithCache(currentProject, currentPath());
    
    if (successCount > 0) {
      clearSelection();
    }

    if (failCount === 0) {
      console.log(`🎉 Successfully moved ${successCount} files`);
    } else {
      console.warn(`⚠️ Moved ${successCount} files, ${failCount} failed`);
      setError(`Moved ${successCount} files, ${failCount} failed`);
    }
  };

  const handleCreateScript = () => {
    setShowScriptDialog(true);
  };

  const handleConfirmCreateScript = async (scriptName) => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) {
      console.error('No project loaded for script creation');
      setError('No project loaded for script creation');
      return;
    }

    let cleanScriptName = scriptName.trim();
    if (!cleanScriptName.endsWith('.js') && !cleanScriptName.endsWith('.ts') && 
        !cleanScriptName.endsWith('.jsx') && !cleanScriptName.endsWith('.tsx')) {
      cleanScriptName += '.js';
    }

    try {
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

      const targetPath = viewMode() === 'folder' ? currentPath() : '';
      const fullPath = targetPath ? `${targetPath}/${cleanScriptName}` : cleanScriptName;

      const scriptPath = targetPath ? `projects/${currentProject.name}/assets/${targetPath}/${cleanScriptName}` : `projects/${currentProject.name}/assets/${cleanScriptName}`;
      
      await writeFile(scriptPath, scriptContent);
      
      const result = {
        filePath: scriptPath
      };
      console.log(`Script created successfully: ${result.filePath}`);

      setError(null);

      if (viewMode() === 'folder') {
        assetsActions.invalidateAssetPaths([targetPath]);
        await fetchAssetsWithCache(currentProject, currentPath());
        
        if (targetPath.includes('/')) {
          assetsActions.invalidateFolderTree();
          await fetchFolderTreeWithCache(currentProject);
        }
      } else {
        assetsActions.invalidateCategories();
        await fetchAssetCategoriesWithCache(currentProject);
      }

    } catch (error) {
      console.error('Error creating script:', error);
      setError(`Failed to create script: ${error.message}`);
      throw error;
    }
  };

  const handleCreateFolder = async () => {
    const folderName = prompt('Enter folder name:');
    if (!folderName || !folderName.trim()) {
      return;
    }

    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) {
      console.error('No project loaded for folder creation');
      return;
    }

    try {
      await createFolder(currentProject, folderName.trim(), viewMode() === 'folder' ? currentPath() : '');
      assetsActions.invalidateFolderTree();
      assetsActions.invalidateAssetPaths([currentPath()]);
      await fetchFolderTreeWithCache(currentProject);
      await fetchAssetsWithCache(currentProject, currentPath());
    } catch (error) {
      console.error('Error creating folder:', error);
      setError(`Failed to create folder: ${error.message}`);
    }
  };

  const handleFolderClick = (folderPath) => {
    console.log('🦀 Folder clicked:', folderPath);
    setCurrentPath(folderPath);
  };

  const handleFolderToggle = (folderPath) => {
    toggleFolderExpansion(folderPath);
  };

  const handleBreadcrumbClick = (path) => {
    setCurrentPath(path);
  };

  const handleAssetDoubleClick = (asset) => {
    // Check if it's a RenScript file
    if (asset.extension?.toLowerCase() === '.ren') {
      // Extract filename without path for display
      const fileName = asset.name || asset.fileName || asset.path.split('/').pop() || asset.path.split('\\').pop();
      
      // Debug the asset data
      console.log('Asset data:', asset);
      console.log('Asset path:', asset.path);
      console.log('File name:', fileName);
      
      // Open in script editor
      scriptEditorActions.openScript(asset.path, fileName);
      
      // Switch to script editor tab (this will be handled by the tab visibility)
      console.log('Opening RenScript file in editor:', fileName);
    } else {
      // Handle other file types (could expand this later)
      console.log('Double-clicked on:', asset.name, 'Type:', asset.extension);
    }
  };

  createEffect(() => {
    const handleClickOutside = () => {
      setContextMenu(null);
    };
    
    if (contextMenu()) {
      document.addEventListener('click', handleClickOutside);
      onCleanup(() => document.removeEventListener('click', handleClickOutside));
    }
  });

  const renderFolderTree = (node, depth = 0) => {
    if (!node) return null;

    const isExpanded = () => expandedFolders().has(node.path);
    const isSelected = () => currentPath() === node.path;
    const hasChildren = node.children && node.children.length > 0;
    
    return (
      <div class="select-none relative">
        <div
          class={`flex items-center py-1 px-2 text-xs cursor-pointer transition-colors relative overflow-hidden ${ 
            dragOverTreeFolder() === node.path 
              ? 'bg-primary/30 border-2 border-primary border-dashed rounded'
              : isSelected() 
                ? 'bg-primary text-primary-content' 
                : 'text-base-content/70 hover:bg-base-200 hover:text-base-content'
          }`}
          style={{ 'padding-left': `${8 + depth * 20}px` }}
          onClick={() => handleFolderClick(node.path)}
          onDragOver={(e) => {
            if (isInternalDrag() && viewMode() === 'folder') {
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              setDragOverTreeFolder(node.path);
            }
          }}
          onDragEnter={(e) => {
            if (isInternalDrag() && viewMode() === 'folder') {
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
            if (isInternalDrag() && viewMode() === 'folder') {
              setDragOverTreeFolder(null);
              
              try {
                const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
                
                if (dragData.type === 'multiple-assets') {
                  const validAssets = dragData.assets.filter(asset => {
                    if (asset.assetType === 'folder' && node.path.startsWith(asset.path)) {
                      console.warn(`Cannot move folder ${asset.name} into itself or its children`);
                      return false;
                    }
                    return asset.path !== node.path;
                  });
                  
                  if (validAssets.length > 0) {
                    handleMoveMultipleItems(validAssets, node.path);
                  }
                } else if (dragData.type === 'asset' && dragData.path !== node.path) {
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
          <Show when={isSelected()}>
            <div class="absolute left-0 top-0 bottom-0 w-0.5 bg-primary pointer-events-none" />
          </Show>
          
          <Show when={depth > 0}>
            <div class="absolute left-0 top-0 bottom-0 pointer-events-none">
              <div
                class="absolute top-0 bottom-0 w-px bg-base-content/30"
                style={{ left: `${8 + (depth - 1) * 20 + 10}px` }}
              />
              <div
                class="absolute top-1/2 w-3 h-px bg-base-content/30"
                style={{ left: `${8 + (depth - 1) * 20 + 10}px` }}
              />
            </div>
          </Show>
          
          <Show when={hasChildren}>
            <button
              onClick={(e) => {
                e.stopPropagation();
                handleFolderToggle(node.path);
              }}
              class="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-base-200/50"
            >
              <ChevronRight 
                class={`w-3 h-3 transition-all duration-200 ${
                  isExpanded() 
                    ? 'rotate-90 text-primary' 
                    : 'text-base-content/50 hover:text-base-content/70'
                }`} 
              />
            </button>
          </Show>
          <Show when={!hasChildren}>
            <div class="w-4 mr-1" />
          </Show>
          <Folder class={`w-4 h-4 mr-2 ${
            isSelected() ? 'text-primary-content' : 'text-warning'
          }`} />
          <span class="flex-1 text-base-content/80 truncate">{node.name}</span>
          <Show when={node.files && node.files.length > 0}>
            <span class={`ml-auto text-[10px] px-1.5 py-0.5 rounded-full ${
              isSelected() 
                ? 'text-primary-content bg-primary' 
                : 'text-base-content/60 bg-base-300'
            }`}>
              {node.files.length}
            </span>
          </Show>
        </div>
        
        <Show when={hasChildren && isExpanded()}>
          <div class="transition-all duration-300 ease-out">
            <For each={node.children}>
              {(child) => renderFolderTree(child, depth + 1)}
            </For>
          </div>
        </Show>
      </div>
    );
  };


  const renderAssetItem = (asset, index) => {
    const getAssetCategory = (extension) => {
      const ext = extension?.toLowerCase() || '';
      if (['.glb', '.gltf', '.obj', '.fbx'].includes(ext)) return '3d-models';
      if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(ext)) return 'textures';
      if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) return 'audio';
      if (['.js', '.jsx', '.ts', '.tsx', '.py', '.ren'].includes(ext)) return 'scripts';
      return 'misc';
    };

    const startDrag = (e, asset) => {
      setIsInternalDrag(true);
      
      if (!isAssetSelected(asset.id)) {
        setSelectedAssets(new Set([asset.id]));
        setLastSelectedAsset(asset);
      }
      
      const selectedAssetIds = Array.from(selectedAssets());
      const allAssets = filteredAssets();
      const selectedAssetObjects = allAssets.filter(a => selectedAssetIds.includes(a.id));
      
      const dragData = {
        type: selectedAssetObjects.length > 1 ? 'multiple-assets' : 'asset',
        assets: selectedAssetObjects.map(a => ({
          id: a.id,
          name: a.name,
          path: a.path,
          assetType: a.type,
          fileName: a.fileName,
          extension: a.extension,
          mimeType: a.mimeType,
          category: getAssetCategory(a.extension),
          fileType: getAssetCategory(a.extension) === 'scripts' ? 'script' : getAssetCategory(a.extension)
        })),
        ...(selectedAssetObjects.length === 1 ? {
          id: asset.id,
          name: asset.name,
          path: asset.path,
          assetType: asset.type,
          fileName: asset.fileName,
          extension: asset.extension,
          mimeType: asset.mimeType,
          category: getAssetCategory(asset.extension),
          fileType: getAssetCategory(asset.extension) === 'scripts' ? 'script' : getAssetCategory(asset.extension)
        } : {})
      };
      
      e.dataTransfer.setData('application/json', JSON.stringify(dragData));
      e.dataTransfer.setData('text/plain', JSON.stringify(dragData));
      e.dataTransfer.effectAllowed = 'move';
      
      if (selectedAssetObjects.length > 1) {
        const dragImage = document.createElement('div');
        dragImage.className = 'fixed top-[-1000px] bg-primary text-primary-content px-3 py-2 rounded-lg font-medium shadow-lg';
        dragImage.textContent = `Moving ${selectedAssetObjects.length} files`;
        document.body.appendChild(dragImage);
        e.dataTransfer.setDragImage(dragImage, 50, 25);
        setTimeout(() => document.body.removeChild(dragImage), 0);
      }
    };

    if (layoutMode() === 'list') {
      return (
        <div
          class={`group cursor-pointer transition-all duration-200 p-2 flex items-center gap-3 ${
            isAssetSelected(asset.id)
              ? 'bg-primary/20 border-l-2 border-primary hover:bg-primary/30'
              : typeof index === 'function' && index() % 2 === 0 
                ? 'bg-base-200/50 hover:bg-base-300/50' 
                : 'bg-base-300/30 hover:bg-base-300/50'
          }`}
          data-asset-id={asset.id}
          draggable={true}
          onMouseEnter={() => setHoveredItem(asset.id)}
          onMouseLeave={() => setHoveredItem(null)}
          onClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            toggleAssetSelection(asset, e.ctrlKey || e.metaKey, e.shiftKey);
          }}
          onDblClick={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleAssetDoubleClick(asset);
          }}
          onDragStart={(e) => startDrag(e, asset)}
          onDragEnd={() => {
            setIsInternalDrag(false);
            setDragOverFolder(null);
            setDragOverTreeFolder(null);
            setDragOverBreadcrumb(null);
          }}
        >
          <div class="w-10 h-10 flex items-center justify-center flex-shrink-0 relative">
            <Show when={is3DModelFile(asset.extension)} fallback={
              <Show when={isImageFile(asset.extension)} fallback={
                <div class={`w-full h-full bg-base-300 rounded flex items-center justify-center ${
                    loadedAssets().includes(asset.id) 
                      ? 'opacity-100' 
                      : failedAssets().includes(asset.id) 
                        ? 'opacity-40 grayscale' 
                        : 'opacity-60'
                  }`}>
                  {isScriptFile(asset.extension) ? (
                    <Code class="w-5 h-5 text-primary" />
                  ) : (
                    <Photo class="w-5 h-5 text-base-content/60" />
                  )}
                </div>
              }>
                <ImageThumbnail asset={asset} />
              </Show>
            }>
              <ModelThumbnail asset={asset} />
            </Show>

            <div class="absolute -bottom-1 -right-1">
              <Show when={preloadingAssets().includes(asset.id)}>
                <div class="w-3 h-3 bg-warning rounded-full flex items-center justify-center">
                  <div class="w-1.5 h-1.5 border border-white border-t-transparent rounded-full animate-spin"></div>
                </div>
              </Show>
              <Show when={failedAssets().includes(asset.id)}>
                <div class="w-3 h-3 bg-error rounded-full flex items-center justify-center">
                  <X class="w-2 h-2 text-white" />
                </div>
              </Show>
              <Show when={loadedAssets().includes(asset.id)}>
                <div class="w-3 h-3 bg-success rounded-full flex items-center justify-center">
                  <Check class="w-2 h-2 text-white" />
                </div>
              </Show>
            </div>
          </div>
          
          <div class="flex-1 min-w-0">
            <div class="text-sm text-base-content/70 group-hover:text-base-content transition-colors truncate">
              {asset.name}
            </div>
            <div class="text-xs text-base-content/50 truncate">
              {asset.extension?.toUpperCase()} • {asset.size ? `${Math.round(asset.size / 1024)} KB` : 'Unknown size'}
            </div>
          </div>

          <Show when={asset.extension}>
            {(() => {
              const style = getExtensionStyle(asset.extension);
              return (
                <div class="flex-shrink-0">
                  <div class={`${style.bgColor} ${style.textColor} text-xs font-bold px-2 py-1 rounded-full flex items-center transition-colors ${style.hoverColor} ${style.icon ? 'gap-1' : ''} shadow-sm`}>
                    {style.icon}
                    <span>{asset.extension.replace('.', '').toUpperCase()}</span>
                  </div>
                </div>
              );
            })()}
          </Show>
        </div>
      );
    }

    return (
      <div
        class={`group cursor-pointer transition-all duration-200 p-2 rounded-lg hover:bg-base-300/30 ${
          isAssetSelected(asset.id) ? 'bg-primary/20 ring-2 ring-primary/50' : ''
        }`}
        data-asset-id={asset.id}
        draggable={true}
        onMouseEnter={() => setHoveredItem(asset.id)}
        onMouseLeave={() => setHoveredItem(null)}
        onClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          
          if (failedAssets().includes(asset.id)) {
            setFailedAssets(prev => prev.filter(id => id !== asset.id));
            setPreloadingAssets(prev => [...prev, asset.id]);
            setTimeout(() => {
              setPreloadingAssets(prev => prev.filter(id => id !== asset.id));
              setLoadedAssets(prev => [...prev, asset.id]);
            }, 1000);
          } else {
            toggleAssetSelection(asset, e.ctrlKey || e.metaKey, e.shiftKey);
          }
        }}
        onDblClick={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleAssetDoubleClick(asset);
        }}
        onDragStart={(e) => startDrag(e, asset)}
        onDragEnd={() => {
          setIsInternalDrag(false);
          setDragOverFolder(null);
          setDragOverTreeFolder(null);
          setDragOverBreadcrumb(null);
        }}
      >
        <div class="relative">
          <div class="w-full aspect-square mb-2 flex items-center justify-center relative">
            <Show when={is3DModelFile(asset.extension)} fallback={
              <Show when={isImageFile(asset.extension)} fallback={
                <div class={`w-full h-full bg-base-300 rounded flex items-center justify-center transition-all group-hover:scale-105 ${
                    loadedAssets().includes(asset.id) 
                      ? 'opacity-100' 
                      : failedAssets().includes(asset.id) 
                        ? 'opacity-40 grayscale' 
                        : 'opacity-60'
                  }`}>
                  {isScriptFile(asset.extension) ? (
                    <Code class="w-8 h-8 text-primary" />
                  ) : (
                    <Photo class="w-8 h-8 text-base-content/60" />
                  )}
                </div>
              }>
                <ImageThumbnail asset={asset} />
              </Show>
            }>
              <ModelThumbnail asset={asset} />
            </Show>
            
            <Show when={asset.extension}>
              {(() => {
                const style = getExtensionStyle(asset.extension);
                return (
                  <div class={`absolute top-0 right-0 ${style.bgColor} ${style.textColor} text-xs font-bold px-2 py-1 rounded-full text-center leading-none flex items-center transition-colors ${style.hoverColor} ${style.icon ? 'gap-1' : ''} shadow-sm`}>
                    {style.icon}
                    <span>{asset.extension.replace('.', '').toUpperCase()}</span>
                  </div>
                );
              })()}
            </Show>

            <div class="absolute -bottom-1 -right-1">
              <Show when={preloadingAssets().includes(asset.id)}>
                <div class="w-6 h-6 bg-warning rounded-full flex items-center justify-center">
                  <div class="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                </div>
              </Show>
              <Show when={failedAssets().includes(asset.id)}>
                <div class="w-6 h-6 bg-error rounded-full flex items-center justify-center" title={`Failed to load ${asset.name}`}>
                  <X class="w-3 h-3 text-white" />
                </div>
              </Show>
              <Show when={loadedAssets().includes(asset.id)}>
                <div class="w-6 h-6 bg-success rounded-full flex items-center justify-center">
                  <Check class="w-3 h-3 text-white" />
                </div>
              </Show>
            </div>
          </div>
        </div>
        
        <div class="text-xs text-base-content/70 group-hover:text-base-content transition-colors truncate text-center leading-tight" title={asset.name}>
          {asset.name}
        </div>
      </div>
    );
  };

  return (
    <div class="h-full flex bg-base-200 no-select">
      <div 
        class="bg-base-300 border-r border-base-300 flex flex-col relative"
        style={{ width: `${treePanelWidth()}px` }}
      >
        <div
          class={`absolute right-0 top-0 bottom-0 w-0.5 resize-handle cursor-col-resize ${isResizing() ? 'dragging' : ''}`}
          onMouseDown={handleResizeMouseDown}
        />
        <div class="px-2 py-2 border-b border-base-300">
          <div class="flex items-center justify-between mb-2">
            <div class="text-xs font-medium text-base-content/70">Project Assets</div>
            <div class="flex items-center gap-2">
              <button
                onClick={() => {
                  console.log('🔄 AssetLibrary: Manual refresh triggered via button');
                  handleFileChange({ source: 'manual-button' });
                }}
                class="px-2 py-1 text-xs rounded bg-base-200 text-base-content/60 hover:text-base-content hover:bg-base-300 transition-colors"
                title="Refresh Assets"
              >
                <Refresh class="w-3 h-3" />
              </button>
              <div class="flex bg-base-200 rounded overflow-hidden">
                <button
                  onClick={() => setViewMode('folder')}
                  class={`px-2 py-1 text-xs transition-colors ${
                    viewMode() === 'folder'
                      ? 'bg-primary text-primary-content'
                      : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
                  }`}
                  title="Folder View"
                >
                  <Folder class="w-3 h-3" />
                </button>
                <button
                  onClick={() => setViewMode('type')}
                  class={`px-2 py-1 text-xs transition-colors ${
                    viewMode() === 'type'
                      ? 'bg-primary text-primary-content'
                      : 'text-base-content/60 hover:text-base-content hover:bg-base-300'
                  }`}
                  title="Asset Type View"
                >
                  <Cube class="w-3 h-3" />
                </button>
              </div>
            </div>
          </div>
          <div class="relative">
            <Show when={isSearching()} fallback={
              <Search class="w-3 h-3 absolute left-2 top-1.5 text-base-content/40" />
            }>
              <div class="w-3 h-3 absolute left-2 top-1.5 animate-spin">
                <div class="w-3 h-3 border border-base-content/40 border-t-primary rounded-full"></div>
              </div>
            </Show>
            <input
              type="text"
              placeholder="Search all assets..."
              value={searchQuery()}
              onInput={(e) => setSearchQuery(e.target.value)}
              class="w-full pl-6 pr-2 py-1 bg-base-200 border border-base-300 rounded text-xs text-base-content placeholder-base-content/50 focus:outline-none focus:border-primary transition-colors"
            />
          </div>
        </div>
        
        <div class="flex-1 overflow-y-auto scrollbar-thin">
          <Show when={viewMode() === 'folder'} fallback={
            <Show when={categoryList().length > 0} fallback={
              <div class="p-4 text-center text-base-content/50 text-xs">
                {error() ? error() : 'Loading asset categories...'}
              </div>
            }>
              <div class="space-y-0.5 p-1">
                <For each={categoryList()}>
                  {(category) => (
                    <button
                      onClick={() => setSelectedCategory(category.id)}
                      class={`w-full flex items-center justify-between px-2 py-1.5 text-left text-xs rounded hover:bg-base-200 transition-colors ${
                        selectedCategory() === category.id 
                          ? 'bg-primary text-primary-content' 
                          : 'text-base-content/70 hover:text-base-content'
                      }`}
                    >
                      <span class="flex items-center">
                        <category.icon class={`w-3 h-3 mr-2 ${
                          selectedCategory() === category.id ? 'text-primary-content' : 'text-base-content/60'
                        }`} />
                        {category.label}
                      </span>
                      <span class={`text-[10px] px-1.5 py-0.5 rounded-full ${
                        selectedCategory() === category.id 
                          ? 'text-primary-content bg-primary' 
                          : 'text-base-content/60 bg-base-300'
                      }`}>{category.count}</span>
                    </button>
                  )}
                </For>
              </div>
            </Show>
          }>
            <Show when={folderTree()} fallback={
              <div class="p-4 text-center text-base-content/50 text-xs">
                {(() => {
                  console.log('🦀 UI Render - folderTree() is falsy:', folderTree());
                  return error() ? error() : 'Loading directory tree...';
                })()}
              </div>
            }>
              <div class="py-1">
                {(() => {
                  console.log('🦀 UI Render - folderTree() is truthy:', folderTree(), 'length:', folderTree()?.length);
                  return null;
                })()}
                <For each={Array.isArray(folderTree()) ? folderTree() : [folderTree()]}>
                  {(node) => renderFolderTree(node)}
                </For>
              </div>
            </Show>
          </Show>
        </div>
      </div>
      
      <div 
        class={`flex-1 flex flex-col transition-all duration-200 relative ${
          isDragOver() ? 'bg-primary/20 border-2 border-primary border-dashed' : 'bg-base-200'
        }`}
      >
        <div class="bg-base-200 flex-shrink-0 border-b border-base-300">
          <div class="flex items-center justify-between px-3 py-2">
            <div class="flex items-center text-xs">
              <Show when={viewMode() === 'folder' && breadcrumbs().length > 0} fallback={
                <span class="text-base-content/60 px-2 py-1">
                  {viewMode() === 'type' && assetCategories() && assetCategories()[selectedCategory()] 
                    ? assetCategories()[selectedCategory()].name 
                    : 'Assets'
                  }
                </span>
              }>
                <For each={breadcrumbs()}>
                  {(crumb, index) => (
                    <>
                      <button 
                        onClick={() => handleBreadcrumbClick(crumb.path)}
                        class={`px-2 py-1 rounded transition-colors ${
                          dragOverBreadcrumb() === crumb.path
                            ? 'bg-primary/30 border border-primary border-dashed text-primary'
                            : index() === breadcrumbs().length - 1 
                              ? 'text-base-content font-medium hover:text-primary' 
                              : 'text-base-content/60 hover:text-primary'
                        }`}
                        onDragOver={(e) => {
                          if (isInternalDrag()) {
                            e.preventDefault();
                            e.dataTransfer.dropEffect = 'move';
                            setDragOverBreadcrumb(crumb.path);
                          }
                        }}
                        onDragEnter={(e) => {
                          if (isInternalDrag()) {
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
                          if (isInternalDrag()) {
                            setDragOverBreadcrumb(null);
                            
                            try {
                              const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
                              
                              if (dragData.type === 'multiple-assets') {
                                const validAssets = dragData.assets.filter(asset => {
                                  if (asset.assetType === 'folder' && crumb.path.startsWith(asset.path)) {
                                    console.warn(`Cannot move folder ${asset.name} into itself or its children`);
                                    return false;
                                  }
                                  return asset.path !== crumb.path;
                                });
                                
                                if (validAssets.length > 0) {
                                  handleMoveMultipleItems(validAssets, crumb.path);
                                }
                              } else if (dragData.type === 'asset' && dragData.path !== crumb.path) {
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
                      <Show when={index() < breadcrumbs().length - 1}>
                        <ChevronRight class="w-3 h-3 mx-1 text-base-content/40" />
                      </Show>
                    </>
                  )}
                </For>
              </Show>
            </div>
            
            <div class="flex items-center gap-3">
              <Show when={selectedAssets().size > 0}>
                <span class="text-xs text-primary font-medium bg-primary/20 px-2 py-1 rounded">
                  {selectedAssets().size} selected
                </span>
              </Show>
              <span class="text-xs text-base-content/60">{filteredAssets().length} items</span>
              
              <Show when={isUploading()}>
                <div class="flex items-center gap-2 transition-all duration-300 opacity-100">
                  <div class="w-20 h-1.5 bg-base-300 rounded-full overflow-hidden">
                    <div class="h-full bg-primary rounded-full animate-pulse" style={{ width: '100%' }} />
                  </div>
                  <span class="text-xs text-base-content/60">Uploading...</span>
                </div>
              </Show>
              
              <div class="flex bg-base-300 rounded overflow-hidden">
                <button
                  onClick={() => setLayoutMode('grid')}
                  class={`px-2 py-1 text-xs transition-colors ${
                    layoutMode() === 'grid'
                      ? 'bg-primary text-primary-content'
                      : 'text-base-content/60 hover:text-base-content hover:bg-base-200'
                  }`}
                  title="Grid View"
                >
                  <Grid class="w-3 h-3" />
                </button>
                <button
                  onClick={() => setLayoutMode('list')}
                  class={`px-2 py-1 text-xs transition-colors ${
                    layoutMode() === 'list'
                      ? 'bg-primary text-primary-content'
                      : 'text-base-content/60 hover:text-base-content hover:bg-base-200'
                  }`}
                  title="List View"
                >
                  <Menu class="w-3 h-3" />
                </button>
              </div>
              
              <Show when={isUploading()} fallback={
                <Show when={filteredAssets().length > 0}>
                </Show>
              }>
                <div class="flex items-center gap-1.5 text-primary/80 bg-primary/10 px-2 py-1 rounded-md border border-primary/20">
                  <div class="w-2 h-2 bg-primary rounded-full animate-spin" />
                  <span class="text-xs font-medium">Uploading...</span>
                </div>
              </Show>
            </div>
          </div>
        </div>
        
        <div 
          ref={mainContentRef}
          class="flex-1 flex flex-col p-3 overflow-y-auto overflow-x-hidden scrollbar-thin relative user-select-none"
          onDragOver={handleDragOver}
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onContextMenu={handleContextMenu}
          onMouseDown={startDragSelection}
        >
          <Show when={loading()}>
            <div class="flex-1 flex items-center justify-center">
              <div class="text-center text-base-content/60">
                <p class="text-sm">Loading assets...</p>
              </div>
            </div>
          </Show>
          
          <Show when={error()}>
            <div class="flex-1 flex items-center justify-center">
              <div class="text-center text-error">
                <p class="text-sm">Error: {error()}</p>
              </div>
            </div>
          </Show>
          
          <Show when={isUploading()}>
            <div class="flex-1 flex items-center justify-center">
              <div class="text-center text-primary">
                <div class="flex items-center justify-center gap-2">
                  <div class="w-4 h-4 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
                  <p class="text-sm">Uploading files...</p>
                </div>
              </div>
            </div>
          </Show>
          
          <Show when={isDragOver()}>
            <div class="absolute inset-0 flex items-center justify-center bg-primary/20 backdrop-blur-sm z-10">
              <div class="text-center">
                <div class="w-16 h-16 mx-auto mb-4 border-2 border-primary border-dashed rounded-lg flex items-center justify-center">
                  <Upload class="w-8 h-8 text-primary" />
                </div>
                <p class="text-lg font-medium text-primary">Drop files to upload</p>
                <p class="text-sm text-primary/80">Supports 3D models, textures, audio, and more</p>
              </div>
            </div>
          </Show>
          
          <Show when={!loading() && !error() && !isUploading()}>
            <Show when={filteredAssets().length === 0}>
              <div class="flex-1 flex items-center justify-center">
                <Show when={searchQuery()} fallback={
                  <div class="text-center">
                  <div class="w-16 h-16 sm:w-20 sm:h-20 mx-auto mb-4 sm:mb-6 border-2 border-base-content/40 border-dashed rounded-xl flex items-center justify-center bg-base-200/30">
                    {(() => {
                      if (viewMode() === 'folder') {
                        return <FolderOpen class="w-8 h-8 sm:w-10 sm:h-10 text-base-content/50" />;
                      } else {
                        const CategoryIcon = getCategoryIcon(selectedCategory());
                        return <CategoryIcon class="w-8 h-8 sm:w-10 sm:h-10 text-base-content/50" />;
                      }
                    })()}
                  </div>
                  
                  <h3 class="text-base sm:text-lg font-medium text-base-content/70 mb-2">
                    {viewMode() === 'folder' 
                      ? 'Empty folder'
                      : `No ${assetCategories()?.[selectedCategory()]?.name?.toLowerCase() || 'assets'} found`
                    }
                  </h3>
                  
                  
                  <Show when={viewMode() === 'folder'}>
                    <div class="flex flex-col sm:flex-row gap-3 mb-3 sm:mb-4">
                      <button
                        onClick={() => fileInputRef?.click()}
                        class="flex items-center justify-center gap-2 px-4 py-2 bg-primary hover:bg-primary/80 text-primary-content text-sm font-medium rounded-lg transition-colors min-w-[120px]"
                      >
                        <Upload class="w-4 h-4" />
                        Upload Files
                      </button>
                      
                      <button
                        onClick={() => folderInputRef?.click()}
                        class="flex items-center justify-center gap-2 px-4 py-2 border border-base-300 hover:border-base-content/50 hover:bg-base-200/50 text-base-content/70 text-sm font-medium rounded-lg transition-colors min-w-[120px]"
                      >
                        <Folder class="w-4 h-4" />
                        Upload Folder
                      </button>
                    </div>
                    
                    <p class="text-xs text-base-content/50">
                      Or drag and drop files anywhere in this area
                    </p>
                  </Show>
                  </div>
                }>
                  <div class="text-center text-base-content/50">
                    <p class="text-sm">No assets found matching "{searchQuery()}"</p>
                    <p class="text-xs text-base-content/40 mt-2">Try adjusting your search or upload new assets</p>
                  </div>
                </Show>
              </div>
            </Show>
            
            <Show when={filteredAssets().length > 0}>
              <Show when={layoutMode() === 'grid'} fallback={
                <div class="space-y-0">
                  <For each={filteredAssets()}>
                    {(asset, index) => renderAssetItem(asset, index)}
                  </For>
                </div>
              }>
                <div 
                  ref={assetGridRef}
                  class="grid grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-8 2xl:grid-cols-10 gap-3 relative"
                >
                  <For each={filteredAssets()}>
                    {(asset) => renderAssetItem(asset)}
                  </For>
                </div>
              </Show>
            </Show>
          </Show>
          
          <input
            ref={fileInputRef}
            type="file"
            multiple
            accept=".glb,.gltf,.obj,.fbx,.dae,.3ds,.blend,.max,.ma,.mb,.stl,.ply,.x3d,.jpg,.jpeg,.png,.gif,.webp,.bmp,.tga,.tiff,.ico,.svg,.mp3,.wav,.ogg,.m4a,.aac,.flac,.mp4,.avi,.mov,.mkv,.webm,.wmv,.js,.jsx,.ts,.tsx,.json,.xml,.txt,.md,.css,.html,.yml,.yaml,.csv,.log,.py,.ini,.conf,.cfg,.properties"
            onChange={handleFileInputChange}
            style={{ display: 'none' }}
          />
          
          <input
            ref={folderInputRef}
            type="file"
            webkitdirectory=""
            multiple
            onChange={handleFolderInputChange}
            style={{ display: 'none' }}
          />
          
          <Show when={contextMenu()}>
            <ContextMenu
              items={contextMenu().items}
              position={contextMenu().position}
              onClose={() => setContextMenu(null)}
            />
          </Show>

          <ScriptCreationDialog
            isOpen={showScriptDialog()}
            onClose={() => setShowScriptDialog(false)}
            onConfirm={handleConfirmCreateScript}
          />
          
          <Show when={isSelecting() && selectionRect()}>
            <div
              class="absolute border-2 border-primary bg-primary/10 pointer-events-none z-20"
              style={{
                left: `${selectionRect().x}px`,
                top: `${selectionRect().y}px`,
                width: `${selectionRect().width}px`,
                height: `${selectionRect().height}px`
              }}
            />
          </Show>
        </div>
      </div>
    </div>
  );
}

export default AssetLibrary;
