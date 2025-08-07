// Central auto-save coordinator that works with the project system
import { subscribe } from 'valtio'

class AutoSaveManager {
  constructor() {
    this.pluginStores = new Map()
    this.saveTimeout = null
    this.isEnabled = true
    this.debounceTime = 1000 // 1 second debounce
    this.projectManager = null
    this.lastSavedData = null // Store last saved data for comparison
  }

  // Set the project manager instance for saving
  setProjectManager(projectManager) {
    // Prevent duplicate connections
    if (this.projectManager === projectManager) {
      return
    }
    
    this.projectManager = projectManager
    // AutoSaveManager connected to ProjectManager
  }

  // Register a plugin store for auto-saving
  registerStore(pluginName, store, options = {}) {
    const config = {
      store,
      // Function to extract data that should be saved
      extractSaveData: options.extractSaveData || (() => ({ ...store })),
      // Function to restore data from project
      restoreData: options.restoreData || ((data) => Object.assign(store, data)),
      ...options
    }

    this.pluginStores.set(pluginName, config)

    // Subscribe to store changes for auto-save
    subscribe(store, () => {
      if (!this.isEnabled) {
        console.log(`🔇 Auto-save disabled for ${pluginName}`)
        return
      }
      
      // Skip auto-save during panel resizing to prevent loading screen triggers
      if (pluginName === 'editor' && store.panels?.isResizingPanels) {
        console.log('⏸️ Skipping auto-save during panel resize')
        return
      }
      
      console.log(`📝 ${pluginName} store changed, scheduling auto-save`)
      this.scheduleAutoSave()
    })

    console.log(`🔌 Registered ${pluginName} store for auto-save`)
    return config
  }

  // Remove a plugin store
  unregisterStore(pluginName) {
    this.pluginStores.delete(pluginName)
    console.log(`🔌 Unregistered ${pluginName} store`)
  }

  // Schedule an auto-save (debounced)
  scheduleAutoSave() {
    console.log('⏰ Scheduling auto-save with debounce')
    if (this.saveTimeout) {
      console.log('🔄 Clearing existing auto-save timeout')
      clearTimeout(this.saveTimeout)
    }

    this.saveTimeout = setTimeout(() => {
      // Auto-save timeout fired
      this.performAutoSave()
    }, this.debounceTime)
  }

  // Cancel any pending auto-save
  cancelPendingAutoSave() {
    if (this.saveTimeout) {
      console.log('🚫 Canceling pending auto-save')
      clearTimeout(this.saveTimeout)
      this.saveTimeout = null
    }
  }

  // Compare two data objects to check if they're different
  hasDataChanged(currentData, lastData) {
    if (!lastData) return true // First save
    
    try {
      const currentStr = JSON.stringify(currentData)
      const lastStr = JSON.stringify(lastData)
      return currentStr !== lastStr
    } catch (error) {
      console.warn('Error comparing save data:', error)
      return true // Safe default - save if comparison fails
    }
  }

  // Perform auto-save using project system
  async performAutoSave() {
    console.log('🚀 AutoSaveManager.performAutoSave() called')
    
    if (!this.isEnabled || !this.projectManager) {
      console.log('❌ Auto-save skipped - disabled or no project manager')
      return
    }

    // Check if panels are currently being resized
    const editorStore = this.pluginStores.get('editor')
    if (editorStore && editorStore.store.panels?.isResizingPanels) {
      console.log('⏸️ Skipping auto-save - panels are being resized')
      return
    }

    try {
      // Get current data from all stores
      console.log('📊 Getting current store data')
      const currentData = this.getAllStoreData()
      
      // Check if data has actually changed
      if (!this.hasDataChanged(currentData, this.lastSavedData)) {
        // No changes detected, skipping save
        return
      }

      console.log('💾 Data changed, calling ProjectManager.autoSaveCurrentProject()')
      // Use the project manager's existing auto-save functionality
      await this.projectManager.autoSaveCurrentProject()
      
      // Store the saved data for future comparisons
      this.lastSavedData = JSON.parse(JSON.stringify(currentData)) // Deep copy
      
      console.log('✅ Auto-save completed via project system')
    } catch (error) {
      console.warn('❌ Auto-save failed:', error)
    }
  }

  // Load data from project into all registered stores
  loadFromProject(projectData) {
    console.log('🔄 AutoSaveManager loadFromProject called with:', projectData)
    // Registered stores for auto-saving
    
    if (!projectData) {
      console.warn('❌ No project data provided to loadFromProject')
      return
    }

    for (const [pluginName, config] of this.pluginStores) {
      try {
        // Look for plugin data in the project
        let pluginData = null
        
        // Check different possible locations for plugin data
        if (projectData[pluginName]) {
          pluginData = projectData[pluginName]
          console.log(`📂 Found ${pluginName} data directly:`, pluginData)
        } else if (pluginName === 'scene' && projectData.scene) {
          pluginData = projectData.scene
          console.log(`📂 Found scene data:`, pluginData)
        } else if (pluginName === 'editor' && projectData.editor) {
          pluginData = projectData.editor
          console.log(`📂 Found editor data:`, pluginData)
        } else if (pluginName === 'render' && projectData.render) {
          pluginData = projectData.render
          console.log(`📂 Found render data:`, pluginData)
        } else {
          console.log(`❌ No data found for ${pluginName} plugin`)
        }

        if (pluginData) {
          console.log(`🔄 Restoring ${pluginName} state with:`, pluginData)
          config.restoreData(pluginData)
          console.log(`✅ ${pluginName} state loaded from project`)
        }
      } catch (error) {
        console.warn(`❌ Failed to load ${pluginName} state from project:`, error)
      }
    }
    
    // After loading, store the current data as our baseline for change detection
    setTimeout(() => {
      this.lastSavedData = JSON.parse(JSON.stringify(this.getAllStoreData()))
      // Baseline data set for change detection
    }, 100) // Small delay to ensure all stores have been restored
  }

  // Get current state from all stores (used by project manager)
  getAllStoreData() {
    const storeData = {}
    
    for (const [pluginName, config] of this.pluginStores) {
      try {
        storeData[pluginName] = config.extractSaveData()
      } catch (error) {
        console.warn(`Failed to extract ${pluginName} data:`, error)
      }
    }
    
    return storeData
  }

  // Manual save trigger
  async saveNow() {
    if (this.saveTimeout) {
      clearTimeout(this.saveTimeout)
      this.saveTimeout = null
    }
    await this.performAutoSave()
  }

  // Enable/disable auto-save
  enable() {
    this.isEnabled = true
    console.log('✅ Auto-save enabled')
  }

  disable() {
    this.isEnabled = false
    if (this.saveTimeout) {
      clearTimeout(this.saveTimeout)
      this.saveTimeout = null
    }
    console.log('❌ Auto-save disabled')
  }

  // Check if there are unsaved changes
  hasUnsavedChanges() {
    try {
      const currentData = this.getAllStoreData()
      return this.hasDataChanged(currentData, this.lastSavedData)
    } catch (error) {
      console.warn('Error checking for unsaved changes:', error)
      return false
    }
  }

  // Get save status
  getSaveStatus() {
    return {
      isEnabled: this.isEnabled,
      hasProjectManager: !!this.projectManager,
      registeredStores: Array.from(this.pluginStores.keys()),
      pendingSave: !!this.saveTimeout,
      hasUnsavedChanges: this.hasUnsavedChanges()
    }
  }
}

// Create global auto-save manager instance
export const autoSaveManager = new AutoSaveManager()

// Expose globally for debugging
if (typeof window !== 'undefined') {
  window.autoSaveManager = autoSaveManager
  console.log('AutoSaveManager exposed globally: window.autoSaveManager')
}