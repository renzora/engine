import { createSignal } from 'solid-js'
import { Vector3, Matrix, Plane } from '@babylonjs/core/Maths/math'
import '@babylonjs/loaders'
import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader'
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder'
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial'
import { Texture } from '@babylonjs/core/Materials/Textures/texture'
import { editorActions } from '@/layout/stores/EditorStore'
import { renderActions } from '@/render/store'
import { getCurrentProject } from '@/api/bridge/projects'

export const useAssetLoader = (sceneInstance, canvasRef) => {
  const [loadingTooltip, setLoadingTooltip] = createSignal({
    isVisible: false,
    message: '',
    position: { x: 0, y: 0 },
    progress: null
  })

  const getWorldPositionFromMouse = async (event, scene) => {
    if (!scene || !scene.activeCamera) {
      return Vector3.Zero()
    }
    
    const canvas = canvasRef()
    if (!canvas) return Vector3.Zero()
    
    const rect = canvas.getBoundingClientRect()
    const x = event.clientX - rect.left
    const y = event.clientY - rect.top
    const ray = scene.createPickingRay(x, y, Matrix.Identity(), scene.activeCamera)
    const hit = scene.pickWithRay(ray)
    if (hit.hit && hit.pickedPoint) {
      return hit.pickedPoint.add(new Vector3(0, 0.5, 0))
    }
    
    const groundPlane = Plane.FromPositionAndNormal(
      Vector3.Zero(), 
      new Vector3(0, 1, 0)
    )
    
    const distance = ray.intersectsPlane(groundPlane)
    if (distance !== null) {
      const worldPoint = ray.origin.add(ray.direction.scale(distance))
      return worldPoint
    }
    
    return Vector3.Zero()
  }

  const handleDragOver = (e) => {
    e.preventDefault()
    console.log('🔄 Drag over canvas:', e.dataTransfer.types)
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      e.dataTransfer.dropEffect = 'copy'
    }
  }

  const handleDrop = async (e) => {
    e.preventDefault()
    console.log('📦 Drop event on canvas:', e.dataTransfer.types)
    console.log('📦 Available data formats:', Array.from(e.dataTransfer.types))
    
    // Try different data formats
    for (const type of e.dataTransfer.types) {
      console.log(`📦 Data for ${type}:`, e.dataTransfer.getData(type))
    }
    
    // Check for our custom asset drag format
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      try {
        const assetData = JSON.parse(e.dataTransfer.getData('application/json'))
        console.log('Asset dropped in viewport:', assetData)
        
        if (assetData.type === 'asset' && assetData.assetType === 'file') {
          setLoadingTooltip({
            isVisible: true,
            message: `Loading ${assetData.name}...`,
            position: { x: e.clientX, y: e.clientY },
            progress: 0
          })
          
          const scene = sceneInstance()
          const dropPosition = await getWorldPositionFromMouse(e, scene)
          await loadAssetIntoScene(assetData, dropPosition)
        }
      } catch (error) {
        console.error('Error handling asset drop:', error)
        editorActions.addConsoleMessage(`Failed to load asset: ${error.message}`, 'error')
        setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      }
    }
    // Check for standard JSON format
    else if (e.dataTransfer.types.includes('application/json')) {
      try {
        const assetData = JSON.parse(e.dataTransfer.getData('application/json'))
        console.log('JSON asset dropped in viewport:', assetData)
        
        // Handle different asset data formats
        if (assetData.name && assetData.path) {
          setLoadingTooltip({
            isVisible: true,
            message: `Loading ${assetData.name}...`,
            position: { x: e.clientX, y: e.clientY },
            progress: 0
          })
          
          const scene = sceneInstance()
          const dropPosition = await getWorldPositionFromMouse(e, scene)
          await loadAssetIntoScene(assetData, dropPosition)
        }
      } catch (error) {
        console.error('Error handling JSON asset drop:', error)
      }
    }
    // Check for text format (might contain file paths)
    else if (e.dataTransfer.types.includes('text/plain')) {
      const textData = e.dataTransfer.getData('text/plain')
      console.log('Text dropped in viewport:', textData)
    }
  }

  const loadAssetIntoScene = async (assetData, position = null, importSettings = null) => {
    const scene = sceneInstance()
    
    if (!scene || scene.isDisposed) {
      console.warn('Scene not ready for asset loading')
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      return
    }

    const currentProject = getCurrentProject()
    
    if (!currentProject?.name) {
      console.error('No project loaded')
      editorActions.addConsoleMessage('No project loaded', 'error')
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      return
    }

    // The assetData.path already includes 'assets/', so don't add it again
    const assetPath = `projects/${currentProject.name}/${assetData.path}`;
    const assetUrl = `http://localhost:3001/file/${encodeURIComponent(assetPath)}`
    
    console.log('🔗 Asset URL:', assetUrl)
    
    try {
      editorActions.addConsoleMessage(`Loading asset: ${assetData.name}`, 'info')
      
      const extension = assetData.extension?.toLowerCase()
      
      if (['.glb', '.gltf', '.obj'].includes(extension)) {
        setLoadingTooltip(prev => ({ ...prev, message: `Loading ${assetData.name}...` }))
        
        const result = await SceneLoader.ImportMeshAsync(
          "", 
          "", 
          assetUrl, 
          scene,
          (progress) => {
            if (progress.lengthComputable) {
              const progressPercent = progress.loaded / progress.total
              setLoadingTooltip(prev => ({ ...prev, progress: progressPercent }))
            }
          }
        )
        
        if (result.animationGroups && result.animationGroups.length > 0) {
          result.animationGroups.forEach(animGroup => {
            animGroup.stop()
            console.log(`Stopped animation: ${animGroup.name}`)
          })
        }
        
        if (result.meshes.length > 0) {
          const rootMesh = result.meshes[0]
          rootMesh.position = position || Vector3.Zero()
          
          // Add to render store hierarchy and select it
          renderActions.addObject(rootMesh);
          renderActions.selectObject(rootMesh);
          
          editorActions.addConsoleMessage(`Successfully loaded: ${assetData.name}`, 'success')
          console.log('Loaded meshes:', result.meshes)
        }
      } else if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(extension)) {
        setLoadingTooltip(prev => ({ ...prev, message: `Loading texture: ${assetData.name}...` }))
        
        const plane = MeshBuilder.CreatePlane(assetData.name, { size: 2 }, scene)
        const material = new StandardMaterial(assetData.name + "_material", scene)
        const texture = new Texture(assetUrl, scene, undefined, undefined, undefined, () => {
          // Texture loaded successfully
          setLoadingTooltip(prev => ({ ...prev, progress: 1 }))
        })
        material.diffuseTexture = texture
        plane.material = material
        plane.position = position || Vector3.Zero()
        
        // Add to render store hierarchy and select it
        renderActions.addObject(plane);
        renderActions.selectObject(plane);
        
        editorActions.addConsoleMessage(`Successfully loaded texture: ${assetData.name}`, 'success')
      } else if (['.jsx', '.js'].includes(extension) && assetData.path.includes('/materials/')) {
        setLoadingTooltip(prev => ({ ...prev, message: `Loading material: ${assetData.name}...` }))
        
        try {
          // Import the material module dynamically
          const module = await import(/* webpackIgnore: true */ assetUrl);
          
          // Look for common material creation functions
          let materialFunction = null;
          if (module.createCheckerMaterial) {
            materialFunction = module.createCheckerMaterial;
          } else if (module.createMaterial) {
            materialFunction = module.createMaterial;
          } else if (module.default && typeof module.default === 'function') {
            materialFunction = module.default;
          }
          
          if (materialFunction) {
            // Create a test object to show the material
            const sphere = MeshBuilder.CreateSphere(assetData.name, { diameter: 2 }, scene);
            const material = materialFunction(assetData.name + "_material", scene);
            sphere.material = material;
            sphere.position = position || Vector3.Zero();
            
            // Store material source for later use
            if (!sphere.metadata) sphere.metadata = {};
            sphere.metadata.materialSource = assetPath;
            sphere.metadata.materialFunction = materialFunction.name;
            
            // Add to render store hierarchy and select it
            renderActions.addObject(sphere);
            renderActions.selectObject(sphere);
            
            editorActions.addConsoleMessage(`Successfully loaded material: ${assetData.name}`, 'success');
          } else {
            throw new Error('No material creation function found in module');
          }
        } catch (error) {
          console.error('Error loading material:', error);
          editorActions.addConsoleMessage(`Failed to load material: ${error.message}`, 'error');
        }
      } else {
        editorActions.addConsoleMessage(`Unsupported asset type: ${extension}`, 'warning')
      }
      
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      
    } catch (error) {
      console.error('Error loading asset:', error)
      editorActions.addConsoleMessage(`Failed to load ${assetData.name}: ${error.message}`, 'error')
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
    }
  }

  return {
    loadingTooltip,
    setLoadingTooltip,
    handleDragOver,
    handleDrop,
    loadAssetIntoScene
  }
}