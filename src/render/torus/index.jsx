import { IRenderAPI } from '../../api/render/IRenderAPI.jsx';
import { WebGLContext } from './core/WebGLContext.jsx';
import { ShaderManager } from './shaders/ShaderManager.jsx';
import { GeometryManager } from './geometry/GeometryManager.jsx';
import { MaterialManager } from './materials/MaterialManager.jsx';
import { SceneManager } from './scene/SceneManager.jsx';
import { CameraController } from './camera/CameraController.jsx';
import { GizmoManager } from './gizmos/GizmoManager.jsx';
import { RaycastUtils } from './math/RaycastUtils.jsx';
import { MathUtils } from './math/MathUtils.jsx';

/**
 * Torus - Modular WebGL2 Graphics Engine
 * Core renderer that orchestrates all subsystems
 */
export class TorusRenderer extends IRenderAPI {
  constructor(canvas, options = {}) {
    super(canvas, options);
    
    // Core subsystems
    this.webgl = new WebGLContext(canvas, options);
    this.shaders = new ShaderManager();
    this.geometry = new GeometryManager();
    this.materials = new MaterialManager();
    this.scene = new SceneManager();
    this.cameraController = null; // Will be initialized after canvas is ready
    this.gizmoManager = null; // Will be initialized after canvas is ready
    
    // Rendering state
    this.renderQueue = [];
    this.nextId = 1;
    this.animationId = null;
    
    console.log('[Torus] Modular renderer initialized');
  }

  // ============= Lifecycle =============

  async initialize() {
    if (this.isInitialized) return;

    try {
      // Initialize subsystems in order
      await this.webgl.initialize();
      await this.shaders.initialize(this.webgl.gl);
      await this.geometry.initialize(this.webgl.gl);
      await this.materials.initialize(this.webgl.gl);
      await this.scene.initialize();
      
      // Create default scene
      this.createScene();
      this.createCamera('perspective');
      
      // Add default lighting
      this.createLight('ambient', { intensity: 0.4 });
      this.createLight('directional', { 
        position: { x: 10, y: 10, z: 5 }, 
        intensity: 1 
      });
      
      // Initialize camera controls
      this.cameraController = new CameraController(this.webgl.canvas, this.scene);
      
      // Initialize gizmo system
      this.gizmoManager = new GizmoManager(this);
      this.setupGizmoControls();
      
      // Start render loop
      this.startRenderLoop();
      
      this.isInitialized = true;
      console.log('[Torus] Modular renderer initialized successfully with camera controls');
      
    } catch (error) {
      console.error('[Torus] Initialization failed:', error);
      throw error;
    }
  }

  async dispose() {
    if (this.animationId) {
      cancelAnimationFrame(this.animationId);
    }
    
    // Dispose camera controller
    if (this.cameraController) {
      this.cameraController.dispose();
    }
    
    // Dispose gizmo manager
    if (this.gizmoManager) {
      this.gizmoManager.dispose();
    }
    
    // Dispose subsystems in reverse order
    await this.scene.dispose();
    await this.materials.dispose();
    await this.geometry.dispose();
    await this.shaders.dispose();
    await this.webgl.dispose();
    
    this.isInitialized = false;
    console.log('[Torus] Disposed');
  }

  resize(width, height) {
    this.webgl.resize(width, height);
  }

  // ============= API Implementation =============

  createScene(options = {}) {
    return this.scene.createScene(options);
  }

  clearScene() {
    this.scene.clearScene();
    this.renderQueue = [];
  }

  setSceneBackground(color) {
    this.webgl.setBackgroundColor(color);
  }

  createCamera(type, options = {}) {
    return this.scene.createCamera(type, options);
  }

  setActiveCamera(camera) {
    this.scene.setActiveCamera(camera);
  }

  createLight(type, options = {}) {
    const id = `light_${this.nextId++}`;
    this.scene.createLight(id, type, options);
    return id;
  }

  createPrimitive(type, options = {}) {
    const id = `mesh_${this.nextId++}`;
    
    // Create geometry
    const geometry = this.geometry.createPrimitive(type, options);
    
    // Create WebGL buffers
    const buffers = this.createMeshBuffers(geometry);
    
    // Create mesh object
    const mesh = this.scene.createMesh(id, geometry, options);
    mesh.vertexBuffer = buffers.vertexBuffer;
    mesh.indexBuffer = buffers.indexBuffer;
    
    // Add to render queue
    this.renderQueue.push(mesh);
    
    console.log(`[Torus] Created ${type} primitive: ${id} with ${geometry.vertexCount} triangles`);
    return id;
  }

  createMeshBuffers(geometry) {
    const gl = this.webgl.getContext();
    
    // Create vertex buffer
    const vertexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vertexBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, geometry.vertices, gl.STATIC_DRAW);
    
    // Create index buffer
    const indexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, geometry.indices, gl.STATIC_DRAW);
    
    return { vertexBuffer, indexBuffer };
  }

  createMaterial(type, options = {}) {
    const id = `material_${this.nextId++}`;
    this.materials.createMaterial(id, type, options);
    return id;
  }

  createGrid(options = {}) {
    const id = `grid_${this.nextId++}`;
    
    // Create grid geometry
    const geometry = this.geometry.createGrid(options);
    
    // Create grid mesh
    const mesh = this.scene.createMesh(id, geometry, {
      ...options,
      type: 'grid'
    });
    
    this.renderQueue.push(mesh);
    return id;
  }

  // ============= Rendering =============

  render() {
    if (!this.webgl.isReady()) return;
    
    const gl = this.webgl.getContext();
    
    // Clear framebuffer
    this.webgl.clear();
    
    // Get rendering matrices
    const viewMatrix = this.scene.getViewMatrix();
    const projectionMatrix = this.scene.getProjectionMatrix();
    
    // Update projection matrix with current canvas size
    const size = this.webgl.getCanvasSize();
    this.scene.updateProjectionMatrix(size.width, size.height);
    
    // Use basic shader program
    const program = this.shaders.useProgram('basic');
    
    // Set global uniforms
    this.setGlobalUniforms(program, viewMatrix, projectionMatrix);
    
    // Render each mesh
    for (const mesh of this.renderQueue) {
      if (mesh.visible) {
        this.renderMesh(mesh, program);
      }
    }
  }

  setGlobalUniforms(program, viewMatrix, projectionMatrix) {
    // Set matrices
    this.shaders.setUniformMatrix4fv(program, 'u_viewMatrix', viewMatrix);
    this.shaders.setUniformMatrix4fv(program, 'u_projectionMatrix', projectionMatrix);
    
    // Set lighting
    this.shaders.setUniform3f(program, 'u_lightDirection', 0.5, -1.0, 0.5);
    this.shaders.setUniform3f(program, 'u_lightColor', 1.0, 1.0, 1.0);
    this.shaders.setUniform3f(program, 'u_ambientLight', 0.3, 0.3, 0.3);
  }

  renderMesh(mesh, program) {
    const gl = this.webgl.getContext();
    
    // Bind buffers
    gl.bindBuffer(gl.ARRAY_BUFFER, mesh.vertexBuffer);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, mesh.indexBuffer);
    
    // Set up vertex attributes
    this.setupVertexAttributes(program);
    
    // Create world matrix
    const worldMatrix = this.createWorldMatrix(mesh);
    this.shaders.setUniformMatrix4fv(program, 'u_worldMatrix', worldMatrix);
    
    // Create normal matrix (simplified for now)
    const normalMatrix = new Float32Array(9);
    MathUtils.normalMatrix(normalMatrix, worldMatrix);
    this.shaders.setUniformMatrix3fv(program, 'u_normalMatrix', normalMatrix);
    
    // Set mesh color
    this.shaders.setUniform3f(program, 'u_color', mesh.color.r, mesh.color.g, mesh.color.b);
    
    // Draw based on geometry type
    if (mesh.geometry.primitive === 'lines') {
      gl.drawElements(gl.LINES, mesh.geometry.vertexCount, gl.UNSIGNED_SHORT, 0);
    } else {
      gl.drawElements(gl.TRIANGLES, mesh.geometry.vertexCount, gl.UNSIGNED_SHORT, 0);
    }
  }

  setupVertexAttributes(program) {
    const gl = this.webgl.getContext();
    
    // Vertex format: position(3) + normal(3) + texCoord(2) = 8 floats
    const stride = 8 * 4;
    
    const positionLocation = this.shaders.enableAttribute(program, 'a_position');
    this.shaders.setAttributePointer(positionLocation, 3, gl.FLOAT, false, stride, 0);
    
    const normalLocation = this.shaders.enableAttribute(program, 'a_normal');
    this.shaders.setAttributePointer(normalLocation, 3, gl.FLOAT, false, stride, 3 * 4);
    
    const texCoordLocation = this.shaders.enableAttribute(program, 'a_texCoord');
    this.shaders.setAttributePointer(texCoordLocation, 2, gl.FLOAT, false, stride, 6 * 4);
  }

  createWorldMatrix(mesh) {
    const matrix = MathUtils.createMatrix();
    
    // Start with identity
    MathUtils.identity(matrix);
    
    // Apply rotation (proper 3D rotation)
    if (mesh.rotation && (mesh.rotation.x !== 0 || mesh.rotation.y !== 0 || mesh.rotation.z !== 0)) {
      // Create rotation matrices for each axis
      const rx = mesh.rotation.x;
      const ry = mesh.rotation.y;
      const rz = mesh.rotation.z;
      
      const cosX = Math.cos(rx), sinX = Math.sin(rx);
      const cosY = Math.cos(ry), sinY = Math.sin(ry);
      const cosZ = Math.cos(rz), sinZ = Math.sin(rz);
      
      // Combined rotation matrix (ZYX order)
      matrix[0] = cosY * cosZ;
      matrix[1] = sinX * sinY * cosZ - cosX * sinZ;
      matrix[2] = cosX * sinY * cosZ + sinX * sinZ;
      matrix[3] = 0;
      
      matrix[4] = cosY * sinZ;
      matrix[5] = sinX * sinY * sinZ + cosX * cosZ;
      matrix[6] = cosX * sinY * sinZ - sinX * cosZ;
      matrix[7] = 0;
      
      matrix[8] = -sinY;
      matrix[9] = sinX * cosY;
      matrix[10] = cosX * cosY;
      matrix[11] = 0;
      
      matrix[12] = 0;
      matrix[13] = 0;
      matrix[14] = 0;
      matrix[15] = 1;
    }
    
    // Apply scale
    matrix[0] *= mesh.scale.x;
    matrix[4] *= mesh.scale.x;
    matrix[8] *= mesh.scale.x;
    
    matrix[1] *= mesh.scale.y;
    matrix[5] *= mesh.scale.y;
    matrix[9] *= mesh.scale.y;
    
    matrix[2] *= mesh.scale.z;
    matrix[6] *= mesh.scale.z;
    matrix[10] *= mesh.scale.z;
    
    // Apply translation
    matrix[12] = mesh.position.x;
    matrix[13] = mesh.position.y;
    matrix[14] = mesh.position.z;
    
    return matrix;
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

  // ============= Camera Controls =============

  resetCamera() {
    if (this.cameraController) {
      this.cameraController.reset();
    }
  }

  setCameraTarget(x, y, z) {
    if (this.cameraController) {
      this.cameraController.setTarget(x, y, z);
    }
  }

  setCameraDistance(distance) {
    if (this.cameraController) {
      this.cameraController.setDistance(distance);
    }
  }

  // ============= Animation Support =============

  getRenderQueue() {
    return this.renderQueue;
  }

  updateMeshRotation(meshId, rotation) {
    const mesh = this.renderQueue.find(m => m.id === meshId);
    if (mesh) {
      mesh.rotation = rotation;
    }
  }

  // ============= Gizmo System =============

  setupGizmoControls() {
    if (!this.webgl.canvas || !this.gizmoManager) return;
    
    const canvas = this.webgl.canvas;
    
    // Add unified event handlers that properly separate gizmo and camera interactions
    let mouseDownPos = { x: 0, y: 0 };
    let hasMouseMoved = false;
    const clickThreshold = 3; // pixels
    
    canvas.addEventListener('mousedown', (e) => {
      mouseDownPos = { x: e.clientX, y: e.clientY };
      hasMouseMoved = false;
      
      // Try gizmo first, then camera
      if (!this.gizmoManager.onMouseDown(e, this.scene)) {
        this.cameraController.onMouseDown(e);
      }
      e.preventDefault();
    });
    
    canvas.addEventListener('mousemove', (e) => {
      // Track if mouse has moved significantly
      const deltaX = Math.abs(e.clientX - mouseDownPos.x);
      const deltaY = Math.abs(e.clientY - mouseDownPos.y);
      if (deltaX > clickThreshold || deltaY > clickThreshold) {
        hasMouseMoved = true;
      }
      
      // Try gizmo first, then camera
      if (!this.gizmoManager.onMouseMove(e, this.scene)) {
        this.cameraController.onMouseMove(e);
      }
      e.preventDefault();
    });
    
    canvas.addEventListener('mouseup', (e) => {
      // Handle gizmo mouseup first
      const gizmoHandled = this.gizmoManager.onMouseUp(e);
      
      // Only handle mesh selection if it was a click (not a drag) and gizmo didn't handle it
      if (!hasMouseMoved && !gizmoHandled && e.button === 0) {
        this.selectMeshForGizmo(e);
      }
      
      // Always try camera controller for mouseup
      if (!gizmoHandled) {
        this.cameraController.onMouseUp(e);
      }
      
      e.preventDefault();
    });
    
    // Handle mouse wheel separately (always for camera)
    canvas.addEventListener('wheel', (e) => {
      this.cameraController.onWheel(e);
    });
  }

  selectMeshForGizmo(event) {
    // Use ray casting to select the object under the mouse
    const canvas = this.webgl.canvas;
    const viewMatrix = this.scene.getViewMatrix();
    const projectionMatrix = this.scene.getProjectionMatrix();
    
    // Get selectable meshes (exclude gizmos and grid)
    const selectableMeshes = this.renderQueue.filter(mesh => 
      !mesh.id.includes('gizmo') && 
      mesh.type !== 'grid' &&
      mesh.id.includes('mesh_') // Only select actual objects
    );
    
    // Create ray from mouse position
    const ray = RaycastUtils.createRayFromMouse(
      event.clientX,
      event.clientY,
      canvas,
      viewMatrix,
      projectionMatrix
    );
    
    // Debug ray
    if (selectableMeshes.length > 0) {
      console.log('[Ray] Origin:', ray.origin, 'Direction:', ray.direction);
      console.log('[Ray] Testing against', selectableMeshes.length, 'meshes');
      selectableMeshes.forEach(mesh => {
        console.log('[Ray] Mesh:', mesh.id, 'at', mesh.position);
      });
    }
    
    let closestHit = null;
    let closestDistance = Infinity;
    
    // Test ray against each selectable mesh
    selectableMeshes.forEach(mesh => {
      let distance = null;
      
      // Test based on primitive type
      if (mesh.id.includes('box')) {
        // Box: use AABB
        const size = 1; // Default box size
        const halfSize = size / 2;
        const aabb = {
          min: [mesh.position.x - halfSize, mesh.position.y - halfSize, mesh.position.z - halfSize],
          max: [mesh.position.x + halfSize, mesh.position.y + halfSize, mesh.position.z + halfSize]
        };
        distance = RaycastUtils.rayAABBIntersection(ray, aabb.min, aabb.max);
      } else if (mesh.id.includes('sphere')) {
        // Sphere: use sphere intersection
        distance = RaycastUtils.raySphereIntersection(
          ray,
          [mesh.position.x, mesh.position.y, mesh.position.z],
          0.5 // Default sphere radius
        );
      } else if (mesh.id.includes('cylinder')) {
        // Cylinder: approximate as AABB for now
        const radius = 0.5;
        const height = 1;
        const halfHeight = height / 2;
        const aabb = {
          min: [mesh.position.x - radius, mesh.position.y - halfHeight, mesh.position.z - radius],
          max: [mesh.position.x + radius, mesh.position.y + halfHeight, mesh.position.z + radius]
        };
        distance = RaycastUtils.rayAABBIntersection(ray, aabb.min, aabb.max);
      } else if (mesh.id.includes('torus')) {
        // Torus: approximate as sphere for now
        distance = RaycastUtils.raySphereIntersection(
          ray,
          [mesh.position.x, mesh.position.y, mesh.position.z],
          1.1 // Approximate torus outer radius
        );
      }
      
      // Track closest hit
      if (distance !== null && distance > 0 && distance < closestDistance) {
        closestDistance = distance;
        closestHit = mesh;
      }
    });
    
    // Select the closest hit mesh
    if (closestHit) {
      this.gizmoManager.selectMesh(closestHit);
      console.log('[Gizmo] Ray cast selected mesh:', closestHit.id, 'at distance:', closestDistance);
    } else {
      // If no hit, deselect current mesh
      this.gizmoManager.deselectMesh();
      console.log('[Gizmo] No mesh hit by ray, deselecting');
    }
  }

  getSelectedMesh() {
    return this.gizmoManager?.selectedMesh || null;
  }

  selectMesh(meshId) {
    const mesh = this.renderQueue.find(m => m.id === meshId);
    if (mesh && this.gizmoManager) {
      this.gizmoManager.selectMesh(mesh);
    }
  }

  // ============= Renderer Info =============

  getRendererName() {
    return 'Torus';
  }

  getCapabilities() {
    return this.webgl.getCapabilities();
  }

  getStats() {
    return {
      fps: 60, // Will calculate actual FPS later
      frameTime: 16.67,
      drawCalls: this.renderQueue.length,
      triangles: this.geometry.getTotalTriangles(),
      meshes: this.objects.size,
      materials: this.materials.getCount(),
      textures: 0 // TODO: implement texture counting
    };
  }
}