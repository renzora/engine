import { createSignal, createEffect } from 'solid-js'
import { assetsStore, assetsActions } from '@/plugins/editor/stores/AssetStore'
import { createAssetAPI } from '@/plugins/editor/hooks/useAssetAPI'
import { bridgeService as projects } from '@/plugins/core/bridge'

export const useAssetManager = () => {
  const [assets, setAssets] = createSignal([])
  const [folderTree, setFolderTree] = createSignal(null)
  const [assetCategories, setAssetCategories] = createSignal(null)
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal(null)
  // Use persistent expanded folders from store
  const expandedFolders = () => assetsStore.expandedFolders
  
  const { 
    isInitialized, 
    fetchFolderTree, 
    fetchAssetCategories, 
    fetchAssets, 
    searchAssets, 
    createFolder, 
    moveAsset, 
    deleteAsset, 
    addFileChangeListener 
  } = createAssetAPI()

  // Initialize asset data
  const initializeAssets = async () => {
    if (!isInitialized()) return
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) {
      setError('No project selected')
      setLoading(false)
      return
    }

    try {
      setLoading(true)
      setError(null)
      
      // Set project in store
      assetsActions.setAssetsProject(currentProject.name)
      
      // Fetch folder tree
      const tree = await fetchFolderTree(currentProject)
      if (tree) {
        setFolderTree(tree)
        assetsActions.setFolderTree(tree)
      }
      
      // Fetch asset categories
      const categories = await fetchAssetCategories(currentProject)
      if (categories) {
        setAssetCategories(categories)
        assetsActions.setAssetCategories(categories)
      }
      
      setLoading(false)
    } catch (err) {
      console.error('Failed to initialize assets:', err)
      setError(err.message)
      setLoading(false)
    }
  }

  // Load assets for a specific path
  const loadAssetsForPath = async (path = '') => {
    if (!isInitialized()) return
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) {
      setError('No project selected')
      return
    }
    
    try {
      setLoading(true)
      
      // Check cache first
      const cached = assetsActions.getAssetsForPath(path)
      if (cached) {
        setAssets(cached)
        setLoading(false)
        return cached
      }
      
      // Fetch from API
      const fetchedAssets = await fetchAssets(currentProject, path)
      if (fetchedAssets) {
        setAssets(fetchedAssets)
        assetsActions.setAssetsForPath(path, fetchedAssets)
      }
      
      setLoading(false)
      return fetchedAssets
    } catch (err) {
      console.error('Failed to load assets:', err)
      setError(err.message)
      setLoading(false)
      return []
    }
  }

  // Search for assets globally
  const searchAssetsGlobally = async (query) => {
    if (!isInitialized() || !query.trim()) return []
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) return []
    
    try {
      setLoading(true)
      const results = await searchAssets(currentProject, query)
      setLoading(false)
      return results || []
    } catch (err) {
      console.error('Failed to search assets:', err)
      setError(err.message)
      setLoading(false)
      return []
    }
  }

  // Create new folder
  const createNewFolder = async (path, name) => {
    if (!isInitialized()) return false
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) return false
    
    try {
      const success = await createFolder(currentProject, name, path)
      if (success) {
        // Refresh folder tree
        const tree = await fetchFolderTree(currentProject)
        if (tree) {
          setFolderTree(tree)
          assetsActions.setFolderTree(tree)
        }
        
        // Invalidate cache for the path
        assetsActions.invalidateAssetPath(path)
        
        // Reload assets for current path
        await loadAssetsForPath(path)
      }
      return success
    } catch (err) {
      console.error('Failed to create folder:', err)
      setError(err.message)
      return false
    }
  }

  // Move asset to new location
  const moveAssetToPath = async (asset, newPath) => {
    if (!isInitialized()) return false
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) return false
    
    try {
      const success = await moveAsset(currentProject, asset.path, newPath)
      if (success) {
        // Invalidate both old and new path caches
        assetsActions.invalidateAssetPath(asset.parentPath || '')
        assetsActions.invalidateAssetPath(newPath)
        
        // Refresh folder tree
        const tree = await fetchFolderTree(currentProject)
        if (tree) {
          setFolderTree(tree)
          assetsActions.setFolderTree(tree)
        }
      }
      return success
    } catch (err) {
      console.error('Failed to move asset:', err)
      setError(err.message)
      return false
    }
  }

  // Delete asset
  const deleteAssetItem = async (asset) => {
    if (!isInitialized()) return false
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) return false
    
    try {
      const success = await deleteAsset(currentProject, asset.path)
      if (success) {
        // Invalidate cache for the asset's path
        assetsActions.invalidateAssetPath(asset.parentPath || '')
        
        // Reload assets for current path
        await loadAssetsForPath(asset.parentPath || '')
        
        // Refresh folder tree if it was a folder
        if (asset.type === 'folder') {
          const tree = await fetchFolderTree(currentProject)
          if (tree) {
            setFolderTree(tree)
            assetsActions.setFolderTree(tree)
          }
        }
      }
      return success
    } catch (err) {
      console.error('Failed to delete asset:', err)
      setError(err.message)
      return false
    }
  }

  // Toggle folder expansion using persistent store
  const toggleFolderExpansion = (folderPath) => {
    assetsActions.toggleFolderExpansion(folderPath)
  }

  // Check if folder is expanded
  const isFolderExpanded = (folderPath) => {
    return expandedFolders().has(folderPath)
  }

  // Initialize on mount
  createEffect(() => {
    if (isInitialized()) {
      initializeAssets()
    }
  })

  // Listen for file changes
  createEffect(() => {
    if (isInitialized()) {
      const unsubscribe = addFileChangeListener(() => {
        // Refresh data when files change
        initializeAssets()
      })
      
      return unsubscribe
    }
  })

  return {
    // State
    assets,
    folderTree,
    assetCategories,
    loading,
    error,
    expandedFolders,
    
    // Actions
    loadAssetsForPath,
    searchAssetsGlobally,
    createNewFolder,
    moveAssetToPath,
    deleteAssetItem,
    toggleFolderExpansion,
    isFolderExpanded,
    initializeAssets
  }
}