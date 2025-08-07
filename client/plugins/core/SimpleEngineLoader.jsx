import { useState, useEffect } from 'react'
import { projectManager } from '@/plugins/projects/projectManager.js'
import AssetLoader from '@/plugins/projects/components/AssetLoader.jsx'

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
          await projectManager.initializeDefaultProject()
          console.log('✅ Project system initialized')
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
      
      <AssetLoader
        isVisible={isLoading}
        progress={progress}
        currentAsset={currentSystem}
        onComplete={() => {}}
      />
    </div>
  )
}

export const loadAsset = async (assetPath) => {
  console.log('📦 Loading asset on-demand:', assetPath)
}