import { useState, useEffect } from 'react'
import { projectManager } from '@/plugins/projects/projectManager.js'
import AssetLoader from '@/plugins/projects/components/AssetLoader.jsx'

// Helper function to load asset registry with retry logic
const loadAssetRegistryWithRetry = async (projectName, maxRetries = 3) => {
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      console.log(`📦 Loading asset registry (attempt ${attempt}/${maxRetries})`)
      
      const response = await fetch(`/api/projects/${projectName}/assets/categories`)
      
      if (response.ok) {
        const categoryData = await response.json()
        // Store in global registry for on-demand loading
        window.assetRegistry = categoryData
        console.log('📦 Asset registry loaded successfully')
        return categoryData
      } else if (response.status === 500) {
        console.warn(`Asset registry attempt ${attempt} failed with 500 error`)
        if (attempt < maxRetries) {
          // Wait longer between retries for server errors
          await new Promise(resolve => setTimeout(resolve, 1000 * attempt))
          continue
        }
      } else {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`)
      }
    } catch (error) {
      console.warn(`Asset registry attempt ${attempt} failed:`, error.message)
      
      if (attempt < maxRetries) {
        // Exponential backoff with jitter
        const delay = (500 * Math.pow(2, attempt - 1)) + Math.random() * 500
        console.log(`Retrying in ${Math.round(delay)}ms...`)
        await new Promise(resolve => setTimeout(resolve, delay))
      } else {
        throw error
      }
    }
  }
}

export default function EngineLoader({ children, onLoadComplete }) {
  const [isLoading, setIsLoading] = useState(true) 
  const [progress, setProgress] = useState(0)
  const [currentSystem, setCurrentSystem] = useState('')

  useEffect(() => {
    let isMounted = true
    
    const initializeEngine = async () => {
      try {
        console.log('🚀 Renzora Engine starting...')
        setIsLoading(true)
        setCurrentSystem('Initializing Project System')
        setProgress(20)
        
        // Initialize with default project
        try {
          await projectManager.initializeDefaultProject()
          console.log('✅ Project system initialized')
        } catch (error) {
          console.warn('⚠️ Project system initialization failed:', error)
        }
        
        if (!isMounted) return
        
        setProgress(60)
        setCurrentSystem('Loading Asset Registry')
        
        // Simple asset registry - just get the list, don't preload
        try {
          const currentProject = projectManager.getCurrentProject()
          if (currentProject.name) {
            // Get simple asset list for the registry with retry logic
            await loadAssetRegistryWithRetry(currentProject.name, 3)
          }
        } catch (error) {
          console.warn('⚠️ Asset registry loading failed after retries:', error)
        }
        
        if (!isMounted) return
        
        setProgress(90)
        setCurrentSystem('Engine Ready!')
        
        console.log('🎉 Renzora Engine loaded successfully!')
        
        // Quick completion
        setTimeout(() => {
          if (isMounted) {
            setProgress(100)
            onLoadComplete?.()
            
            setTimeout(() => {
              if (isMounted) {
                setIsLoading(false)
              }
            }, 200)
          }
        }, 100)
        
      } catch (error) {
        console.error('❌ Engine initialization failed:', error)
        
        if (isMounted) {
          setCurrentSystem(`Error: ${error.message}`)
          setTimeout(() => {
            if (isMounted) {
              setIsLoading(false)
              onLoadComplete?.()
            }
          }, 2000)
        }
      }
    }

    const timer = setTimeout(() => initializeEngine(), 10)
    
    return () => {
      isMounted = false
      clearTimeout(timer)
    }
  }, [onLoadComplete])

  return (
    <div data-engine-loader="true">
      {children}
      
      <AssetLoader
        isVisible={isLoading}
        progress={progress}
        currentAsset={currentSystem}
        onComplete={() => {}}
      />
    </div>
  )
}

// Simple asset loading utility using Babylon.js native loading
export const loadAsset = async (assetPath) => {
  // Use Babylon.js ImportMesh or AssetContainer for on-demand loading
  // This is much simpler and faster than the complex OptimizedAssetManager
  console.log('📦 Loading asset on-demand:', assetPath)
  
  // TODO: Implement actual Babylon.js asset loading here
  // Example: 
  // const result = await BABYLON.SceneLoader.ImportMeshAsync("", assetPath, "")
  // return result
}