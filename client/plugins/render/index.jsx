import { useRef, useEffect, useState } from 'react'
import ModelImportDialog from '@/plugins/editor/components/ui/ModelImportDialog'
import * as BABYLON from '@babylonjs/core'
import '@babylonjs/core/Cameras/universalCamera'
import { useCameraController } from './CameraController'
import { useGrid } from './Grid'
import '@babylonjs/core/Engines/webgpuEngine'
import '@babylonjs/loaders/glTF'
import '@babylonjs/loaders/OBJ'
import '@babylonjs/core/Loading/sceneLoader'
import '@babylonjs/core/Events/pointerEvents'
import '@babylonjs/core/Events/keyboardEvents'
import '@babylonjs/core/Layers/highlightLayer'
import { actions, globalStore } from '@/store.js'
import { useSnapshot } from 'valtio'
import { projectManager } from '@/plugins/projects/projectManager.js'
import Stats from 'stats.js'
import LoadingTooltip from '@/plugins/editor/components/ui/LoadingTooltip.jsx'

function Viewport({ children, style = {}, onContextMenu }) {
  const canvasRef = useRef()
  const engineRef = useRef()
  const sceneRef = useRef()
  const statsRef = useRef()
  const copiedObjectRef = useRef(null) // Store copied object data
  const settings = useSnapshot(globalStore.editor.settings)
  const viewport = useSnapshot(globalStore.editor.viewport)
  
  // Track canvas and scene for camera controller reinitialization
  const [canvasInstance, setCanvasInstance] = useState(null)
  const [sceneInstance, setSceneInstance] = useState(null)
  
  // Loading tooltip state
  const [loadingTooltip, setLoadingTooltip] = useState({
    isVisible: false,
    message: '',
    position: { x: 0, y: 0 },
    progress: null
  })

  // Model import dialog state
  const [importDialog, setImportDialog] = useState({
    isOpen: false,
    modelName: '',
    assetData: null,
    position: null,
    modelAnalysis: null
  })
  
  // Initialize camera controller hook - will be set up after scene creation
  const cameraController = useCameraController(
    sceneInstance?._camera, 
    canvasInstance, 
    sceneInstance
  )

  // Initialize grid system
  useGrid(sceneInstance)

  const createEngine = async (renderingEngine) => {
    if (!canvasRef.current) return null
    
    let engine
    try {
      if (renderingEngine === 'webgpu') {
        // Comprehensive WebGPU support detection
        if (!navigator.gpu) {
          console.warn('WebGPU not available in this browser, falling back to WebGL')
          actions.editor.updateViewportSettings({ renderingEngine: 'webgl' })
          actions.editor.addConsoleMessage('WebGPU not supported - using WebGL instead', 'warning')
          engine = new BABYLON.Engine(canvasRef.current, true)
        } else {
          try {
            console.log('Testing WebGPU compatibility...')
            
            // Step 1: Test adapter availability
            const adapter = await navigator.gpu.requestAdapter({
              powerPreference: 'high-performance',
              forceFallbackAdapter: false
            })
            
            if (!adapter) {
              throw new Error('No WebGPU adapter available')
            }
            
            console.log('WebGPU adapter found:', adapter)
            
            // Step 2: Test device creation
            const device = await adapter.requestDevice({
              requiredFeatures: [],
              requiredLimits: {}
            })
            
            if (!device) {
              throw new Error('Failed to create WebGPU device')
            }
            
            console.log('WebGPU device created successfully')
            
            // Step 3: Ensure clean canvas for WebGPU context
            // Check if canvas already has a context
            const existingContext = canvasRef.current.getContext('webgl') || canvasRef.current.getContext('webgl2')
            if (existingContext) {
              console.log('Canvas already has WebGL context, recreating canvas for WebGPU...')
              
              // Store the current canvas properties
              const currentCanvas = canvasRef.current
              const parentElement = currentCanvas.parentElement
              const canvasStyle = currentCanvas.style.cssText
              const canvasAttributes = {
                tabIndex: currentCanvas.tabIndex
              }
              
              // Create a new canvas element
              const newCanvas = document.createElement('canvas')
              newCanvas.style.cssText = canvasStyle
              newCanvas.tabIndex = canvasAttributes.tabIndex
              
              // Replace the old canvas with the new one
              parentElement.replaceChild(newCanvas, currentCanvas)
              canvasRef.current = newCanvas
              setCanvasInstance(newCanvas) // Update state to trigger camera controller reinitialization
              
              console.log('Canvas recreated successfully for WebGPU')
            }
            
            const context = canvasRef.current.getContext('webgpu')
            if (!context) {
              throw new Error('Failed to get WebGPU context from canvas')
            }
            
            console.log('WebGPU context acquired successfully')
            
            // Step 4: Test context configuration
            const canvasFormat = navigator.gpu.getPreferredCanvasFormat()
            try {
              context.configure({
                device: device,
                format: canvasFormat,
                alphaMode: 'premultiplied'
              })
              console.log('WebGPU context configured successfully')
            } catch (configError) {
              throw new Error(`Context configuration failed: ${configError.message}`)
            }
            
            // Step 5: Create Babylon.js WebGPU engine
            const webGPUEngine = new BABYLON.WebGPUEngine(canvasRef.current, {
              deviceDescriptor: {
                requiredFeatures: [],
                requiredLimits: {}
              },
              adaptToDeviceRatio: true,
              antialias: true
            })
            
            await webGPUEngine.initAsync()
            engine = webGPUEngine
            console.log('WebGPU engine initialized successfully')
            actions.editor.addConsoleMessage('WebGPU renderer initialized successfully', 'success')
            
          } catch (webgpuError) {
            console.warn('WebGPU initialization failed:', webgpuError.message)
            console.warn('Error details:', webgpuError)
            console.warn('Falling back to WebGL')
            actions.editor.updateViewportSettings({ renderingEngine: 'webgl' })
            actions.editor.addConsoleMessage(`WebGPU failed: ${webgpuError.message} - using WebGL instead`, 'warning')
            engine = new BABYLON.Engine(canvasRef.current, true)
          }
        }
      } else {
        // Create WebGL engine - check for WebGPU context first
        const existingWebGPUContext = canvasRef.current.getContext('webgpu')
        if (existingWebGPUContext) {
          console.log('Canvas has WebGPU context, recreating canvas for WebGL...')
          
          // Store the current canvas properties
          const currentCanvas = canvasRef.current
          const parentElement = currentCanvas.parentElement
          const canvasStyle = currentCanvas.style.cssText
          const canvasAttributes = {
            tabIndex: currentCanvas.tabIndex
          }
          
          // Create a new canvas element
          const newCanvas = document.createElement('canvas')
          newCanvas.style.cssText = canvasStyle
          newCanvas.tabIndex = canvasAttributes.tabIndex
          
          // Replace the old canvas with the new one
          parentElement.replaceChild(newCanvas, currentCanvas)
          canvasRef.current = newCanvas
          setCanvasInstance(newCanvas) // Update state to trigger camera controller reinitialization
          
          console.log('Canvas recreated successfully for WebGL')
        }
        
        engine = new BABYLON.Engine(canvasRef.current, true)
        console.log('WebGL engine initialized successfully')
        actions.editor.addConsoleMessage('WebGL renderer initialized', 'success')
      }
    } catch (error) {
      console.error(`Failed to create ${renderingEngine} engine:`, error)
      // Last resort fallback to WebGL if we're not already using it
      if (renderingEngine === 'webgpu') {
        try {
          actions.editor.updateViewportSettings({ renderingEngine: 'webgl' })
          actions.editor.addConsoleMessage('Renderer fallback: Using WebGL', 'error')
          engine = new BABYLON.Engine(canvasRef.current, true)
        } catch (fallbackError) {
          console.error('Even WebGL fallback failed:', fallbackError)
          actions.editor.addConsoleMessage('Critical: All rendering engines failed to initialize', 'error')
          throw fallbackError
        }
      } else {
        throw error
      }
    }
    
    return engine
  }


  const focusOnObject = (targetObject, camera, scene) => {
    if (!targetObject || !camera) return
    
    let center, size
    
    if (targetObject.getClassName() === 'TransformNode') {
      // For TransformNodes (like GLB assets), get bounding info from all child meshes
      const childMeshes = targetObject.getChildMeshes()
      
      if (childMeshes.length === 0) {
        console.warn('TransformNode has no child meshes to focus on')
        return
      }
      
      // Calculate combined bounding box of all child meshes
      let min = new BABYLON.Vector3(Infinity, Infinity, Infinity)
      let max = new BABYLON.Vector3(-Infinity, -Infinity, -Infinity)
      
      childMeshes.forEach(childMesh => {
        childMesh.computeWorldMatrix(true)
        const boundingInfo = childMesh.getBoundingInfo()
        const meshMin = boundingInfo.boundingBox.minimumWorld
        const meshMax = boundingInfo.boundingBox.maximumWorld
        
        min = BABYLON.Vector3.Minimize(min, meshMin)
        max = BABYLON.Vector3.Maximize(max, meshMax)
      })
      
      center = min.add(max).scale(0.5)
      size = max.subtract(min)
    } else {
      // For regular meshes, use their own bounding info
      targetObject.computeWorldMatrix(true)
      const boundingInfo = targetObject.getBoundingInfo()
      
      center = boundingInfo.boundingBox.centerWorld
      size = boundingInfo.boundingBox.maximumWorld.subtract(boundingInfo.boundingBox.minimumWorld)
    }
    
    const maxSize = Math.max(size.x, size.y, size.z)
    
    // Calculate distance to fit the object in view (add some padding)
    const distance = Math.max(maxSize * 3, 10) // Minimum distance of 10
    
    console.log(`Focusing on: ${targetObject.name}`)
    console.log('Center:', center)
    console.log('Size:', size)
    console.log('Distance:', distance)
    
    // Calculate camera position to view the object from a good angle
    const currentForward = camera.getForwardRay().direction.normalize()
    const cameraPosition = center.subtract(currentForward.scale(distance))
    
    // Make sure camera is above ground level
    cameraPosition.y = Math.max(cameraPosition.y, center.y + distance * 0.3)
    
    console.log('Flying camera to position:', cameraPosition)
    
    // Animate camera position to the calculated position (faster)
    BABYLON.Animation.CreateAndStartAnimation(
      'flyCameraPosition',
      camera,
      'position',
      60, // 60 FPS
      15, // 15 frames = 0.25 seconds (was 30 frames)
      camera.position.clone(),
      cameraPosition,
      BABYLON.Animation.ANIMATIONLOOPMODE_CONSTANT,
      null,
      () => {
        // After position animation completes, smoothly look at the object
        const lookDirection = center.subtract(camera.position).normalize()
        const targetRotation = BABYLON.Vector3.Zero()
        
        // Calculate pitch (X rotation)
        targetRotation.x = Math.asin(-lookDirection.y)
        
        // Calculate yaw (Y rotation) 
        targetRotation.y = Math.atan2(lookDirection.x, lookDirection.z)
        
        // Animate rotation to look at object (faster)
        BABYLON.Animation.CreateAndStartAnimation(
          'flyCameraRotation',
          camera,
          'rotation',
          60, // 60 FPS
          8, // 8 frames = 0.13 seconds (was 15 frames)
          camera.rotation.clone(),
          targetRotation,
          BABYLON.Animation.ANIMATIONLOOPMODE_CONSTANT
        )
      }
    )
  }

  const createScene = (engine) => {
    const scene = new BABYLON.Scene(engine)
    scene.clearColor = new BABYLON.Color3(0.3, 0.6, 1.0); // Sky blue background

    var gizmoManager = new BABYLON.GizmoManager(scene)
    gizmoManager.positionGizmoEnabled = true
    gizmoManager.rotationGizmoEnabled = false
    gizmoManager.scaleGizmoEnabled = false
    scene.shadowsEnabled =
    
    // Improve gizmo responsiveness and visibility
    gizmoManager.thickness = 30.0 // Make gizmo lines much thicker for easier selection
    gizmoManager.scaleRatio = 2.5 // Smaller base size to prevent oversized gizmos when zoomed out
    
    // Configure individual gizmos for better interaction and visibility
    if (gizmoManager.gizmos.positionGizmo) {
      gizmoManager.gizmos.positionGizmo.sensitivity = 100 // More sensitive to mouse
      gizmoManager.gizmos.positionGizmo.updateGizmoRotationToMatchAttachedMesh = false
      // Make position gizmo arrows thicker
      if (gizmoManager.gizmos.positionGizmo.xGizmo) {
        gizmoManager.gizmos.positionGizmo.xGizmo.thickness = 40.0
      }
      if (gizmoManager.gizmos.positionGizmo.yGizmo) {
        gizmoManager.gizmos.positionGizmo.yGizmo.thickness = 40.0
      }
      if (gizmoManager.gizmos.positionGizmo.zGizmo) {
        gizmoManager.gizmos.positionGizmo.zGizmo.thickness = 40.0
      }
    }
    
    // Configure rotation gizmo for better visibility
    if (gizmoManager.gizmos.rotationGizmo) {
      gizmoManager.gizmos.rotationGizmo.sensitivity = 100
      if (gizmoManager.gizmos.rotationGizmo.xGizmo) {
        gizmoManager.gizmos.rotationGizmo.xGizmo.thickness = 40.0
      }
      if (gizmoManager.gizmos.rotationGizmo.yGizmo) {
        gizmoManager.gizmos.rotationGizmo.yGizmo.thickness = 40.0
      }
      if (gizmoManager.gizmos.rotationGizmo.zGizmo) {
        gizmoManager.gizmos.rotationGizmo.zGizmo.thickness = 40.0
      }
    }
    
    // Configure scale gizmo for better visibility
    if (gizmoManager.gizmos.scaleGizmo) {
      gizmoManager.gizmos.scaleGizmo.sensitivity = 100
      if (gizmoManager.gizmos.scaleGizmo.xGizmo) {
        gizmoManager.gizmos.scaleGizmo.xGizmo.thickness = 40.0
      }
      if (gizmoManager.gizmos.scaleGizmo.yGizmo) {
        gizmoManager.gizmos.scaleGizmo.yGizmo.thickness = 40.0
      }
      if (gizmoManager.gizmos.scaleGizmo.zGizmo) {
        gizmoManager.gizmos.scaleGizmo.zGizmo.thickness = 40.0
      }
    }
    
    // Store gizmo manager reference for later use
    scene._gizmoManager = gizmoManager
    
    // Helper function to ensure gizmo thickness when attached to objects
    const ensureGizmoThickness = () => {
      setTimeout(() => {
        // Configure position gizmo thickness
        if (gizmoManager.gizmos.positionGizmo) {
          ['xGizmo', 'yGizmo', 'zGizmo'].forEach(axis => {
            if (gizmoManager.gizmos.positionGizmo[axis]) {
              gizmoManager.gizmos.positionGizmo[axis].thickness = 40.0
            }
          })
        }
        
        // Configure rotation gizmo thickness  
        if (gizmoManager.gizmos.rotationGizmo) {
          ['xGizmo', 'yGizmo', 'zGizmo'].forEach(axis => {
            if (gizmoManager.gizmos.rotationGizmo[axis]) {
              gizmoManager.gizmos.rotationGizmo[axis].thickness = 40.0
            }
          })
        }
        
        // Configure scale gizmo thickness
        if (gizmoManager.gizmos.scaleGizmo) {
          ['xGizmo', 'yGizmo', 'zGizmo'].forEach(axis => {
            if (gizmoManager.gizmos.scaleGizmo[axis]) {
              gizmoManager.gizmos.scaleGizmo[axis].thickness = 40.0
            }
          })
        }
      }, 100) // Small delay to ensure gizmos are created
    }
    
    // Store the thickness helper for later use
    scene._ensureGizmoThickness = ensureGizmoThickness
    
    // Create highlight layer for selected objects
    const highlightLayer = new BABYLON.HighlightLayer("highlight", scene)
    highlightLayer.outerGlow = true
    highlightLayer.innerGlow = false
    scene._highlightLayer = highlightLayer

    var camera = new BABYLON.UniversalCamera(
      "camera",
      new BABYLON.Vector3(0, 5, -10),
      scene
    );
    camera.setTarget(BABYLON.Vector3.Zero());
    
    // Set up proper field of view and aspect ratio handling
    camera.fov = Math.PI / 3; // 60 degrees
    
    // Force initial aspect ratio calculation
    if (canvasRef.current) {
      const canvas = canvasRef.current
      const aspectRatio = canvas.clientWidth / canvas.clientHeight
      // The engine will handle aspect ratio automatically, but we ensure it's set up properly
      camera.getProjectionMatrix(true) // Force recalculation
    }
    
    // Create skybox
    const skybox = BABYLON.MeshBuilder.CreateSphere("skybox", {diameter: 200}, scene);
    const skyMaterial = new BABYLON.StandardMaterial("skyMaterial", scene);
    skyMaterial.emissiveColor = new BABYLON.Color3(0.3, 0.6, 1.0); // Sky blue as emissive
    skyMaterial.diffuseColor = BABYLON.Color3.Black();
    skyMaterial.specularColor = BABYLON.Color3.Black();
    skyMaterial.disableLighting = true; // Make it unlit so it appears evenly colored
    skyMaterial.backFaceCulling = false;
    skybox.material = skyMaterial;
    skybox.infiniteDistance = true;
    skybox.isPickable = false;

    // Create directional light (sun)
    const sunLight = new BABYLON.DirectionalLight("sunLight", new BABYLON.Vector3(-1, -1, -1), scene);
    sunLight.diffuse = new BABYLON.Color3(1, 0.95, 0.8); // Warm sunlight
    sunLight.specular = new BABYLON.Color3(1, 1, 1);
    sunLight.intensity = 2;

    // Create ambient light for softer shadows
    const ambientLight = new BABYLON.HemisphericLight("ambientLight", new BABYLON.Vector3(0, 1, 0), scene);
    ambientLight.diffuse = new BABYLON.Color3(0.4, 0.6, 1); // Sky color
    ambientLight.specular = BABYLON.Color3.Black();
    ambientLight.intensity = 0.3;

    // Create ground/floor (solid plane) - 20x20 meters (realistic studio/room size)
    const floorSize = globalStore.editor.settings.scene?.floorSize || 20;
    const ground = BABYLON.MeshBuilder.CreateGround("ground", {width: floorSize, height: floorSize}, scene);
    const groundMaterial = new BABYLON.StandardMaterial("groundMaterial", scene);
    
    // Simple solid ground material
    groundMaterial.diffuseColor = new BABYLON.Color3(0.3, 0.3, 0.3); // Dark gray
    groundMaterial.specularColor = BABYLON.Color3.Black();
    ground.material = groundMaterial;
    ground.isPickable = false; // Make ground non-selectable
  
    // Disable default camera controls - we'll implement custom Unreal-style controls
    camera.attachControl(canvasRef.current, false)
    
    // Store camera reference for focus functionality
    scene._camera = camera

    // Camera controller will be initialized via hook in the component
    // We'll store the reference here after the hook is called
    
    // Add render mode functionality
    scene._applyRenderMode = (mode) => {
      scene.meshes.forEach(mesh => {
        if (mesh.name === 'skybox' || mesh.name === 'ground') return; // Skip skybox and ground
        if (!mesh.material) return;
        
        switch (mode) {
          case 'wireframe':
            mesh.material.wireframe = true;
            break;
          case 'solid':
            mesh.material.wireframe = false;
            // Reset to default material properties for solid mode
            break;
          case 'material':
            mesh.material.wireframe = false;
            // Material mode shows materials as they are (default)
            break;
          case 'rendered':
            mesh.material.wireframe = false;
            // Rendered mode with full lighting (default)
            break;
        }
      });
    };
    
    scene.onPointerObservable.add((pointerInfo) => {
      switch (pointerInfo.type) {
        case BABYLON.PointerEventTypes.POINTERDOWN:
          // Let camera controller handle this, but we track for selection
          break
          
        case BABYLON.PointerEventTypes.POINTERMOVE:
          // Camera controller handles this
          break
          
        case BABYLON.PointerEventTypes.POINTERUP:
          // Only handle selection on click (not drag) and not when doing camera controls
          const isDragging = cameraController.getIsDragging()
          const mouseDownPos = cameraController.getMouseDownPos()
          const keysPressed = cameraController.getKeysPressed()
          
          if (!isDragging && mouseDownPos && !keysPressed.size) {
            const pickInfo = pointerInfo.pickInfo
            
            if (pickInfo?.hit) {
              let targetMesh = pickInfo.pickedMesh
              
              // If we picked an internal mesh, find its parent container
              if (targetMesh && targetMesh._isInternalMesh) {
                let parent = targetMesh.parent
                while (parent && (parent._isInternalMesh || parent._isInternalNode)) {
                  parent = parent.parent
                }
                if (parent) {
                  targetMesh = parent
                }
              }
              
              if (targetMesh) {
                // Use unified selection function that handles all gizmo, highlighting, and store updates
                const objectId = targetMesh.uniqueId || targetMesh.name
                console.log('🎯 3D Viewport - Selecting object:', targetMesh.name, 'ID:', objectId);
                actions.editor.selectObject(objectId)
              } else {
                // Clicked on empty space or default objects - clear selection
                console.log('🎯 3D Viewport - Clearing selection (no valid target)');
                actions.editor.selectObject(null)
              }
            } else {
              // Clicked on empty space - clear selection
              console.log('🎯 3D Viewport - Clearing selection (no hit)');
              actions.editor.selectObject(null)
            }
          }
          
          // Reset drag state in camera controller
          cameraController.resetDragState()
          break
      }
    })


    return scene
  }

  const snapObjectToGround = (targetObject, scene) => {
    if (!targetObject || !scene) return
    
    console.log('Snapping object to nearest surface:', targetObject.name)
    
    // Get the object's current position and bounding box
    targetObject.computeWorldMatrix(true)
    
    let boundingInfo, objectBottom, objectCenter;
    
    if (targetObject.getClassName() === 'TransformNode') {
      // For TransformNodes, get bounding info from child meshes
      const childMeshes = targetObject.getChildMeshes();
      if (childMeshes.length > 0) {
        // Calculate combined bounding box from all child meshes
        let minX = Infinity, minY = Infinity, minZ = Infinity;
        let maxX = -Infinity, maxY = -Infinity, maxZ = -Infinity;
        
        childMeshes.forEach(mesh => {
          mesh.computeWorldMatrix(true);
          const meshBounding = mesh.getBoundingInfo();
          const min = meshBounding.boundingBox.minimumWorld;
          const max = meshBounding.boundingBox.maximumWorld;
          
          minX = Math.min(minX, min.x);
          minY = Math.min(minY, min.y);
          minZ = Math.min(minZ, min.z);
          maxX = Math.max(maxX, max.x);
          maxY = Math.max(maxY, max.y);
          maxZ = Math.max(maxZ, max.z);
        });
        
        objectBottom = minY;
        objectCenter = new BABYLON.Vector3((minX + maxX) / 2, (minY + maxY) / 2, (minZ + maxZ) / 2);
      } else {
        // No child meshes, use the transform node position
        objectBottom = targetObject.position.y;
        objectCenter = targetObject.position.clone();
      }
    } else {
      // For direct meshes (legacy support)
      boundingInfo = targetObject.getBoundingInfo();
      objectBottom = boundingInfo.boundingBox.minimumWorld.y;
      objectCenter = boundingInfo.boundingBox.centerWorld;
    }
    
    // Try multiple rays to find the closest surface in different directions
    const rayDirections = [
      { dir: new BABYLON.Vector3(0, -1, 0), name: "down" },   // Down (primary)
      { dir: new BABYLON.Vector3(0, 1, 0), name: "up" },     // Up
      { dir: new BABYLON.Vector3(1, 0, 0), name: "right" },  // Right
      { dir: new BABYLON.Vector3(-1, 0, 0), name: "left" },  // Left
      { dir: new BABYLON.Vector3(0, 0, 1), name: "forward" }, // Forward
      { dir: new BABYLON.Vector3(0, 0, -1), name: "back" }   // Back
    ]
    
    let closestHit = null
    let closestDistance = Infinity
    let hitDirection = null
    
    // Cast rays in all directions to find the nearest surface
    rayDirections.forEach(({ dir, name }) => {
      // Start ray from object center and cast outward to find surfaces
      const ray = new BABYLON.Ray(objectCenter, dir)
      
      const hit = scene.pickWithRay(ray, (mesh) => {
        // Hit all visible meshes except the object itself and internal/UI meshes
        return mesh !== targetObject && 
               !mesh._isInternalMesh && 
               mesh.isVisible &&
               !mesh.name.startsWith('__') && // Skip internal meshes with __ prefix
               mesh.geometry // Only hit meshes with actual geometry
      })
      
      if (hit.hit && hit.distance < closestDistance) {
        closestDistance = hit.distance
        closestHit = hit
        hitDirection = name
        console.log(`Found ${name} surface at distance: ${hit.distance.toFixed(2)} on mesh: ${hit.pickedMesh?.name}`)
      }
    })
    
    if (closestHit && closestHit.pickedPoint) {
      const hitPoint = closestHit.pickedPoint
      
      // Handle different snap directions
      switch (hitDirection) {
        case "down":
          // Snap bottom of object to surface
          const heightDifference = objectBottom - targetObject.position.y
          targetObject.position.y = hitPoint.y - heightDifference
          break
        case "up":
          // Snap top of object to surface
          const objectTop = boundingInfo.boundingBox.maximumWorld.y
          const topHeightDiff = objectTop - targetObject.position.y
          targetObject.position.y = hitPoint.y - topHeightDiff
          break
        case "right":
          // Snap left side of object to surface
          const objectLeft = boundingInfo.boundingBox.minimumWorld.x
          const leftDiff = objectLeft - targetObject.position.x
          targetObject.position.x = hitPoint.x - leftDiff
          break
        case "left":
          // Snap right side of object to surface
          const objectRight = boundingInfo.boundingBox.maximumWorld.x
          const rightDiff = objectRight - targetObject.position.x
          targetObject.position.x = hitPoint.x - rightDiff
          break
        case "forward":
          // Snap back of object to surface
          const objectBack = boundingInfo.boundingBox.minimumWorld.z
          const backDiff = objectBack - targetObject.position.z
          targetObject.position.z = hitPoint.z - backDiff
          break
        case "back":
          // Snap front of object to surface
          const objectFront = boundingInfo.boundingBox.maximumWorld.z
          const frontDiff = objectFront - targetObject.position.z
          targetObject.position.z = hitPoint.z - frontDiff
          break
      }
      
      console.log(`Snapped ${targetObject.name} to ${hitDirection} surface at distance: ${closestDistance.toFixed(2)}`)
      actions.editor.addConsoleMessage(`Snapped ${targetObject.name} to ${hitDirection} surface`, 'success')
      
      // Refresh scene data to update UI
      actions.editor.refreshSceneData()
    } else {
      // Fallback: snap to Y = 0 (ground level) considering object's bottom
      const heightDifference = objectBottom - targetObject.position.y
      targetObject.position.y = -heightDifference
      
      console.log(`Snapped ${targetObject.name} to default ground level`)
      actions.editor.addConsoleMessage(`Snapped ${targetObject.name} to ground level`, 'success')
      
      // Refresh scene data to update UI
      actions.editor.refreshSceneData()
    }
  }

  const initializeViewport = async () => {
    try {
      // Clean up any existing engine/scene first
      if (engineRef.current) {
        try {
          engineRef.current.dispose()
        } catch (e) {
          console.warn('Error disposing existing engine:', e)
        }
        engineRef.current = null
      }
      
      if (sceneRef.current) {
        try {
          sceneRef.current.dispose()
        } catch (e) {
          console.warn('Error disposing existing scene:', e)
        }
        sceneRef.current = null
      }
      
      // Create engine based on current setting
      const engine = await createEngine(settings.viewport.renderingEngine || 'webgl')
      if (!engine) return
      
      // Add error handling for engine
      engine.onDisposeObservable.add(() => {
        console.log('Engine disposed')
        engineRef.current = null
        
        // Clear any pending environment texture operations
        if (window.cancelIdleCallback) {
          // Cancel any pending idle callbacks that might be related to texture processing
          for (let i = 1; i < 1000; i++) {
            window.cancelIdleCallback(i)
          }
        }
      })
      
      engineRef.current = engine
      
      // Create scene
      const scene = createScene(engine)
      if (!scene) {
        engine.dispose()
        return
      }
      
      // Add error handling for scene
      scene.onDisposeObservable.add(() => {
        console.log('Scene disposed')
        sceneRef.current = null
      })
      
      sceneRef.current = scene
      setSceneInstance(scene) // Update state to trigger camera controller reinitialization

      // Notify editor that scene is ready
      actions.editor.updateBabylonScene(scene)

      // Start render loop with stats integration
      engine.runRenderLoop(() => {
        if (statsRef.current) {
          statsRef.current.begin()
        }
        
        // Handle keyboard movement every frame
        if (cameraController) {
          cameraController.handleKeyboardMovement()
        }
        
        scene.render()
        
        if (statsRef.current) {
          statsRef.current.end()
        }
      })

      // Handle resize with better detection for dynamic layouts
      const handleResize = () => {
        if (canvasRef.current && engine) {
          // Force engine to recalculate viewport size
          engine.resize()
          
          // Update camera aspect ratio to prevent squashing
          if (scene._camera) {
            const canvas = canvasRef.current
            const aspectRatio = canvas.clientWidth / canvas.clientHeight
            
            // For UniversalCamera, we need to set the field of view based on aspect ratio
            if (scene._camera.fov) {
              // Maintain consistent field of view regardless of aspect ratio
              scene._camera.fov = Math.PI / 3 // 60 degrees
            }
          }
        }
      }
      
      // Listen for window resize
      window.addEventListener('resize', handleResize)
      
      // Use ResizeObserver to detect when the canvas container changes size
      // This is crucial for dynamic panel layouts
      let resizeObserver = null
      if (canvasRef.current && window.ResizeObserver) {
        resizeObserver = new ResizeObserver((entries) => {
          // Debounce resize calls to avoid excessive updates
          clearTimeout(window._resizeTimeout)
          window._resizeTimeout = setTimeout(() => {
            handleResize()
          }, 16) // ~60fps
        })
        
        // Observe the canvas element itself
        resizeObserver.observe(canvasRef.current)
        
        // Also observe the parent container in case that's what's changing
        if (canvasRef.current.parentElement) {
          resizeObserver.observe(canvasRef.current.parentElement)
        }
      }

      // Store cleanup function
      return () => {
        window.removeEventListener('resize', handleResize)
        
        // Clean up ResizeObserver
        if (resizeObserver) {
          resizeObserver.disconnect()
        }
        
        // Clear any pending resize timeout
        if (window._resizeTimeout) {
          clearTimeout(window._resizeTimeout)
        }
        
        // Notify editor that scene is being disposed
        actions.editor.updateBabylonScene(null)
        
        // Clean up stats
        if (statsRef.current && statsRef.current.dom.parentElement) {
          statsRef.current.dom.parentElement.removeChild(statsRef.current.dom)
          statsRef.current = null
        }
        
        // Camera controller cleanup is handled by the hook automatically
        
        // Dispose scene first, then engine
        if (scene && !scene.isDisposed) {
          try {
            scene.dispose()
          } catch (e) {
            console.warn('Error disposing scene in cleanup:', e)
          }
        }
        
        if (engine && !engine.isDisposed) {
          try {
            engine.dispose()
          } catch (e) {
            console.warn('Error disposing engine in cleanup:', e)
          }
        }
      }
    } catch (error) {
      console.error('Failed to initialize viewport:', error)
      actions.editor.addConsoleMessage(`Failed to initialize ${settings.viewport.renderingEngine} renderer`, 'error')
    }
  }

  useEffect(() => {
    if (!canvasRef.current) return
    
    // Set initial canvas instance
    setCanvasInstance(canvasRef.current)
    
    // Add global error handler for Babylon.js async errors
    const handleUnhandledRejection = (event) => {
      if (event.reason && event.reason.message && 
          event.reason.message.includes('postProcessManager')) {
        console.warn('Caught Babylon.js environment texture error:', event.reason.message)
        event.preventDefault() // Prevent error from propagating
      }
    }
    
    window.addEventListener('unhandledrejection', handleUnhandledRejection)
    
    let cleanup
    initializeViewport().then(cleanupFn => {
      cleanup = cleanupFn
    })

    // Add global keyboard listener for F, Delete, S, R keys
    const handleKeyDown = (e) => {
      if (e.key.toLowerCase() === 'f' && sceneRef.current) {
        console.log('F key pressed!')
        const scene = sceneRef.current
        console.log('Scene:', scene)
        console.log('Gizmo manager:', scene._gizmoManager)
        console.log('Attached mesh:', scene._gizmoManager?.attachedMesh)
        console.log('Camera:', scene._camera)
        
        if (scene._gizmoManager?.attachedMesh && scene._camera) {
          const objectName = scene._gizmoManager.attachedMesh.name
          focusOnObject(scene._gizmoManager.attachedMesh, scene._camera, scene)
          actions.editor.addConsoleMessage(`Flying to ${objectName}`, 'info')
          e.preventDefault()
        } else {
          console.log('No object selected or camera not available')
          actions.editor.addConsoleMessage('No object selected to focus on', 'warning')
        }
      } else if (e.key === 'Delete' && sceneRef.current) {
        const scene = sceneRef.current
        const attachedMesh = scene._gizmoManager?.attachedMesh
        
        if (attachedMesh && attachedMesh.name !== 'ground' && attachedMesh.name !== 'skybox') {
          // Delete the selected object
          attachedMesh.dispose()
          
          // Clear gizmo and highlight
          scene._gizmoManager.attachToMesh(null)
          if (scene._highlightLayer) {
            scene._highlightLayer.removeAllMeshes()
          }
          
          // Clear selection in store
          actions.editor.setSelectedEntity(null)
          actions.editor.selectSceneObject(null)
          
          // Refresh scene data
          actions.editor.refreshSceneData()
          
          console.log('Deleted object:', attachedMesh.name)
          e.preventDefault()
        }
      } else if (e.key.toLowerCase() === 's' && sceneRef.current) {
        // Switch to scale gizmo
        const scene = sceneRef.current
        if (scene._gizmoManager?.attachedMesh) {
          scene._gizmoManager.positionGizmoEnabled = false
          scene._gizmoManager.rotationGizmoEnabled = false
          scene._gizmoManager.scaleGizmoEnabled = true
          console.log('Switched to scale gizmo')
          e.preventDefault()
        }
      } else if (e.key.toLowerCase() === 'r' && sceneRef.current) {
        // Switch to rotation gizmo
        const scene = sceneRef.current
        if (scene._gizmoManager?.attachedMesh) {
          scene._gizmoManager.positionGizmoEnabled = false
          scene._gizmoManager.rotationGizmoEnabled = true
          scene._gizmoManager.scaleGizmoEnabled = false
          console.log('Switched to rotation gizmo')
          e.preventDefault()
        }
      } else if (e.key.toLowerCase() === 'g' && sceneRef.current) {
        // Switch to position/grab gizmo
        const scene = sceneRef.current
        if (scene._gizmoManager?.attachedMesh) {
          scene._gizmoManager.positionGizmoEnabled = true
          scene._gizmoManager.rotationGizmoEnabled = false
          scene._gizmoManager.scaleGizmoEnabled = false
          console.log('Switched to position gizmo')
          e.preventDefault()
        }
      } else if (e.ctrlKey && e.key.toLowerCase() === 'c' && sceneRef.current) {
        // Copy selected object
        const scene = sceneRef.current
        const attachedMesh = scene._gizmoManager?.attachedMesh
        
        if (attachedMesh) {
          // Store copy data
          copiedObjectRef.current = {
            name: attachedMesh.name,
            position: attachedMesh.position.clone(),
            rotation: attachedMesh.rotation ? attachedMesh.rotation.clone() : null,
            scaling: attachedMesh.scaling ? attachedMesh.scaling.clone() : null,
            className: attachedMesh.getClassName(),
            babylonObject: attachedMesh
          }
          console.log('Copied object:', attachedMesh.name)
          e.preventDefault()
        }
      } else if (e.ctrlKey && e.key.toLowerCase() === 'v' && sceneRef.current) {
        // Paste copied object
        const scene = sceneRef.current
        const copiedData = copiedObjectRef.current
        
        if (copiedData) {
          try {
            let newObject = null
            
            if (copiedData.className === 'TransformNode') {
              // Clone TransformNode and all its children
              newObject = copiedData.babylonObject.createInstance(copiedData.name + '_copy')
              if (!newObject) {
                // If createInstance doesn't work, try cloning
                newObject = copiedData.babylonObject.clone(copiedData.name + '_copy', null)
              }
            } else {
              // Clone regular meshes
              newObject = copiedData.babylonObject.createInstance(copiedData.name + '_copy')
              if (!newObject) {
                newObject = copiedData.babylonObject.clone(copiedData.name + '_copy', null)
              }
            }
            
            if (newObject) {
              // Offset position slightly so it doesn't overlap
              newObject.position = copiedData.position.add(new BABYLON.Vector3(2, 0, 2))
              if (copiedData.rotation && newObject.rotation) {
                newObject.rotation = copiedData.rotation.clone()
              }
              if (copiedData.scaling && newObject.scaling) {
                newObject.scaling = copiedData.scaling.clone()
              }
              
              // Refresh scene data to show new object in hierarchy
              actions.editor.refreshSceneData()
              
              console.log('Pasted object:', newObject.name)
            }
          } catch (error) {
            console.error('Failed to paste object:', error)
            actions.editor.addConsoleMessage(`Failed to paste object: ${error.message}`, 'error')
          }
        }
        e.preventDefault()
      } else if (e.ctrlKey && e.key.toLowerCase() === 'd' && sceneRef.current) {
        // Duplicate selected object (copy + paste in one step)
        const scene = sceneRef.current
        const attachedMesh = scene._gizmoManager?.attachedMesh
        
        if (attachedMesh) {
          try {
            let newObject = null
            
            console.log('Duplicating object:', attachedMesh.name, 'Type:', attachedMesh.getClassName())
            
            // Deep clone with ALL properties, materials, and children
            newObject = attachedMesh.clone(attachedMesh.name + '_duplicate', null, false, true)
            console.log('Deep cloned object with all properties:', newObject)
            
            // Recursively copy all properties from original to clone
            const copyAllProperties = (original, clone) => {
              // Copy material properties
              if (original.material && clone.material) {
                if (original.material.clone) {
                  clone.material = original.material.clone(original.material.name + '_duplicate')
                } else {
                  // For materials that can't be cloned, copy properties manually
                  Object.keys(original.material).forEach(key => {
                    if (typeof original.material[key] !== 'function' && key !== 'name') {
                      try {
                        clone.material[key] = original.material[key]
                      } catch (e) {
                        // Skip read-only properties
                      }
                    }
                  })
                }
              }
              
              // Copy all mesh properties
              const propertiesToCopy = [
                'visibility', 'isVisible', 'renderingGroupId', 'alphaIndex',
                'infiniteDistance', 'isPickable', 'showBoundingBox',
                'showSubMeshesBoundingBox', 'isBlocker', 'enablePointerMoveEvents',
                'renderOutline', 'outlineColor', 'outlineWidth', 'renderOverlay',
                'overlayColor', 'overlayAlpha', 'hasVertexAlpha', 'useVertexAlpha',
                'computeBonesUsingShaders', 'numBoneInfluencers', 'applyFog',
                'layerMask', 'alwaysSelectAsActiveMesh', 'actionManager',
                'ellipsoid', 'ellipsoidOffset', 'edgesWidth', 'edgesColor',
                'checkCollisions', 'collisionMask', 'collisionGroup'
              ]
              
              propertiesToCopy.forEach(prop => {
                if (original[prop] !== undefined && clone[prop] !== undefined) {
                  try {
                    if (original[prop] && original[prop].clone) {
                      clone[prop] = original[prop].clone()
                    } else {
                      clone[prop] = original[prop]
                    }
                  } catch (e) {
                    console.warn(`Could not copy property ${prop}:`, e)
                  }
                }
              })
              
              // Copy light properties if it's a light
              if (original.getClassName && original.getClassName().includes('Light')) {
                const lightProps = ['intensity', 'range', 'innerAngle', 'outerAngle', 'shadowEnabled']
                lightProps.forEach(prop => {
                  if (original[prop] !== undefined && clone[prop] !== undefined) {
                    clone[prop] = original[prop]
                  }
                })
                
                // Copy color properties
                if (original.diffuse && clone.diffuse) {
                  clone.diffuse = original.diffuse.clone()
                }
                if (original.specular && clone.specular) {
                  clone.specular = original.specular.clone()
                }
                
                // Ensure light is enabled
                if (clone.setEnabled) {
                  clone.setEnabled(true)
                }
              }
              
              // Recursively copy properties for all children
              if (original.getChildren && clone.getChildren) {
                const originalChildren = original.getChildren()
                const clonedChildren = clone.getChildren()
                
                originalChildren.forEach((child, index) => {
                  if (clonedChildren[index]) {
                    copyAllProperties(child, clonedChildren[index])
                  }
                })
              }
              
              // Special handling for objects with associated RectAreaLights (like boxlights)
              if (scene) {
                // Find any RectAreaLight that's parented to the original object
                const associatedLights = scene.lights.filter(light => 
                  light.parent === original && light.getClassName() === 'RectAreaLight'
                )
                
                if (associatedLights.length > 0) {
                  console.log('Found', associatedLights.length, 'associated RectAreaLight(s) for object:', original.name)
                  
                  // First, remove any cloned lights that Babylon.js automatically created
                  const clonedLights = scene.lights.filter(light => 
                    light.parent === clone && light.getClassName() === 'RectAreaLight'
                  )
                  clonedLights.forEach(light => {
                    console.log('Removing auto-cloned light:', light.name)
                    light.dispose()
                  })
                  
                  // Create new lights for each associated light
                  associatedLights.forEach(associatedLight => {
                    // Create a new RectAreaLight for the cloned object with proper naming
                    const newLightName = associatedLight.name + '_duplicate'
                    
                    const newLight = new BABYLON.RectAreaLight(
                      newLightName,
                      new BABYLON.Vector3(0, 0, 0), // Position relative to parent
                      associatedLight.width || 6,
                      associatedLight.height || 6,
                      scene
                    )
                    
                    // Copy all light properties
                    newLight.parent = clone
                    newLight.specular = associatedLight.specular ? associatedLight.specular.clone() : BABYLON.Color3.White()
                    newLight.diffuse = associatedLight.diffuse ? associatedLight.diffuse.clone() : BABYLON.Color3.White()
                    newLight.intensity = associatedLight.intensity || 0.7
                    
                    // Copy additional light properties if they exist
                    if (associatedLight.range !== undefined) newLight.range = associatedLight.range
                    if (associatedLight.shadowEnabled !== undefined) newLight.shadowEnabled = associatedLight.shadowEnabled
                    
                    console.log('Created new RectAreaLight for duplicated object:', newLight.name, 'with intensity:', newLight.intensity)
                  })
                }
              }
            }
            
            // Apply comprehensive property copying
            copyAllProperties(attachedMesh, newObject)
            
            if (newObject) {
              console.log('New object created successfully:', newObject.name, 'ID:', newObject.uniqueId)
              
              // Make sure the new object is at root level (no parent)
              newObject.parent = null
              
              // Offset position slightly so it doesn't overlap
              newObject.position = attachedMesh.position.add(new BABYLON.Vector3(2, 0, 2))
              if (attachedMesh.rotation && newObject.rotation) {
                newObject.rotation = attachedMesh.rotation.clone()
              }
              if (attachedMesh.scaling && newObject.scaling) {
                newObject.scaling = attachedMesh.scaling.clone()
              }
              
              console.log('New object position set to:', newObject.position)
              console.log('New object parent:', newObject.parent)
              
              // Select the new duplicated object
              const objectId = newObject.uniqueId || newObject.name
              
              // Clear previous highlight and gizmo
              if (scene._highlightLayer) {
                scene._highlightLayer.removeAllMeshes()
              }
              
              // Attach gizmo to new object
              scene._gizmoManager.attachToMesh(newObject)
              
              // Add yellow outline to new object (with error handling)
              if (scene._highlightLayer) {
                try {
                  scene._highlightLayer.addMesh(newObject, BABYLON.Color3.Yellow())
                } catch (highlightError) {
                  console.warn('Could not add highlight to duplicated object:', highlightError)
                  // Continue without highlight - not critical
                }
              }
              
              // Update selection in store
              actions.editor.setSelectedEntity(objectId)
              actions.editor.selectSceneObject(objectId)
              
              // Check if the object is properly added to the scene
              console.log('Scene meshes count:', scene.meshes.length)
              console.log('Scene transform nodes count:', scene.transformNodes.length)
              console.log('New object in scene meshes:', scene.meshes.includes(newObject))
              console.log('New object in scene transform nodes:', scene.transformNodes.includes(newObject))
              
              // Add a small delay before refreshing to ensure the object is fully registered
              setTimeout(() => {
                actions.editor.refreshSceneData()
                console.log('Scene data refreshed after duplication')
              }, 100)
              
              console.log('Duplicated and selected object:', newObject.name)
            }
          } catch (error) {
            console.error('Failed to duplicate object:', error)
            actions.editor.addConsoleMessage(`Failed to duplicate object: ${error.message}`, 'error')
          }
        }
        e.preventDefault()
      } else if (e.key === 'End' && sceneRef.current) {
        // Snap selected object to ground/nearest surface
        const scene = sceneRef.current
        const attachedMesh = scene._gizmoManager?.attachedMesh
        
        if (attachedMesh && attachedMesh.name !== 'ground' && attachedMesh.name !== 'skybox') {
          snapObjectToGround(attachedMesh, scene)
          e.preventDefault()
        }
      }
    }
    
    window.addEventListener('keydown', handleKeyDown)

    return () => {
      window.removeEventListener('keydown', handleKeyDown)
      window.removeEventListener('unhandledrejection', handleUnhandledRejection)
      if (cleanup) cleanup()
    }
  }, [settings.viewport.renderingEngine])

  // Update background color when it changes
  useEffect(() => {
    if (sceneRef.current) {
      const bgColor = BABYLON.Color3.FromHexString(settings.viewport.backgroundColor || '#1a202c')
      sceneRef.current.clearColor = bgColor
    }
  }, [settings.viewport.backgroundColor])

  // Handle render mode changes
  useEffect(() => {
    if (sceneRef.current && sceneRef.current._applyRenderMode) {
      const renderMode = viewport.renderMode || 'solid'
      sceneRef.current._applyRenderMode(renderMode)
    }
  }, [viewport.renderMode])

  // Handle stats visibility toggle and recreation after engine changes
  useEffect(() => {
    if (!canvasRef.current) return

    if (settings.editor.showStats && !statsRef.current) {
      // Create and show stats
      const stats = new Stats()
      stats.showPanel(0) // 0: fps, 1: ms, 2: mb, 3+: custom
      stats.dom.style.position = 'absolute'
      stats.dom.style.left = '10px'
      stats.dom.style.bottom = '10px'
      stats.dom.style.top = 'auto'
      stats.dom.style.zIndex = '1000'
      
      // Find the viewport container and append stats
      const viewportContainer = canvasRef.current.parentElement
      viewportContainer.appendChild(stats.dom)
      statsRef.current = stats
    } else if (!settings.editor.showStats && statsRef.current) {
      // Hide and cleanup stats
      if (statsRef.current.dom.parentElement) {
        statsRef.current.dom.parentElement.removeChild(statsRef.current.dom)
      }
      statsRef.current = null
    }
  }, [settings.editor.showStats, settings.viewport.renderingEngine])

  const getWorldPositionFromMouse = (event, scene) => {
    if (!scene || !scene._camera) {
      return BABYLON.Vector3.Zero()
    }
    
    // Get canvas bounds
    const canvas = canvasRef.current
    const rect = canvas.getBoundingClientRect()
    
    // Calculate mouse coordinates relative to canvas
    const x = event.clientX - rect.left
    const y = event.clientY - rect.top
    
    // Create a ray from camera through mouse position
    const ray = scene.createPickingRay(x, y, BABYLON.Matrix.Identity(), scene._camera)
    
    // Raycast against all meshes to find drop position
    const hit = scene.pickWithRay(ray)
    if (hit.hit && hit.pickedPoint) {
      // If we hit something, place it slightly above the hit point
      return hit.pickedPoint.add(new BABYLON.Vector3(0, 0.5, 0))
    }
    
    // Fallback: project to ground plane at Y=0
    const groundPlane = BABYLON.Plane.FromPositionAndNormal(
      BABYLON.Vector3.Zero(), 
      new BABYLON.Vector3(0, 1, 0)
    )
    
    const distance = ray.intersectsPlane(groundPlane)
    if (distance !== null) {
      const worldPoint = ray.origin.add(ray.direction.scale(distance))
      return worldPoint
    }
    
    // Final fallback
    return BABYLON.Vector3.Zero()
  }

  const handleDragOver = (e) => {
    e.preventDefault()
    // Check if this is an asset drag
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      e.dataTransfer.dropEffect = 'copy'
    }
  }

  const handleDrop = async (e) => {
    e.preventDefault()
    
    // Check if this is an asset drag
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      try {
        const assetData = JSON.parse(e.dataTransfer.getData('application/json'))
        console.log('Asset dropped in viewport:', assetData)
        
        if (assetData.type === 'asset' && assetData.assetType === 'file') {
          // Show loading tooltip at drop position
          setLoadingTooltip({
            isVisible: true,
            message: `Loading ${assetData.name}...`,
            position: { x: e.clientX, y: e.clientY },
            progress: 0
          })
          
          // Get mouse position for placement
          const dropPosition = getWorldPositionFromMouse(e, sceneRef.current)
          await loadAssetIntoScene(assetData, dropPosition)
        }
      } catch (error) {
        console.error('Error handling asset drop:', error)
        actions.editor.addConsoleMessage(`Failed to load asset: ${error.message}`, 'error')
        // Hide tooltip on error
        setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      }
    }
  }

  // Function to analyze a GLB/GLTF model before import
  const analyzeModel = async (assetUrl) => {
    try {
      // Check if engine is available and valid
      if (!engineRef.current || engineRef.current.isDisposed) {
        console.warn('Engine not available for model analysis')
        return null
      }
      
      // Load the model in a temporary scene to analyze it
      const tempScene = new BABYLON.Scene(engineRef.current)
      
      // Add error handling for the temp scene
      let result
      try {
        result = await BABYLON.SceneLoader.ImportMeshAsync("", "", assetUrl, tempScene)
      } catch (importError) {
        console.warn('Failed to import model for analysis:', importError)
        tempScene.dispose()
        return null
      }
      
      if (!result || !result.meshes) {
        console.warn('Invalid model result for analysis')
        tempScene.dispose()
        return null
      }
      
      const analysis = {
        totalObjects: result.meshes.length + (result.transformNodes?.length || 0),
        totalMeshes: result.meshes.length,
        totalMaterials: new Set(result.meshes.map(mesh => mesh.material?.name).filter(Boolean)).size,
        maxDepth: calculateHierarchyDepth(result.meshes, result.transformNodes || [])
      }
      
      // Clean up temp scene safely
      try {
        tempScene.dispose()
      } catch (disposeError) {
        console.warn('Error disposing temp scene:', disposeError)
      }
      
      return analysis
    } catch (error) {
      console.warn('Could not analyze model:', error)
      return null
    }
  }

  // Helper function to calculate hierarchy depth
  const calculateHierarchyDepth = (meshes, transformNodes) => {
    const allObjects = [...meshes, ...transformNodes]
    let maxDepth = 0
    
    const getDepth = (obj, currentDepth = 0) => {
      let depth = currentDepth
      const children = allObjects.filter(child => child.parent === obj)
      
      if (children.length > 0) {
        children.forEach(child => {
          depth = Math.max(depth, getDepth(child, currentDepth + 1))
        })
      }
      
      return depth
    }
    
    // Check root objects (no parent)
    allObjects.filter(obj => !obj.parent).forEach(rootObj => {
      maxDepth = Math.max(maxDepth, getDepth(rootObj, 1))
    })
    
    return maxDepth
  }

  // Process imported model based on import settings
  const processImportedModel = async (result, assetData, position, importSettings) => {
    const baseModelName = assetData.name.replace('.glb', '').replace('.gltf', '')
    
    if (!importSettings || importSettings.mode === 'smart') {
      // Smart mode: Auto-group similar objects and limit hierarchy depth
      await processSmartImport(result, baseModelName, position, importSettings)
    } else if (importSettings.mode === 'simplified') {
      // Simplified mode: Combine objects by material type
      await processSimplifiedImport(result, baseModelName, position, importSettings)
    } else if (importSettings.mode === 'individual') {
      // Individual mode: Keep all objects separate
      await processIndividualImport(result, baseModelName, position, importSettings)
    } else if (importSettings.mode === 'single') {
      // Single mesh mode: Merge everything into one object
      await processSingleMeshImport(result, baseModelName, position, importSettings)
    }
    
    // Refresh the scene data in the store so UI updates with new objects
    actions.editor.refreshSceneData()
    actions.editor.addConsoleMessage(`Successfully imported: ${assetData.name} (${importSettings?.mode || 'smart'} mode)`, 'success')
  }

  // Smart import mode implementation
  const processSmartImport = async (result, baseModelName, position, importSettings) => {
    // Find the root transform node or use the first mesh as the main container
    let mainContainer = null
    
    if (result.transformNodes && result.transformNodes.length > 0) {
      // Use the first root transform node as main container
      const rootTransforms = result.transformNodes.filter(node => node.parent === null)
      if (rootTransforms.length > 0) {
        mainContainer = rootTransforms[0]
      } else {
        mainContainer = result.transformNodes[0]
      }
    } else {
      // No transform nodes, use first mesh
      mainContainer = result.meshes[0]
    }
    
    // Rename the main container to the asset name
    mainContainer.name = baseModelName
    
    // Set position
    if (position) {
      mainContainer.position = position
    } else {
      mainContainer.position = BABYLON.Vector3.Zero()
    }
    
    // Remove parent so it appears at root level in hierarchy
    mainContainer.parent = null
    
    // Apply smart grouping based on hierarchy depth and max objects settings
    const maxObjects = importSettings?.maxObjects || 50
    const hierarchyDepth = importSettings?.hierarchyDepth || 3
    
    // Group objects intelligently
    const allObjects = [...result.meshes, ...(result.transformNodes || [])]
    let objectsToShow = [mainContainer]
    
    // If we have too many objects, group them by depth level
    if (allObjects.length > maxObjects) {
      // Mark deeper objects as internal
      const markObjectsAsInternal = (obj, currentDepth = 0) => {
        if (currentDepth > hierarchyDepth && obj !== mainContainer) {
          if (obj.getClassName() === 'Mesh') {
            obj._isInternalMesh = true
          } else {
            obj._isInternalNode = true
          }
        }
        
        // Process children
        allObjects.filter(child => child.parent === obj).forEach(child => {
          markObjectsAsInternal(child, currentDepth + 1)
        })
      }
      
      markObjectsAsInternal(mainContainer, 0)
    } else {
      // Mark all other objects as internal except those within hierarchy depth
      allObjects.forEach(obj => {
        if (obj !== mainContainer) {
          const depth = getObjectDepth(obj, allObjects)
          if (depth > hierarchyDepth) {
            if (obj.getClassName() === 'Mesh') {
              obj._isInternalMesh = true
            } else {
              obj._isInternalNode = true
            }
          }
        }
      })
    }
  }

  // Get depth of an object in the hierarchy
  const getObjectDepth = (obj, allObjects) => {
    let depth = 0
    let current = obj
    while (current.parent && allObjects.includes(current.parent)) {
      depth++
      current = current.parent
    }
    return depth
  }

  // Simplified import mode implementation
  const processSimplifiedImport = async (result, baseModelName, position, importSettings) => {
    // Group objects by material type
    const materialGroups = new Map()
    
    result.meshes.forEach(mesh => {
      const materialKey = mesh.material ? mesh.material.name || 'default' : 'no_material'
      if (!materialGroups.has(materialKey)) {
        materialGroups.set(materialKey, [])
      }
      materialGroups.get(materialKey).push(mesh)
    })
    
    // Create parent containers for each material group
    materialGroups.forEach((meshes, materialName) => {
      if (meshes.length > 1) {
        // Create a parent transform node for this material group
        const groupNode = new BABYLON.TransformNode(`${baseModelName}_${materialName}`, sceneRef.current)
        groupNode.position = position || BABYLON.Vector3.Zero()
        
        // Parent all meshes with this material to the group node
        meshes.forEach(mesh => {
          mesh.parent = groupNode
        })
      } else {
        // Single mesh, just position it
        meshes[0].position = position || BABYLON.Vector3.Zero()
        meshes[0].parent = null
      }
    })
  }

  // Individual import mode implementation  
  const processIndividualImport = async (result, baseModelName, position, importSettings) => {
    // Keep all objects separate at root level
    const allObjects = [...result.meshes, ...(result.transformNodes || [])]
    
    allObjects.forEach((obj, index) => {
      // Remove parent to make them all root level
      obj.parent = null
      
      // Spread them out in a grid if there are many
      if (position) {
        const gridSize = Math.ceil(Math.sqrt(allObjects.length))
        const spacing = 2
        const row = Math.floor(index / gridSize)
        const col = index % gridSize
        
        obj.position = position.add(new BABYLON.Vector3(
          (col - gridSize/2) * spacing,
          0,
          (row - gridSize/2) * spacing
        ))
      } else {
        obj.position = new BABYLON.Vector3(index * 2, 0, 0)
      }
      
      // Give meaningful names
      if (!obj.name || obj.name.includes('primitive')) {
        obj.name = `${baseModelName}_${obj.getClassName()}_${index}`
      }
    })
  }

  // Single mesh import mode implementation
  const processSingleMeshImport = async (result, baseModelName, position, importSettings) => {
    // Create a single parent transform node
    const singleContainer = new BABYLON.TransformNode(baseModelName, sceneRef.current)
    singleContainer.position = position || BABYLON.Vector3.Zero()
    
    // Parent all objects to this single container
    const allObjects = [...result.meshes, ...(result.transformNodes || [])]
    allObjects.forEach(obj => {
      obj.parent = singleContainer
      // Mark as internal so only the main container shows
      if (obj.getClassName() === 'Mesh') {
        obj._isInternalMesh = true
      } else {
        obj._isInternalNode = true
      }
    })
  }

  // Handle model import from dialog
  const handleModelImport = async (importSettings) => {
    // Close the dialog first
    setImportDialog(prev => ({ ...prev, isOpen: false }))
    
    // Show loading tooltip again
    setLoadingTooltip({
      isVisible: true,
      message: `Importing ${importDialog.modelName} with ${importSettings.mode} mode...`,
      position: { x: window.innerWidth / 2, y: window.innerHeight / 2 },
      progress: 0
    })
    
    // Call loadAssetIntoScene with the import settings
    await loadAssetIntoScene(importDialog.assetData, importDialog.position, importSettings)
  }

  const loadAssetIntoScene = async (assetData, position = null, importSettings = null) => {
    if (!sceneRef.current || sceneRef.current.isDisposed) {
      console.warn('Scene not ready for asset loading')
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      return
    }

    if (!engineRef.current || engineRef.current.isDisposed) {
      console.warn('Engine not ready for asset loading')
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      return
    }

    const scene = sceneRef.current
    const currentProject = projectManager.getCurrentProject()
    
    if (!currentProject?.name) {
      console.error('No project loaded')
      actions.editor.addConsoleMessage('No project loaded', 'error')
      return
    }

    const assetUrl = `/api/projects/${currentProject.name}/assets/file/${encodeURIComponent(assetData.path)}`
    
    try {
      actions.editor.addConsoleMessage(`Loading asset: ${assetData.name}`, 'info')
      
      // Handle different asset types
      const extension = assetData.extension?.toLowerCase()
      
      if (['.glb', '.gltf'].includes(extension)) {
        // For GLB/GLTF models, show import dialog first if no import settings provided
        if (!importSettings) {
          // Hide loading tooltip first
          setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
          
          // Analyze the model first
          const modelAnalysis = await analyzeModel(assetUrl)
          
          // Show import dialog
          setImportDialog({
            isOpen: true,
            modelName: assetData.name,
            assetData: assetData,
            position: position,
            modelAnalysis: modelAnalysis
          })
          
          return // Stop here - the import dialog will call this function again with settings
        }
        
        // Load GLTF/GLB models with progress tracking
        const result = await BABYLON.SceneLoader.ImportMeshAsync(
          "", 
          "", 
          assetUrl, 
          scene,
          (progress) => {
            // Update progress tooltip
            if (progress.lengthComputable) {
              const progressPercent = progress.loaded / progress.total
              setLoadingTooltip(prev => ({ ...prev, progress: progressPercent }))
            }
          }
        )
        
        // Stop any animations that might have started automatically
        if (result.animationGroups && result.animationGroups.length > 0) {
          result.animationGroups.forEach(animGroup => {
            animGroup.stop()
            console.log(`Stopped animation: ${animGroup.name}`)
          })
        }
        
        if (result.meshes.length > 0) {
          // Process the import based on import settings
          await processImportedModel(result, assetData, position, importSettings)
        }
      } else if (['.obj'].includes(extension)) {
        // Load OBJ models with progress tracking
        const { meshes } = await BABYLON.SceneLoader.ImportMeshAsync(
          "", 
          "", 
          assetUrl, 
          scene,
          (progress) => {
            // Update progress tooltip
            if (progress.lengthComputable) {
              const progressPercent = progress.loaded / progress.total
              setLoadingTooltip(prev => ({ ...prev, progress: progressPercent }))
            }
          }
        )
        
        if (meshes.length > 0) {
          const rootMesh = meshes[0]
          rootMesh.position = position || BABYLON.Vector3.Zero()
          
          // Refresh the scene data in the store so UI updates with new objects
          actions.editor.refreshSceneData()
          
          actions.editor.addConsoleMessage(`Successfully loaded: ${assetData.name}`, 'success')
          console.log('Loaded meshes:', meshes)
        }
      } else if (['.fbx'].includes(extension)) {
        // Load FBX models with progress tracking
        const { meshes } = await BABYLON.SceneLoader.ImportMeshAsync(
          "", 
          "", 
          assetUrl, 
          scene,
          (progress) => {
            // Update progress tooltip
            if (progress.lengthComputable) {
              const progressPercent = progress.loaded / progress.total
              setLoadingTooltip(prev => ({ ...prev, progress: progressPercent }))
            }
          }
        )
        
        if (meshes.length > 0) {
          const rootMesh = meshes[0]
          rootMesh.position = position || BABYLON.Vector3.Zero()
          
          // Refresh the scene data in the store so UI updates with new objects
          actions.editor.refreshSceneData()
          
          actions.editor.addConsoleMessage(`Successfully loaded: ${assetData.name}`, 'success')
          console.log('Loaded meshes:', meshes)
        }
      } else if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(extension)) {
        // Load textures - create a plane with the texture
        const plane = BABYLON.MeshBuilder.CreatePlane(assetData.name, { size: 2 }, scene)
        const material = new BABYLON.StandardMaterial(assetData.name + "_material", scene)
        const texture = new BABYLON.Texture(assetUrl, scene)
        
        material.diffuseTexture = texture
        plane.material = material
        plane.position = position || BABYLON.Vector3.Zero()
        
        // Refresh the scene data in the store so UI updates with new objects
        actions.editor.refreshSceneData()
        
        actions.editor.addConsoleMessage(`Successfully loaded texture: ${assetData.name}`, 'success')
      } else {
        actions.editor.addConsoleMessage(`Unsupported asset type: ${extension}`, 'warning')
      }
      
      // Hide loading tooltip on success
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      
    } catch (error) {
      console.error('Error loading asset:', error)
      actions.editor.addConsoleMessage(`Failed to load ${assetData.name}: ${error.message}`, 'error')
      // Hide loading tooltip on error
      setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
    }
  }

  return (
    <div 
      style={{ 
        width: '100%', 
        height: '100%', 
        backgroundColor: '#333333',
        position: 'relative',
        ...style 
      }}
      onClick={() => {
        canvasRef.current?.focus()
      }}
      onContextMenu={(e) => {
        e.preventDefault()
        // Context menu disabled for 3D viewport
      }}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      <canvas
        ref={canvasRef}
        style={{ 
          width: '100%', 
          height: '100%',
          outline: 'none',
          display: 'block',
          minWidth: 0,
          minHeight: 0,
          maxWidth: '100%',
          maxHeight: '100%',
          objectFit: 'contain' // Maintain aspect ratio when container changes
        }}
        tabIndex={0}
      />
      
      {children}
      
      {/* Loading tooltip for model drops */}
      <LoadingTooltip
        isVisible={loadingTooltip.isVisible}
        message={loadingTooltip.message}
        position={loadingTooltip.position}
        progress={loadingTooltip.progress}
      />
      
      {/* Model import dialog */}
      <ModelImportDialog
        isOpen={importDialog.isOpen}
        onClose={() => setImportDialog(prev => ({ ...prev, isOpen: false }))}
        onImport={handleModelImport}
        modelName={importDialog.modelName}
        modelAnalysis={importDialog.modelAnalysis}
      />
    </div>
  )
}

export default function RenderPlugin({ children, embedded = false, style = {}, onContextMenu, viewportBounds }) {
  if (embedded) {
    return <Viewport style={style} onContextMenu={onContextMenu}>{children}</Viewport>
  }

  // Use custom viewport bounds if provided, otherwise default to full screen
  const defaultStyle = viewportBounds ? {
    position: 'fixed',
    top: viewportBounds.top || 0,
    left: viewportBounds.left || 0,
    right: viewportBounds.right || 0,
    bottom: viewportBounds.bottom || 0,
    width: 'auto',
    height: 'auto'
  } : { width: '100vw', height: '100vh' }

  return (
    <Viewport style={{ ...defaultStyle, ...style }} onContextMenu={onContextMenu}>
      {children}
    </Viewport>
  )
}

export { Viewport as ViewportCanvas }