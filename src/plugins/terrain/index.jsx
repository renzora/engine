import { createPlugin } from '@/api/plugin';
import { IconMountain, IconBrush } from '@tabler/icons-solidjs';
import TerrainPropertiesPanel from './TerrainPropertiesPanel.jsx';
import { renderStore, renderActions } from '@/render/store.jsx';
import { editorActions } from '@/layout/stores/EditorStore.jsx';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { VertexData } from '@babylonjs/core/Meshes/mesh.vertexData';
import { Mesh } from '@babylonjs/core/Meshes/mesh';
import '@babylonjs/core/Meshes/Builders/groundBuilder';

// Handle terrain creation function
const handleCreateTerrain = async () => {
  console.log('Creating terrain...');
  
  const scene = renderStore.scene;
  if (!scene) {
    editorActions.addConsoleMessage('No active scene available', 'error');
    return;
  }

  try {
    // Create terrain mesh with simpler approach first
    const terrainSize = 32; // Smaller size for testing
    const subdivisions = 16; // Fewer subdivisions for testing
    
    // Use BabylonJS built-in ground builder for simplicity
    const terrainMesh = MeshBuilder.CreateGround('terrain', {
      width: terrainSize,
      height: terrainSize,
      subdivisions: subdivisions
    }, scene);
    
    // Position terrain at world origin  
    terrainMesh.position = new Vector3(0, 0, 0);
    
    // Create a simple material that's guaranteed to be visible
    const material = new StandardMaterial('terrain_material', scene);
    material.diffuseColor = new Color3(0.2, 0.8, 0.2); // Bright green for visibility
    material.backFaceCulling = false; // Ensure both sides are visible
    terrainMesh.material = material;
    
    console.log('Created terrain at position:', terrainMesh.position, 'size:', terrainSize);
    
    // Mark as terrain object
    terrainMesh._terrainData = {
      size: terrainSize,
      subdivisions: subdivisions,
      heightmapData: [], // Empty for now since we're using built-in ground
      brushSize: 5,
      brushStrength: 0.1,
      brushFalloff: 'smooth'
    };
    
    
    // Add to scene hierarchy and select
    renderActions.addObject(terrainMesh);
    renderActions.selectObject(terrainMesh);
    editorActions.addConsoleMessage('Created terrain', 'info');
    
    // Switch to terrain properties tab
    const { editorActions: actions } = await import('@/layout/stores/EditorStore.jsx');
    actions.setSelectedTool('terrain');
    
  } catch (error) {
    console.error('Failed to create terrain:', error);
    editorActions.addConsoleMessage(`Failed to create terrain: ${error.message}`, 'error');
  }
};

const generateSimpleHeightmap = (width, height) => {
  const data = new Array(width * height);
  
  // Create a flat plane with slight elevation to make it visible
  for (let i = 0; i < data.length; i++) {
    data[i] = 0.1; // Small elevation to ensure visibility
  }
  
  return data;
};

const createTerrainMesh = (name, size, subdivisions, heightmapData, scene) => {
  const positions = [];
  const indices = [];
  const normals = [];
  const uvs = [];
  
  const verticesPerSide = subdivisions + 1;
  const stepSize = size / subdivisions;
  
  // Generate vertices
  for (let y = 0; y < verticesPerSide; y++) {
    for (let x = 0; x < verticesPerSide; x++) {
      const posX = (x - subdivisions / 2) * stepSize;
      const posZ = (y - subdivisions / 2) * stepSize;
      const posY = heightmapData[y * verticesPerSide + x] || 0;
      
      positions.push(posX, posY, posZ);
      
      // UV coordinates
      uvs.push(x / subdivisions, y / subdivisions);
    }
  }
  
  // Generate indices (triangles)
  for (let y = 0; y < subdivisions; y++) {
    for (let x = 0; x < subdivisions; x++) {
      const topLeft = y * verticesPerSide + x;
      const topRight = topLeft + 1;
      const bottomLeft = (y + 1) * verticesPerSide + x;
      const bottomRight = bottomLeft + 1;
      
      // First triangle
      indices.push(topLeft, bottomLeft, topRight);
      // Second triangle
      indices.push(topRight, bottomLeft, bottomRight);
    }
  }
  
  // Calculate normals
  const vertexData = new VertexData();
  vertexData.positions = positions;
  vertexData.indices = indices;
  vertexData.uvs = uvs;
  
  // Auto-calculate normals
  VertexData.ComputeNormals(positions, indices, normals);
  vertexData.normals = normals;
  
  // Create mesh
  const mesh = new Mesh(name, scene);
  vertexData.applyToMesh(mesh);
  
  return mesh;
};

export default createPlugin({
  id: 'terrain-plugin',
  name: 'Terrain Plugin',
  version: '1.0.0',
  description: 'Terrain creation and editing tools',
  author: 'Renzora Engine Team',

  async onInit() {
    console.log('[TerrainPlugin] Initializing...');
  },

  async onStart(api) {
    console.log('[TerrainPlugin] Starting...');
    
    // Register terrain properties tab that only shows for terrain objects
    api.tab('terrain', {
      title: 'Terrain',
      icon: IconMountain,
      component: TerrainPropertiesPanel,
      condition: (selectedObject) => {
        return selectedObject && selectedObject._terrainData;
      }
    });

    // Listen for terrain creation events from menu
    document.addEventListener('engine:create-terrain', handleCreateTerrain);
    
    console.log('[TerrainPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[TerrainPlugin] Stopping...');
    document.removeEventListener('engine:create-terrain', handleCreateTerrain);
  },

  async onDispose() {
    console.log('[TerrainPlugin] Disposing...');
  }
});