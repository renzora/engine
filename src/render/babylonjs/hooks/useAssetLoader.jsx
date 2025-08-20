import { createSignal } from 'solid-js'
import { Vector3, Matrix, Plane } from '@babylonjs/core/Maths/math'
import '@babylonjs/loaders'
import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader'
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder'
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial'
import { Texture } from '@babylonjs/core/Materials/Textures/texture'
import { editorActions } from '@/layout/stores/EditorStore'
import { bridgeService as projects } from '@/plugins/core/bridge'

export const useAssetLoader = (sceneInstance, canvasRef) => {
  const [loadingTooltip, setLoadingTooltip] = createSignal({
    isVisible: false,
    message: '',
    position: { x: 0, y: 0 },
    progress: null
  })

  const getWorldPositionFromMouse = async (event, scene) => {
    if (!scene || !scene._camera) {
      return Vector3.Zero()
    }
    
    const canvas = canvasRef
    const rect = canvas.getBoundingClientRect()
    const x = event.clientX - rect.left
    const y = event.clientY - rect.top
    const ray = scene.createPickingRay(x, y, Matrix.Identity(), scene._camera)
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
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      e.dataTransfer.dropEffect = 'copy'
    }
  }

  const handleDrop = async (e) => {
    e.preventDefault()
    
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
          const dropPosition = getWorldPositionFromMouse(e, scene)
          await loadAssetIntoScene(assetData, dropPosition)
        }
      } catch (error) {
        console.error('Error handling asset drop:', error)
        editorActions.addConsoleMessage(`Failed to load asset: ${error.message}`, 'error')
        setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      }
    }
  }

  const loadAssetIntoScene = async (assetData, position = null, importSettings = null) => {
    const scene = sceneInstance()
    
    if (!scene || scene.isDisposed) {
      console.warn('Scene not ready for asset loading')
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      return
    }

    const currentProject = projects.getCurrentProject()
    
    if (!currentProject?.name) {
      console.error('No project loaded')
      editorActions.addConsoleMessage('No project loaded', 'error')
      return
    }

    const assetUrl = `http://localhost:3001/api/projects/${currentProject.name}/assets/file/${encodeURIComponent(assetData.path)}`
    
    // Use imported modules
    
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
          // CLEAN SCENE: No store refresh needed
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
        // CLEAN SCENE: No store refresh needed
        editorActions.addConsoleMessage(`Successfully loaded texture: ${assetData.name}`, 'success')
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