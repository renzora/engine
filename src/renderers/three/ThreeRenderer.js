import { IRenderAPI, MaterialType, LightType, PrimitiveType } from '../api/IRenderAPI.js';
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

/**
 * Three.js implementation of IRenderAPI
 */
export class ThreeRenderer extends IRenderAPI {
  constructor(canvas, options = {}) {
    super(canvas, options);
    this.renderer = null;
    this.animationId = null;
    this.renderLoopCallback = null;
    this.nextId = 1;
    this.controls = null;
  }

  // ============= Lifecycle =============

  async initialize() {
    if (this.isInitialized) return;

    try {
      // Create WebGL renderer
      this.renderer = new THREE.WebGLRenderer({
        canvas: this.canvas,
        antialias: this.options.antialias !== false,
        alpha: this.options.alpha !== false,
        powerPreference: this.options.powerPreference || 'high-performance'
      });

      // Configure renderer
      this.renderer.setSize(this.canvas.clientWidth, this.canvas.clientHeight);
      this.renderer.setPixelRatio(window.devicePixelRatio);
      this.renderer.shadowMap.enabled = true;
      this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      this.renderer.outputColorSpace = THREE.SRGBColorSpace;
      this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
      this.renderer.toneMappingExposure = 1.0;
      
      // Create default scene with test content
      this.createScene();
      this.createCamera('perspective');
      
      // Add basic lighting
      this.createLight('ambient', { intensity: 0.4 });
      this.createLight('directional', { 
        position: { x: 10, y: 10, z: 5 }, 
        intensity: 1,
        castShadow: true 
      });
      
      // Add a test cube
      this.createPrimitive('box', {
        position: { x: 0, y: 0.5, z: 0 },
        color: { r: 0, g: 1, b: 0 },
        castShadow: true,
        receiveShadow: true
      });
      
      // Add ground plane
      this.createPrimitive('plane', {
        width: 10,
        height: 10,
        position: { x: 0, y: 0, z: 0 },
        rotation: { x: -Math.PI / 2, y: 0, z: 0 },
        color: { r: 0.5, g: 0.5, b: 0.5 },
        receiveShadow: true
      });
      
      // Grid will be added via createGrid() method in demo scene
      
      // Start render loop
      this.startRenderLoop();
      
      this.isInitialized = true;
      console.log('[ThreeRenderer] Initialized successfully');
    } catch (error) {
      console.error('[ThreeRenderer] Initialization failed:', error);
      throw error;
    }
  }

  async dispose() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
    }
    
    if (this.controls) {
      this.controls.dispose();
      this.controls = null;
    }
    
    if (this.renderer) {
      this.renderer.dispose();
    }
    
    this.isInitialized = false;
    console.log('[ThreeRenderer] Disposed');
  }

  resize(width, height) {
    if (this.renderer) {
      this.renderer.setSize(width, height);
    }
  }

  // ============= Scene Management =============

  createScene(options = {}) {
    this.scene = new THREE.Scene();
    
    if (options.backgroundColor) {
      this.scene.background = new THREE.Color(options.backgroundColor.r, options.backgroundColor.g, options.backgroundColor.b);
    }
    
    console.log('[ThreeRenderer] Scene created');
    return this.scene;
  }

  clearScene() {
    if (this.scene) {
      this.scene.clear();
    }
    this.objects.clear();
    this.materials.clear();
  }

  setSceneBackground(color) {
    console.log('[ThreeRenderer] Setting background color:', color);
    if (this.scene) {
      this.scene.background = new THREE.Color(color.r, color.g, color.b);
    }
  }

  // ============= Camera =============

  createCamera(type, options = {}) {
    console.log(`[ThreeRenderer] Creating ${type} camera`);
    
    let camera;
    const aspect = options.aspect || this.canvas.clientWidth / this.canvas.clientHeight;
    
    switch (type) {
      case 'perspective':
        camera = new THREE.PerspectiveCamera(
          options.fov || 75,
          aspect,
          options.near || 0.1,
          options.far || 1000
        );
        break;
      case 'orthographic':
        const size = options.size || 10;
        camera = new THREE.OrthographicCamera(
          -size * aspect, size * aspect,
          size, -size,
          options.near || 0.1,
          options.far || 1000
        );
        break;
      default:
        throw new Error(`Unknown camera type: ${type}`);
    }
    
    if (options.position) {
      camera.position.set(options.position.x, options.position.y, options.position.z);
    } else {
      camera.position.set(0, 5, -10);
    }
    
    if (options.lookAt) {
      camera.lookAt(options.lookAt.x, options.lookAt.y, options.lookAt.z);
    }
    
    // Set up orbit controls for perspective camera to match Babylon.js arcRotate behavior
    if (type === 'perspective' && this.canvas) {
      this.controls = new OrbitControls(camera, this.canvas);
      
      // Configure controls to match Babylon.js arcRotate camera
      this.controls.enableDamping = true;
      this.controls.dampingFactor = 0.05;
      this.controls.screenSpacePanning = false;
      this.controls.minDistance = 1;
      this.controls.maxDistance = 100;
      this.controls.maxPolarAngle = Math.PI;
      
      // Set target to match lookAt or default
      if (options.lookAt) {
        this.controls.target.set(options.lookAt.x, options.lookAt.y, options.lookAt.z);
      } else {
        this.controls.target.set(0, 0, 0);
      }
      
      this.controls.update();
    }
    
    this.activeCamera = camera;
    return camera;
  }

  setActiveCamera(camera) {
    this.activeCamera = camera;
  }

  // ============= Lighting =============

  createLight(type, options = {}) {
    console.log(`[ThreeRenderer] Creating ${type} light`);
    
    const id = `light_${this.nextId++}`;
    let light;
    
    switch (type) {
      case 'directional':
        light = new THREE.DirectionalLight(
          options.color ? new THREE.Color(options.color.r, options.color.g, options.color.b) : 0xffffff,
          options.intensity || 1
        );
        if (options.position) {
          light.position.set(options.position.x, options.position.y, options.position.z);
        }
        if (options.castShadow) {
          light.castShadow = true;
          light.shadow.mapSize.width = options.shadowMapSize || 2048;
          light.shadow.mapSize.height = options.shadowMapSize || 2048;
        }
        break;
      case 'point':
        light = new THREE.PointLight(
          options.color ? new THREE.Color(options.color.r, options.color.g, options.color.b) : 0xffffff,
          options.intensity || 1,
          options.distance || 0
        );
        if (options.position) {
          light.position.set(options.position.x, options.position.y, options.position.z);
        }
        break;
      case 'ambient':
        light = new THREE.AmbientLight(
          options.color ? new THREE.Color(options.color.r, options.color.g, options.color.b) : 0x404040,
          options.intensity || 0.4
        );
        break;
      case 'spot':
        light = new THREE.SpotLight(
          options.color ? new THREE.Color(options.color.r, options.color.g, options.color.b) : 0xffffff,
          options.intensity || 1,
          options.distance || 0,
          options.angle || Math.PI / 3
        );
        if (options.position) {
          light.position.set(options.position.x, options.position.y, options.position.z);
        }
        if (options.target) {
          light.target.position.set(options.target.x, options.target.y, options.target.z);
        }
        break;
      default:
        throw new Error(`Unknown light type: ${type}`);
    }
    
    this.lights.set(id, light);
    if (this.scene) {
      this.scene.add(light);
    }
    return id;
  }

  // ============= Geometry & Meshes =============

  createPrimitive(type, options = {}) {
    console.log(`[ThreeRenderer] Creating ${type} primitive`);
    
    const id = `mesh_${this.nextId++}`;
    let geometry;
    
    switch (type) {
      case 'box':
        geometry = new THREE.BoxGeometry(
          options.width || 1,
          options.height || 1,
          options.depth || 1
        );
        break;
      case 'sphere':
        geometry = new THREE.SphereGeometry(
          options.radius || 0.5,
          options.widthSegments || 32,
          options.heightSegments || 16
        );
        break;
      case 'plane':
        geometry = new THREE.PlaneGeometry(
          options.width || 1,
          options.height || 1
        );
        break;
      case 'cylinder':
        geometry = new THREE.CylinderGeometry(
          options.radiusTop || 1,
          options.radiusBottom || 1,
          options.height || 1,
          options.radialSegments || 8
        );
        break;
      default:
        throw new Error(`Unknown primitive type: ${type}`);
    }
    
    const material = options.material || new THREE.MeshStandardMaterial({ 
      color: options.color ? new THREE.Color(options.color.r, options.color.g, options.color.b) : 0xffffff 
    });
    
    const mesh = new THREE.Mesh(geometry, material);
    
    if (options.position) {
      mesh.position.set(options.position.x, options.position.y, options.position.z);
    }
    if (options.rotation) {
      mesh.rotation.set(options.rotation.x, options.rotation.y, options.rotation.z);
    }
    if (options.scale) {
      mesh.scale.set(options.scale.x, options.scale.y, options.scale.z);
    }
    if (options.castShadow) {
      mesh.castShadow = true;
    }
    if (options.receiveShadow) {
      mesh.receiveShadow = true;
    }
    
    this.objects.set(id, mesh);
    if (this.scene) {
      this.scene.add(mesh);
    }
    return id;
  }

  // ============= Materials =============

  createMaterial(type, options = {}) {
    console.log(`[ThreeRenderer] Creating ${type} material`);
    
    const id = `material_${this.nextId++}`;
    let material;
    
    // Convert color if provided
    const materialOptions = { ...options };
    if (options.color) {
      materialOptions.color = new THREE.Color(options.color.r, options.color.g, options.color.b);
    }
    
    switch (type) {
      case 'standard':
        material = new THREE.MeshStandardMaterial(materialOptions);
        break;
      case 'basic':
        material = new THREE.MeshBasicMaterial(materialOptions);
        break;
      case 'physical':
        material = new THREE.MeshPhysicalMaterial(materialOptions);
        break;
      case 'lambert':
        material = new THREE.MeshLambertMaterial(materialOptions);
        break;
      case 'phong':
        material = new THREE.MeshPhongMaterial(materialOptions);
        break;
      default:
        throw new Error(`Unknown material type: ${type}`);
    }
    
    this.materials.set(id, material);
    return id;
  }

  // ============= Rendering =============

  render() {
    if (this.controls) {
      this.controls.update();
    }
    
    if (this.renderer && this.scene && this.activeCamera) {
      this.renderer.render(this.scene, this.activeCamera);
    }
  }

  startRenderLoop(callback) {
    const animate = () => {
      if (callback) callback();
      this.render();
      this.animationId = requestAnimationFrame(animate);
    };
    
    animate();
  }

  stopRenderLoop() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
      this.animationId = null;
    }
  }

  // ============= Utilities =============

  raycast(x, y) {
    if (!this.activeCamera || !this.scene) return null;

    const raycaster = new THREE.Raycaster();
    const mouse = new THREE.Vector2();

    // Convert screen coordinates to normalized device coordinates
    mouse.x = (x / this.canvas.clientWidth) * 2 - 1;
    mouse.y = -(y / this.canvas.clientHeight) * 2 + 1;

    raycaster.setFromCamera(mouse, this.activeCamera);
    const intersects = raycaster.intersectObjects(this.scene.children, true);

    return intersects.length > 0 ? intersects[0] : null;
  }

  worldToScreen(position) {
    if (!this.activeCamera) return { x: 0, y: 0 };

    const vector = new THREE.Vector3(position.x, position.y, position.z);
    vector.project(this.activeCamera);

    // Convert to screen coordinates
    const x = (vector.x * 0.5 + 0.5) * this.canvas.clientWidth;
    const y = (vector.y * -0.5 + 0.5) * this.canvas.clientHeight;

    return { x, y };
  }

  // ============= Grid & Helpers =============

  createGrid(options = {}) {
    console.log('[ThreeRenderer] Creating line-based grid to match Babylon.js');
    
    const id = `grid_${this.nextId++}`;
    const size = options.size || 10;
    const divisions = options.divisions || 10;
    
    // Create a line-based grid identical to Babylon.js implementation
    const gridPoints = [];
    const halfSize = size / 2;
    const step = size / divisions;
    
    // Create vertical lines
    for (let i = 0; i <= divisions; i++) {
      const x = -halfSize + i * step;
      gridPoints.push(
        new THREE.Vector3(x, 0, -halfSize),
        new THREE.Vector3(x, 0, halfSize)
      );
    }
    
    // Create horizontal lines  
    for (let i = 0; i <= divisions; i++) {
      const z = -halfSize + i * step;
      gridPoints.push(
        new THREE.Vector3(-halfSize, 0, z),
        new THREE.Vector3(halfSize, 0, z)
      );
    }
    
    // Create line geometry
    const geometry = new THREE.BufferGeometry().setFromPoints(gridPoints);
    
    // Create material that matches Babylon.js grid appearance
    const material = new THREE.LineBasicMaterial({ 
      color: 0x555555,  // Same as Babylon.js line color
      transparent: true,
      opacity: 1.0
    });
    
    const gridLines = new THREE.LineSegments(geometry, material);
    
    if (options.position) {
      gridLines.position.set(options.position.x, options.position.y, options.position.z);
    }
    
    this.objects.set(id, gridLines);
    if (this.scene) {
      this.scene.add(gridLines);
    }
    
    return id;
  }

  // ============= Renderer Info =============

  getRendererName() {
    return 'Three.js';
  }

  getCapabilities() {
    const gl = this.renderer?.getContext();
    return {
      webgl: !!gl,
      webgl2: gl instanceof WebGL2RenderingContext,
      webgpu: false,
      maxTextureSize: gl ? gl.getParameter(gl.MAX_TEXTURE_SIZE) : 4096,
      maxLights: 16,
      supportsInstancing: true,
      supportsPhysics: false,
      supportsPostProcessing: true
    };
  }

  getStats() {
    const info = this.renderer ? this.renderer.info : { render: { calls: 0, triangles: 0 }, memory: { textures: 0, geometries: 0 } };
    return {
      fps: 60, // Would need to calculate actual FPS
      frameTime: 16.67,
      drawCalls: info.render.calls,
      triangles: info.render.triangles,
      meshes: this.objects.size,
      materials: this.materials.size,
      textures: info.memory.textures
    };
  }
}