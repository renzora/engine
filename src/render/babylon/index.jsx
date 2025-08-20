import { IRenderAPI, MaterialType, LightType, PrimitiveType } from '../../api/render/IRenderAPI.jsx';
import { Engine } from '@babylonjs/core/Engines/engine.js';
import { WebGPUEngine } from '@babylonjs/core/Engines/webgpuEngine.js';
import { Scene } from '@babylonjs/core/scene.js';
import { Color3, Color4 } from '@babylonjs/core/Maths/math.color.js';
import { Vector3 } from '@babylonjs/core/Maths/math.vector.js';

// Cameras
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera.js';
import { ArcRotateCamera } from '@babylonjs/core/Cameras/arcRotateCamera.js';
import { FreeCamera } from '@babylonjs/core/Cameras/freeCamera.js';

// Lights
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight.js';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight.js';
import { PointLight } from '@babylonjs/core/Lights/pointLight.js';
import { SpotLight } from '@babylonjs/core/Lights/spotLight.js';

// Materials
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial.js';
import { PBRMaterial } from '@babylonjs/core/Materials/PBR/pbrMaterial.js';

// Meshes
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder.js';
import { Mesh } from '@babylonjs/core/Meshes/mesh.js';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode.js';

// Loaders
import { SceneLoader } from '@babylonjs/core/Loading/sceneLoader.js';
import '@babylonjs/loaders';

// Textures
import { Texture } from '@babylonjs/core/Materials/Textures/texture.js';
import { CubeTexture } from '@babylonjs/core/Materials/Textures/cubeTexture.js';

// Post-processing
import { GlowLayer } from '@babylonjs/core/Layers/glowLayer.js';
import { HighlightLayer } from '@babylonjs/core/Layers/highlightLayer.js';

// Utilities
import { Ray } from '@babylonjs/core/Culling/ray.js';
import { Animation } from '@babylonjs/core/Animations/animation.js';

/**
 * Babylon.js implementation of IRenderAPI
 */
export class BabylonRenderer extends IRenderAPI {
  constructor(canvas, options = {}) {
    super(canvas, options);
    this.renderLoopCallback = null;
    this.animationGroups = new Map();
    this.postEffects = new Map();
    this.nextId = 1;
  }

  // ============= Lifecycle =============

  async initialize() {
    if (this.isInitialized) return;

    try {
      // Try WebGPU first if requested
      if (this.options.preferWebGPU && navigator.gpu) {
        try {
          const webGPUEngine = new WebGPUEngine(this.canvas, {
            adaptToDeviceRatio: true,
            antialias: true,
            ...this.options.engineOptions
          });
          await webGPUEngine.initAsync();
          this.engine = webGPUEngine;
          console.log('[BabylonRenderer] Initialized with WebGPU');
        } catch (err) {
          console.warn('[BabylonRenderer] WebGPU failed, falling back to WebGL:', err);
        }
      }

      // Fall back to WebGL
      if (!this.engine) {
        this.engine = new Engine(this.canvas, true, {
          preserveDrawingBuffer: true,
          stencil: true,
          ...this.options.engineOptions
        });
        console.log('[BabylonRenderer] Initialized with WebGL');
      }

      // Handle resize
      window.addEventListener('resize', this.handleResize);
      
      // Create default scene and camera
      this.createScene();
      
      this.isInitialized = true;
    } catch (error) {
      console.error('[BabylonRenderer] Initialization failed:', error);
      throw error;
    }
  }

  async dispose() {
    if (this.renderLoopCallback) {
      this.stopRenderLoop();
    }

    // Dispose all resources safely
    this.objects.forEach(obj => {
      if (Array.isArray(obj)) {
        // Handle grid lines (array of line meshes)
        obj.forEach(item => {
          if (item && typeof item.dispose === 'function') {
            item.dispose();
          }
        });
      } else if (obj && typeof obj.dispose === 'function') {
        obj.dispose();
      }
    });
    
    this.materials.forEach(mat => {
      if (mat && typeof mat.dispose === 'function') {
        mat.dispose();
      }
    });
    
    this.textures.forEach(tex => {
      if (tex && typeof tex.dispose === 'function') {
        tex.dispose();
      }
    });
    
    this.lights.forEach(light => {
      if (light && typeof light.dispose === 'function') {
        light.dispose();
      }
    });
    
    if (this.scene) {
      this.scene.dispose();
    }
    
    if (this.engine) {
      this.engine.dispose();
    }

    window.removeEventListener('resize', this.handleResize);
    
    this.isInitialized = false;
    console.log('[BabylonRenderer] Disposed');
  }

  handleResize = () => {
    if (this.engine) {
      this.engine.resize();
    }
  }

  resize(width, height) {
    if (this.engine) {
      this.engine.setSize(width, height);
    }
  }

  // ============= Scene Management =============

  createScene(options = {}) {
    this.scene = new Scene(this.engine);
    
    // Set default background
    const bgColor = options.backgroundColor || { r: 0.1, g: 0.1, b: 0.15, a: 1 };
    this.scene.clearColor = new Color4(bgColor.r, bgColor.g, bgColor.b, bgColor.a);
    
    // Create default camera if none exists
    if (!this.scene.activeCamera) {
      const defaultCamera = this.createCamera('perspective', {
        position: { x: 0, y: 5, z: -10 },
        target: { x: 0, y: 0, z: 0 }
      });
      this.setActiveCamera(defaultCamera);
      
      // Add default lighting
      this.createLight(LightType.DIRECTIONAL, {
        direction: { x: 0, y: -1, z: 0.5 },
        position: { x: 10, y: 10, z: 5 },
        intensity: 1
      });
      
      this.createLight(LightType.HEMISPHERIC, {
        direction: { x: 0, y: 1, z: 0 },
        intensity: 0.4
      });
    }
    
    // Enable default features
    if (options.enablePhysics) {
      // Physics would be initialized here
    }
    
    return this.scene;
  }

  clearScene() {
    if (!this.scene) return;
    
    // Dispose all meshes
    this.scene.meshes.forEach(mesh => {
      if (!mesh._isSystemObject) {
        mesh.dispose();
      }
    });
    
    // Clear tracking maps
    this.objects.clear();
    this.materials.clear();
    this.textures.clear();
    this.lights.clear();
  }

  setSceneBackground(color) {
    if (!this.scene) return;
    this.scene.clearColor = new Color4(color.r, color.g, color.b, color.a || 1);
  }

  setFog(fogOptions) {
    if (!this.scene) return;
    
    if (fogOptions.enabled) {
      this.scene.fogMode = Scene.FOGMODE_LINEAR;
      this.scene.fogColor = new Color3(fogOptions.color.r, fogOptions.color.g, fogOptions.color.b);
      this.scene.fogStart = fogOptions.start || 20;
      this.scene.fogEnd = fogOptions.end || 100;
    } else {
      this.scene.fogMode = Scene.FOGMODE_NONE;
    }
  }

  // ============= Camera =============

  createCamera(type, options = {}) {
    if (!this.scene) throw new Error('Scene not created');
    
    let camera;
    const position = options.position || { x: 0, y: 5, z: -10 };
    const target = options.target || { x: 0, y: 0, z: 0 };
    
    switch (type) {
      case 'universal':
        camera = new UniversalCamera('camera', new Vector3(position.x, position.y, position.z), this.scene);
        camera.setTarget(new Vector3(target.x, target.y, target.z));
        break;
        
      case 'arcRotate':
      case 'perspective':
        // Create ArcRotate camera with proper orientation to match other renderers
        const targetVec = new Vector3(target.x, target.y, target.z);
        
        // Calculate radius
        const radius = options.radius || Math.sqrt(
          Math.pow(position.x - target.x, 2) + 
          Math.pow(position.y - target.y, 2) + 
          Math.pow(position.z - target.z, 2)
        );
        
        // Calculate alpha (horizontal angle) and beta (vertical angle) for proper orientation
        const alpha = Math.atan2(position.x - target.x, position.z - target.z);
        const beta = Math.acos((position.y - target.y) / radius);
        
        camera = new ArcRotateCamera(
          'camera',
          alpha,
          beta,
          radius,
          targetVec,
          this.scene
        );
        
        // Ensure proper up vector (Y-up) and invert Y controls to match other renderers
        camera.upVector = new Vector3(0, 1, 0);
        camera.invertRotation = false;
        camera.inputs.attached.pointers.invertY = false;
        
        console.log(`[BabylonRenderer] ArcRotate camera positioned at: (${position.x}, ${position.y}, ${position.z}) with alpha: ${alpha}, beta: ${beta}`);
        break;
        
      case 'free':
        camera = new FreeCamera('camera', new Vector3(position.x, position.y, position.z), this.scene);
        camera.setTarget(new Vector3(target.x, target.y, target.z));
        break;
        
      default:
        throw new Error(`Unknown camera type: ${type}`);
    }
    
    // Set common properties
    if (options.fov) camera.fov = options.fov;
    if (options.minZ) camera.minZ = options.minZ;
    if (options.maxZ) camera.maxZ = options.maxZ;
    
    // Attach controls if canvas available
    if (this.canvas) {
      camera.attachControl(this.canvas, true);
    }
    
    return camera;
  }

  setActiveCamera(camera) {
    if (!this.scene) return;
    this.scene.activeCamera = camera;
    this.activeCamera = camera;
  }

  setCameraPosition(camera, position) {
    if (!camera) return;
    camera.position = new Vector3(position.x, position.y, position.z);
  }

  setCameraTarget(camera, target) {
    if (!camera) return;
    
    if (camera.setTarget) {
      camera.setTarget(new Vector3(target.x, target.y, target.z));
    } else if (camera.target) {
      camera.target = new Vector3(target.x, target.y, target.z);
    }
  }

  // ============= Lighting =============

  createLight(type, options = {}) {
    if (!this.scene) throw new Error('Scene not created');
    
    let light;
    const id = `light_${this.nextId++}`;
    
    switch (type) {
      case LightType.HEMISPHERIC:
        light = new HemisphericLight(
          id,
          new Vector3(options.direction?.x || 0, options.direction?.y || 1, options.direction?.z || 0),
          this.scene
        );
        break;
        
      case LightType.DIRECTIONAL:
        light = new DirectionalLight(
          id,
          new Vector3(options.direction?.x || 0, options.direction?.y || -1, options.direction?.z || 0),
          this.scene
        );
        if (options.position) {
          light.position = new Vector3(options.position.x, options.position.y, options.position.z);
        }
        break;
        
      case LightType.POINT:
        light = new PointLight(
          id,
          new Vector3(options.position?.x || 0, options.position?.y || 5, options.position?.z || 0),
          this.scene
        );
        break;
        
      case LightType.SPOT:
        light = new SpotLight(
          id,
          new Vector3(options.position?.x || 0, options.position?.y || 5, options.position?.z || 0),
          new Vector3(options.direction?.x || 0, options.direction?.y || -1, options.direction?.z || 0),
          options.angle || Math.PI / 4,
          options.exponent || 2,
          this.scene
        );
        break;
        
      default:
        throw new Error(`Unknown light type: ${type}`);
    }
    
    // Set common properties
    if (options.intensity !== undefined) light.intensity = options.intensity;
    if (options.color) light.diffuse = new Color3(options.color.r, options.color.g, options.color.b);
    if (options.specular) light.specular = new Color3(options.specular.r, options.specular.g, options.specular.b);
    
    this.lights.set(id, light);
    return id;
  }

  updateLight(lightId, properties) {
    const light = this.lights.get(lightId);
    if (!light) return;
    
    if (properties.intensity !== undefined) light.intensity = properties.intensity;
    if (properties.color) light.diffuse = new Color3(properties.color.r, properties.color.g, properties.color.b);
    if (properties.position && light.position) {
      light.position = new Vector3(properties.position.x, properties.position.y, properties.position.z);
    }
    if (properties.direction && light.direction) {
      light.direction = new Vector3(properties.direction.x, properties.direction.y, properties.direction.z);
    }
  }

  removeLight(lightId) {
    const light = this.lights.get(lightId);
    if (light) {
      light.dispose();
      this.lights.delete(lightId);
    }
  }

  // ============= Geometry & Meshes =============

  createPrimitive(type, options = {}) {
    if (!this.scene) throw new Error('Scene not created');
    
    let mesh;
    const id = `mesh_${this.nextId++}`;
    
    switch (type) {
      case PrimitiveType.BOX:
        mesh = MeshBuilder.CreateBox(id, {
          size: options.size || 1,
          width: options.width,
          height: options.height,
          depth: options.depth
        }, this.scene);
        break;
        
      case PrimitiveType.SPHERE:
        mesh = MeshBuilder.CreateSphere(id, {
          diameter: options.diameter || 1,
          segments: options.segments || 32
        }, this.scene);
        break;
        
      case PrimitiveType.PLANE:
        mesh = MeshBuilder.CreatePlane(id, {
          size: options.size || 1,
          width: options.width,
          height: options.height
        }, this.scene);
        break;
        
      case PrimitiveType.CYLINDER:
        mesh = MeshBuilder.CreateCylinder(id, {
          height: options.height || 2,
          diameter: options.diameter || 1,
          tessellation: options.tessellation || 24
        }, this.scene);
        break;
        
      case PrimitiveType.TORUS:
        mesh = MeshBuilder.CreateTorus(id, {
          diameter: options.diameter || 1,
          thickness: options.thickness || 0.3,
          tessellation: options.tessellation || 16
        }, this.scene);
        break;
        
      default:
        throw new Error(`Unknown primitive type: ${type}`);
    }
    
    // Apply transform if provided
    if (options.position) {
      mesh.position = new Vector3(options.position.x, options.position.y, options.position.z);
    }
    if (options.rotation) {
      mesh.rotation = new Vector3(options.rotation.x, options.rotation.y, options.rotation.z);
    }
    if (options.scale) {
      mesh.scaling = new Vector3(options.scale.x, options.scale.y, options.scale.z);
    }
    
    this.objects.set(id, mesh);
    return id;
  }

  updateMeshTransform(meshId, transform) {
    const mesh = this.objects.get(meshId);
    if (!mesh) return;
    
    if (transform.position) {
      mesh.position = new Vector3(transform.position.x, transform.position.y, transform.position.z);
    }
    if (transform.rotation) {
      mesh.rotation = new Vector3(transform.rotation.x, transform.rotation.y, transform.rotation.z);
    }
    if (transform.scale) {
      mesh.scaling = new Vector3(transform.scale.x, transform.scale.y, transform.scale.z);
    }
  }

  removeMesh(meshId) {
    const mesh = this.objects.get(meshId);
    if (mesh) {
      mesh.dispose();
      this.objects.delete(meshId);
    }
  }

  // ============= Materials =============

  createMaterial(type, options = {}) {
    if (!this.scene) throw new Error('Scene not created');
    
    let material;
    const id = `material_${this.nextId++}`;
    
    switch (type) {
      case MaterialType.STANDARD:
        material = new StandardMaterial(id, this.scene);
        if (options.diffuseColor) {
          material.diffuseColor = new Color3(options.diffuseColor.r, options.diffuseColor.g, options.diffuseColor.b);
        }
        if (options.specularColor) {
          material.specularColor = new Color3(options.specularColor.r, options.specularColor.g, options.specularColor.b);
        }
        if (options.emissiveColor) {
          material.emissiveColor = new Color3(options.emissiveColor.r, options.emissiveColor.g, options.emissiveColor.b);
        }
        break;
        
      case MaterialType.PBR:
        material = new PBRMaterial(id, this.scene);
        if (options.albedoColor) {
          material.albedoColor = new Color3(options.albedoColor.r, options.albedoColor.g, options.albedoColor.b);
        }
        if (options.metallic !== undefined) material.metallic = options.metallic;
        if (options.roughness !== undefined) material.roughness = options.roughness;
        break;
        
      default:
        throw new Error(`Unknown material type: ${type}`);
    }
    
    // Common properties
    if (options.alpha !== undefined) material.alpha = options.alpha;
    if (options.backFaceCulling !== undefined) material.backFaceCulling = options.backFaceCulling;
    if (options.wireframe !== undefined) material.wireframe = options.wireframe;
    
    this.materials.set(id, material);
    return id;
  }

  applyMaterial(meshId, materialId) {
    const mesh = this.objects.get(meshId);
    const material = this.materials.get(materialId);
    
    if (mesh && material) {
      mesh.material = material;
    }
  }

  // ============= Textures =============

  async loadTexture(url, options = {}) {
    if (!this.scene) throw new Error('Scene not created');
    
    const id = `texture_${this.nextId++}`;
    const texture = new Texture(url, this.scene);
    
    if (options.uScale) texture.uScale = options.uScale;
    if (options.vScale) texture.vScale = options.vScale;
    if (options.uOffset) texture.uOffset = options.uOffset;
    if (options.vOffset) texture.vOffset = options.vOffset;
    
    this.textures.set(id, texture);
    return id;
  }

  applyTexture(materialId, textureId, channel) {
    const material = this.materials.get(materialId);
    const texture = this.textures.get(textureId);
    
    if (!material || !texture) return;
    
    switch (channel) {
      case 'diffuse':
        if (material.diffuseTexture !== undefined) material.diffuseTexture = texture;
        if (material.albedoTexture !== undefined) material.albedoTexture = texture;
        break;
      case 'normal':
        if (material.bumpTexture !== undefined) material.bumpTexture = texture;
        break;
      case 'specular':
        if (material.specularTexture !== undefined) material.specularTexture = texture;
        break;
      case 'emissive':
        if (material.emissiveTexture !== undefined) material.emissiveTexture = texture;
        break;
    }
  }

  // ============= Models & Assets =============

  async loadModel(url, options = {}) {
    if (!this.scene) throw new Error('Scene not created');
    
    return new Promise((resolve, reject) => {
      SceneLoader.LoadAssetContainer(
        url,
        '',
        this.scene,
        (container) => {
          const id = `model_${this.nextId++}`;
          
          // Add all meshes to scene
          container.addAllToScene();
          
          // Store root mesh
          if (container.meshes.length > 0) {
            this.objects.set(id, container.meshes[0]);
          }
          
          resolve(id);
        },
        null,
        (scene, message) => {
          reject(new Error(`Failed to load model: ${message}`));
        }
      );
    });
  }

  // ============= Rendering =============

  render() {
    if (this.scene && this.scene.activeCamera) {
      this.scene.render();
    }
  }

  startRenderLoop(callback) {
    if (!this.engine) return;
    
    this.renderLoopCallback = callback;
    this.engine.runRenderLoop(() => {
      if (callback) callback();
      this.render();
    });
  }

  stopRenderLoop() {
    if (this.engine) {
      this.engine.stopRenderLoop();
    }
    this.renderLoopCallback = null;
  }

  async screenshot(options = {}) {
    if (!this.engine) throw new Error('Engine not initialized');
    
    return new Promise((resolve) => {
      this.engine.onEndFrameObservable.addOnce(() => {
        const canvas = this.engine.getRenderingCanvas();
        canvas.toBlob((blob) => {
          resolve(blob);
        }, options.mimeType || 'image/png', options.quality || 0.95);
      });
    });
  }

  // ============= Utilities =============

  raycast(x, y) {
    if (!this.scene || !this.scene.activeCamera) return null;
    
    const pickResult = this.scene.pick(x, y);
    
    if (pickResult.hit) {
      return {
        hit: true,
        point: {
          x: pickResult.pickedPoint.x,
          y: pickResult.pickedPoint.y,
          z: pickResult.pickedPoint.z
        },
        normal: pickResult.getNormal ? {
          x: pickResult.getNormal().x,
          y: pickResult.getNormal().y,
          z: pickResult.getNormal().z
        } : null,
        mesh: pickResult.pickedMesh,
        distance: pickResult.distance
      };
    }
    
    return null;
  }

  worldToScreen(position) {
    if (!this.scene || !this.scene.activeCamera || !this.engine) return null;
    
    const coordinates = Vector3.Project(
      new Vector3(position.x, position.y, position.z),
      this.scene.activeCamera.getViewMatrix(),
      this.scene.activeCamera.getProjectionMatrix(),
      this.scene.activeCamera.viewport.toGlobal(
        this.engine.getRenderWidth(),
        this.engine.getRenderHeight()
      )
    );
    
    return {
      x: coordinates.x,
      y: coordinates.y
    };
  }

  // ============= Grid & Helpers =============

  createGrid(options = {}) {
    console.log('[BabylonRenderer] Creating line-based grid helper');
    
    const id = `grid_${this.nextId++}`;
    const size = options.size || 10;
    const divisions = options.divisions || 10;
    
    // Create a line-based grid similar to Three.js GridHelper
    const gridPoints = [];
    const halfSize = size / 2;
    const step = size / divisions;
    
    // Create vertical lines
    for (let i = 0; i <= divisions; i++) {
      const x = -halfSize + i * step;
      gridPoints.push([
        new Vector3(x, 0, -halfSize),
        new Vector3(x, 0, halfSize)
      ]);
    }
    
    // Create horizontal lines
    for (let i = 0; i <= divisions; i++) {
      const z = -halfSize + i * step;
      gridPoints.push([
        new Vector3(-halfSize, 0, z),
        new Vector3(halfSize, 0, z)
      ]);
    }
    
    // Create line meshes
    const gridLines = [];
    gridPoints.forEach((points, index) => {
      const line = MeshBuilder.CreateLines(`${id}_line_${index}`, {
        points: points
      }, this.scene);
      
      // Standardized grid colors - identical to Three.js
      line.color = new Color3(0.333, 0.333, 0.333); // 0x555555 equivalent
      line.alpha = 1.0;
      gridLines.push(line);
    });
    
    // Position the grid
    if (options.position) {
      gridLines.forEach(line => {
        line.position = new Vector3(options.position.x, options.position.y, options.position.z);
      });
    }
    
    // Store the grid lines as a group
    this.objects.set(id, gridLines);
    
    return id;
  }

  // ============= Renderer Info =============

  getRendererName() {
    return 'Babylon.js';
  }

  getCapabilities() {
    const caps = this.engine?.getCaps();
    
    return {
      webgl: true,
      webgl2: caps?.version === 2,
      webgpu: this.engine?.constructor.name === 'WebGPUEngine',
      maxTextureSize: caps?.maxTextureSize || 0,
      maxLights: 32,
      supportsInstancing: caps?.instancedArrays || false,
      supportsPhysics: true,
      supportsPostProcessing: true
    };
  }

  getStats() {
    if (!this.engine || !this.scene) {
      return super.getStats();
    }
    
    return {
      fps: this.engine.getFps(),
      frameTime: this.engine.getDeltaTime(),
      drawCalls: this.scene.getActiveMeshes().length,
      triangles: this.scene.getTotalVertices(),
      meshes: this.scene.meshes.length,
      materials: this.scene.materials.length,
      textures: this.scene.textures.length
    };
  }
}