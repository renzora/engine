import { createSignal, createEffect } from 'solid-js';
import { bridgeService, bridgeService as projects } from '@/plugins/core/bridge';

export function createAssetAPI() {
  const [isInitialized, setIsInitialized] = createSignal(true);

  createEffect(() => {
    // Removed Electron initialization
  });

  // Helper function to build tree structure from Rust bridge data
  const buildTreeFromAssets = (assets, projectName = null) => {
    if (!assets || !Array.isArray(assets)) {
      console.warn('buildTreeFromAssets: Invalid assets data:', assets);
      return null;
    }

    // Convert Rust bridge data format to expected format
    const folders = assets.filter(asset => asset.is_directory);
    const files = assets.filter(asset => !asset.is_directory);

    // If we have a project name, filter out paths that contain the project name
    // and adjust the remaining paths to be relative to the project
    let processedFolders = folders;
    let processedFiles = files;
    
    if (projectName) {
      processedFolders = folders.map(folder => {
        // Remove the project name from the path if it exists (handle both forward and back slashes)
        let adjustedPath = folder.path;
        const projectPrefix1 = `projects/${projectName}/`;
        const projectPrefix2 = `projects\\${projectName}\\`;
        const projectRoot1 = `projects/${projectName}`;
        const projectRoot2 = `projects\\${projectName}`;
        
        if (adjustedPath.startsWith(projectPrefix1)) {
          adjustedPath = adjustedPath.replace(projectPrefix1, '');
        } else if (adjustedPath.startsWith(projectPrefix2)) {
          adjustedPath = adjustedPath.replace(projectPrefix2, '');
        } else if (adjustedPath === projectRoot1 || adjustedPath === projectRoot2) {
          // Skip the project root folder itself
          return null;
        }
        
        // Convert backslashes to forward slashes for consistency
        adjustedPath = adjustedPath.replace(/\\/g, '/');
        
        return {
          ...folder,
          path: adjustedPath
        };
      }).filter(Boolean); // Remove null entries

      processedFiles = files.map(file => {
        // Remove the project name from the path if it exists
        let adjustedPath = file.path;
        const projectPrefix1 = `projects/${projectName}/`;
        const projectPrefix2 = `projects\\${projectName}\\`;
        
        if (adjustedPath.startsWith(projectPrefix1)) {
          adjustedPath = adjustedPath.replace(projectPrefix1, '');
        } else if (adjustedPath.startsWith(projectPrefix2)) {
          adjustedPath = adjustedPath.replace(projectPrefix2, '');
        }
        
        // Convert backslashes to forward slashes for consistency
        adjustedPath = adjustedPath.replace(/\\/g, '/');
        
        return {
          ...file,
          path: adjustedPath
        };
      });
    }

    // Build hierarchical tree structure
    const buildTree = (parentPath = '') => {
      const rootFolders = processedFolders.filter(folder => {
        const folderDepth = folder.path.split('/').length;
        const parentDepth = parentPath ? parentPath.split('/').length : 0;
        
        // Check if this folder is a direct child of the parent
        if (parentPath) {
          return folder.path.startsWith(parentPath + '/') && folderDepth === parentDepth + 1;
        } else {
          // Root level folders (no slashes or exactly one level deep)
          return folderDepth === 1 || !folder.path.includes('/');
        }
      });

      return rootFolders.map(folder => {
        // Get direct child files
        const folderFiles = processedFiles.filter(file => {
          const filePath = file.path.substring(0, file.path.lastIndexOf('/')) || '';
          return filePath === folder.path;
        });

        // Recursively build children
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

    return buildTree();
  };

  const fetchFolderTree = async (currentProject) => {
    console.log('🦀 Using Rust bridge for folder tree, project:', currentProject.name);
    
    try {
      // Use Rust bridge for direct file system access
      const projects = await bridgeService.getProjects();
      console.log('🦀 All projects from bridge:', projects);
      const currentProjectData = projects.find(p => p.name === currentProject.name);
      console.log('🦀 Current project data:', currentProjectData);
      
      if (currentProjectData && currentProjectData.files) {
        // Convert flat list to tree structure
        const tree = buildTreeFromAssets(currentProjectData.files, currentProject.name);
        console.log('🦀 Built tree from project files:', tree);
        return tree;
      }
      
      // Fallback: list project assets directory directly
      console.log('🦀 Falling back to listing project assets directory directly');
      const projectFiles = await bridgeService.listDirectory(`projects/${currentProject.name}/assets`);
      console.log('🦀 Project files from direct listing:', projectFiles);
      const tree = buildTreeFromAssets(projectFiles, currentProject.name);
      console.log('🦀 Built tree from direct listing:', tree);
      return tree;
      
    } catch (error) {
      console.error('🦀 Rust bridge failed:', error);
      // Return empty array as fallback
      return [];
    }
  };

  const fetchAssetCategories = async (currentProject) => {
    console.log('🦀 Generating asset categories from bridge data');
    
    try {
      // Use Rust bridge to get all files and categorize them
      const allAssets = await bridgeService.listDirectory(`projects/${currentProject.name}/assets`);
      console.log('🦀 RAW ASSETS FROM BRIDGE:', allAssets);
      console.log('🦀 TOTAL ASSET COUNT:', allAssets.length);
      
      // Categorize assets by file extension
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
      
      // Categorize assets
      allAssets.forEach(asset => {
        console.log('🦀 PROCESSING ASSET:', asset.name, 'is_directory:', asset.is_directory);
        
        if (asset.is_directory) {
          // Skip folders - only show them in the left tree panel
          console.log('🦀 SKIPPING FOLDER:', asset.name);
          return;
        }
        
        const extension = asset.name.toLowerCase().match(/\.[^.]+$/)?.[0] || '';
        console.log('🦀 FILE EXTENSION:', extension, 'for', asset.name);
        let categorized = false;
        
        for (const [key, category] of Object.entries(categories)) {
          if (key === 'other') continue; // Handle 'other' last
          
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
        
        // If not categorized, add to 'other'
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
      
      // Fallback: return empty categories
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
    console.log('🦀 Using Rust bridge for assets in path:', path);
    
    try {
      // Use Rust bridge for direct file system access
      const dirPath = path 
        ? `projects/${currentProject.name}/assets/${path}` 
        : `projects/${currentProject.name}/assets`;
      
      console.log('🦀 Requesting directory:', dirPath);
      const rawAssets = await bridgeService.listDirectory(dirPath);
      console.log('🦀 Got raw assets from Rust bridge:', rawAssets.length, rawAssets);
      
      // Convert Rust bridge format to expected AssetLibrary format
      const assets = rawAssets.map(asset => {
        console.log(`🦀 Processing asset: ${asset.name}, is_directory: ${asset.is_directory}, type: ${typeof asset.is_directory}`);
        const hasExtension = asset.name.includes('.') && !asset.is_directory;
        const convertedAsset = {
          id: asset.path,
          name: asset.name,
          path: path ? `${path}/${asset.name}` : asset.name, // Relative path
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
      console.error('🦀 Rust bridge failed:', error);
      return [];
    }
  };

  const searchAssets = async (currentProject, query) => {
    console.log('🦀 Using Rust bridge for asset search');
    
    try {
      // Use Rust bridge to get all files and search client-side
      const allAssets = await bridgeService.listDirectory(`projects/${currentProject.name}/assets`);
      const searchLower = query.toLowerCase();
      
      const results = allAssets.filter(asset => 
        asset.name.toLowerCase().includes(searchLower)
      );
      
      console.log('🦀 Found', results.length, 'assets matching search');
      return results;
      
    } catch (error) {
      console.error('🦀 Rust bridge search failed:', error);
      return [];
    }
  };

  const createFolder = async (currentProject, folderName, parentPath = '') => {
    try {
      const folderPath = parentPath 
        ? `projects/${currentProject.name}/assets/${parentPath}/${folderName.trim()}`
        : `projects/${currentProject.name}/assets/${folderName.trim()}`;
      
      // Create folder by writing an empty file and then deleting it (to create the directory structure)
      await bridgeService.writeFile(`${folderPath}/.gitkeep`, '');
      console.log('🦀 Created folder:', folderPath);
      return { success: true, path: folderPath };
    } catch (error) {
      throw new Error(`Failed to create folder: ${error.message}`);
    }
  };

  // Helper function to detect binary files
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

  const moveAsset = async (currentProject, sourcePath, targetPath, assetType = null) => {
    try {
      const fullSourcePath = `projects/${currentProject.name}/assets/${sourcePath}`;
      const fullTargetPath = `projects/${currentProject.name}/assets/${targetPath}`;
      
      // First, check if the source is a file or folder by listing the parent directory
      const sourceParentPath = sourcePath.includes('/') 
        ? `projects/${currentProject.name}/assets/${sourcePath.split('/').slice(0, -1).join('/')}`
        : `projects/${currentProject.name}/assets`;
      
      const parentContents = await bridgeService.listDirectory(sourceParentPath);
      const sourceFileName = sourcePath.split('/').pop();
      const sourceItem = parentContents.find(item => item.name === sourceFileName);
      
      console.log('🦀 Source parent path:', sourceParentPath);
      console.log('🦀 Source filename:', sourceFileName);
      console.log('🦀 Parent directory contents:', parentContents.map(item => item.name));
      console.log('🦀 Source item found:', sourceItem ? 'YES' : 'NO');
      
      if (!sourceItem) {
        throw new Error(`Source item not found: ${sourcePath}. Available files: ${parentContents.map(item => item.name).join(', ')}`);
      }
      
      if (sourceItem.is_directory) {
        // Handle folder move - recursively copy all contents
        console.log('🦀 Moving folder:', sourcePath, '->', targetPath);
        await moveFolderRecursively(currentProject, sourcePath, targetPath);
      } else {
        // Handle file move - detect if binary or text
        console.log('🦀 Moving file:', sourcePath, '->', targetPath);
        console.log('🦀 Full source path:', fullSourcePath);
        console.log('🦀 Full target path:', fullTargetPath);
        
        // Ensure target directory exists
        const targetDir = fullTargetPath.substring(0, fullTargetPath.lastIndexOf('/'));
        if (targetDir !== `projects/${currentProject.name}/assets`) {
          try {
            await bridgeService.listDirectory(targetDir);
          } catch (error) {
            // Directory doesn't exist, we should create it
            console.log('🦀 Target directory does not exist:', targetDir);
            throw new Error(`Target directory does not exist: ${targetDir}`);
          }
        }
        
        // Source file existence already verified above
        
        if (isBinaryFile(sourceFileName)) {
          // Handle binary file
          console.log('🦀 Moving binary file:', sourceFileName);
          try {
            const base64Content = await bridgeService.readBinaryFile(fullSourcePath);
            await bridgeService.writeBinaryFile(fullTargetPath, base64Content);
          } catch (error) {
            console.error('🦀 Failed to read/write binary file:', error);
            throw new Error(`Failed to move binary file ${sourceFileName}: ${error.message}`);
          }
        } else {
          // Handle text file
          console.log('🦀 Moving text file:', sourceFileName);
          try {
            const content = await bridgeService.readFile(fullSourcePath);
            await bridgeService.writeFile(fullTargetPath, content);
          } catch (error) {
            console.error('🦀 Failed to read/write text file:', error);
            throw new Error(`Failed to move text file ${sourceFileName}: ${error.message}`);
          }
        }
        
        // Delete source file
        await bridgeService.deleteFile(fullSourcePath);
      }
      
      console.log('🦀 Successfully moved asset:', sourcePath, '->', targetPath);
      return { success: true, sourcePath, targetPath };
    } catch (error) {
      throw new Error(`Failed to move item: ${error.message}`);
    }
  };

  const moveFolderRecursively = async (currentProject, sourceFolderPath, targetFolderPath) => {
    const fullSourcePath = `projects/${currentProject.name}/assets/${sourceFolderPath}`;
    
    // Get all contents of the source folder
    const folderContents = await bridgeService.listDirectory(fullSourcePath);
    
    // Create target folder structure by creating a .gitkeep file
    const fullTargetPath = `projects/${currentProject.name}/assets/${targetFolderPath}`;
    await bridgeService.writeFile(`${fullTargetPath}/.gitkeep`, '');
    
    // Process each item in the folder
    for (const item of folderContents) {
      const itemSourcePath = `${sourceFolderPath}/${item.name}`;
      const itemTargetPath = `${targetFolderPath}/${item.name}`;
      
      if (item.is_directory) {
        // Recursively move subfolder
        await moveFolderRecursively(currentProject, itemSourcePath, itemTargetPath);
      } else {
        // Move file
        const fileSourcePath = `projects/${currentProject.name}/assets/${itemSourcePath}`;
        const fileTargetPath = `projects/${currentProject.name}/assets/${itemTargetPath}`;
        
        if (isBinaryFile(item.name)) {
          // Handle binary file
          console.log('🦀 Moving binary file in folder:', item.name);
          const base64Content = await bridgeService.readBinaryFile(fileSourcePath);
          await bridgeService.writeBinaryFile(fileTargetPath, base64Content);
        } else {
          // Handle text file
          console.log('🦀 Moving text file in folder:', item.name);
          const content = await bridgeService.readFile(fileSourcePath);
          await bridgeService.writeFile(fileTargetPath, content);
        }
        
        // Delete source file
        await bridgeService.deleteFile(fileSourcePath);
      }
    }
    
    // Remove the original .gitkeep if it was the only file
    try {
      await bridgeService.deleteFile(`${fullTargetPath}/.gitkeep`);
    } catch (e) {
      // Ignore errors when deleting .gitkeep
    }
    
    // Delete the source folder by deleting any remaining .gitkeep
    try {
      await bridgeService.deleteFile(`${fullSourcePath}/.gitkeep`);
    } catch (e) {
      // Ignore errors
    }
  };

  const deleteAsset = async (currentProject, assetPath) => {
    try {
      const fullAssetPath = `projects/${currentProject.name}/assets/${assetPath}`;
      await bridgeService.deleteFile(fullAssetPath);
      console.log('🦀 Deleted asset:', assetPath);
      return { success: true, path: assetPath };
    } catch (error) {
      throw new Error(`Failed to delete asset: ${error.message}`);
    }
  };

  const addFileChangeListener = (callback) => {
    // Listen for project-selected events from the engine
    const handleProjectSelect = (event) => callback(event.detail);
    document.addEventListener('engine:project-selected', handleProjectSelect);
    return () => document.removeEventListener('engine:project-selected', handleProjectSelect);
  };

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