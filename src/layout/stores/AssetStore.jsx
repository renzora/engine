import { createStore } from 'solid-js/store';

// Initial state for asset management
const [assetsStore, setAssetsStore] = createStore({
  viewMode: 'grid', // 'grid' or 'list'
  expandedFolders: new Set(),
  folderTree: [],
  assetCategories: [],
  assetsCache: new Map(),
  currentProject: null,
});

// Actions for asset management
export const assetsActions = {
  setViewMode: (mode) => {
    setAssetsStore('viewMode', mode);
  },
  
  setAssetsProject: (projectName) => {
    setAssetsStore('currentProject', projectName);
  },
  
  setFolderTree: (tree) => {
    setAssetsStore('folderTree', tree);
  },
  
  setAssetCategories: (categories) => {
    setAssetsStore('assetCategories', categories);
  },
  
  setAssetsForPath: (path, assets) => {
    setAssetsStore('assetsCache', (cache) => {
      const newCache = new Map(cache);
      newCache.set(path, assets);
      return newCache;
    });
  },
  
  getAssetsForPath: (path) => {
    return assetsStore.assetsCache.get(path);
  },
  
  invalidateAssetPath: (path) => {
    setAssetsStore('assetsCache', (cache) => {
      const newCache = new Map(cache);
      newCache.delete(path);
      return newCache;
    });
  },
  
  toggleFolderExpanded: (folderPath) => {
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
  
  clearAllAssetCache: () => {
    setAssetsStore('assetsCache', new Map());
    setAssetsStore('expandedFolders', new Set());
    setAssetsStore('folderTree', []);
    setAssetsStore('assetCategories', []);
  }
};

export { assetsStore };