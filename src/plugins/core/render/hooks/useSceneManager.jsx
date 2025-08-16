import { createSignal } from 'solid-js'
import { Scene } from '@babylonjs/core/scene'
import { Color3 } from '@babylonjs/core/Maths/math.color'
import { Vector3 } from '@babylonjs/core/Maths/math.vector'
import { GizmoManager } from '@babylonjs/core/Gizmos/gizmoManager'
import { HighlightLayer } from '@babylonjs/core/Layers/highlightLayer'
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera'
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder'
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial'
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight'
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight'
import '@babylonjs/core/Layers/effectLayerSceneComponent'
import '@babylonjs/core/Meshes/Builders/sphereBuilder'
import '@babylonjs/core/Meshes/Builders/groundBuilder'
import { editorStore } from '@/plugins/editor/stores/EditorStore'
import { sceneActions, babylonScene } from '../store'

export const useSceneManager = () => {
  const [sceneInstance, setSceneInstance] = createSignal(null)
  
  const createScene = async (engine) => {
    const scene = new Scene(engine)
    scene.clearColor = new Color3(0.3, 0.6, 1.0)

    // Setup gizmo manager
    var gizmoManager = new GizmoManager(scene)
    gizmoManager.positionGizmoEnabled = true
    gizmoManager.rotationGizmoEnabled = false
    gizmoManager.scaleGizmoEnabled = false
    scene.shadowsEnabled = true
    
    gizmoManager.thickness = 30.0
    gizmoManager.scaleRatio = 2.5
    
    // Configure position gizmo
    if (gizmoManager.gizmos.positionGizmo) {
      gizmoManager.gizmos.positionGizmo.sensitivity = 100
      gizmoManager.gizmos.positionGizmo.updateGizmoRotationToMatchAttachedMesh = false

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
    
    // Configure rotation gizmo
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
    
    // Configure scale gizmo
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
    
    scene._gizmoManager = gizmoManager
    
    // Gizmo thickness ensurer
    const ensureGizmoThickness = () => {
      setTimeout(() => {
        if (gizmoManager.gizmos.positionGizmo) {
          ['xGizmo', 'yGizmo', 'zGizmo'].forEach(axis => {
            if (gizmoManager.gizmos.positionGizmo[axis]) {
              gizmoManager.gizmos.positionGizmo[axis].thickness = 40.0
            }
          })
        }
        
        if (gizmoManager.gizmos.rotationGizmo) {
          ['xGizmo', 'yGizmo', 'zGizmo'].forEach(axis => {
            if (gizmoManager.gizmos.rotationGizmo[axis]) {
              gizmoManager.gizmos.rotationGizmo[axis].thickness = 40.0
            }
          })
        }
        
        if (gizmoManager.gizmos.scaleGizmo) {
          ['xGizmo', 'yGizmo', 'zGizmo'].forEach(axis => {
            if (gizmoManager.gizmos.scaleGizmo[axis]) {
              gizmoManager.gizmos.scaleGizmo[axis].thickness = 40.0
            }
          })
        }
      }, 100)
    }
    
    scene._ensureGizmoThickness = ensureGizmoThickness
    
    // Setup highlight layer
    const highlightLayer = new HighlightLayer("highlight", scene)
    highlightLayer.outerGlow = true
    highlightLayer.innerGlow = false
    scene._highlightLayer = highlightLayer

    // Setup camera
    var camera = new UniversalCamera(
      "camera",
      new Vector3(0, 5, -10),
      scene
    )
    camera.setTarget(Vector3.Zero())
    camera.fov = Math.PI / 3
    scene._camera = camera
    
    // Setup skybox
    const skybox = MeshBuilder.CreateSphere("skybox", {diameter: 200}, scene)
    const skyMaterial = new StandardMaterial("skyMaterial", scene)
    skyMaterial.emissiveColor = new Color3(0.3, 0.6, 1.0)
    skyMaterial.diffuseColor = Color3.Black()
    skyMaterial.specularColor = Color3.Black()
    skyMaterial.disableLighting = true
    skyMaterial.backFaceCulling = false
    skybox.material = skyMaterial
    skybox.infiniteDistance = true
    skybox.isPickable = false
    
    // Setup lighting
    const sunLight = new DirectionalLight("sunLight", new Vector3(-1, -1, -1), scene)
    sunLight.diffuse = new Color3(1, 0.95, 0.8)
    sunLight.specular = new Color3(1, 1, 1)
    sunLight.intensity = 2
    
    const ambientLight = new HemisphericLight("ambientLight", new Vector3(0, 1, 0), scene)
    ambientLight.diffuse = new Color3(0.4, 0.6, 1)
    ambientLight.specular = Color3.Black()
    ambientLight.intensity = 0.3
    
    // Setup ground
    const settings = editorStore.settings
    const floorSize = settings.scene?.floorSize || 20
    const ground = MeshBuilder.CreateGround("ground", {width: floorSize, height: floorSize}, scene)
    const groundMaterial = new StandardMaterial("groundMaterial", scene)
    groundMaterial.diffuseColor = new Color3(0.3, 0.3, 0.3)
    groundMaterial.specularColor = Color3.Black()
    ground.material = groundMaterial
    ground.isPickable = false
    
    // Apply render mode function
    scene._applyRenderMode = (mode) => {
      scene.meshes.forEach(mesh => {
        if (mesh.name === 'skybox' || mesh.name === 'ground') return
        if (!mesh.material) return
        
        switch (mode) {
          case 'wireframe':
            mesh.material.wireframe = true
            break
          case 'solid':
            mesh.material.wireframe = false
            break
          case 'material':
            mesh.material.wireframe = false
            break
          case 'rendered':
            mesh.material.wireframe = false
            break
        }
      })
    }
    
    scene.onDisposeObservable.add(() => {
      console.log('Scene disposed')
      setSceneInstance(null)
    })
    
    setSceneInstance(scene)
    sceneActions.updateBabylonScene(scene)
    
    console.log('Scene created and stored in render store:', {
      sceneName: scene ? scene.constructor.name : 'null',
      babylonSceneCurrent: babylonScene?.current ? babylonScene.current.constructor.name : 'null'
    })
    
    return scene
  }
  
  const disposeScene = () => {
    const scene = sceneInstance()
    if (scene && !scene.isDisposed) {
      try {
        scene.dispose()
      } catch (e) {
        console.warn('Error disposing scene:', e)
      }
    }
    setSceneInstance(null)
  }
  
  return {
    sceneInstance,
    createScene,
    disposeScene
  }
}