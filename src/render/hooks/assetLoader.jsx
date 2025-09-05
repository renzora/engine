import { createSignal } from 'solid-js'
import { Vector3, Matrix, Plane } from '@babylonjs/core/Maths/math'
import { Color3 } from '@babylonjs/core/Maths/math.color'
import '@babylonjs/loaders'
import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader'
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder'
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial';
import { Texture } from '@babylonjs/core/Materials/Textures/texture'
import { Mesh } from '@babylonjs/core/Meshes/mesh'
import { TransformNode } from '@babylonjs/core/Meshes/transformNode'
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
  
  const [previewMesh, setPreviewMesh] = createSignal(null)
  const [isPositioning, setIsPositioning] = createSignal(false)

  const getWorldPositionFromMouse = async (event, scene) => {
    if (!scene || !scene.activeCamera) {
      return Vector3.Zero()
    }
    
    const canvas = canvasRef()
    if (!canvas) return Vector3.Zero()
    
    const rect = canvas.getBoundingClientRect()
    const x = event.clientX - rect.left
    const y = event.clientY - rect.top
    
    console.log(`Mouse pos: ${x}, ${y}, Canvas size: ${rect.width}x${rect.height}`)
    
    // Create picking ray - this should handle coordinate conversion correctly
    const ray = scene.createPickingRay(x, y, Matrix.Identity(), scene.activeCamera)
    
    // Always intersect with ground plane at y=0
    const groundPlane = Plane.FromPositionAndNormal(
      Vector3.Zero(), 
      new Vector3(0, 1, 0)  // Normal pointing up
    )
    
    const distance = ray.intersectsPlane(groundPlane)
    if (distance !== null && distance > 0) {
      const worldPoint = ray.origin.add(ray.direction.scale(distance))
      console.log(`Drop position: ${worldPoint.x}, ${worldPoint.y}, ${worldPoint.z}`)
      return worldPoint
    }
    
    // Fallback to origin if ray doesn't intersect ground
    console.log('Ray missed ground plane, using origin')
    return Vector3.Zero()
  }


  const handleDragOver = (e) => {
    e.preventDefault()
    
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      e.dataTransfer.dropEffect = 'copy'
      
      // Update preview position during drag
      if (isPositioning()) {
        const preview = previewMesh()
        if (preview) {
          const scene = sceneInstance()
          if (scene) {
            getWorldPositionFromMouse(e, scene).then(position => {
              preview.position = position
            })
          }
        }
      }
    }
  }

  const handleDrop = async (e) => {
    e.preventDefault()
    console.log('📦 Drop event on canvas:', e.dataTransfer.types)
    
    // Check if we have a preview mesh to convert to final
    const preview = previewMesh()
    if (preview && isPositioning()) {
      console.log('✅ Converting preview mesh to final model')
      
      // Get the final position
      const finalPosition = preview.position.clone()
      
      // Get the original asset data
      const assetData = preview.metadata?.originalAssetData
      
      // Remove preview state
      setPreviewMesh(null)
      setIsPositioning(false)
      
      // Make the preview mesh fully opaque and permanent
      const scene = sceneInstance()
      if (scene) {
        // Restore full opacity to all materials
        scene.meshes.forEach(mesh => {
          if (mesh.parent === preview && mesh.material) {
            mesh.material.alpha = 1.0
          }
        })
        
        // Rename the preview container to remove "_preview" suffix
        if (assetData) {
          preview.name = assetData.name
        }
        
        // Add to scene hierarchy and select
        renderActions.addObject(preview)
        renderActions.selectObject(preview)
        // Don't automatically set transform mode - let user choose from toolbar
        
        // Initialize object properties
        const objectId = preview.uniqueId || preview.name
        const { objectPropertiesActions } = await import('@/layout/stores/ViewportStore')
        objectPropertiesActions.ensureDefaultComponents(objectId)
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.position', [finalPosition.x, finalPosition.y, finalPosition.z])
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.rotation', [0, 0, 0])
        objectPropertiesActions.updateObjectProperty(objectId, 'transform.scale', [1, 1, 1])
        
        console.log('✅ Preview converted to final model:', preview.name)
        editorActions.addConsoleMessage(`Loaded ${assetData?.name || 'model'}`, 'success')
        
        // Hide loading tooltip
        setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      }
      return
    }
    
    // Fallback: Check for our custom asset drag format (for non-3D assets)
    if (e.dataTransfer.types.includes('application/x-asset-drag')) {
      try {
        const assetData = JSON.parse(e.dataTransfer.getData('application/json'))
        console.log('Asset dropped in viewport:', assetData)
        
        if (assetData.type === 'asset' && assetData.assetType === 'file') {
          const scene = sceneInstance()
          const dropPosition = await getWorldPositionFromMouse(e, scene)
          await loadAssetIntoScene(assetData, dropPosition)
        }
      } catch (error) {
        console.error('Error handling asset drop:', error)
        editorActions.addConsoleMessage(`Failed to load asset: ${error.message}`, 'error')
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
          const targetPosition = position || Vector3.Zero()
          
          console.log(`Target position from cursor: ${targetPosition.x}, ${targetPosition.y}, ${targetPosition.z}`)
          console.log(`Total meshes loaded: ${result.meshes.length}`)
          
          // Create a single container that groups all meshes
          const cleanName = assetData.name.replace(/\.[^/.]+$/, "") // Remove extension
          const container = new TransformNode(cleanName, scene)
          
          console.log(`Creating container: ${cleanName} for ${result.meshes.length} meshes`)
          console.log(`📋 Container details - Name: ${container.name}, ID: ${container.uniqueId}, Class: ${container.getClassName()}`)
          
          // Recursively collect ALL nodes from the entire hierarchy
          const collectAllNodes = (nodeArray) => {
            const allNodes = [];
            const visited = new Set();
            
            const traverse = (node) => {
              if (!node || visited.has(node)) return;
              visited.add(node);
              allNodes.push(node);
              
              // Get children recursively
              if (node.getChildren) {
                node.getChildren().forEach(child => traverse(child));
              }
            };
            
            nodeArray.forEach(node => traverse(node));
            return allNodes;
          };
          
          // Collect all nodes from meshes and transform nodes
          const allMeshes = collectAllNodes(result.meshes || []);
          const allTransformNodes = collectAllNodes(result.transformNodes || []);
          const allNodes = [...allMeshes, ...allTransformNodes];
          
          console.log(`Found ${allNodes.length} total nodes to parent (${allMeshes.length} meshes, ${allTransformNodes.length} transform nodes)`);
          
          // Log the entire loaded hierarchy before parenting
          console.log(`📊 LOADED HIERARCHY BEFORE PARENTING:`);
          result.meshes.forEach((mesh, index) => {
            console.log(`  Mesh ${index}: ${mesh.name} (ID: ${mesh.uniqueId}) - Parent: ${mesh.parent?.name || 'none'} (ID: ${mesh.parent?.uniqueId || 'none'})`);
          });
          if (result.transformNodes) {
            result.transformNodes.forEach((node, index) => {
              console.log(`  TransformNode ${index}: ${node.name} (ID: ${node.uniqueId}) - Parent: ${node.parent?.name || 'none'} (ID: ${node.parent?.uniqueId || 'none'})`);
            });
          }
          
          // Parent only the original root meshes and transform nodes to maintain hierarchy
          result.meshes.forEach(mesh => {
            if (!mesh.parent) {
              console.log(`🔗 Parenting root mesh ${mesh.name} (ID: ${mesh.uniqueId}) to container`);
              mesh.setParent(container);
            } else {
              console.log(`⏭️ Skipping mesh ${mesh.name} (ID: ${mesh.uniqueId}) - already has parent: ${mesh.parent.name}`);
            }
          });
          
          if (result.transformNodes) {
            result.transformNodes.forEach(node => {
              if (node !== container && !node.parent) {
                console.log(`🔗 Parenting root TransformNode ${node.name} (ID: ${node.uniqueId}) to container`);
                node.setParent(container);
              } else if (node !== container) {
                console.log(`⏭️ Skipping TransformNode ${node.name} (ID: ${node.uniqueId}) - already has parent: ${node.parent?.name || 'none'}`);
              }
            });
          }
          
          // Log the final hierarchy after parenting
          console.log(`📊 FINAL HIERARCHY AFTER PARENTING:`);
          console.log(`  Container: ${container.name} (ID: ${container.uniqueId})`);
          const containerChildren = container.getChildren();
          containerChildren.forEach((child, index) => {
            console.log(`    Child ${index}: ${child.name} (ID: ${child.uniqueId}) - Class: ${child.getClassName()}`);
            if (child.getChildren) {
              const grandChildren = child.getChildren();
              grandChildren.forEach((grandChild, gIndex) => {
                console.log(`      GrandChild ${gIndex}: ${grandChild.name} (ID: ${grandChild.uniqueId}) - Class: ${grandChild.getClassName()}`);
              });
            }
          });
          
          // If there are skeletons, attach them to the container
          if (result.skeletons && result.skeletons.length > 0) {
            container.skeleton = result.skeletons[0]
          }
          
          // If there are animations, store them in metadata
          if (result.animationGroups && result.animationGroups.length > 0) {
            container.metadata = container.metadata || {}
            container.metadata.animationGroups = result.animationGroups
          }
          
          const finalMesh = container
          console.log(`Container created: ${finalMesh.name}`)
          
          // Store asset source metadata for scene serialization
          if (!finalMesh.metadata) finalMesh.metadata = {};
          finalMesh.metadata.assetSource = assetData.path; // Store the original asset path
          console.log(`📝 AssetLoader: Set assetSource metadata to: ${assetData.path}`);
          
          // Set initial position at ground level
          finalMesh.position = new Vector3(targetPosition.x, 0, targetPosition.z)
          
          // Calculate bounding box for positioning on ground
          finalMesh.computeWorldMatrix(true)
          const boundingInfo = finalMesh.getHierarchyBoundingVectors()
          const minY = boundingInfo.min.y
          
          console.log(`Bounding box minY: ${minY}, adjusting position to: ${-minY}`)
          
          // Adjust Y position so the bottom sits exactly on ground (y=0)
          finalMesh.position.y = -minY
          
          console.log(`Final position: ${finalMesh.position.x}, ${finalMesh.position.y}, ${finalMesh.position.z}`)
          
          
          // Add shadow casting and receiving to all child meshes
          if (scene.shadowGenerator) {
            const allChildren = finalMesh.getChildMeshes();
            allChildren.forEach(childMesh => {
              if (childMesh.getClassName && childMesh.getClassName() === 'Mesh') {
                scene.shadowGenerator.addShadowCaster(childMesh);
                childMesh.receiveShadows = true;
              }
            });
            console.log(`🌑 Added ${allChildren.filter(m => m.getClassName && m.getClassName() === 'Mesh').length} child meshes to shadow casting and receiving`);
          }
          
          // Add to render store hierarchy and select it
          renderActions.addObject(finalMesh);
          renderActions.selectObject(finalMesh);
          
          editorActions.addConsoleMessage(`Successfully loaded: ${assetData.name}`, 'success')
          console.log('Loaded meshes:', result.meshes)
        }
      } else if (['.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tga'].includes(extension)) {
        setLoadingTooltip(prev => ({ ...prev, message: `Loading texture: ${assetData.name}...` }))
        
        const plane = MeshBuilder.CreatePlane(assetData.name, { size: 2 }, scene)
        const material = new PBRMaterial(assetData.name + "_material", scene)
        const texture = new Texture(assetUrl, scene, undefined, undefined, undefined, () => {
          // Texture loaded successfully
          setLoadingTooltip(prev => ({ ...prev, progress: 1 }))
        })
        material.baseTexture = texture
        material.metallicFactor = 0.0
        material.roughnessFactor = 0.9
        material.enableSpecularAntiAliasing = true
        
        // Enable reflections from environment
        material.environmentIntensity = 1.0
        material.usePhysicalLightFalloff = true
        plane.material = material
        plane.position = position || Vector3.Zero()
        
        // Add shadow casting and receiving
        if (scene.shadowGenerator) {
          scene.shadowGenerator.addShadowCaster(plane);
          plane.receiveShadows = true;
          console.log(`🌑 Added texture plane ${plane.name} to shadow casting and receiving`);
        }
        
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
            
            // Add shadow casting and receiving
            if (scene.shadowGenerator) {
              scene.shadowGenerator.addShadowCaster(sphere);
              sphere.receiveShadows = true;
              console.log(`🌑 Added material sphere ${sphere.name} to shadow casting and receiving`);
            }
            
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

    const handleDragEnter = async (e) => {
    console.log('🔍 DragEnter event:', e.dataTransfer.types)
    if (!e.dataTransfer.types.includes('application/x-asset-drag')) {
      console.log('❌ No asset drag type found')
      return
    }
    
    e.preventDefault()
    
    // Use global drag data to load actual model for preview
    const assetData = window._currentDragData
    console.log('🔍 Current drag data:', assetData)
    if (!assetData) {
      console.log('❌ No current drag data')
      return
    }
    
    const extension = assetData.extension?.toLowerCase()
    console.log('🔍 File extension:', extension)
    
    // Load actual model for 3D assets during drag
    if (['.glb', '.gltf', '.obj'].includes(extension)) {
      const scene = sceneInstance()
      if (!scene) {
        console.log('❌ No scene available')
        return
      }
      if (isPositioning()) {
        console.log('❌ Already positioning')
        return
      }
      
      console.log('🎯 Loading actual model for preview:', assetData.name)
      setIsPositioning(true)
      
      // Show loading tooltip
      setLoadingTooltip({
        isVisible: true,
        message: `Loading ${assetData.name}...`,
        position: { x: e.clientX, y: e.clientY },
        progress: 0
      })
      
      try {
        // Get current project
        const currentProject = getCurrentProject();
        
        if (!currentProject?.name) {
          console.error('❌ No project loaded');
          setIsPositioning(false);
          return;
        }
        
        // Construct proper project path (same as main loading function)
        const assetPath = `projects/${currentProject.name}/${assetData.path}`;
        const assetUrl = `http://localhost:3001/file/${encodeURIComponent(assetPath)}`
        console.log('🔗 Loading from URL:', assetUrl)
        const result = await SceneLoader.ImportMeshAsync("", "", assetUrl, scene)
        
        if (result.meshes && result.meshes.length > 0) {
          console.log('✅ Model loaded successfully, meshes:', result.meshes.length)
          
          // Update loading tooltip to show completion
          setLoadingTooltip(prev => ({ ...prev, progress: 1, message: `${assetData.name} loaded` }))
          
          // Create container for the preview
          const previewContainer = new TransformNode(assetData.name + "_preview", scene)
          
          // Parent all loaded meshes to the container
          result.meshes.forEach(mesh => {
            if (mesh.name !== "__root__") {
              mesh.setParent(previewContainer)
            }
          })
          
          // Make preview semi-transparent and add shadow casting
          result.meshes.forEach(mesh => {
            if (mesh.material) {
              mesh.material.alpha = 0.7
            }
            // Add shadow casting to preview meshes
            if (scene.shadowGenerator && mesh.getClassName && mesh.getClassName() === 'Mesh') {
              scene.shadowGenerator.addShadowCaster(mesh);
              mesh.receiveShadows = true;
            }
          })
          
          previewContainer.metadata = { originalAssetData: assetData }
          setPreviewMesh(previewContainer)
          
          // Position at mouse location
          const dropPosition = await getWorldPositionFromMouse(e, scene)
          previewContainer.position = dropPosition
          
          console.log('✅ Preview mesh created and positioned')
          
          // Hide loading tooltip after a brief delay
          setTimeout(() => {
            setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
          }, 500)
        } else {
          console.log('❌ No meshes found in loaded model')
          setIsPositioning(false)
          setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
        }
      } catch (error) {
        console.error('❌ Error loading preview model:', error)
        setIsPositioning(false)
        setLoadingTooltip(prev => ({ ...prev, isVisible: false }))
      }
    } else {
      console.log('❌ Not a 3D model extension')
    }
  }

  const handleDragLeave = (e) => {
    // Clean up preview when leaving canvas
    if (!e.currentTarget.contains(e.relatedTarget)) {
      const preview = previewMesh()
      if (preview) {
        preview.dispose()
        setPreviewMesh(null)
        setIsPositioning(false)
        console.log('🧹 Cleaned up preview mesh on drag leave')
      }
      window._currentDragData = null
    }
  }

  return {
    loadingTooltip,
    setLoadingTooltip,
    handleDragOver,
    handleDragEnter,
    handleDragLeave,
    handleDrop,
    loadAssetIntoScene,
    isPositioning
  }
}