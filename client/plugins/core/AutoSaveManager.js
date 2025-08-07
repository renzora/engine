import { subscribe } from 'valtio'

class AutoSaveManager {
  constructor() {
    this.pluginStores = new Map()
    this.saveTimeout = null
    this.isEnabled = true
    this.debounceTime = 1000
    this.projectManager = null
    this.lastSavedData = null
  }

  setProjectManager(projectManager) {
    if (this.projectManager === projectManager) {
      return
    }
    
    this.projectManager = projectManager
  }

  registerStore(pluginName, store, options = {}) {
    const config = {
      store,
      extractSaveData: options.extractSaveData || (() => ({ ...store })),
      restoreData: options.restoreData || ((data) => Object.assign(store, data)),
      ...options
    }

    this.pluginStores.set(pluginName, config)

    subscribe(store, () => {
      if (!this.isEnabled) {
        console.log(`🔇 Auto-save disabled for ${pluginName}`)
        return
      }
      
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

  unregisterStore(pluginName) {
    this.pluginStores.delete(pluginName)
    console.log(`🔌 Unregistered ${pluginName} store`)
  }

  scheduleAutoSave() {
    console.log('⏰ Scheduling auto-save with debounce')
    if (this.saveTimeout) {
      console.log('🔄 Clearing existing auto-save timeout')
      clearTimeout(this.saveTimeout)
    }

    this.saveTimeout = setTimeout(() => {
      this.performAutoSave()
    }, this.debounceTime)
  }

  cancelPendingAutoSave() {
    if (this.saveTimeout) {
      console.log('🚫 Canceling pending auto-save')
      clearTimeout(this.saveTimeout)
      this.saveTimeout = null
    }
  }

  hasDataChanged(currentData, lastData) {
    if (!lastData) return true
    
    try {
      const currentStr = JSON.stringify(currentData)
      const lastStr = JSON.stringify(lastData)
      return currentStr !== lastStr
    } catch (error) {
      console.warn('Error comparing save data:', error)
      return true
    }
  }

  async performAutoSave() {
    console.log('🚀 AutoSaveManager.performAutoSave() called')
    
    if (!this.isEnabled || !this.projectManager) {
      console.log('❌ Auto-save skipped - disabled or no project manager')
      return
    }

    const editorStore = this.pluginStores.get('editor')
    if (editorStore && editorStore.store.panels?.isResizingPanels) {
      console.log('⏸️ Skipping auto-save - panels are being resized')
      return
    }

    try {
      console.log('📊 Getting current store data')
      const currentData = this.getAllStoreData()
      
      if (!this.hasDataChanged(currentData, this.lastSavedData)) {
        return
      }

      console.log('💾 Data changed, calling ProjectManager.autoSaveCurrentProject()')
      await this.projectManager.autoSaveCurrentProject()
      
      this.lastSavedData = JSON.parse(JSON.stringify(currentData))
      
      console.log('✅ Auto-save completed via project system')
    } catch (error) {
      console.warn('❌ Auto-save failed:', error)
    }
  }

  loadFromProject(projectData) {
    console.log('🔄 AutoSaveManager loadFromProject called with:', projectData)
    
    if (!projectData) {
      console.warn('❌ No project data provided to loadFromProject')
      return
    }

    for (const [pluginName, config] of this.pluginStores) {
      try {
        let pluginData = null
        
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
    
    setTimeout(() => {
      this.lastSavedData = JSON.parse(JSON.stringify(this.getAllStoreData()))
    }, 100)
  }

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

  async saveNow() {
    if (this.saveTimeout) {
      clearTimeout(this.saveTimeout)
      this.saveTimeout = null
    }
    await this.performAutoSave()
  }

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

  hasUnsavedChanges() {
    try {
      const currentData = this.getAllStoreData()
      return this.hasDataChanged(currentData, this.lastSavedData)
    } catch (error) {
      console.warn('Error checking for unsaved changes:', error)
      return false
    }
  }

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

export const autoSaveManager = new AutoSaveManager()

if (typeof window !== 'undefined') {
  window.autoSaveManager = autoSaveManager
  console.log('AutoSaveManager exposed globally: window.autoSaveManager')
}