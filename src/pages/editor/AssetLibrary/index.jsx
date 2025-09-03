import { createSignal, createEffect, onMount, onCleanup, Show, createMemo, batch } from 'solid-js';
import { IconPhoto, IconWaveSawTool, IconFileText, IconFile, IconCube, IconVideo, IconCode, IconCircle, IconRectangle, IconGrid3x3, IconBulb, IconPlus, IconRefresh } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { assetsStore, assetsActions } from '@/layout/stores/AssetStore';
import { createContextMenuActions } from '@/ui/ContextMenuActions.jsx';
import ContextMenu from '@/ui/ContextMenu.jsx';
import ScriptCreationDialog from '../ScriptCreationDialog.jsx';
import { getCurrentProject, setCurrentProject, getProjects } from '@/api/bridge/projects';
import { getFileUrl, writeFile, writeBinaryFile, readFile, readBinaryFile, deleteFile, listDirectory } from '@/api/bridge/files';
import { generateThumbnail } from '@/api/bridge/thumbnails';

// Components
import AssetSidebar from './AssetSidebar';
import AssetHeader from './AssetHeader';
import AssetBreadcrumbs from './AssetBreadcrumbs';
import AssetUploadArea from './AssetUploadArea';
import AssetGrid from './AssetGrid';
import CodeEditorPanel from './CodeEditorPanel';

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
  const [loading, setLoading] = createSignal(false); // Start with false, only set true for initial load
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
  const [isCodeEditorOpen, setIsCodeEditorOpen] = createSignal(false);
  const [selectedFileForEdit, setSelectedFileForEdit] = createSignal(null);
  const [tooltip, setTooltip] = createSignal(null);
  
  let fileInputRef;
  let folderInputRef;
  let assetGridRef;
  let mainContentRef;

  const ui = () => editorStore.ui;
  const assetCache = () => assetsStore;
  const treePanelWidth = () => ui().assetsLibraryWidth && ui().assetsLibraryWidth > 100 ? ui().assetsLibraryWidth : 100;
  const { setAssetsLibraryWidth: setTreePanelWidth } = editorActions;
  const contextMenuActions = createContextMenuActions(editorActions);
  const { handleCreateObject } = contextMenuActions;
  const projectManager = getProjectManager();

  // Helper functions
  const isWindowsReservedName = (name) => {
    const reservedNames = [
      'con', 'prn', 'aux', 'nul',
      'com1', 'com2', 'com3', 'com4', 'com5', 'com6', 'com7', 'com8', 'com9',
      'lpt1', 'lpt2', 'lpt3', 'lpt4', 'lpt5', 'lpt6', 'lpt7', 'lpt8', 'lpt9'
    ];
    const baseName = name.toLowerCase().split('.')[0];
    return reservedNames.includes(baseName);
  };

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
        icon: <IconPhoto class="w-3 h-3" />,
        bgColor: 'bg-success', 
        hoverColor: 'hover:bg-success/80',
        textColor: 'text-white'
      };
    }
    
    if (['.mp3', '.wav', '.ogg', '.m4a'].includes(ext)) {
      return {
        icon: <IconWaveSawTool class="w-3 h-3" />,
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
        icon: <IconFileText class="w-3 h-3" />,
        bgColor: 'bg-info',
        hoverColor: 'hover:bg-info/80',
        textColor: 'text-white'
      };
    }
    
    return {
      icon: <IconFile class="w-3 h-3" />,
      bgColor: 'bg-base-300',
      hoverColor: 'hover:bg-base-300/80',
      textColor: 'text-base-content'
    };
  };

  // Bridge API functions
  const buildTreeFromAssets = (assets, projectName = null) => {
    if (!assets || !Array.isArray(assets)) {
      console.warn('buildTreeFromAssets: Invalid assets data:', assets);
      return null;
    }

    const folders = assets.filter(asset => asset.is_directory && !isWindowsReservedName(asset.name));
    const files = assets.filter(asset => !asset.is_directory && !isWindowsReservedName(asset.name));

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
      const rootFolders = processedFolders.filter(folder => {
        const folderDepth = folder.path.split('/').length;
        const parentDepth = parentPath ? parentPath.split('/').length : 0;
        
        if (parentPath) {
          return folder.path.startsWith(parentPath + '/') && folderDepth === parentDepth + 1;
        } else {
          return folderDepth === 1 || !folder.path.includes('/');
        }
      });

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
    
    if (!finalTree || finalTree.length === 0) {
      const simpleTree = processedFolders.map(folder => ({
        name: folder.name,
        path: folder.path,
        type: 'folder',
        children: [],
        files: []
      }));
      return simpleTree;
    }
    
    return finalTree;
  };

  const fetchFolderTree = async (currentProject) => {
    try {
      console.log('🌳 fetchFolderTree called with project:', currentProject);
      
      const projects = await getProjects();
      console.log('🌳 All projects:', projects);
      
      const currentProjectData = projects.find(p => p.name === currentProject.name);
      console.log('🌳 Current project data:', currentProjectData);
      
      if (currentProjectData && currentProjectData.files && currentProjectData.files.length > 0) {
        const tree = buildTreeFromAssets(currentProjectData.files, currentProject.name);
        console.log('🌳 Built tree from assets:', tree);
        return tree;
      }
      
      console.log('🌳 No files in project data, using listDirectory');
      const projectFiles = await listDirectory(`projects/${currentProject.name}`);
      console.log('🌳 Project files from listDirectory:', projectFiles);
      
      const buildNestedTree = async (items, parentPath = '') => {
        const tree = [];
        
        for (const item of items.filter(i => i.is_directory && !isWindowsReservedName(i.name))) {
          // Build the correct full path for this folder
          const fullPath = parentPath ? `${parentPath}/${item.name}` : item.name;
          
          try {
            const subItems = await listDirectory(`projects/${currentProject.name}/${fullPath}`);
            const children = await buildNestedTree(subItems, fullPath);
            const files = subItems.filter(subItem => !subItem.is_directory && !isWindowsReservedName(subItem.name));
            
            console.log('🔵 Building tree node - name:', item.name, 'path:', fullPath);
            tree.push({
              name: item.name,
              path: fullPath,
              type: 'folder',
              children: children,
              files: files
            });
          } catch (err) {
            tree.push({
              name: item.name,
              path: fullPath,
              type: 'folder',
              children: [],
              files: []
            });
          }
        }
        
        return tree;
      };
      
      const nestedTree = await buildNestedTree(projectFiles);
      console.log('🌳 Final nested tree:', nestedTree);
      return nestedTree;
      
    } catch (error) {
      console.error('🌳 Bridge API failed in fetchFolderTree:', error);
      return [];
    }
  };

  const fetchAssetCategories = async (currentProject) => {
    try {
      const allAssets = await listDirectory(`projects/${currentProject.name}`);
      
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
          extensions: ['.js', '.ts', '.jsx', '.tsx', '.json', '.ren'],
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
        if (asset.is_directory) return;
        
        const extension = asset.name.toLowerCase().match(/\.[^.]+$/)?.[0] || '';
        let categorized = false;
        
        for (const [key, category] of Object.entries(categories)) {
          if (key === 'other') continue;
          
          if (category.extensions.includes(extension)) {
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
      
      return categories;
      
    } catch (error) {
      console.warn('Bridge failed for categories, using fallback:', error);
      
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
    try {
      const dirPath = path 
        ? `projects/${currentProject.name}/${path}` 
        : `projects/${currentProject.name}`;
      
      const rawAssets = await listDirectory(dirPath);
      
      const assets = rawAssets
        .filter(asset => !isWindowsReservedName(asset.name))
        .map(asset => {
          const hasExtension = asset.name.includes('.') && !asset.is_directory;
          return {
            id: asset.path,
            name: asset.name,
            path: path ? `${path}/${asset.name}` : asset.name,
            type: asset.is_directory ? 'folder' : 'file',
            extension: hasExtension ? '.' + asset.name.split('.').pop() : null,
            size: asset.size || 0,
            fileName: asset.name
          };
        });
      
      return assets;
      
    } catch (error) {
      console.error('Bridge API failed:', error);
      return [];
    }
  };

  const searchAssets = async (currentProject, query) => {
    try {
      const allAssets = await listDirectory(`projects/${currentProject.name}`);
      const searchLower = query.toLowerCase();
      
      const results = allAssets.filter(asset => 
        asset.name.toLowerCase().includes(searchLower) && !isWindowsReservedName(asset.name)
      );
      
      return results;
      
    } catch (error) {
      console.error('Bridge API search failed:', error);
      return [];
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
      
      const sourceFileName = sourcePath.split('/').pop();
      
      if (isBinaryFile(sourceFileName)) {
        const base64Content = await readBinaryFile(fullSourcePath);
        await writeBinaryFile(fullTargetPath, base64Content);
      } else {
        const content = await readFile(fullSourcePath);
        await writeFile(fullTargetPath, content);
      }
      
      await deleteFile(fullSourcePath);
      return { success: true, sourcePath, targetPath };
    } catch (error) {
      throw new Error(`Failed to move item: ${error.message}`);
    }
  };

  const deleteAsset = async (currentProject, assetPath) => {
    try {
      const fullAssetPath = `projects/${currentProject.name}/assets/${assetPath}`;
      await deleteFile(fullAssetPath);
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

  // Asset fetching and caching
  const fetchAssetsWithCache = async (currentProject, path = '', forceRefresh = false, showLoading = true) => {
    const cachedAssets = !forceRefresh ? assetsActions.getAssetsForPath(path || currentPath()) : null;
    if (cachedAssets) {
      setAssets(cachedAssets);
      return;
    }

    try {
      // Only show loading for initial load or manual navigation
      if (showLoading) {
        setLoading(true);
      }
      setError(null);
      const assetsList = await fetchAssets(currentProject, path);
      
      assetsActions.setAssetsForPath(path || currentPath(), assetsList);
      setAssets(assetsList);
    } catch (error) {
      console.error('Failed to fetch assets:', error);
      setError(error.message);
      setAssets([]);
    } finally {
      if (showLoading) {
        setLoading(false);
      }
    }
  };

  // Incremental file change handler
  const handleIncrementalFileChanges = async (currentProject, changes) => {
    console.log('📝 Processing incremental file changes:', changes);
    
    const currentAssets = [...assets()];
    let updatedAssets = currentAssets;
    
    // Process all changes
    for (const change of changes) {
      const { event_type, paths } = change;
      
      for (const filePath of paths) {
        // Remove project prefix from path
        const relativePath = filePath.replace(`${currentProject.name}/`, '').replace(`${currentProject.name}\\`, '');
        
        // Check if this change affects the current directory
        const pathParts = relativePath.split(/[/\\]/);
        const fileName = pathParts.pop();
        const parentPath = pathParts.join('/');
        
        // Process changes in current directory
        if (parentPath === currentPath()) {
          console.log(`📝 Processing ${event_type} for ${fileName} in current directory`);
          
          if (event_type === 'create') {
            // Check if file already exists in list
            const existingIndex = updatedAssets.findIndex(a => a.name === fileName);
            if (existingIndex === -1) {
              // Determine if it's a file or folder by checking extension
              const hasExtension = fileName.includes('.');
              const newAsset = {
                id: relativePath,
                name: fileName,
                path: relativePath,
                type: hasExtension ? 'file' : 'folder',
                extension: hasExtension ? '.' + fileName.split('.').pop() : null,
                size: 0,
                fileName: fileName
              };
              updatedAssets = [...updatedAssets, newAsset];
              console.log('📝 Added new asset:', newAsset);
            }
            
          } else if (event_type === 'delete' || event_type === 'remove') {
            // Remove the file from the list
            updatedAssets = updatedAssets.filter(a => a.name !== fileName);
            console.log('📝 Removed asset:', fileName);
            
          } else if (event_type === 'modify') {
            // For modifications, update the size or other metadata if needed
            const assetIndex = updatedAssets.findIndex(a => a.name === fileName);
            if (assetIndex !== -1) {
              // Just trigger a re-render by creating a new object
              updatedAssets = [
                ...updatedAssets.slice(0, assetIndex),
                { ...updatedAssets[assetIndex] },
                ...updatedAssets.slice(assetIndex + 1)
              ];
              console.log('📝 Modified asset:', fileName);
            }
          }
        }
      }
    }
    
    // Update the assets list with the changes
    setAssets(updatedAssets);
    
    // Update cache with new asset list
    assetsActions.setAssetsForPath(currentPath(), updatedAssets);
    
  };
  
  // Update folder tree without full reload
  const updateFolderTreeIncrementally = async (currentProject, affectedPaths = []) => {
    try {
      // For immediate UI update, we can update the tree locally
      const currentTree = folderTree();
      
      if (currentTree) {
        // Keep existing tree without updating file counts
        console.log('📊 Keeping existing folder tree');
      } else {
        // Fallback to fetching new tree if we don't have one
        console.log('📊 Fetching new folder tree');
        const newTree = await fetchFolderTree(currentProject);
        
        batch(() => {
          setFolderTree(newTree);
          assetsActions.setFolderTree(newTree);
        });
      }
    } catch (error) {
      console.error('Failed to update folder tree:', error);
    }
  };
  

  // Event handlers
  const handleFileChange = async (changeData) => {
    console.log('File change detected:', changeData);
    
    if (changeData.message) {
      const message = changeData.message.toLowerCase();
      if (message.includes('.tmp.') || message.includes('%') || message.includes('~')) {
        console.log('Ignoring temporary/system file change');
        return;
      }
    }
    
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) return;
    
    // For manual refresh, do a full refresh but don't show loading
    if (changeData.source === 'manual-button') {
      assetsActions.clearAllAssetCache();
      await Promise.all([
        fetchAssetsWithCache(currentProject, currentPath(), true, false), // Don't show loading
        updateFolderTreeIncrementally(currentProject),
        (async () => {
          const categories = await fetchAssetCategories(currentProject);
          setAssetCategories(categories);
        })()
      ]);
      return;
    }
    
    // For SSE file changes, do incremental updates
    if (changeData.changes && Array.isArray(changeData.changes)) {
      await handleIncrementalFileChanges(currentProject, changeData.changes);
    } else {
      // Fallback: refresh current directory and update tree
      console.log('📡 Fallback: Refreshing due to file change without details');
      assetsActions.invalidateAssetPaths([currentPath()]);
      
      // Refresh assets and tree in parallel
      await Promise.all([
        fetchAssetsWithCache(currentProject, currentPath(), true, false), // Force refresh, no loading
        updateFolderTreeIncrementally(currentProject)
      ]);
    }
  };

  const handleResizeMouseDown = (e) => {
    setIsResizing(true);
    document.body.classList.add('dragging-horizontal');
    e.preventDefault();
  };

  const handleResizeMouseMove = (e) => {
    if (!isResizing()) return;
    const newWidth = e.clientX;
    setTreePanelWidth(Math.max(100, Math.min(400, newWidth)));
  };

  const handleResizeMouseUp = () => {
    setIsResizing(false);
    document.body.classList.remove('dragging-horizontal');
  };

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

    try {
      for (const file of files) {
        const targetPath = currentPath() 
          ? `projects/${currentProject.name}/assets/${currentPath()}/${file.name}`
          : `projects/${currentProject.name}/assets/${file.name}`;
        
        if (isBinaryFile(file.name)) {
          const reader = new FileReader();
          const base64 = await new Promise((resolve, reject) => {
            reader.onload = () => {
              const base64String = reader.result.split(',')[1];
              resolve(base64String);
            };
            reader.onerror = reject;
            reader.readAsDataURL(file);
          });
          await writeBinaryFile(targetPath, base64);
        } else {
          const text = await file.text();
          await writeFile(targetPath, text);
        }
        
        console.log('Successfully uploaded:', file.name);
      }
      
      await fetchAssetsWithCache(currentProject, currentPath());
      
    } catch (error) {
      console.error('Error uploading files:', error);
      setError(`Failed to upload files: ${error.message}`);
    } finally {
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
    
    if (!isInternalDrag()) {
      const files = Array.from(e.dataTransfer.files);
      if (files.length > 0) {
        uploadFiles(files);
      }
    }
  };

  const handleContextMenu = (e) => {
    e.preventDefault();
    e.stopPropagation();
    
    const contextMenuItems = [
      {
        label: 'Create Object',
        action: () => {},
        icon: <IconPlus class="w-4 h-4" />,
        submenu: [
          { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconCube class="w-4 h-4" /> },
          { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
          { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconRectangle class="w-4 h-4" /> },
          { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconGrid3x3 class="w-4 h-4" /> },
          { separator: true },
          { label: 'Light', action: () => handleCreateObject('light'), icon: <IconBulb class="w-4 h-4" /> },
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <IconVideo class="w-4 h-4" /> },
        ]
      },
      { separator: true },
      {
        label: 'Create Script',
        action: () => setShowScriptDialog(true),
        icon: <IconCode class="w-4 h-4" />
      }
    ];
    
    if (onContextMenu) {
      onContextMenu(e, contextMenuItems);
    } else {
      setContextMenu({
        items: contextMenuItems,
        position: { x: e.clientX, y: e.clientY }
      });
    }
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
    }
  };

  const handleMoveMultipleItems = async (assets, targetFolderPath) => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) {
      console.error('No project loaded for move operation');
      return;
    }

    try {
      const pathsToInvalidate = new Set();
      
      for (const asset of assets) {
        const sourceFileName = asset.name || asset.path.split('/').pop();
        const targetPath = targetFolderPath ? `${targetFolderPath}/${sourceFileName}` : sourceFileName;
        
        await moveAsset(currentProject, asset.path, targetPath);
        
        const sourceFolderPath = asset.path.split('/').slice(0, -1).join('/');
        pathsToInvalidate.add(sourceFolderPath);
      }
      
      pathsToInvalidate.add(targetFolderPath);
      assetsActions.invalidateAssetPaths(Array.from(pathsToInvalidate));
      
      await fetchAssetsWithCache(currentProject, currentPath());
      clearSelection();
    } catch (error) {
      console.error('Error moving multiple items:', error);
    }
  };

  const handleConfirmCreateScript = async (scriptName, scriptTemplate) => {
    if (!scriptName || !scriptTemplate) {
      console.error('Script name and template are required');
      return;
    }

    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) {
      console.error('No project loaded for script creation');
      return;
    }

    try {
      const targetPath = currentPath() 
        ? `projects/${currentProject.name}/scripts/${currentPath()}/${scriptName}.ren`
        : `projects/${currentProject.name}/scripts/${scriptName}.ren`;
      
      await writeFile(targetPath, scriptTemplate);
      console.log('Successfully created script:', scriptName);
      
      await fetchAssetsWithCache(currentProject, currentPath());
      
    } catch (error) {
      console.error('Error creating script:', error);
      setError(`Failed to create script: ${error.message}`);
    }
  };

  const handleFolderClick = async (folderPath) => {
    console.log('🔴 Folder clicked:', folderPath);
    setCurrentPath(folderPath);
    
    // Immediately fetch assets for the new path without showing loading
    const currentProject = projectManager.getCurrentProject();
    if (currentProject?.name) {
      await fetchAssetsWithCache(currentProject, folderPath, false, false); // No cache check, no loading
    }
  };

  const handleFolderToggle = (folderPath) => {
    toggleFolderExpansion(folderPath);
  };

  const handleBreadcrumbClick = async (path) => {
    console.log('Breadcrumb clicked:', path);
    setCurrentPath(path);
    
    // Immediately fetch assets for the new path without showing loading
    const currentProject = projectManager.getCurrentProject();
    if (currentProject?.name) {
      await fetchAssetsWithCache(currentProject, path, false, false); // No cache check, no loading
    }
  };

  const handleAssetDoubleClick = (asset) => {
    if (asset.type === 'folder') {
      handleFolderClick(asset.path);
      return;
    }
    
    const isTextFile = asset.extension && ['.js', '.ts', '.jsx', '.tsx', '.json', '.txt', '.md', '.ren', '.html', '.css', '.xml', '.yaml', '.yml'].includes(asset.extension.toLowerCase());
    
    if (isTextFile) {
      setSelectedFileForEdit(asset);
      setIsCodeEditorOpen(true);
      console.log('Opening file in code editor:', asset.name);
    } else {
      console.log('Double-clicked on:', asset.name, 'Type:', asset.extension);
    }
  };

  const handleCodeEditorToggle = () => {
    setIsCodeEditorOpen(!isCodeEditorOpen());
    // Don't clear the selected file when opening, only when closing
  };

  const handleCodeEditorClose = () => {
    setIsCodeEditorOpen(false);
    setSelectedFileForEdit(null);
  };

  const handleImportClick = () => {
    document.dispatchEvent(new CustomEvent('engine:open-model-importer'));
  };

  // Selection handling
  const startDragSelection = (e) => {
    const target = e.target;
    
    const isInteractiveElement = target.closest('button, input, a, select, textarea');
    if (isInteractiveElement) return;
    
    const isAssetElement = target.closest('[data-asset-id]');
    if (isAssetElement) return;
    
    const isDraggableElement = target.closest('[draggable="true"]');
    if (isDraggableElement) return;
    
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

  const updateDragSelection = (e) => {
    if (!isSelecting() || !mainContentRef) return;
    
    const containerRect = mainContentRef.getBoundingClientRect();
    let currentPos = {
      x: e.clientX - containerRect.left,
      y: e.clientY - containerRect.top
    };
    
    currentPos.x = Math.max(0, Math.min(currentPos.x, containerRect.width));
    currentPos.y = Math.max(0, Math.min(currentPos.y, containerRect.height));
    
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
      
      const isIntersecting = (
        selectionBox.x < relativeRect.x + relativeRect.width &&
        selectionBox.x + selectionBox.width > relativeRect.x &&
        selectionBox.y < relativeRect.y + relativeRect.height &&
        selectionBox.y + selectionBox.height > relativeRect.y
      );
      
      if (isIntersecting) {
        const assetId = element.getAttribute('data-asset-id');
        if (assetId) {
          newSelected.add(assetId);
        }
      }
    });
    
    setSelectedAssets(newSelected);
  };

  const endDragSelection = () => {
    setIsSelecting(false);
    setSelectionRect(null);
    setSelectionStart(null);
  };

  const handleKeyDown = async (e) => {
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
      e.preventDefault();
      const currentProject = projectManager.getCurrentProject();
      if (!currentProject?.name) {
        console.error('No project loaded for delete operation');
        return;
      }

      const assetsToDelete = filteredAssets().filter(asset => selectedAssets().has(asset.id));
      let deletedCount = 0;
      let failedCount = 0;
      const failedFiles = [];
      
      for (const asset of assetsToDelete) {
        try {
          // Use the correct path structure - assets are stored directly in the project folder
          const fullAssetPath = `projects/${currentProject.name}/${asset.path}`;
          await deleteFile(fullAssetPath);
          console.log('Deleted asset:', asset.name);
          deletedCount++;
        } catch (error) {
          console.error('Failed to delete asset:', asset.name, error);
          failedCount++;
          failedFiles.push(asset.name);
        }
      }
      
      clearSelection();
      await fetchAssetsWithCache(currentProject, currentPath(), true, false);
      
      if (failedCount > 0) {
        setError(`Deleted ${deletedCount} files. Failed to delete ${failedCount} files with special characters: ${failedFiles.join(', ')}`);
      } else if (deletedCount > 0) {
        console.log(`Successfully deleted ${deletedCount} files`);
      }
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

  // Tree drag and drop handlers
  const handleTreeDragOver = (path) => {
    setDragOverTreeFolder(path);
  };

  const handleTreeDragEnter = (path) => {
    setDragOverTreeFolder(path);
  };

  const handleTreeDragLeave = () => {
    setDragOverTreeFolder(null);
  };

  const handleTreeDrop = (e, path) => {
    setDragOverTreeFolder(null);
    
    try {
      const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
      
      if (dragData.type === 'multiple-assets') {
        const validAssets = dragData.assets.filter(asset => {
          if (asset.assetType === 'folder' && path.startsWith(asset.path)) {
            console.warn(`Cannot move folder ${asset.name} into itself or its children`);
            return false;
          }
          return asset.path !== path;
        });
        
        if (validAssets.length > 0) {
          handleMoveMultipleItems(validAssets, path);
        }
      } else if (dragData.type === 'asset' && dragData.path !== path) {
        if (dragData.assetType === 'folder' && path.startsWith(dragData.path)) {
          console.warn('Cannot move folder into itself or its children');
          return;
        }
        handleMoveItem(dragData.path, path);
      }
    } catch (error) {
      console.error('Error parsing drag data in tree:', error);
    }
  };

  const handleBreadcrumbDrop = (e, path) => {
    setDragOverBreadcrumb(null);
    
    try {
      const dragData = JSON.parse(e.dataTransfer.getData('application/json'));
      
      if (dragData.type === 'multiple-assets') {
        const validAssets = dragData.assets.filter(asset => {
          if (asset.assetType === 'folder' && path.startsWith(asset.path)) {
            console.warn(`Cannot move folder ${asset.name} into itself or its children`);
            return false;
          }
          return asset.path !== path;
        });
        
        if (validAssets.length > 0) {
          handleMoveMultipleItems(validAssets, path);
        }
      } else if (dragData.type === 'asset' && dragData.path !== path) {
        if (dragData.assetType === 'folder' && path.startsWith(dragData.path)) {
          console.warn('Cannot move folder into itself or its children');
          return;
        }
        handleMoveItem(dragData.path, path);
      }
    } catch (error) {
      console.error('Error parsing drag data in breadcrumb:', error);
    }
  };

  // Computed values
  const breadcrumbs = createMemo(() => {
    console.log('🟢 Computing breadcrumbs - viewMode:', viewMode(), 'currentPath:', currentPath());
    
    if (viewMode() !== 'folder') {
      console.log('🟢 Not folder view, returning empty');
      return [];
    }
    
    // Use projectManager directly instead of the local signal
    const project = projectManager.getCurrentProject();
    console.log('🟢 Project from manager:', project);
    
    if (!project?.name) {
      console.log('🟢 No project, returning empty');
      return [];
    }
    
    const parts = currentPath() ? currentPath().split('/') : [];
    console.log('🟢 Path parts:', parts);
    
    const crumbs = [{ name: project.name, path: '' }];
    
    let currentBreadcrumbPath = '';
    for (const part of parts) {
      currentBreadcrumbPath = currentBreadcrumbPath ? `${currentBreadcrumbPath}/${part}` : part;
      crumbs.push({ name: part, path: currentBreadcrumbPath });
    }
    
    console.log('🟢 Final breadcrumbs:', crumbs);
    return crumbs;
  });

  const getCategoryIcon = (categoryId) => {
    const iconMap = {
      '3d-models': IconCube,
      'textures': IconPhoto,
      'audio': IconWaveSawTool,
      'video': IconVideo,
      'scripts': IconCode,
      'documents': IconFileText,
      'other': IconFile
    };
    return iconMap[categoryId] || IconFile;
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
    const allAssets = assets(); // Include both files and folders
    
    if (!searchQuery()) return allAssets;
    
    if (globalSearchResults().length > 0) {
      return globalSearchResults(); // Include both files and folders in search results
    }
    
    return allAssets.filter(asset => {
      const matchesSearch = asset.name.toLowerCase().includes(searchQuery().toLowerCase()) ||
                           asset.fileName?.toLowerCase().includes(searchQuery().toLowerCase());
      return matchesSearch;
    });
  });

  // Effects
  createEffect(() => {
    if (viewMode() === 'type') {
      const currentProject = projectManager.getCurrentProject();
      if (!currentProject?.name) return;
      
      const categories = assetCategories();
      if (!categories) return;
      
      const category = categories[selectedCategory()];
      if (category && category.assets) {
        setAssets(category.assets);
        setLoading(false);
      }
    }
  });

  createEffect(() => {
    const currentProject = projectManager.getCurrentProject();
    if (!currentProject?.name) return;
    
    if (viewMode() === 'folder') {
      fetchAssetsWithCache(currentProject, currentPath());
    }
  });

  createEffect(() => {
    const query = searchQuery();
    if (!query || query.length < 2) {
      setGlobalSearchResults([]);
      setIsSearching(false);
      return;
    }
    
    const debounceTimer = setTimeout(async () => {
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
          console.error('Search failed:', error);
          setGlobalSearchResults([]);
        } finally {
          setIsSearching(false);
        }
      };
      
      performGlobalSearch();
    }, 300);
    
    onCleanup(() => clearTimeout(debounceTimer));
  });

  // Function to initialize project data
  const initializeProjectData = async (projectData) => {
    console.log('🟠 Initializing project:', projectData);
    
    if (!projectData?.name) {
      console.log('🟠 No valid project data to initialize');
      return;
    }
    
    // Only show loading if this is the first project load
    const isFirstLoad = !currentProject()?.name;
    if (isFirstLoad) {
      setLoading(true);
    }
    
    setCurrentProject(projectData);
    
    try {
      const [tree, categories] = await Promise.all([
        fetchFolderTree(projectData),
        fetchAssetCategories(projectData)
      ]);
      
      console.log('🌲 Folder tree fetched:', tree);
      console.log('📂 Categories fetched:', categories);
      
      batch(() => {
        setFolderTree(tree);
        assetsActions.setFolderTree(tree);
        setAssetCategories(categories);
        if (isFirstLoad) {
          setLoading(false);
        }
      });
      
      console.log('🌲 Folder tree after setting:', folderTree());
      
      await fetchAssetsWithCache(projectData, '', false, isFirstLoad);
    } catch (error) {
      console.error('Failed to load initial data:', error);
      setError(error.message);
      if (isFirstLoad) {
        setLoading(false);
      }
    }
  };

  // Initialize
  onMount(async () => {
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('mousemove', handleGlobalMouseMove);
    document.addEventListener('mouseup', handleGlobalMouseUp);
    document.addEventListener('mousemove', handleResizeMouseMove);
    document.addEventListener('mouseup', handleResizeMouseUp);

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
              
              if (data.type === 'file-changes') {
                console.log('📡 AssetLibrary: File changes detected:', data.changes);
                handleFileChange({ source: 'sse', changes: data.changes });
              } else if (data.type === 'file_change') {
                // Legacy format support
                console.log('📡 AssetLibrary: File change detected (legacy):', data.message);
                handleFileChange({ source: 'sse', message: data.message });
              } else if (data.type === 'file-change') {
                // Another possible format
                console.log('📡 AssetLibrary: File change detected (alt format):', data);
                handleFileChange({ source: 'sse', message: data.message || data.path });
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

    // Listen for project selection events
    const handleProjectSelection = (event) => {
      console.log('🟠 Project selection event received:', event.detail);
      const projectData = projectManager.getCurrentProject();
      initializeProjectData(projectData);
    };
    
    // Listen for asset refresh events from model importer
    const handleAssetsRefresh = async () => {
      const currentProject = projectManager.getCurrentProject();
      if (currentProject?.name) {
        // Refresh assets after import
        await Promise.all([
          fetchAssetsWithCache(currentProject, currentPath(), true, false),
          updateFolderTreeIncrementally(currentProject),
          (async () => {
            const categories = await fetchAssetCategories(currentProject);
            setAssetCategories(categories);
          })()
        ]);
      }
    };

    document.addEventListener('engine:project-selected', handleProjectSelection);
    document.addEventListener('engine:assets-refresh', handleAssetsRefresh);
    
    // Check if project is already selected
    const initialProject = projectManager.getCurrentProject();
    console.log('🟠 Initial project data in onMount:', initialProject);
    
    if (initialProject?.name) {
      initializeProjectData(initialProject);
    } else {
      console.log('🟠 No project on mount, waiting for project selection...');
    }
    
    onCleanup(() => {
      document.removeEventListener('engine:project-selected', handleProjectSelection);
      document.removeEventListener('engine:assets-refresh', handleAssetsRefresh);
    });

  });

  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown);
    document.removeEventListener('mousemove', handleGlobalMouseMove);
    document.removeEventListener('mouseup', handleGlobalMouseUp);
    document.removeEventListener('mousemove', handleResizeMouseMove);
    document.removeEventListener('mouseup', handleResizeMouseUp);
  });

  createEffect(() => {
    const handleClickOutside = (e) => {
      if (!isSelecting() && mainContentRef) {
        // Only clear selection if clicking outside the main content area
        if (!mainContentRef.contains(e.target)) {
          clearSelection();
        }
      }
    };
    
    document.addEventListener('click', handleClickOutside);
    
    onCleanup(() => {
      document.removeEventListener('click', handleClickOutside);
    });
  });

  return (
    <div class="h-full flex bg-base-200 no-select overflow-hidden">
      <AssetSidebar
        treePanelWidth={treePanelWidth}
        isResizing={isResizing}
        viewMode={viewMode}
        setViewMode={setViewMode}
        searchQuery={searchQuery}
        setSearchQuery={setSearchQuery}
        isSearching={isSearching}
        folderTree={folderTree}
        categoryList={categoryList}
        selectedCategory={selectedCategory}
        setSelectedCategory={setSelectedCategory}
        expandedFolders={expandedFolders}
        currentPath={currentPath}
        dragOverTreeFolder={dragOverTreeFolder}
        isInternalDrag={isInternalDrag}
        error={error}
        onFolderClick={handleFolderClick}
        onFolderToggle={handleFolderToggle}
        onTreeDragOver={handleTreeDragOver}
        onTreeDragEnter={handleTreeDragEnter}
        onTreeDragLeave={handleTreeDragLeave}
        onTreeDrop={handleTreeDrop}
        onResizeMouseDown={handleResizeMouseDown}
        onRefresh={() => {
          console.log('Manual refresh triggered via button');
          handleFileChange({ source: 'manual-button' });
        }}
      />
      
      <div 
        class={`flex-1 flex flex-col transition-all duration-200 relative overflow-hidden ${
          isDragOver() ? 'bg-primary/20 border-2 border-primary border-dashed' : 'bg-base-200'
        }`}
      >
        <div class="bg-base-200 flex-shrink-0 border-b border-base-300">
          <div class="flex items-center justify-between pr-3 py-2">
            <div class="flex items-center gap-2 ml-2">
              <button
                onClick={() => {
                  console.log('Manual refresh triggered via button');
                  handleFileChange({ source: 'manual-button' });
                }}
                class="p-1 text-xs rounded bg-base-300/70 text-base-content/60 hover:text-base-content hover:bg-base-300/90 transition-colors opacity-80"
                title="Refresh Assets"
              >
                <IconRefresh class="w-3 h-3" />
              </button>
              
              <AssetBreadcrumbs
                breadcrumbs={breadcrumbs}
                viewMode={viewMode}
                selectedCategory={selectedCategory}
                assetCategories={assetCategories}
                onBreadcrumbClick={handleBreadcrumbClick}
                dragOverBreadcrumb={dragOverBreadcrumb}
                setDragOverBreadcrumb={setDragOverBreadcrumb}
                isInternalDrag={isInternalDrag}
                onBreadcrumbDrop={handleBreadcrumbDrop}
              />
            </div>
            
            <AssetHeader
              selectedAssets={selectedAssets}
              filteredAssets={filteredAssets}
              isUploading={isUploading}
              layoutMode={layoutMode}
              setLayoutMode={setLayoutMode}
              onCodeToggle={handleCodeEditorToggle}
              isCodeEditorOpen={isCodeEditorOpen}
              onImport={handleImportClick}
            />
          </div>
        </div>
        
        <div class="flex-1 flex overflow-hidden">
          {/* Show Assets Panel when code editor is closed */}
          <Show when={!isCodeEditorOpen()}>
            <div 
              ref={mainContentRef}
              class="flex flex-col p-3 overflow-y-auto overflow-x-hidden scrollbar-thin relative user-select-none w-full"
              onDragOver={handleDragOver}
              onDragEnter={handleDragEnter}
              onDragLeave={handleDragLeave}
              onDrop={handleDrop}
              onContextMenu={handleContextMenu}
              onMouseDown={startDragSelection}
            >
              <AssetUploadArea
                isDragOver={isDragOver}
                isUploading={isUploading}
                loading={loading}
                error={error}
                filteredAssets={filteredAssets}
                searchQuery={searchQuery}
                viewMode={viewMode}
                selectedCategory={selectedCategory}
                assetCategories={assetCategories}
                fileInputRef={fileInputRef}
                folderInputRef={folderInputRef}
                onFileInputChange={handleFileInputChange}
                onFolderInputChange={handleFolderInputChange}
              />
              
              <AssetGrid
                layoutMode={layoutMode}
                filteredAssets={filteredAssets}
                assetGridRef={assetGridRef}
                isAssetSelected={isAssetSelected}
                hoveredItem={hoveredItem}
                setHoveredItem={setHoveredItem}
                setTooltip={setTooltip}
                toggleAssetSelection={toggleAssetSelection}
                handleAssetDoubleClick={handleAssetDoubleClick}
                isInternalDrag={isInternalDrag}
                setIsInternalDrag={setIsInternalDrag}
                selectedAssets={selectedAssets}
                setSelectedAssets={setSelectedAssets}
                lastSelectedAsset={lastSelectedAsset}
                setLastSelectedAsset={setLastSelectedAsset}
                setDragOverFolder={setDragOverFolder}
                setDragOverTreeFolder={setDragOverTreeFolder}
                setDragOverBreadcrumb={setDragOverBreadcrumb}
                loadedAssets={loadedAssets}
                preloadingAssets={preloadingAssets}
                failedAssets={failedAssets}
                setFailedAssets={setFailedAssets}
                setPreloadingAssets={setPreloadingAssets}
                setLoadedAssets={setLoadedAssets}
                getExtensionStyle={getExtensionStyle}
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
          </Show>
          
          {/* Show Code Editor Panel when editor is open - replaces the asset grid entirely */}
          <Show when={isCodeEditorOpen()}>
            <div class="w-full h-full">
              <CodeEditorPanel
                isOpen={isCodeEditorOpen}
                onClose={handleCodeEditorClose}
                selectedFile={selectedFileForEdit}
                width="100%" // Full width
              />
            </div>
          </Show>
        </div>
        
        {/* Unreal Engine style footer */}
        <div class="bg-base-200 border-t border-base-300 px-3 py-1.5 flex items-center justify-between text-xs text-base-content/60">
          <div class="flex items-center gap-3">
            <span>{filteredAssets().length} items</span>
            <span>•</span>
            <span>{selectedAssets().size} selected</span>
          </div>
          <div class="flex items-center gap-2">
            <span class="text-base-content/40">View:</span>
            <span>{layoutMode()}</span>
          </div>
        </div>
        
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

        {/* Global Tooltip */}
        <Show when={tooltip()}>
          <div class="fixed z-[99999] bg-red-500 text-white text-xs p-3 w-48 pointer-events-none rounded shadow-xl" 
               style={`left: ${tooltip().x}px; top: ${tooltip().y}px;`}>
            <div class="font-semibold mb-1">{tooltip().asset.name}</div>
            <div>Type: {tooltip().asset.extension?.toUpperCase() || 'Unknown'}</div>
            <div>Size: {tooltip().asset.size ? `${Math.round(tooltip().asset.size / 1024)} KB` : 'Unknown'}</div>
          </div>
        </Show>
        
      </div>
    </div>
  );
}

export default AssetLibrary;