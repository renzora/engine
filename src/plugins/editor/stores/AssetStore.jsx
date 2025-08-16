import { createStore } from 'solid-js/store'

// Persistent storage for settings
const createPersistedSignal = (key, defaultValue) => {
  try {
    const stored = localStorage.getItem(`renzora-assets:${key}`)
    return stored ? JSON.parse(stored) : defaultValue
  } catch {
    return defaultValue
  }
}

const setPersistedValue = (key, value) => {
  try {
    localStorage.setItem(`renzora-assets:${key}`, JSON.stringify(value))
  } catch (e) {
    console.warn('Failed to persist value:', e)
  }
}

// Simple normalized asset store using Map
const assetEntitiesMap = new Map()
const assetEntitiesActions = {
  set: (key, value) => assetEntitiesMap.set(key, value),
  get: (key) => assetEntitiesMap.get(key),
  has: (key) => assetEntitiesMap.has(key),
  delete: (key) => assetEntitiesMap.delete(key),
  clear: () => assetEntitiesMap.clear(),
  entries: () => assetEntitiesMap.entries(),
  values: () => assetEntitiesMap.values(),
  keys: () => assetEntitiesMap.keys()
}

const [assetsStore, setAssetsStore] = createStore({
  currentProject: null,
  folderTree: null,
  folderTreeTimestamp: null,
  categories: null,
  categoriesTimestamp: null,
  assetsByPath: {},
  cacheExpiryMs: 5 * 60 * 1000,
  // UI state
  selectedAssets: new Set(),
  expandedFolders: new Set(createPersistedSignal('expandedFolders', [''])),
  viewMode: createPersistedSignal('viewMode', 'grid'),
  selectedCategory: createPersistedSignal('selectedCategory', 'other'),
  sortBy: createPersistedSignal('sortBy', 'name'),
  sortOrder: createPersistedSignal('sortOrder', 'asc')
})

// Normalized asset helpers
const normalizeAssets = (assets) => {
  const normalized = {}
  const ids = []
  
  const processAsset = (asset) => {
    normalized[asset.id] = asset
    ids.push(asset.id)
    
    if (asset.children) {
      asset.children.forEach(processAsset)
    }
  }
  
  assets.forEach(processAsset)
  return { entities: normalized, ids }
}

export const assetsActions = {
  setAssetsProject: (projectName) => {
    if (assetsStore.currentProject !== projectName) {
      // Clear entity map for new project
      assetEntitiesActions.clear()
      
      setAssetsStore({
        currentProject: projectName,
        folderTree: null,
        folderTreeTimestamp: null,
        categories: null,
        categoriesTimestamp: null,
        assetsByPath: {},
        selectedAssets: new Set()
      })
    }
  },

  isCacheValid: (timestamp) => {
    if (!timestamp) return false
    return (Date.now() - timestamp) < assetsStore.cacheExpiryMs
  },

  setFolderTree: (tree) => {
    setAssetsStore({
      folderTree: tree,
      folderTreeTimestamp: Date.now()
    })
  },

  setAssetCategories: (categories) => {
    setAssetsStore({
      categories: categories,
      categoriesTimestamp: Date.now()
    })
  },

  setAssetsForPath: (path, assets) => {
    // Ensure assetsByPath exists and assets is an array
    if (!assetsStore.assetsByPath) {
      setAssetsStore('assetsByPath', {});
    }
    
    const validAssets = Array.isArray(assets) ? assets : [];
    
    // Normalize and store assets in entity map
    const normalized = normalizeAssets(validAssets)
    normalized.entities && Object.entries(normalized.entities).forEach(([id, asset]) => {
      assetEntitiesActions.set(id, asset)
    })
    
    setAssetsStore('assetsByPath', path, {
      assets: validAssets,
      assetIds: normalized.ids,
      timestamp: Date.now()
    })
  },

  getAssetsForPath: (path) => {
    if (!assetsStore.assetsByPath) return null;
    const cached = assetsStore.assetsByPath[path]
    if (cached && cached.assets && assetsActions.isCacheValid(cached.timestamp)) {
      return cached.assets
    }
    return null
  },

  invalidateAssetPath: (path) => {
    setAssetsStore('assetsByPath', path, undefined)
  },

  clearAllAssetCache: () => {
    setAssetsStore({
      folderTree: null,
      folderTreeTimestamp: null,
      categories: null,
      categoriesTimestamp: null,
      assetsByPath: {}
    })
  },

  invalidateAssetPaths: (paths) => {
    paths.forEach(path => {
      setAssetsStore('assetsByPath', path, undefined)
    })
  },

  invalidateCategories: () => {
    setAssetsStore({
      categories: null,
      categoriesTimestamp: null
    })
  },

  invalidateFolderTree: () => {
    setAssetsStore({
      folderTree: null,
      folderTreeTimestamp: null
    })
  },

  // UI state management with persistence
  toggleAssetSelection: (assetId) => {
    const newSelected = new Set(assetsStore.selectedAssets)
    if (newSelected.has(assetId)) {
      newSelected.delete(assetId)
    } else {
      newSelected.add(assetId)
    }
    setAssetsStore('selectedAssets', newSelected)
  },

  clearAssetSelection: () => {
    setAssetsStore('selectedAssets', new Set())
  },

  toggleFolderExpansion: (folderPath) => {
    console.log('🔄 Toggling folder expansion:', folderPath)
    const newExpanded = new Set(assetsStore.expandedFolders)
    if (newExpanded.has(folderPath)) {
      newExpanded.delete(folderPath)
      console.log('🔽 Collapsing folder:', folderPath)
    } else {
      newExpanded.add(folderPath)
      console.log('🔼 Expanding folder:', folderPath)
    }
    setAssetsStore('expandedFolders', newExpanded)
    setPersistedValue('expandedFolders', Array.from(newExpanded))
    console.log('💾 Persisted expanded folders:', Array.from(newExpanded))
  },

  setViewMode: (mode) => {
    console.log('🔄 Setting view mode:', mode)
    setAssetsStore('viewMode', mode)
    setPersistedValue('viewMode', mode)
  },

  setSelectedCategory: (category) => {
    console.log('🔄 Setting selected category:', category)
    setAssetsStore('selectedCategory', category)
    setPersistedValue('selectedCategory', category)
  },

  setSortBy: (sortBy, sortOrder = 'asc') => {
    setAssetsStore({ sortBy, sortOrder })
    setPersistedValue('sortBy', sortBy)
    setPersistedValue('sortOrder', sortOrder)
  },

  // Entity access helpers
  getAssetById: (id) => assetEntitiesMap.get(id),
  
  getAssetsByIds: (ids) => ids.map(id => assetEntitiesMap.get(id)).filter(Boolean),

  setAssetsLibraryWidth: (width) => {
    setPersistedValue('assetsLibraryWidth', width)
  }
}

export { assetsStore, assetEntitiesMap, assetEntitiesActions }

if (typeof window !== 'undefined') {
  window.assetsStore = assetsStore
  window.assetsActions = assetsActions
}