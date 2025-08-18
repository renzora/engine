import { createSignal, createEffect } from 'solid-js'
import { assetsStore, assetsActions } from '../stores/AssetStore'
import { createAssetAPI } from './hooks/useAssetAPI'
import { bridgeService as projects } from '@/plugins/core/bridge'

export const useAssetManager = () => {
  const [assets, setAssets] = createSignal([])
  const [folderTree, setFolderTree] = createSignal(null)
  const [assetCategories, setAssetCategories] = createSignal(null)
  const [loading, setLoading] = createSignal(true)
  const [error, setError] = createSignal(null)
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
      assetsActions.setAssetsProject(currentProject.name)
      const tree = await fetchFolderTree(currentProject)
      if (tree) {
        setFolderTree(tree)
        assetsActions.setFolderTree(tree)
      }
      
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

  const loadAssetsForPath = async (path = '') => {
    if (!isInitialized()) return
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) {
      setError('No project selected')
      return
    }
    
    try {
      setLoading(true)
      
      const cached = assetsActions.getAssetsForPath(path)
      if (cached) {
        setAssets(cached)
        setLoading(false)
        return cached
      }
      
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

  const createNewFolder = async (path, name) => {
    if (!isInitialized()) return false
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) return false
    
    try {
      const success = await createFolder(currentProject, name, path)
      if (success) {
        const tree = await fetchFolderTree(currentProject)
        if (tree) {
          setFolderTree(tree)
          assetsActions.setFolderTree(tree)
        }
        
        assetsActions.invalidateAssetPath(path)
        await loadAssetsForPath(path)
      }
      return success
    } catch (err) {
      console.error('Failed to create folder:', err)
      setError(err.message)
      return false
    }
  }

  const moveAssetToPath = async (asset, newPath) => {
    if (!isInitialized()) return false
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) return false
    
    try {
      const success = await moveAsset(currentProject, asset.path, newPath)
      if (success) {
        assetsActions.invalidateAssetPath(asset.parentPath || '')
        assetsActions.invalidateAssetPath(newPath)
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

  const deleteAssetItem = async (asset) => {
    if (!isInitialized()) return false
    
    const currentProject = projects.getCurrentProject()
    if (!currentProject?.name) return false
    
    try {
      const success = await deleteAsset(currentProject, asset.path)
      if (success) {
        assetsActions.invalidateAssetPath(asset.parentPath || '')
        await loadAssetsForPath(asset.parentPath || '')
      
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

  const toggleFolderExpansion = (folderPath) => {
    assetsActions.toggleFolderExpansion(folderPath)
  }

  const isFolderExpanded = (folderPath) => {
    return expandedFolders().has(folderPath)
  }

  createEffect(() => {
    if (isInitialized()) {
      initializeAssets()
    }
  })

  createEffect(() => {
    if (isInitialized()) {
      const unsubscribe = addFileChangeListener(() => {
        initializeAssets()
      })
      
      return unsubscribe
    }
  })

  return {
    assets,
    folderTree,
    assetCategories,
    loading,
    error,
    expandedFolders,
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
