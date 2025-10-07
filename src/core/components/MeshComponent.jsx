import { BaseComponent } from './BaseComponent.jsx';
import { engineStore } from '@/stores/EngineStore.jsx';
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder';

/**
 * Mesh component - handles 3D geometry rendering
 */
export class MeshComponent extends BaseComponent {
  constructor(babylonObject, data = {}, bridge = null) {
    super(babylonObject, data, bridge);
    this.mesh = null;
  }

  getDefaultData() {
    return {
      geometryId: null,
      materialId: null,
      castShadows: true,
      receiveShadows: true,
      visible: true,
      renderingGroupId: 0,
      alphaIndex: 1000,
      infiniteDistance: false
    };
  }

  static getSchema() {
    return {
      type: 'object',
      properties: {
        geometryId: {
          type: ['string', 'null'],
          description: 'ID of the geometry asset to use'
        },
        materialId: {
          type: ['string', 'null'],
          description: 'ID of the material asset to use'
        },
        castShadows: {
          type: 'boolean',
          description: 'Whether this mesh casts shadows',
          default: true
        },
        receiveShadows: {
          type: 'boolean',
          description: 'Whether this mesh receives shadows',
          default: true
        },
        visible: {
          type: 'boolean',
          description: 'Whether this mesh is visible',
          default: true
        },
        renderingGroupId: {
          type: 'integer',
          description: 'Rendering group ID for render order',
          minimum: 0,
          maximum: 4,
          default: 0
        },
        alphaIndex: {
          type: 'number',
          description: 'Alpha sorting index',
          default: 1000
        },
        infiniteDistance: {
          type: 'boolean',
          description: 'Whether this mesh is at infinite distance (like skybox)',
          default: false
        }
      }
    };
  }

  onCreate() {
    this.createMesh();
  }

  createMesh() {
    if (!this.data.geometryId) {
      console.warn('⚠️ MeshComponent: No geometry ID provided');
      return;
    }

    try {
      // Get geometry from asset store
      const geometryAsset = engineStore.assets.geometries[this.data.geometryId];
      if (!geometryAsset) {
        console.error(`❌ Geometry asset not found: ${this.data.geometryId}`);
        return;
      }

      // Create mesh based on geometry type
      this.mesh = this.createMeshFromGeometry(geometryAsset);
      if (!this.mesh) return;

      // Set parent to the babylon object
      this.mesh.parent = this.babylonObject;
      
      // Apply material if specified
      if (this.data.materialId) {
        this.applyMaterial();
      }

      // Apply mesh properties
      this.applyMeshProperties();

      // Register the mesh as a resource
      this.registerResource(this.mesh);

      console.log(`🔷 Created mesh: ${this.mesh.name} with geometry: ${this.data.geometryId}`);

    } catch (error) {
      console.error('❌ Failed to create mesh:', error);
    }
  }

  createMeshFromGeometry(geometryAsset) {
    switch (geometryAsset.type) {
      case 'primitive':
        return this.createPrimitiveMesh(geometryAsset);
      case 'imported':
        return this.createImportedMesh(geometryAsset);
      case 'procedural':
        return this.createProceduralMesh(geometryAsset);
      default:
        console.error(`❌ Unknown geometry type: ${geometryAsset.type}`);
        return null;
    }
  }

  createPrimitiveMesh(geometryAsset) {
    const { primitive, parameters = {} } = geometryAsset.data;
    const scene = this.babylonObject.getScene();

    switch (primitive) {
      case 'box':
        return MeshBuilder.CreateBox(this.babylonObject.name + '_mesh', {
          width: parameters.width || 1,
          height: parameters.height || 1,
          depth: parameters.depth || 1,
          ...parameters
        }, scene);

      case 'sphere':
        return MeshBuilder.CreateSphere(this.babylonObject.name + '_mesh', {
          diameter: parameters.diameter || 1,
          segments: parameters.segments || 32,
          ...parameters
        }, scene);

      case 'cylinder':
        return MeshBuilder.CreateCylinder(this.babylonObject.name + '_mesh', {
          height: parameters.height || 1,
          diameter: parameters.diameter || 1,
          tessellation: parameters.tessellation || 24,
          ...parameters
        }, scene);

      case 'plane':
        return MeshBuilder.CreatePlane(this.babylonObject.name + '_mesh', {
          width: parameters.width || 1,
          height: parameters.height || 1,
          ...parameters
        }, scene);

      case 'ground':
        return MeshBuilder.CreateGround(this.babylonObject.name + '_mesh', {
          width: parameters.width || 1,
          height: parameters.height || 1,
          subdivisions: parameters.subdivisions || 1,
          ...parameters
        }, scene);

      default:
        console.error(`❌ Unknown primitive type: ${primitive}`);
        return null;
    }
  }

  createImportedMesh(_geometryAsset) {
    // TODO: Handle imported mesh assets (GLB, FBX, etc.)
    // This would typically involve loading the mesh from the asset path
    console.warn('⚠️ Imported mesh creation not yet implemented');
    return null;
  }

  createProceduralMesh(_geometryAsset) {
    // TODO: Handle procedural mesh generation
    console.warn('⚠️ Procedural mesh creation not yet implemented');
    return null;
  }

  applyMaterial() {
    if (!this.mesh || !this.data.materialId) return;

    try {
      // Get material from asset store
      const materialAsset = engineStore.assets.materials[this.data.materialId];
      if (!materialAsset) {
        console.error(`❌ Material asset not found: ${this.data.materialId}`);
        return;
      }

      // Get the material system manager to create/get the Babylon material
      const materialManager = this.bridge?.systems?.materials;
      if (materialManager) {
        const babylonMaterial = materialManager.getMaterial(this.data.materialId);
        if (babylonMaterial) {
          this.mesh.material = babylonMaterial;
        }
      }

    } catch (error) {
      console.error('❌ Failed to apply material:', error);
    }
  }

  applyMeshProperties() {
    if (!this.mesh) return;

    // Shadow properties
    this.mesh.receiveShadows = this.data.receiveShadows;
    
    // Visibility
    this.mesh.setEnabled(this.data.visible);
    
    // Rendering properties
    this.mesh.renderingGroupId = this.data.renderingGroupId;
    this.mesh.alphaIndex = this.data.alphaIndex;
    this.mesh.infiniteDistance = this.data.infiniteDistance;

    // Shadow casting (this requires the shadow generator to be set up)
    if (this.data.castShadows && this.bridge?.systems?.lighting) {
      this.bridge.systems.lighting.addShadowCaster(this.mesh);
    }
  }

  onDataUpdated(newData, oldData) {
    // Handle geometry changes
    if (newData.geometryId && newData.geometryId !== oldData.geometryId) {
      this.recreateMesh();
    }

    // Handle material changes
    if (newData.materialId !== oldData.materialId) {
      this.applyMaterial();
    }

    // Handle property changes
    if (this.mesh) {
      if (newData.visible !== undefined) {
        this.mesh.setEnabled(newData.visible);
      }
      
      if (newData.castShadows !== undefined || newData.receiveShadows !== undefined) {
        this.mesh.receiveShadows = this.data.receiveShadows;
        
        if (this.bridge?.systems?.lighting) {
          if (this.data.castShadows) {
            this.bridge.systems.lighting.addShadowCaster(this.mesh);
          } else {
            this.bridge.systems.lighting.removeShadowCaster(this.mesh);
          }
        }
      }

      if (newData.renderingGroupId !== undefined) {
        this.mesh.renderingGroupId = newData.renderingGroupId;
      }

      if (newData.alphaIndex !== undefined) {
        this.mesh.alphaIndex = newData.alphaIndex;
      }
    }
  }

  recreateMesh() {
    // Dispose old mesh
    if (this.mesh) {
      this.unregisterResource(this.mesh);
      this.mesh.dispose();
      this.mesh = null;
    }

    // Create new mesh
    this.createMesh();
  }

  onActivated() {
    if (this.mesh) {
      this.mesh.setEnabled(true);
    }
  }

  onDeactivated() {
    if (this.mesh) {
      this.mesh.setEnabled(false);
    }
  }

  getRenScriptAPI() {
    const baseAPI = super.getRenScriptAPI();
    
    return {
      ...baseAPI,
      
      // Mesh-specific API
      getMesh: () => this.mesh,
      setGeometry: (geometryId) => this.updateData({ geometryId }),
      setMaterial: (materialId) => this.updateData({ materialId }),
      
      // Visibility
      setVisible: (visible) => this.updateData({ visible }),
      isVisible: () => this.data.visible,
      
      // Shadow properties
      setCastShadows: (cast) => this.updateData({ castShadows: cast }),
      setReceiveShadows: (receive) => this.updateData({ receiveShadows: receive }),
      
      // Rendering properties
      setRenderingGroup: (groupId) => this.updateData({ renderingGroupId: groupId }),
      setAlphaIndex: (index) => this.updateData({ alphaIndex: index }),
      
      // Mesh properties (direct Babylon.js access)
      getBoundingInfo: () => this.mesh?.getBoundingInfo(),
      getVerticesData: (kind) => this.mesh?.getVerticesData(kind),
      getTotalVertices: () => this.mesh?.getTotalVertices() || 0,
      getTotalIndices: () => this.mesh?.getTotalIndices() || 0,
      
      // Instancing
      createInstance: (name) => this.mesh?.createInstance(name),
      registerInstancedBuffer: (kind, stride) => this.mesh?.registerInstancedBuffer(kind, stride),
      
      // LOD
      addLODLevel: (distance, mesh) => this.mesh?.addLODLevel(distance, mesh),
      removeLODLevel: (mesh) => this.mesh?.removeLODLevel(mesh)
    };
  }

  onDestroy() {
    if (this.mesh) {
      // Remove from shadow casters if applicable
      if (this.data.castShadows && this.bridge?.systems?.lighting) {
        this.bridge.systems.lighting.removeShadowCaster(this.mesh);
      }
    }
  }

  serialize() {
    return {
      ...super.serialize(),
      // Add any mesh-specific serialization data
      meshStats: this.mesh ? {
        totalVertices: this.mesh.getTotalVertices(),
        totalIndices: this.mesh.getTotalIndices(),
        boundingBoxSize: this.mesh.getBoundingInfo()?.boundingBox?.extendSize || null
      } : null
    };
  }
}

export default MeshComponent;