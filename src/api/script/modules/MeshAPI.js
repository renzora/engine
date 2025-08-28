// === MESH API MODULE ===

import { 
  MeshBuilder,
  CreateBox,
  CreateSphere,
  CreateCylinder,
  CreatePlane,
  CreateGround,
  CreateTiledGround,
  CreateGroundFromHeightMap,
  CreateCapsule,
  CreateTorus,
  CreateTorusKnot,
  CreateIcoSphere,
  CreatePolyhedron,
  CreateDecal,
  CreateRibbon,
  CreateTube,
  CreateLathe,
  CreateDisc,
  CreatePolygon,
  CreateText,
  Vector3,
  Vector4,
  Color3,
  Color4,
  Path3D,
  Curve3,
  Mesh,
  InstancedMesh,
  TransformNode,
  AbstractMesh,
  VertexData,
  BoundingInfo,
  BoundingSphere,
  BoundingBox
} from '@babylonjs/core';

import { CSG } from '@babylonjs/core/Meshes/csg.js';
import { PolygonMeshBuilder } from '@babylonjs/core/Meshes/polygonMesh.js';

export class MeshAPI {
  constructor(scene) {
    this.scene = scene;
  }

  // === BASIC MESH CREATION ===

  createBox(name, size = 1, options = {}) {
    const mesh = CreateBox(name, { size, ...options }, this.scene);
    return mesh;
  }

  createSphere(name, diameter = 1, options = {}) {
    const mesh = CreateSphere(name, { diameter, ...options }, this.scene);
    return mesh;
  }

  createCylinder(name, height = 2, diameter = 1, options = {}) {
    const mesh = CreateCylinder(name, { height, diameter, ...options }, this.scene);
    return mesh;
  }

  createPlane(name, size = 1, options = {}) {
    const mesh = CreatePlane(name, { size, ...options }, this.scene);
    return mesh;
  }

  createGround(name, width = 1, height = 1, subdivisions = 1) {
    return CreateGround(name, { width, height, subdivisions }, this.scene);
  }

  createTiledGround(name, xmin = -1, zmin = -1, xmax = 1, zmax = 1, options = {}) {
    return CreateTiledGround(name, { xmin, zmin, xmax, zmax, ...options }, this.scene);
  }

  createGroundFromHeightMap(name, url, width = 10, height = 10, subdivisions = 250, options = {}) {
    return CreateGroundFromHeightMap(name, url, { width, height, subdivisions, ...options }, this.scene);
  }

  createCapsule(name, height = 2, radius = 0.5, options = {}) {
    return CreateCapsule(name, { height, radius, ...options }, this.scene);
  }

  createTorus(name, diameter = 1, thickness = 0.5, options = {}) {
    return CreateTorus(name, { diameter, thickness, ...options }, this.scene);
  }

  createTorusKnot(name, radius = 2, tube = 0.5, options = {}) {
    return CreateTorusKnot(name, { radius, tube, ...options }, this.scene);
  }

  createIcoSphere(name, radius = 1, options = {}) {
    return CreateIcoSphere(name, { radius, ...options }, this.scene);
  }

  createPolyhedron(name, type = 0, size = 1, options = {}) {
    return CreatePolyhedron(name, { type, size, ...options }, this.scene);
  }

  createDecal(name, sourceMesh, position, normal, size, angle = 0) {
    return CreateDecal(name, sourceMesh, {
      position: new Vector3(...position),
      normal: new Vector3(...normal),
      size: new Vector3(...size),
      angle
    }, this.scene);
  }

  // === ADVANCED MESH CREATION ===

  createRibbon(name, pathArray, options = {}) {
    const paths = pathArray.map(path => path.map(p => new Vector3(...p)));
    return CreateRibbon(name, { pathArray: paths, ...options }, this.scene);
  }

  createTube(name, path, radius = 1, options = {}) {
    const pathVectors = path.map(p => new Vector3(...p));
    return CreateTube(name, { path: pathVectors, radius, ...options }, this.scene);
  }

  createExtrusion(name, shape, path, options = {}) {
    const shapeVectors = shape.map(s => new Vector3(...s));
    const pathVectors = path.map(p => new Vector3(...p));
    return MeshBuilder.CreateExtrusion(name, { shape: shapeVectors, path: pathVectors, ...options }, this.scene);
  }

  createLathe(name, shape, options = {}) {
    const shapeVectors = shape.map(s => new Vector3(...s));
    return CreateLathe(name, { shape: shapeVectors, ...options }, this.scene);
  }

  createDisc(name, radius = 1, options = {}) {
    return CreateDisc(name, { radius, ...options }, this.scene);
  }

  createPolygon(name, shape, options = {}) {
    const shapeVectors = shape.map(s => new Vector3(...s));
    return CreatePolygon(name, { shape: shapeVectors, ...options }, this.scene);
  }

  createText(name, text, fontData, options = {}) {
    return CreateText(name, text, fontData, { ...options }, this.scene);
  }

  // === MESH TRANSFORMATION ===

  setMeshPosition(mesh, x, y, z) {
    if (!mesh) return false;
    mesh.position = new Vector3(x, y, z);
    return true;
  }

  setMeshRotation(mesh, x, y, z) {
    if (!mesh) return false;
    mesh.rotation = new Vector3(x, y, z);
    return true;
  }

  setMeshScaling(mesh, x, y, z) {
    if (!mesh) return false;
    mesh.scaling = new Vector3(x, y, z);
    return true;
  }

  translateMesh(mesh, x, y, z, space = 'local') {
    if (!mesh) return false;
    const vector = new Vector3(x, y, z);
    if (space === 'world') {
      mesh.translate(vector, 1, 1); // WORLD space
    } else {
      mesh.translate(vector, 1, 0); // LOCAL space
    }
    return true;
  }

  rotateMesh(mesh, x, y, z, space = 'local') {
    if (!mesh) return false;
    const vector = new Vector3(x, y, z);
    if (space === 'world') {
      mesh.rotate(vector, 1, 1); // WORLD space
    } else {
      mesh.rotate(vector, 1, 0); // LOCAL space
    }
    return true;
  }

  lookAtTarget(mesh, targetPosition) {
    if (!mesh || !targetPosition) return false;
    const target = new Vector3(...targetPosition);
    mesh.lookAt(target);
    return true;
  }

  // === MESH PROPERTIES ===

  setMeshVisibility(mesh, visibility) {
    if (!mesh) return false;
    mesh.visibility = Math.max(0, Math.min(1, visibility));
    return true;
  }

  setMeshEnabled(mesh, enabled) {
    if (!mesh) return false;
    mesh.setEnabled(enabled);
    return true;
  }

  setMeshPickable(mesh, pickable) {
    if (!mesh) return false;
    mesh.isPickable = pickable;
    return true;
  }

  setMeshCheckCollisions(mesh, checkCollisions) {
    if (!mesh) return false;
    mesh.checkCollisions = checkCollisions;
    return true;
  }

  setMeshReceiveShadows(mesh, receiveShadows) {
    if (!mesh) return false;
    mesh.receiveShadows = receiveShadows;
    return true;
  }

  setMeshCastShadows(mesh, castShadows) {
    if (!mesh) return false;
    mesh.castShadows = castShadows;
    return true;
  }

  setMeshRenderingGroupId(mesh, groupId) {
    if (!mesh) return false;
    mesh.renderingGroupId = groupId;
    return true;
  }

  setMeshBillboardMode(mesh, mode) {
    if (!mesh) return false;
    // BILLBOARDMODE_NONE = 0, BILLBOARDMODE_X = 1, BILLBOARDMODE_Y = 2, BILLBOARDMODE_Z = 4, BILLBOARDMODE_ALL = 7
    mesh.billboardMode = mode;
    return true;
  }

  // === MESH INSTANCING ===

  createMeshInstance(mesh, name) {
    if (!mesh || !mesh.createInstance) return null;
    return mesh.createInstance(name);
  }

  createMeshInstances(mesh, count, positions = []) {
    if (!mesh) return [];
    
    const instances = [];
    for (let i = 0; i < count; i++) {
      const instance = mesh.createInstance(`${mesh.name}_instance_${i}`);
      if (positions[i]) {
        instance.position = new Vector3(...positions[i]);
      }
      instances.push(instance);
    }
    return instances;
  }

  createThinInstances(mesh, positions, rotations = [], scalings = []) {
    if (!mesh || !positions) return false;
    
    const matrices = [];
    for (let i = 0; i < positions.length; i++) {
      const matrix = new Matrix();
      const pos = positions[i] ? new Vector3(...positions[i]) : Vector3.Zero();
      const rot = rotations[i] ? new Vector3(...rotations[i]) : Vector3.Zero();
      const scale = scalings[i] ? new Vector3(...scalings[i]) : Vector3.One();
      
      Matrix.ComposeToRef(scale, Quaternion.RotationYawPitchRoll(rot.y, rot.x, rot.z), pos, matrix);
      matrices.push(matrix);
    }
    
    mesh.thinInstanceSetBuffer("matrix", matrices, 16);
    return true;
  }

  // === MESH UTILITIES ===

  getMeshBoundingInfo(mesh) {
    if (!mesh || !mesh.getBoundingInfo) return null;
    
    const boundingInfo = mesh.getBoundingInfo();
    return {
      minimum: [boundingInfo.minimum.x, boundingInfo.minimum.y, boundingInfo.minimum.z],
      maximum: [boundingInfo.maximum.x, boundingInfo.maximum.y, boundingInfo.maximum.z],
      center: [boundingInfo.boundingBox.center.x, boundingInfo.boundingBox.center.y, boundingInfo.boundingBox.center.z],
      size: [boundingInfo.boundingBox.size.x, boundingInfo.boundingBox.size.y, boundingInfo.boundingBox.size.z]
    };
  }

  getMeshVertexCount(mesh) {
    if (!mesh || !mesh.getTotalVertices) return 0;
    return mesh.getTotalVertices();
  }

  getMeshTriangleCount(mesh) {
    if (!mesh || !mesh.getTotalIndices) return 0;
    return mesh.getTotalIndices() / 3;
  }

  cloneMesh(mesh, name, newParent = null, doNotCloneChildren = false) {
    if (!mesh || !mesh.clone) return null;
    return mesh.clone(name, newParent, doNotCloneChildren);
  }

  disposeMesh(mesh) {
    if (!mesh || !mesh.dispose) return false;
    mesh.dispose();
    return true;
  }

  mergeMeshes(meshes, disposeSource = true) {
    if (!meshes || meshes.length === 0) return null;
    return Mesh.MergeMeshes(meshes, disposeSource);
  }

  // === MESH OPTIMIZATION ===

  optimizeMesh(mesh) {
    if (!mesh) return false;
    
    // Freeze world matrix if static
    mesh.freezeWorldMatrix();
    
    // Convert to unindexed if small
    if (mesh.getTotalVertices() < 65536) {
      mesh.convertToUnIndexedMesh();
    }
    
    // Freeze normals
    mesh.freezeNormals();
    
    return true;
  }

  simplifyMesh(mesh, quality = 0.5) {
    if (!mesh || !mesh.simplify) return false;
    
    const decimationSettings = {
      quality: Math.max(0.1, Math.min(1.0, quality)),
      distance: 0.01,
      optimizeMesh: true
    };
    
    mesh.simplify([decimationSettings], false, 0, () => {
      console.log('Mesh simplified');
    });
    return true;
  }

  // === MESH LEVEL OF DETAIL ===

  addMeshLOD(mesh, distance, lodMesh) {
    if (!mesh || !mesh.addLODLevel) return false;
    mesh.addLODLevel(distance, lodMesh);
    return true;
  }

  removeMeshLOD(mesh, distance) {
    if (!mesh || !mesh.removeLODLevel) return false;
    mesh.removeLODLevel(distance);
    return true;
  }

  // === MESH MORPHING ===

  addMorphTarget(mesh, name) {
    if (!mesh || !mesh.morphTargetManager) return null;
    
    if (!mesh.morphTargetManager) {
      mesh.morphTargetManager = new MorphTargetManager();
    }
    
    const morphTarget = MorphTarget.FromMesh(mesh, name);
    mesh.morphTargetManager.addTarget(morphTarget);
    return morphTarget;
  }

  setMorphTargetInfluence(mesh, targetIndex, influence) {
    if (!mesh || !mesh.morphTargetManager) return false;
    
    const target = mesh.morphTargetManager.getTarget(targetIndex);
    if (target) {
      target.influence = Math.max(0, Math.min(1, influence));
      return true;
    }
    return false;
  }

  // === CSG OPERATIONS ===

  unionMeshes(meshA, meshB, name = 'union') {
    if (!meshA || !meshB) return null;
    
    const csgA = CSG.FromMesh(meshA);
    const csgB = CSG.FromMesh(meshB);
    const union = csgA.union(csgB);
    
    return union.toMesh(name, meshA.material, this.scene);
  }

  subtractMeshes(meshA, meshB, name = 'subtract') {
    if (!meshA || !meshB) return null;
    
    const csgA = CSG.FromMesh(meshA);
    const csgB = CSG.FromMesh(meshB);
    const subtract = csgA.subtract(csgB);
    
    return subtract.toMesh(name, meshA.material, this.scene);
  }

  intersectMeshes(meshA, meshB, name = 'intersect') {
    if (!meshA || !meshB) return null;
    
    const csgA = CSG.FromMesh(meshA);
    const csgB = CSG.FromMesh(meshB);
    const intersect = csgA.intersect(csgB);
    
    return intersect.toMesh(name, meshA.material, this.scene);
  }

  // === MESH VERTEX MANIPULATION ===

  updateMeshVertices(mesh, positions, normals = null, uvs = null, indices = null) {
    if (!mesh) return false;
    
    const vertexData = new VertexData();
    vertexData.positions = positions;
    if (normals) vertexData.normals = normals;
    if (uvs) vertexData.uvs = uvs;
    if (indices) vertexData.indices = indices;
    
    vertexData.applyToMesh(mesh);
    return true;
  }

  getMeshVertexData(mesh) {
    if (!mesh) return null;
    
    return {
      positions: mesh.getVerticesData(VertexBuffer.PositionKind),
      normals: mesh.getVerticesData(VertexBuffer.NormalKind),
      uvs: mesh.getVerticesData(VertexBuffer.UVKind),
      indices: mesh.getIndices()
    };
  }

  // === MESH PARENTING ===

  setMeshParent(mesh, parent) {
    if (!mesh) return false;
    mesh.parent = parent;
    return true;
  }

  attachMeshToBone(mesh, skeleton, boneName) {
    if (!mesh || !skeleton) return false;
    
    const bone = skeleton.bones.find(b => b.name === boneName);
    if (!bone) return false;
    
    mesh.attachToBone(bone, skeleton.transformMatrices[bone.getIndex()]);
    return true;
  }

  // === MESH COLLISION ===

  setMeshCollisionMesh(mesh, collisionMesh) {
    if (!mesh) return false;
    mesh._collisionMesh = collisionMesh;
    return true;
  }

  setMeshEllipsoid(mesh, x, y, z) {
    if (!mesh) return false;
    mesh.ellipsoid = new Vector3(x, y, z);
    return true;
  }

  setMeshEllipsoidOffset(mesh, x, y, z) {
    if (!mesh) return false;
    mesh.ellipsoidOffset = new Vector3(x, y, z);
    return true;
  }

  // === MESH INFORMATION ===

  getMeshInfo(mesh) {
    if (!mesh) return null;
    
    return {
      name: mesh.name,
      id: mesh.id,
      position: [mesh.position.x, mesh.position.y, mesh.position.z],
      rotation: [mesh.rotation.x, mesh.rotation.y, mesh.rotation.z],
      scaling: [mesh.scaling.x, mesh.scaling.y, mesh.scaling.z],
      visibility: mesh.visibility,
      enabled: mesh.isEnabled(),
      pickable: mesh.isPickable,
      vertexCount: mesh.getTotalVertices ? mesh.getTotalVertices() : 0,
      triangleCount: mesh.getTotalIndices ? mesh.getTotalIndices() / 3 : 0,
      hasCollision: mesh.checkCollisions,
      receiveShadows: mesh.receiveShadows,
      renderingGroupId: mesh.renderingGroupId
    };
  }

  getAllMeshes() {
    return this.scene.meshes.map(mesh => ({
      name: mesh.name,
      id: mesh.id,
      type: mesh.getClassName(),
      visible: mesh.isVisible,
      enabled: mesh.isEnabled()
    }));
  }

  findMeshByName(name) {
    return this.scene.getMeshByName(name);
  }

  findMeshesWithTag(tag) {
    return this.scene.meshes.filter(mesh => mesh.getTags && mesh.getTags().includes(tag));
  }

  // === MESH PICKING ===

  getPickedMesh(x, y) {
    const pickInfo = this.scene.pick(x, y);
    return pickInfo.hit ? pickInfo.pickedMesh : null;
  }

  getPickedPoint(x, y) {
    const pickInfo = this.scene.pick(x, y);
    return pickInfo.hit ? [pickInfo.pickedPoint.x, pickInfo.pickedPoint.y, pickInfo.pickedPoint.z] : null;
  }

  // === MESH ANIMATION HELPERS ===

  animateMeshProperty(mesh, property, targetValue, duration = 1000, easingFunction = null) {
    if (!mesh || !mesh[property]) return null;
    
    const animationName = `${mesh.name}_${property}_animation`;
    const frameRate = 60;
    const totalFrames = Math.floor((duration / 1000) * frameRate);
    
    const animation = Animation.CreateAndStartAnimation(
      animationName,
      mesh,
      property,
      frameRate,
      totalFrames,
      mesh[property],
      targetValue,
      Animation.ANIMATIONLOOPMODE_CONSTANT,
      easingFunction
    );
    
    return animation;
  }

  // === MESH VERTEX COLORS ===

  setMeshVertexColors(mesh, colors) {
    if (!mesh || !colors) return false;
    mesh.setVerticesData(VertexBuffer.ColorKind, colors);
    return true;
  }

  setMeshVertexAlpha(mesh, alpha) {
    if (!mesh) return false;
    
    const vertexCount = mesh.getTotalVertices();
    const colors = new Array(vertexCount * 4);
    
    for (let i = 0; i < vertexCount; i++) {
      colors[i * 4] = 1;     // R
      colors[i * 4 + 1] = 1; // G  
      colors[i * 4 + 2] = 1; // B
      colors[i * 4 + 3] = alpha; // A
    }
    
    mesh.setVerticesData(VertexBuffer.ColorKind, colors);
    mesh.hasVertexAlpha = true;
    return true;
  }

  // === MESH EDGE RENDERING ===

  enableMeshEdgeRendering(mesh, color = [1, 1, 1], width = 1) {
    if (!mesh) return false;
    mesh.enableEdgesRendering();
    mesh.edgesWidth = width;
    if (color) {
      mesh.edgesColor = new Color4(...color);
    }
    return true;
  }

  disableMeshEdgeRendering(mesh) {
    if (!mesh) return false;
    mesh.disableEdgesRendering();
    return true;
  }
}