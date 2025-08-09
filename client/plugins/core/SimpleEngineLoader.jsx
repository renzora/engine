import { useState, useEffect } from 'react'
import { projectManager } from '@/services/ProjectManager'
// AssetLoader component removed - projects plugin doesn't exist

const loadAssetRegistryWithRetry = async (projectName, maxRetries = 3) => {
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      console.log(`📦 Loading asset registry (attempt ${attempt}/${maxRetries})`)
      
      const response = await fetch(`/api/projects/${projectName}/assets/categories`)
      
      if (response.ok) {
        const categoryData = await response.json()
        window.assetRegistry = categoryData
        console.log('📦 Asset registry loaded successfully')
        return categoryData
      } else if (response.status === 500) {
        console.warn(`Asset registry attempt ${attempt} failed with 500 error`)
        if (attempt < maxRetries) {
          await new Promise(resolve => setTimeout(resolve, 1000 * attempt))
          continue
        }
      } else {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`)
      }
    } catch (error) {
      console.warn(`Asset registry attempt ${attempt} failed:`, error.message)
      
      if (attempt < maxRetries) {
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
        
        try {
          // Check if a project was already loaded from splash screen
          if (!projectManager.isInitialized()) {
            await projectManager.initializeDefaultProject()
            console.log('✅ Default project initialized')
          } else {
            console.log('✅ Project already loaded:', projectManager.getCurrentProject()?.name)
          }
        } catch (error) {
          console.warn('⚠️ Project system initialization failed:', error)
        }
        
        if (!isMounted) return
        
        setProgress(60)
        setCurrentSystem('Loading Asset Registry')
        
        try {
          const currentProject = projectManager.getCurrentProject()
          if (currentProject.name) {
            await loadAssetRegistryWithRetry(currentProject.name, 3)
          }
        } catch (error) {
          console.warn('⚠️ Asset registry loading failed after retries:', error)
        }
        
        if (!isMounted) return
        
        setProgress(90)
        setCurrentSystem('Engine Ready!')
        
        console.log('🎉 Renzora Engine loaded successfully!')
        
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
      
      {isLoading && (
        <div className="fixed inset-0 bg-gray-900 bg-opacity-90 flex items-center justify-center z-50">
          <div className="text-white text-center">
            <div className="text-lg mb-2">{currentSystem}</div>
            <div className="w-64 bg-gray-700 rounded-full h-2 mb-2">
              <div 
                className="bg-blue-500 h-2 rounded-full transition-all duration-300" 
                style={{ width: `${progress}%` }}
              ></div>
            </div>
            <div className="text-sm text-gray-400">{progress}%</div>
          </div>
        </div>
      )}
    </div>
  )
}

export const loadAsset = async (assetPath) => {
  console.log('📦 Loading asset on-demand:', assetPath)
}