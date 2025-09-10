// === DEBUG API MODULE ===

import {
  AxesViewer,
  BoneAxesViewer,
  PhysicsViewer,
  Vector3,
  Color3,
  StandardMaterial,
  LinesMesh,
  MeshBuilder,
  Ray,
  RayHelper,
  BoundingBoxGizmo,
  PositionGizmo,
  RotationGizmo,
  ScaleGizmo,
  GizmoManager,
  UtilityLayerRenderer,
  DynamicTexture,
  Mesh,
  Tools,
  SkeletonViewer
} from '@babylonjs/core';

import { AdvancedDynamicTexture, TextBlock, Rectangle } from '@babylonjs/gui';
import '@babylonjs/inspector';

export class DebugAPI {
  constructor(scene) {
    this.scene = scene;
    this.debugOverlay = null;
    this.debugTexts = new Map();
    this.gizmoManager = null;
  }

  // === INSPECTOR ===

  showInspector() {
    if (window.BABYLON && window.BABYLON.Inspector) {
      window.BABYLON.Inspector.Show(this.scene, {});
      return true;
    }
    return false;
  }

  hideInspector() {
    if (window.BABYLON && window.BABYLON.Inspector) {
      window.BABYLON.Inspector.Hide();
      return true;
    }
    return false;
  }

  toggleInspector() {
    if (window.BABYLON && window.BABYLON.Inspector) {
      if (window.BABYLON.Inspector.IsVisible) {
        this.hideInspector();
      } else {
        this.showInspector();
      }
      return true;
    }
    return false;
  }

  // === DEBUG OVERLAY ===

  createDebugOverlay() {
    if (!this.debugOverlay) {
      this.debugOverlay = AdvancedDynamicTexture.CreateFullscreenUI('debugOverlay');
    }
    return this.debugOverlay;
  }

  addDebugText(key, text, x = 10, y = 10, fontSize = 16, color = 'lime') {
    const overlay = this.createDebugOverlay();
    
    // Remove existing text with same key
    if (this.debugTexts.has(key)) {
      overlay.removeControl(this.debugTexts.get(key));
    }
    
    const textBlock = new TextBlock(key, text);
    textBlock.fontSize = fontSize;
    textBlock.color = color;
    textBlock.textHorizontalAlignment = TextBlock.HORIZONTAL_ALIGNMENT_LEFT;
    textBlock.textVerticalAlignment = TextBlock.VERTICAL_ALIGNMENT_TOP;
    textBlock.leftInPixels = x;
    textBlock.topInPixels = y;
    
    overlay.addControl(textBlock);
    this.debugTexts.set(key, textBlock);
    
    return textBlock;
  }

  updateDebugText(key, text) {
    if (this.debugTexts.has(key)) {
      this.debugTexts.get(key).text = text;
      return true;
    }
    return false;
  }

  removeDebugText(key) {
    if (this.debugTexts.has(key)) {
      const textBlock = this.debugTexts.get(key);
      if (this.debugOverlay) {
        this.debugOverlay.removeControl(textBlock);
      }
      this.debugTexts.delete(key);
      return true;
    }
    return false;
  }

  clearDebugTexts() {
    this.debugTexts.forEach((textBlock, key) => {
      if (this.debugOverlay) {
        this.debugOverlay.removeControl(textBlock);
      }
    });
    this.debugTexts.clear();
    return true;
  }

  // === PERFORMANCE MONITORING ===

  showFPS() {
    const updateFPS = () => {
      const fps = Math.round(this.scene.getEngine().getFps());
      this.updateDebugText('fps', `FPS: ${fps}`);
    };
    
    this.addDebugText('fps', 'FPS: --', 10, 10);
    this.scene.registerBeforeRender(updateFPS);
    
    return true;
  }

  showFrameTime() {
    const updateFrameTime = () => {
      const frameTime = this.scene.getEngine().getDeltaTime().toFixed(2);
      this.updateDebugText('frameTime', `Frame Time: ${frameTime}ms`);
    };
    
    this.addDebugText('frameTime', 'Frame Time: --', 10, 30);
    this.scene.registerBeforeRender(updateFrameTime);
    
    return true;
  }

  showMemoryUsage() {
    const updateMemory = () => {
      if (performance.memory) {
        const used = Math.round(performance.memory.usedJSHeapSize / 1024 / 1024);
        const total = Math.round(performance.memory.totalJSHeapSize / 1024 / 1024);
        this.updateDebugText('memory', `Memory: ${used}/${total} MB`);
      }
    };
    
    this.addDebugText('memory', 'Memory: --', 10, 50);
    this.scene.registerBeforeRender(updateMemory);
    
    return true;
  }

  showDrawCalls() {
    const updateDrawCalls = () => {
      const drawCalls = this.scene.getEngine().drawCalls;
      this.updateDebugText('drawCalls', `Draw Calls: ${drawCalls}`);
    };
    
    this.addDebugText('drawCalls', 'Draw Calls: --', 10, 70);
    this.scene.registerBeforeRender(updateDrawCalls);
    
    return true;
  }

  // === VISUAL DEBUG HELPERS ===

  showAxes(mesh, size = 1.0) {
    if (!mesh) return null;
    
    const axes = new AxesViewer(this.scene, size);
    axes.mesh.parent = mesh;
    return axes;
  }

  hideAxes(axesViewer) {
    if (axesViewer) {
      axesViewer.dispose();
      return true;
    }
    return false;
  }

  showBoundingBox(mesh, color = [1, 1, 0]) {
    if (!mesh) return null;
    
    const boundingBox = mesh.getBoundingInfo().boundingBox;
    const min = boundingBox.minimum;
    const max = boundingBox.maximum;
    
    const lines = [
      // Bottom face
      [min, new Vector3(max.x, min.y, min.z)],
      [new Vector3(max.x, min.y, min.z), new Vector3(max.x, min.y, max.z)],
      [new Vector3(max.x, min.y, max.z), new Vector3(min.x, min.y, max.z)],
      [new Vector3(min.x, min.y, max.z), min],
      
      // Top face
      [new Vector3(min.x, max.y, min.z), new Vector3(max.x, max.y, min.z)],
      [new Vector3(max.x, max.y, min.z), max],
      [max, new Vector3(min.x, max.y, max.z)],
      [new Vector3(min.x, max.y, max.z), new Vector3(min.x, max.y, min.z)],
      
      // Vertical edges
      [min, new Vector3(min.x, max.y, min.z)],
      [new Vector3(max.x, min.y, min.z), new Vector3(max.x, max.y, min.z)],
      [new Vector3(max.x, min.y, max.z), max],
      [new Vector3(min.x, min.y, max.z), new Vector3(min.x, max.y, max.z)]
    ];
    
    const boundingBoxMesh = MeshBuilder.CreateLineSystem(`${mesh.name}_boundingBox`, {
      lines: lines
    }, this.scene);
    
    const material = new StandardMaterial(`${mesh.name}_boundingBox_mat`, this.scene);
    material.emissiveColor = new Color3(...color);
    boundingBoxMesh.material = material;
    boundingBoxMesh.parent = mesh;
    
    return boundingBoxMesh;
  }

  showWireframe(mesh, color = [0, 1, 0]) {
    if (!mesh || !mesh.material) return false;
    
    mesh.material.wireframe = true;
    if (mesh.material.emissiveColor) {
      mesh.material.emissiveColor = new Color3(...color);
    }
    return true;
  }

  hideWireframe(mesh) {
    if (!mesh || !mesh.material) return false;
    mesh.material.wireframe = false;
    return true;
  }

  // === RAY DEBUGGING ===

  showRay(origin, direction, length = 10, color = [1, 0, 0]) {
    const ray = new Ray(new Vector3(...origin), new Vector3(...direction), length);
    const rayHelper = new RayHelper(ray);
    rayHelper.show(this.scene, new Color3(...color));
    return rayHelper;
  }

  hideRay(rayHelper) {
    if (rayHelper) {
      rayHelper.hide();
      return true;
    }
    return false;
  }

  // === GIZMOS ===

  createGizmoManager() {
    if (!this.gizmoManager) {
      this.gizmoManager = new GizmoManager(this.scene);
    }
    return this.gizmoManager;
  }

  showPositionGizmo(mesh) {
    if (!mesh) return null;
    
    const gizmoManager = this.createGizmoManager();
    gizmoManager.positionGizmoEnabled = true;
    gizmoManager.attachToMesh(mesh);
    
    return gizmoManager.gizmos.positionGizmo;
  }

  showRotationGizmo(mesh) {
    if (!mesh) return null;
    
    const gizmoManager = this.createGizmoManager();
    gizmoManager.rotationGizmoEnabled = true;
    gizmoManager.attachToMesh(mesh);
    
    return gizmoManager.gizmos.rotationGizmo;
  }

  showScaleGizmo(mesh) {
    if (!mesh) return null;
    
    const gizmoManager = this.createGizmoManager();
    gizmoManager.scaleGizmoEnabled = true;
    gizmoManager.attachToMesh(mesh);
    
    return gizmoManager.gizmos.scaleGizmo;
  }

  showBoundingBoxGizmo(mesh) {
    if (!mesh) return null;
    
    const gizmoManager = this.createGizmoManager();
    gizmoManager.boundingBoxGizmoEnabled = true;
    gizmoManager.attachToMesh(mesh);
    
    return gizmoManager.gizmos.boundingBoxGizmo;
  }

  boundingBoxGizmo(mesh = null) {
    // If no mesh is provided, use the current babylonObject (this context)
    const targetMesh = mesh || this.babylonObject;
    return this.showBoundingBoxGizmo(targetMesh);
  }

  hideAllGizmos() {
    if (this.gizmoManager) {
      this.gizmoManager.positionGizmoEnabled = false;
      this.gizmoManager.rotationGizmoEnabled = false;
      this.gizmoManager.scaleGizmoEnabled = false;
      this.gizmoManager.boundingBoxGizmoEnabled = false;
      return true;
    }
    return false;
  }

  // === PHYSICS DEBUG ===

  showPhysicsImpostors() {
    if (!this.scene.physicsEngine) return false;
    
    const physicsViewer = new PhysicsViewer(this.scene);
    
    this.scene.meshes.forEach(mesh => {
      if (mesh.physicsImpostor) {
        physicsViewer.showImpostor(mesh.physicsImpostor);
      }
    });
    
    return physicsViewer;
  }

  hidePhysicsImpostors(physicsViewer) {
    if (physicsViewer) {
      physicsViewer.dispose();
      return true;
    }
    return false;
  }

  // === SKELETON DEBUG ===

  showSkeleton(skeleton, size = 0.1) {
    if (!skeleton) return null;
    
    const viewer = new SkeletonViewer(skeleton, skeleton.transformMatrices[0], this.scene, false, 3, {
      pauseAnimations: false,
      returnToRest: false,
      displayMode: SkeletonViewer.DISPLAY_LINES,
      displayOptions: {
        sphereBaseSize: size,
        sphereScaleUnit: 2,
        sphereFactor: 0.865,
        midStep: 0.235,
        midStepFactor: 0.155
      }
    });
    
    return viewer;
  }

  hideSkeleton(skeletonViewer) {
    if (skeletonViewer) {
      skeletonViewer.dispose();
      return true;
    }
    return false;
  }

  showBoneAxes(skeleton, size = 0.1) {
    if (!skeleton) return [];
    
    const boneAxes = [];
    skeleton.bones.forEach((bone, index) => {
      const axes = new BoneAxesViewer(this.scene, bone, skeleton, size);
      boneAxes.push(axes);
    });
    
    return boneAxes;
  }

  // === SCENE DEBUG INFO ===

  getSceneInfo() {
    const info = {
      meshCount: this.scene.meshes.length,
      lightCount: this.scene.lights.length,
      cameraCount: this.scene.cameras.length,
      materialCount: this.scene.materials.length,
      textureCount: this.scene.textures.length,
      animationGroupCount: this.scene.animationGroups.length,
      particleSystemCount: this.scene.particleSystems.length,
      soundCount: this.scene.sounds ? this.scene.sounds.length : 0,
      totalVertices: 0,
      totalTriangles: 0,
      drawCalls: this.scene.getEngine().drawCalls,
      activeCamera: this.scene.activeCamera ? this.scene.activeCamera.name : 'none',
      physicsEnabled: !!this.scene.physicsEngine
    };
    
    // Calculate total geometry
    this.scene.meshes.forEach(mesh => {
      if (mesh.getTotalVertices) {
        info.totalVertices += mesh.getTotalVertices();
      }
      if (mesh.getTotalIndices) {
        info.totalTriangles += mesh.getTotalIndices() / 3;
      }
    });
    
    return info;
  }

  showSceneInfo(x = 10, y = 100) {
    const updateInfo = () => {
      const info = this.getSceneInfo();
      const text = [
        `Meshes: ${info.meshCount}`,
        `Vertices: ${info.totalVertices}`,
        `Triangles: ${Math.floor(info.totalTriangles)}`,
        `Draw Calls: ${info.drawCalls}`,
        `Materials: ${info.materialCount}`,
        `Textures: ${info.textureCount}`,
        `Lights: ${info.lightCount}`,
        `Cameras: ${info.cameraCount}`,
        `Particles: ${info.particleSystemCount}`,
        `Sounds: ${info.soundCount}`,
        `Physics: ${info.physicsEnabled ? 'ON' : 'OFF'}`
      ].join('\n');
      
      this.updateDebugText('sceneInfo', text);
    };
    
    this.addDebugText('sceneInfo', 'Scene Info...', x, y, 14, 'cyan');
    this.scene.registerBeforeRender(updateInfo);
    
    return true;
  }

  // === MESH DEBUG ===

  showMeshInfo(mesh, x = 200, y = 10) {
    if (!mesh) return false;
    
    const updateMeshInfo = () => {
      const pos = mesh.position;
      const rot = mesh.rotation;
      const scale = mesh.scaling;
      const text = [
        `Mesh: ${mesh.name}`,
        `Pos: ${pos.x.toFixed(2)}, ${pos.y.toFixed(2)}, ${pos.z.toFixed(2)}`,
        `Rot: ${rot.x.toFixed(2)}, ${rot.y.toFixed(2)}, ${rot.z.toFixed(2)}`,
        `Scale: ${scale.x.toFixed(2)}, ${scale.y.toFixed(2)}, ${scale.z.toFixed(2)}`,
        `Visible: ${mesh.isVisible}`,
        `Enabled: ${mesh.isEnabled()}`,
        `Material: ${mesh.material ? mesh.material.name : 'none'}`,
        `Vertices: ${mesh.getTotalVertices ? mesh.getTotalVertices() : 0}`,
        `Triangles: ${mesh.getTotalIndices ? Math.floor(mesh.getTotalIndices() / 3) : 0}`
      ].join('\n');
      
      this.updateDebugText('meshInfo', text);
    };
    
    this.addDebugText('meshInfo', 'Mesh Info...', x, y, 14, 'yellow');
    this.scene.registerBeforeRender(updateMeshInfo);
    
    return true;
  }

  highlightMesh(mesh, color = [1, 1, 0]) {
    if (!mesh) return null;
    
    const highlight = mesh.clone(`${mesh.name}_highlight`);
    highlight.material = new StandardMaterial(`${mesh.name}_highlight_mat`, this.scene);
    highlight.material.emissiveColor = new Color3(...color);
    highlight.material.wireframe = true;
    highlight.scaling = mesh.scaling.scale(1.01); // Slightly larger
    
    return highlight;
  }

  // === CAMERA DEBUG ===

  showCameraInfo(camera = null, x = 400, y = 10) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return false;
    
    const updateCameraInfo = () => {
      const pos = targetCamera.position;
      const target = targetCamera.getTarget();
      const text = [
        `Camera: ${targetCamera.name}`,
        `Pos: ${pos.x.toFixed(2)}, ${pos.y.toFixed(2)}, ${pos.z.toFixed(2)}`,
        `Target: ${target.x.toFixed(2)}, ${target.y.toFixed(2)}, ${target.z.toFixed(2)}`,
        `FOV: ${targetCamera.fov ? (targetCamera.fov * 180 / Math.PI).toFixed(1) : 'N/A'}°`,
        `Near: ${targetCamera.minZ || 'N/A'}`,
        `Far: ${targetCamera.maxZ || 'N/A'}`
      ].join('\n');
      
      this.updateDebugText('cameraInfo', text);
    };
    
    this.addDebugText('cameraInfo', 'Camera Info...', x, y, 14, 'orange');
    this.scene.registerBeforeRender(updateCameraInfo);
    
    return true;
  }

  showCameraFrustum(camera = null, color = [1, 0, 1]) {
    const targetCamera = camera || this.scene.activeCamera;
    if (!targetCamera) return null;
    
    // Create frustum visualization
    const frustumLines = [];
    const near = targetCamera.minZ || 0.1;
    const far = targetCamera.maxZ || 100;
    const fov = targetCamera.fov || Math.PI / 4;
    
    const nearHeight = 2 * near * Math.tan(fov / 2);
    const nearWidth = nearHeight * targetCamera.getEngine().getAspectRatio(targetCamera);
    const farHeight = 2 * far * Math.tan(fov / 2);
    const farWidth = farHeight * targetCamera.getEngine().getAspectRatio(targetCamera);
    
    // Near plane corners
    const nearTL = new Vector3(-nearWidth / 2, nearHeight / 2, -near);
    const nearTR = new Vector3(nearWidth / 2, nearHeight / 2, -near);
    const nearBL = new Vector3(-nearWidth / 2, -nearHeight / 2, -near);
    const nearBR = new Vector3(nearWidth / 2, -nearHeight / 2, -near);
    
    // Far plane corners
    const farTL = new Vector3(-farWidth / 2, farHeight / 2, -far);
    const farTR = new Vector3(farWidth / 2, farHeight / 2, -far);
    const farBL = new Vector3(-farWidth / 2, -farHeight / 2, -far);
    const farBR = new Vector3(farWidth / 2, -farHeight / 2, -far);
    
    // Create lines
    const lines = [
      [nearTL, nearTR], [nearTR, nearBR], [nearBR, nearBL], [nearBL, nearTL], // Near plane
      [farTL, farTR], [farTR, farBR], [farBR, farBL], [farBL, farTL], // Far plane
      [nearTL, farTL], [nearTR, farTR], [nearBL, farBL], [nearBR, farBR] // Connecting lines
    ];
    
    const frustumMesh = MeshBuilder.CreateLineSystem(`${targetCamera.name}_frustum`, {
      lines: lines
    }, this.scene);
    
    frustumMesh.parent = targetCamera;
    frustumMesh.color = new Color3(...color);
    
    return frustumMesh;
  }

  // === CONSOLE LOGGING ===

  logMeshHierarchy(rootMesh = null) {
    const meshes = rootMesh ? [rootMesh] : this.scene.meshes;
    
    const logMesh = (mesh, indent = '') => {
      console.log(`${indent}${mesh.name} (${mesh.getClassName()})`);
      console.log(`${indent}  Position: ${mesh.position.x.toFixed(2)}, ${mesh.position.y.toFixed(2)}, ${mesh.position.z.toFixed(2)}`);
      console.log(`${indent}  Visible: ${mesh.isVisible}, Enabled: ${mesh.isEnabled()}`);
      
      if (mesh.getChildren) {
        mesh.getChildren().forEach(child => {
          if (child.position) { // Is a mesh
            logMesh(child, indent + '  ');
          }
        });
      }
    };
    
    meshes.forEach(mesh => logMesh(mesh));
    return true;
  }

  logSceneStatistics() {
    const info = this.getSceneInfo();
    console.group('Scene Statistics');
    Object.entries(info).forEach(([key, value]) => {
      console.log(`${key}: ${value}`);
    });
    console.groupEnd();
    return true;
  }

  // === DEBUG UTILITIES ===

  isDebugMode() {
    return !!this.debugOverlay && this.debugTexts.size > 0;
  }

  enableDebugMode(showFPS = true, showFrameTime = true, showMemory = false) {
    this.createDebugOverlay();
    
    if (showFPS) this.showFPS();
    if (showFrameTime) this.showFrameTime();
    if (showMemory) this.showMemoryUsage();
    
    return true;
  }

  disableDebugMode() {
    this.clearDebugTexts();
    if (this.debugOverlay) {
      this.debugOverlay.dispose();
      this.debugOverlay = null;
    }
    this.hideAllGizmos();
    return true;
  }

  takeScreenshot(width = 1920, height = 1080) {
    const engine = this.scene.getEngine();
    return new Promise((resolve) => {
      Tools.ToBlob(engine.getRenderingCanvas(), (blob) => {
        resolve(blob);
      }, 'image/png', width, height);
    });
  }

  // === CUSTOM DEBUG SHAPES ===

  createDebugSphere(name, position = [0, 0, 0], radius = 0.5, color = [1, 0, 0]) {
    const sphere = MeshBuilder.CreateSphere(name, { diameter: radius * 2 }, this.scene);
    sphere.position = new Vector3(...position);
    
    const material = new StandardMaterial(`${name}_mat`, this.scene);
    material.emissiveColor = new Color3(...color);
    material.wireframe = true;
    sphere.material = material;
    
    return sphere;
  }

  createDebugLine(name, start = [0, 0, 0], end = [1, 1, 1], color = [1, 1, 1]) {
    const points = [new Vector3(...start), new Vector3(...end)];
    const line = MeshBuilder.CreateLines(name, { points }, this.scene);
    line.color = new Color3(...color);
    return line;
  }

  createDebugText3D(name, text, position = [0, 2, 0], size = 1, color = [1, 1, 1]) {
    // Create a plane with dynamic texture for 3D text
    const plane = MeshBuilder.CreatePlane(name, { size }, this.scene);
    plane.position = new Vector3(...position);
    
    const texture = new DynamicTexture(`${name}_texture`, 512, this.scene);
    texture.hasAlpha = true;
    
    const material = new StandardMaterial(`${name}_mat`, this.scene);
    material.diffuseTexture = texture;
    material.emissiveColor = new Color3(...color);
    material.backFaceCulling = false;
    
    plane.material = material;
    
    // Draw text on texture
    texture.drawText(text, null, null, '60px Arial', 'white', 'transparent', true);
    
    // Make it face camera
    plane.billboardMode = Mesh.BILLBOARD_ALL;
    
    return { plane, texture, updateText: (newText) => {
      texture.clear();
      texture.drawText(newText, null, null, '60px Arial', 'white', 'transparent', true);
    }};
  }

  // === DEBUG CLEANUP ===

  disposeDebugObjects() {
    // Dispose debug meshes (those with debug keywords in name)
    const debugMeshes = this.scene.meshes.filter(mesh => 
      mesh.name.includes('debug') || 
      mesh.name.includes('_boundingBox') || 
      mesh.name.includes('_frustum') ||
      mesh.name.includes('_highlight')
    );
    
    debugMeshes.forEach(mesh => mesh.dispose());
    
    // Clear debug texts
    this.clearDebugTexts();
    
    return debugMeshes.length;
  }
}