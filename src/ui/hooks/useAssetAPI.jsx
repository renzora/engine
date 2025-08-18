import { createSignal, createEffect } from 'solid-js';
import { bridgeService, bridgeService as projects } from '@/plugins/core/bridge';

export function createAssetAPI() {
  const [isInitialized, setIsInitialized] = createSignal(true);

  createEffect(() => {

  });

  const buildTreeFromAssets = (assets, projectName = null) => {
    if (!assets || !Array.isArray(assets)) {
      console.warn('buildTreeFromAssets: Invalid assets data:', assets);
      return null;
    }

    const folders = assets.filter(asset => asset.is_directory);
    const files = assets.filter(asset => !asset.is_directory);

    let processedFolders = folders;
    let processedFiles = files;
    
    if (projectName) {
      processedFolders = folders.map(folder => {
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
          return null;
        }
        
        adjustedPath = adjustedPath.replace(/\\/g, '/');
        
        return {
          ...folder,
          path: adjustedPath
        };
      }).filter(Boolean);

      processedFiles = files.map(file => {
        let adjustedPath = file.path;
        const projectPrefix1 = `projects/${projectName}/`;
        const projectPrefix2 = `projects\\${projectName}\\`;
        
        if (adjustedPath.startsWith(projectPrefix1)) {
          adjustedPath = adjustedPath.replace(projectPrefix1, '');
        } else if (adjustedPath.startsWith(projectPrefix2)) {
          adjustedPath = adjustedPath.replace(projectPrefix2, '');
        }
        
        adjustedPath = adjustedPath.replace(/\\/g, '/');
        
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

    return buildTree();
  };

  const fetchFolderTree = async (currentProject) => {
    console.log('🦀 Using Rust bridge for folder tree, project:', currentProject.name);
    
    try {
      const projects = await bridgeService.getProjects();
      console.log('🦀 All projects from bridge:', projects);
      const currentProjectData = projects.find(p => p.name === currentProject.name);
      console.log('🦀 Current project data:', currentProjectData);
      
      if (currentProjectData && currentProjectData.files) {
        const tree = buildTreeFromAssets(currentProjectData.files, currentProject.name);
        console.log('🦀 Built tree from project files:', tree);
        return tree;
      }
      
      console.log('🦀 Falling back to listing project assets directory directly');
      const projectFiles = await bridgeService.listDirectory(`projects/${currentProject.name}/assets`);
      console.log('🦀 Project files from direct listing:', projectFiles);
      const tree = buildTreeFromAssets(projectFiles, currentProject.name);
      console.log('🦀 Built tree from direct listing:', tree);
      return tree;
      
    } catch (error) {
      console.error('🦀 Rust bridge failed:', error);
      return [];
    }
  };

  const fetchAssetCategories = async (currentProject) => {
    console.log('🦀 Generating asset categories from bridge data');
    
    try {
      const allAssets = await bridgeService.listDirectory(`projects/${currentProject.name}/assets`);
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
    console.log('🦀 Using Rust bridge for assets in path:', path);
    
    try {
      const dirPath = path 
        ? `projects/${currentProject.name}/assets/${path}` 
        : `projects/${currentProject.name}/assets`;
      
      console.log('🦀 Requesting directory:', dirPath);
      const rawAssets = await bridgeService.listDirectory(dirPath);
      console.log('🦀 Got raw assets from Rust bridge:', rawAssets.length, rawAssets);
      
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
      console.error('🦀 Rust bridge failed:', error);
      return [];
    }
  };

  const searchAssets = async (currentProject, query) => {
    console.log('🦀 Using Rust bridge for asset search');
    
    try {
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
      
      await bridgeService.writeFile(`${folderPath}/.gitkeep`, '');
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

  const moveAsset = async (currentProject, sourcePath, targetPath, assetType = null) => {
    try {
      const fullSourcePath = `projects/${currentProject.name}/assets/${sourcePath}`;
      const fullTargetPath = `projects/${currentProject.name}/assets/${targetPath}`;
      
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
        console.log('🦀 Moving folder:', sourcePath, '->', targetPath);
        await moveFolderRecursively(currentProject, sourcePath, targetPath);
      } else {
        console.log('🦀 Moving file:', sourcePath, '->', targetPath);
        console.log('🦀 Full source path:', fullSourcePath);
        console.log('🦀 Full target path:', fullTargetPath);
        
        const targetDir = fullTargetPath.substring(0, fullTargetPath.lastIndexOf('/'));
        if (targetDir !== `projects/${currentProject.name}/assets`) {
          try {
            await bridgeService.listDirectory(targetDir);
          } catch (error) {
            console.log('🦀 Target directory does not exist:', targetDir);
            throw new Error(`Target directory does not exist: ${targetDir}`);
          }
        }
        
        if (isBinaryFile(sourceFileName)) {
          console.log('🦀 Moving binary file:', sourceFileName);
          try {
            const base64Content = await bridgeService.readBinaryFile(fullSourcePath);
            await bridgeService.writeBinaryFile(fullTargetPath, base64Content);
          } catch (error) {
            console.error('🦀 Failed to read/write binary file:', error);
            throw new Error(`Failed to move binary file ${sourceFileName}: ${error.message}`);
          }
        } else {
          console.log('🦀 Moving text file:', sourceFileName);
          try {
            const content = await bridgeService.readFile(fullSourcePath);
            await bridgeService.writeFile(fullTargetPath, content);
          } catch (error) {
            console.error('🦀 Failed to read/write text file:', error);
            throw new Error(`Failed to move text file ${sourceFileName}: ${error.message}`);
          }
        }
        
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
    const folderContents = await bridgeService.listDirectory(fullSourcePath);
    const fullTargetPath = `projects/${currentProject.name}/assets/${targetFolderPath}`;
    await bridgeService.writeFile(`${fullTargetPath}/.gitkeep`, '');

    for (const item of folderContents) {
      const itemSourcePath = `${sourceFolderPath}/${item.name}`;
      const itemTargetPath = `${targetFolderPath}/${item.name}`;
      
      if (item.is_directory) {
        await moveFolderRecursively(currentProject, itemSourcePath, itemTargetPath);
      } else {
        const fileSourcePath = `projects/${currentProject.name}/assets/${itemSourcePath}`;
        const fileTargetPath = `projects/${currentProject.name}/assets/${itemTargetPath}`;
        
        if (isBinaryFile(item.name)) {
          console.log('🦀 Moving binary file in folder:', item.name);
          const base64Content = await bridgeService.readBinaryFile(fileSourcePath);
          await bridgeService.writeBinaryFile(fileTargetPath, base64Content);
        } else {
          console.log('🦀 Moving text file in folder:', item.name);
          const content = await bridgeService.readFile(fileSourcePath);
          await bridgeService.writeFile(fileTargetPath, content);
        }
        
        await bridgeService.deleteFile(fileSourcePath);
      }
    }
    
    try {
      await bridgeService.deleteFile(`${fullTargetPath}/.gitkeep`);
    } catch (e) {

    }
    
    try {
      await bridgeService.deleteFile(`${fullSourcePath}/.gitkeep`);
    } catch (e) {

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
