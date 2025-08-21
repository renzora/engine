import { createStore } from 'solid-js/store';

// Initial state for asset management
const [assetsStore, setAssetsStore] = createStore({
  viewMode: 'folder', // 'folder' or 'type' - default to folder view
  layoutMode: 'grid', // 'grid' or 'list'
  selectedCategory: '3d-models', // default category for type view
  expandedFolders: new Set(),
  folderTree: null,
  categories: null,
  assetsByPath: {},
  categoriesTimestamp: null,
  folderTreeTimestamp: null,
  currentProject: null,
});

// Actions for asset management
export const assetsActions = {
  setViewMode: (mode) => {
    setAssetsStore('viewMode', mode);
  },
  
  setSelectedCategory: (category) => {
    setAssetsStore('selectedCategory', category);
  },
  
  setAssetsProject: (projectName) => {
    setAssetsStore('currentProject', projectName);
  },
  
  setFolderTree: (tree) => {
    setAssetsStore('folderTree', tree);
    setAssetsStore('folderTreeTimestamp', Date.now());
    
    // Auto-expand root folders by default
    if (tree && Array.isArray(tree)) {
      const expandedSet = new Set();
      tree.forEach(rootFolder => {
        if (rootFolder.path) {
          expandedSet.add(rootFolder.path);
        }
      });
      setAssetsStore('expandedFolders', expandedSet);
    }
  },
  
  setAssetCategories: (categories) => {
    setAssetsStore('categories', categories);
    setAssetsStore('categoriesTimestamp', Date.now());
  },
  
  setAssetsForPath: (path, assets) => {
    setAssetsStore('assetsByPath', path, {
      assets,
      timestamp: Date.now()
    });
  },
  
  getAssetsForPath: (path) => {
    const pathData = assetsStore.assetsByPath[path];
    return pathData && assetsActions.isCacheValid(pathData.timestamp) ? pathData.assets : null;
  },
  
  invalidateAssetPaths: (paths) => {
    paths.forEach(path => {
      setAssetsStore('assetsByPath', path, undefined);
    });
  },
  
  invalidateCategories: () => {
    setAssetsStore('categories', null);
    setAssetsStore('categoriesTimestamp', null);
  },
  
  invalidateFolderTree: () => {
    setAssetsStore('folderTree', null);
    setAssetsStore('folderTreeTimestamp', null);
  },
  
  toggleFolderExpansion: (folderPath) => {
    setAssetsStore('expandedFolders', (expanded) => {
      const newExpanded = new Set(expanded);
      if (newExpanded.has(folderPath)) {
        newExpanded.delete(folderPath);
      } else {
        newExpanded.add(folderPath);
      }
      return newExpanded;
    });
  },
  
  isCacheValid: (timestamp) => {
    if (!timestamp) return false;
    const maxAge = 5 * 60 * 1000; // 5 minutes
    return Date.now() - timestamp < maxAge;
  },
  
  clearAllAssetCache: () => {
    setAssetsStore('assetsByPath', {});
    setAssetsStore('expandedFolders', new Set());
    setAssetsStore('folderTree', null);
    setAssetsStore('categories', null);
    setAssetsStore('folderTreeTimestamp', null);
    setAssetsStore('categoriesTimestamp', null);
  }
};

export { assetsStore };