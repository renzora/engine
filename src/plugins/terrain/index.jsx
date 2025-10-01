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
import { PointerEventTypes } from '@babylonjs/core/Events/pointerEvents';
import { createSignal } from 'solid-js';
import '@babylonjs/core/Meshes/Builders/groundBuilder';
import '@babylonjs/core/Meshes/Builders/discBuilder';

// Terrain editing state
const [currentTool, setCurrentTool] = createSignal('raise');

// Check if we're in sculpting mode
const isTerrainEditMode = () => {
  const { editorStore } = require('@/layout/stores/EditorStore.jsx');
  return editorStore.ui.currentMode === 'sculpting';
};
let pointerObserver = null;
let brushCursorMesh = null;
let wheelEventListener = null;
let isMouseDown = false;
let lastBrushPosition = null; // Track last brush position for continuous strokes
let lastBrushTime = 0; // Throttle brush application
const brushThrottleMs = 16; // ~60fps brush application rate

// Apply brush stroke between two points for smooth continuous painting
const applyBrushStroke = (terrainMesh, startPos, endPos, tool, brushSize, brushStrength) => {
  if (!startPos || !endPos) {
    // If no start position, just apply at end position
    updateTerrainHeightmap(terrainMesh, endPos.x, endPos.z, tool, brushSize, brushStrength);
    return;
  }
  
  // Calculate distance between start and end
  const distance = Math.sqrt(
    Math.pow(endPos.x - startPos.x, 2) + 
    Math.pow(endPos.z - startPos.z, 2)
  );
  
  // Determine number of interpolation steps based on brush size and distance
  // Use smaller steps for smoother strokes
  const stepSize = brushSize * 0.3; // Step size relative to brush size
  const numSteps = Math.max(1, Math.ceil(distance / stepSize));
  
  // Only log for debug when there are multiple steps (actual interpolation)
  if (numSteps > 1) {
    console.log('Applying brush stroke from', startPos, 'to', endPos, 'with', numSteps, 'steps');
  }
  
  // Interpolate and apply brush at each step
  for (let i = 0; i <= numSteps; i++) {
    const t = numSteps === 0 ? 0 : i / numSteps;
    const interpX = startPos.x + (endPos.x - startPos.x) * t;
    const interpZ = startPos.z + (endPos.z - startPos.z) * t;
    
    // Apply terrain modification at interpolated position
    updateTerrainHeightmap(terrainMesh, interpX, interpZ, tool, brushSize, brushStrength);
  }
};

// Terrain editing functions
const updateTerrainHeightmap = (terrainMesh, x, z, tool, brushSize, brushStrength) => {
  if (!terrainMesh || !terrainMesh._terrainData) return;
  
  const terrainData = terrainMesh._terrainData;
  const subdivisions = terrainData.subdivisions;
  const verticesPerSide = subdivisions + 1;
  const terrainSize = terrainData.size;
  
  // Account for terrain scaling - transform world coordinates to original terrain space
  const scaleX = terrainMesh.scaling.x;
  const scaleZ = terrainMesh.scaling.z;
  
  // Transform coordinates back to original terrain space
  const originalX = x / scaleX;
  const originalZ = z / scaleZ;
  
  console.log('World coords:', x, z, 'Terrain scale:', scaleX, scaleZ, 'Original coords:', originalX, originalZ);
  
  // Convert world coordinates to heightmap coordinates (using original terrain space)
  const halfSize = terrainSize / 2;
  const localX = originalX + halfSize;
  const localZ = originalZ + halfSize;
  
  // Convert to grid coordinates
  const gridX = (localX / terrainSize) * subdivisions;
  const gridZ = (localZ / terrainSize) * subdivisions;
  
  console.log('Editing at grid coordinates:', gridX, gridZ, 'terrain bounds: 0 to', subdivisions);
  
  // Ensure we have heightmap data
  if (!terrainData.heightmapData || terrainData.heightmapData.length !== verticesPerSide * verticesPerSide) {
    terrainData.heightmapData = new Array(verticesPerSide * verticesPerSide).fill(0);
  }
  
  const heightmapData = terrainData.heightmapData;
  let modifiedAny = false;
  
  // Apply brush effect in radius (brushSize is in grid units)
  const radius = brushSize;
  const strength = brushStrength;
  
  console.log('Applying brush effect - radius:', radius, 'grid units, strength:', strength);
  
  for (let dy = -radius; dy <= radius; dy++) {
    for (let dx = -radius; dx <= radius; dx++) {
      const checkX = Math.round(gridX + dx);
      const checkZ = Math.round(gridZ + dy);
      
      // Allow modification even slightly outside bounds to enable terrain extension
      // But still need to be within the heightmap array bounds
      if (checkX < 0 || checkX >= verticesPerSide || checkZ < 0 || checkZ >= verticesPerSide) {
        console.log('Skipping out-of-bounds coordinate:', checkX, checkZ);
        continue;
      }
      
      // Calculate distance from brush center
      const distance = Math.sqrt(dx * dx + dy * dy);
      if (distance > radius) continue;
      
      // Calculate falloff (smooth)
      const falloff = Math.max(0, 1 - (distance / radius));
      const effectiveStrength = strength * falloff;
      
      const index = checkZ * verticesPerSide + checkX;
      
      if (index >= 0 && index < heightmapData.length) {
        // Apply tool effect
        switch (tool) {
          case 'raise':
            heightmapData[index] += effectiveStrength;
            modifiedAny = true;
            break;
          case 'lower':
            heightmapData[index] -= effectiveStrength;
            modifiedAny = true;
            break;
          case 'smooth':
            // Average with surrounding heights
            let sum = heightmapData[index];
            let count = 1;
            
            for (let sy = -1; sy <= 1; sy++) {
              for (let sx = -1; sx <= 1; sx++) {
                const sampleX = checkX + sx;
                const sampleZ = checkZ + sy;
                if (sampleX >= 0 && sampleX < verticesPerSide && sampleZ >= 0 && sampleZ < verticesPerSide) {
                  sum += heightmapData[sampleZ * verticesPerSide + sampleX];
                  count++;
                }
              }
            }
            
            const average = sum / count;
            heightmapData[index] = heightmapData[index] * (1 - effectiveStrength) + average * effectiveStrength;
            modifiedAny = true;
            break;
          case 'flatten':
            // Flatten to target height (0 for now)
            heightmapData[index] = heightmapData[index] * (1 - effectiveStrength);
            modifiedAny = true;
            break;
          case 'paint':
            // Paint texture - for now just mark terrain for texture painting
            console.log('Paint tool applied at index:', index);
            break;
          case 'noise':
            // Add random noise to terrain
            const noiseValue = (Math.random() - 0.5) * effectiveStrength * 2;
            heightmapData[index] += noiseValue;
            modifiedAny = true;
            break;
        }
        
        // Clamp height values
        heightmapData[index] = Math.max(-10, Math.min(10, heightmapData[index]));
      }
    }
  }
  
  if (modifiedAny) {
    console.log('Terrain modified, updating geometry');
    // Update mesh geometry
    updateTerrainMeshGeometry(terrainMesh, heightmapData);
  } else {
    console.log('No terrain modifications made');
  }
};

const updateTerrainMeshGeometry = (terrainMesh, heightmapData) => {
  const terrainData = terrainMesh._terrainData;
  const subdivisions = terrainData.subdivisions;
  const verticesPerSide = subdivisions + 1;
  
  // Get existing vertex data
  const positions = terrainMesh.getVerticesData('position');
  if (!positions) {
    console.error('No position data found for terrain mesh');
    return;
  }
  
  console.log('Updating terrain geometry - vertices:', verticesPerSide, 'heightmap length:', heightmapData.length);
  
  // Update Y positions based on heightmap
  for (let z = 0; z < verticesPerSide; z++) {
    for (let x = 0; x < verticesPerSide; x++) {
      const heightmapIndex = z * verticesPerSide + x;
      const vertexIndex = heightmapIndex * 3; // 3 components per vertex (x, y, z)
      
      // Update Y component with heightmap value
      const oldY = positions[vertexIndex + 1];
      const newY = heightmapData[heightmapIndex];
      positions[vertexIndex + 1] = newY;
      
      // Log a few updates for debugging
      if (heightmapIndex < 5) {
        console.log(`Vertex ${heightmapIndex}: Y ${oldY} -> ${newY}`);
      }
    }
  }
  
  // Force update vertex data with updatable flag
  terrainMesh.updateVerticesData('position', positions, true);
  
  // Recalculate normals
  const indices = terrainMesh.getIndices();
  if (indices) {
    const normals = [];
    VertexData.ComputeNormals(positions, indices, normals);
    terrainMesh.updateVerticesData('normal', normals, true);
  }
  
  // Force mesh refresh
  terrainMesh.refreshBoundingInfo();
  terrainMesh.markVerticesDataAsUpdatable('position', true);
  terrainMesh.markVerticesDataAsUpdatable('normal', true);
  
  console.log('Terrain geometry updated');
};

// Create brush cursor mesh that conforms to terrain
const createBrushCursor = (scene, brushSize) => {
  console.log('Creating brush cursor with size:', brushSize);
  
  if (brushCursorMesh) {
    console.log('Disposing existing brush cursor');
    brushCursorMesh.dispose();
  }
  
  // Create a solid disc mesh for the brush cursor with more subdivisions for terrain conforming
  brushCursorMesh = MeshBuilder.CreateDisc('brushCursor', {
    radius: brushSize,
    tessellation: 32, // Reduced tessellation for better performance
    sideOrientation: 2 // Double-sided
  }, scene);
  
  console.log('Created disc mesh with radius:', brushSize);
  
  // Create material for brush cursor
  const material = new StandardMaterial('brushCursorMaterial', scene);
  material.diffuseColor = new Color3(1, 0.8, 0); // Orange-yellow
  material.emissiveColor = new Color3(0.5, 0.3, 0); // Brighter emissive
  material.alpha = 0.8; // More visible
  material.wireframe = false; // Solid, not wireframe
  material.backFaceCulling = false;
  material.disableLighting = true; // Make it always visible
  
  brushCursorMesh.material = material;
  brushCursorMesh.isPickable = false;
  brushCursorMesh.visibility = 1.0; // Full visibility
  brushCursorMesh.renderingGroupId = 1; // Render on top
  
  // Rotate to lay flat on the ground (disc is created vertical by default)
  brushCursorMesh.rotation.x = Math.PI / 2;
  
  // Mark as updatable so we can deform it to match terrain
  brushCursorMesh.markVerticesDataAsUpdatable('position', true);
  brushCursorMesh.markVerticesDataAsUpdatable('normal', true);
  
  console.log('Brush cursor created successfully, position:', brushCursorMesh.position);
  console.log('Brush cursor visibility:', brushCursorMesh.visibility);
  console.log('Brush cursor enabled:', brushCursorMesh.isEnabled());
  
  return brushCursorMesh;
};

// Sample terrain height at a given world position
const sampleTerrainHeight = (terrainMesh, worldX, worldZ) => {
  if (!terrainMesh || !terrainMesh._terrainData) return 0;
  
  const terrainData = terrainMesh._terrainData;
  const terrainSize = terrainData.size;
  const subdivisions = terrainData.subdivisions;
  const heightmapData = terrainData.heightmapData;
  const verticesPerSide = subdivisions + 1;
  
  // Account for terrain scaling - transform world coordinates to original terrain space
  const scaleX = terrainMesh.scaling.x;
  const scaleZ = terrainMesh.scaling.z;
  
  // Transform coordinates back to original terrain space
  const originalX = worldX / scaleX;
  const originalZ = worldZ / scaleZ;
  
  // Convert world coordinates to heightmap coordinates (relative to terrain center)
  const halfSize = terrainSize / 2;
  const localX = originalX + halfSize;
  const localZ = originalZ + halfSize;
  
  // Convert to grid coordinates
  const gridX = (localX / terrainSize) * subdivisions;
  const gridZ = (localZ / terrainSize) * subdivisions;
  
  // Clamp to valid range
  const clampedX = Math.max(0, Math.min(subdivisions, gridX));
  const clampedZ = Math.max(0, Math.min(subdivisions, gridZ));
  
  // Bilinear interpolation for smooth height sampling
  const x0 = Math.floor(clampedX);
  const x1 = Math.min(subdivisions, x0 + 1);
  const z0 = Math.floor(clampedZ);
  const z1 = Math.min(subdivisions, z0 + 1);
  
  const fx = clampedX - x0;
  const fz = clampedZ - z0;
  
  const h00 = heightmapData[z0 * verticesPerSide + x0] || 0;
  const h10 = heightmapData[z0 * verticesPerSide + x1] || 0;
  const h01 = heightmapData[z1 * verticesPerSide + x0] || 0;
  const h11 = heightmapData[z1 * verticesPerSide + x1] || 0;
  
  // Bilinear interpolation
  const h0 = h00 * (1 - fx) + h10 * fx;
  const h1 = h01 * (1 - fx) + h11 * fx;
  const height = h0 * (1 - fz) + h1 * fz;
  
  return height;
};

// Update brush cursor position and size
const updateBrushCursor = (scene, pickedPoint, brushSize, terrainMesh) => {
  if (!brushCursorMesh || !pickedPoint || !terrainMesh) {
    console.log('Missing brush cursor elements:', {
      cursor: !!brushCursorMesh,
      point: !!pickedPoint, 
      terrain: !!terrainMesh
    });
    return;
  }
  
  console.log('Updating brush cursor at position:', pickedPoint, 'brush size:', brushSize);
  
  // Convert brush size from grid coordinates to world coordinates (accounting for terrain scale)
  const terrainData = terrainMesh._terrainData;
  const terrainSize = terrainData.size;
  const subdivisions = terrainData.subdivisions;
  const stepSize = terrainSize / subdivisions;
  
  // Account for terrain scaling - brush should appear correct size in world space
  const scaleX = terrainMesh.scaling.x;
  const scaleZ = terrainMesh.scaling.z;
  const averageScale = (scaleX + scaleZ) / 2; // Use average scale for circular brush
  
  const worldRadius = brushSize * stepSize * averageScale;
  
  console.log('World radius calculated:', worldRadius, 'step size:', stepSize);
  
  // Position the brush cursor at the picked point
  brushCursorMesh.position.copyFrom(pickedPoint);
  brushCursorMesh.position.y = pickedPoint.y + 0.1; // Fixed height above the point
  
  // Update brush cursor size
  const originalRadius = brushCursorMesh._originalRadius || worldRadius;
  const scaleFactor = worldRadius / originalRadius;
  brushCursorMesh.scaling.setAll(scaleFactor);
  
  if (!brushCursorMesh._originalRadius) {
    brushCursorMesh._originalRadius = worldRadius;
  }
  
  console.log('Brush cursor updated - position:', brushCursorMesh.position, 'scale:', scaleFactor, 'enabled:', brushCursorMesh.isEnabled());
};

// Deform the cursor mesh to conform to the terrain surface
const conformCursorToTerrain = (cursorMesh, terrainMesh, centerPoint, radius) => {
  if (!cursorMesh || !terrainMesh) return;
  
  const positions = cursorMesh.getVerticesData('position');
  if (!positions) return;
  
  // Convert world position to terrain local coordinates
  const terrainLocalCenter = centerPoint.subtract(terrainMesh.position);
  
  // Update each vertex height to match terrain
  for (let i = 0; i < positions.length; i += 3) {
    // Get vertex position in cursor local space
    const localX = positions[i];
    const localZ = positions[i + 2];
    
    // Convert to world space (accounting for cursor scaling)
    const scale = cursorMesh.scaling.x;
    const worldX = terrainLocalCenter.x + (localX * scale);
    const worldZ = terrainLocalCenter.z + (localZ * scale);
    
    // Sample terrain height at this position
    const terrainHeight = sampleTerrainHeight(terrainMesh, worldX, worldZ);
    
    // Set Y position to match terrain height (in cursor local space)
    positions[i + 1] = (terrainHeight - centerPoint.y) / scale;
  }
  
  // Update the mesh geometry
  cursorMesh.updateVerticesData('position', positions, true);
  
  // Recalculate normals
  const indices = cursorMesh.getIndices();
  if (indices) {
    const normals = [];
    VertexData.ComputeNormals(positions, indices, normals);
    cursorMesh.updateVerticesData('normal', normals, true);
  }
};

// Handle mouse movement for brush cursor
const handleMouseMove = (pointerInfo) => {
  if (!isTerrainEditMode() || !renderStore.scene) return;
  
  const scene = renderStore.scene;
  
  // Check if mouse is outside the canvas bounds
  const canvas = scene.getEngine().getRenderingCanvas();
  if (canvas && pointerInfo.event) {
    const rect = canvas.getBoundingClientRect();
    const mouseX = pointerInfo.event.clientX;
    const mouseY = pointerInfo.event.clientY;
    
    // Hide cursor if mouse is outside canvas
    if (mouseX < rect.left || mouseX > rect.right || mouseY < rect.top || mouseY > rect.bottom) {
      if (brushCursorMesh) {
        brushCursorMesh.setEnabled(false);
      }
      return;
    }
  }
  
  // First try to pick terrain specifically
  let terrainPickInfo = scene.pick(scene.pointerX, scene.pointerY, (mesh) => {
    return mesh && mesh._terrainData;
  });
  
  // If no terrain hit, pick against a ground plane to get world position
  let worldPosition = null;
  let selectedTerrain = renderStore.selectedObject && renderStore.selectedObject._terrainData ? renderStore.selectedObject : null;
  
  if (terrainPickInfo.hit && terrainPickInfo.pickedPoint) {
    // Hit terrain directly
    worldPosition = terrainPickInfo.pickedPoint;
    selectedTerrain = terrainPickInfo.pickedMesh;
  } else if (selectedTerrain) {
    // No terrain hit, but we have a selected terrain - project onto ground plane
    const groundPickInfo = scene.pick(scene.pointerX, scene.pointerY);
    if (groundPickInfo.hit) {
      worldPosition = groundPickInfo.pickedPoint;
    } else {
      // Create a virtual ground plane pick
      const ray = scene.createPickingRay(scene.pointerX, scene.pointerY, null, scene.activeCamera);
      const groundPlane = new Vector3(0, 1, 0); // Y-up plane
      const planeDistance = 0; // At Y=0
      
      // Ray-plane intersection
      const denom = Vector3.Dot(groundPlane, ray.direction);
      if (Math.abs(denom) > 0.0001) {
        const t = -(Vector3.Dot(ray.origin, groundPlane) + planeDistance) / denom;
        if (t >= 0) {
          worldPosition = ray.origin.add(ray.direction.scale(t));
        }
      }
    }
  }
  
  // Reduced logging - only log when mouse is down for dragging
  if (isMouseDown) {
    console.log('Mouse move during drag - terrain mode:', isTerrainEditMode(), 'world pos:', worldPosition, 'selected terrain:', !!selectedTerrain);
  }
  
  if (worldPosition && selectedTerrain) {
    const terrainData = selectedTerrain._terrainData;
    const brushSize = terrainData.brushSize || 5;
    
    console.log('Showing cursor at world position:', worldPosition, 'brush size:', brushSize);
    
    if (!brushCursorMesh) {
      console.log('Creating new brush cursor');
      // Create brush cursor with initial world radius
      const terrainSize = terrainData.size;
      const subdivisions = terrainData.subdivisions;
      const stepSize = terrainSize / subdivisions;
      const worldRadius = brushSize * stepSize;
      createBrushCursor(scene, worldRadius);
    }
    
    updateBrushCursor(scene, worldPosition, brushSize, selectedTerrain);
    if (brushCursorMesh) {
      brushCursorMesh.setEnabled(true);
      console.log('Brush cursor enabled, position:', brushCursorMesh.position);
    }
  } else {
    console.log('No valid position or terrain, disabling cursor');
    if (brushCursorMesh) {
      brushCursorMesh.setEnabled(false);
    }
  }
};

const handleTerrainEdit = (pointerInfo) => {
  console.log('Terrain edit attempt - isEditMode:', isTerrainEditMode());
  
  if (!isTerrainEditMode()) return false;
  
  const scene = renderStore.scene;
  if (!scene) return false;
  
  // Use the same logic as mouse move to get world position
  let terrainPickInfo = scene.pick(scene.pointerX, scene.pointerY, (mesh) => {
    return mesh && mesh._terrainData;
  });
  
  let worldPosition = null;
  let selectedTerrain = renderStore.selectedObject && renderStore.selectedObject._terrainData ? renderStore.selectedObject : null;
  
  if (terrainPickInfo.hit && terrainPickInfo.pickedPoint) {
    // Hit terrain directly
    worldPosition = terrainPickInfo.pickedPoint;
    selectedTerrain = terrainPickInfo.pickedMesh;
  } else if (selectedTerrain) {
    // No terrain hit, use ground plane projection
    const groundPickInfo = scene.pick(scene.pointerX, scene.pointerY);
    if (groundPickInfo.hit) {
      worldPosition = groundPickInfo.pickedPoint;
    } else {
      // Create a virtual ground plane pick
      const ray = scene.createPickingRay(scene.pointerX, scene.pointerY, null, scene.activeCamera);
      const groundPlane = new Vector3(0, 1, 0);
      const planeDistance = 0;
      
      const denom = Vector3.Dot(groundPlane, ray.direction);
      if (Math.abs(denom) > 0.0001) {
        const t = -(Vector3.Dot(ray.origin, groundPlane) + planeDistance) / denom;
        if (t >= 0) {
          worldPosition = ray.origin.add(ray.direction.scale(t));
        }
      }
    }
  }
  
  if (!worldPosition || !selectedTerrain) {
    console.log('No valid position or terrain for editing');
    return false;
  }
  
  // Get terrain data and brush settings
  const terrainData = selectedTerrain._terrainData;
  const brushSize = terrainData.brushSize || 5;
  const brushStrength = terrainData.brushStrength || 0.1;
  
  console.log('Terrain editing:', currentTool(), 'at world pos:', worldPosition, 'brush size:', brushSize, 'strength:', brushStrength);
  
  // Convert world position to local terrain coordinates (accounting for transform)
  const localPos = worldPosition.subtract(selectedTerrain.position);
  const currentPos = { x: localPos.x, z: localPos.z };
  
  console.log('About to apply brush stroke at local position:', localPos.x, localPos.z, 'terrain scale:', selectedTerrain.scaling.x, selectedTerrain.scaling.z);
  
  // Apply continuous brush stroke from last position to current position
  applyBrushStroke(selectedTerrain, lastBrushPosition, currentPos, currentTool(), brushSize, brushStrength);
  
  // Update last brush position for next stroke
  lastBrushPosition = { x: currentPos.x, z: currentPos.z };
  
  editorActions.addConsoleMessage(`Applied ${currentTool()} stroke to (${localPos.x.toFixed(1)}, ${localPos.z.toFixed(1)})`, 'info');
  
  return true; // Event handled
};

// Handle wheel events for brush size adjustment
const handleWheelEvent = (event) => {
  if (!isTerrainEditMode() || !event.ctrlKey) return;
  
  event.preventDefault();
  
  // Find the selected terrain object
  const selectedObject = renderStore.selectedObject;
  if (!selectedObject || !selectedObject._terrainData) return;
  
  const terrainData = selectedObject._terrainData;
  let currentBrushSize = terrainData.brushSize || 5;
  
  // Adjust brush size based on wheel direction
  const delta = event.deltaY > 0 ? -0.5 : 0.5;
  const newBrushSize = Math.max(1, Math.min(32, currentBrushSize + delta));
  
  // Update terrain data
  terrainData.brushSize = newBrushSize;
  
  // Update brush cursor if it exists
  if (brushCursorMesh && renderStore.scene) {
    const scene = renderStore.scene;
    const pickInfo = scene.pick(scene.pointerX, scene.pointerY, (mesh) => {
      return mesh && mesh._terrainData;
    });
    
    if (pickInfo.hit && pickInfo.pickedPoint && pickInfo.pickedMesh) {
      updateBrushCursor(scene, pickInfo.pickedPoint, newBrushSize, pickInfo.pickedMesh);
    }
  }
  
  editorActions.addConsoleMessage(`Brush size: ${newBrushSize.toFixed(1)}`, 'info');
};

// Listen for mode changes
let currentSculptMode = false;

const handleModeChange = (event) => {
  const { mode } = event.detail;
  const isSculpting = mode === 'sculpting';
  
  console.log('Mode changed to:', mode, 'isSculpting:', isSculpting);
  
  if (isSculpting !== currentSculptMode) {
    currentSculptMode = isSculpting;
    if (isSculpting) {
      startTerrainEditMode();
    } else {
      stopTerrainEditMode();
    }
  }
};

const startTerrainEditMode = () => {
  console.log('Starting terrain sculpting mode');
  
  // Hide the system cursor
  if (renderStore.scene) {
    const canvas = renderStore.scene.getEngine().getRenderingCanvas();
    if (canvas) {
      canvas.style.cursor = 'none';
      console.log('Hidden system cursor');
    }
  }
  
  // Camera controls are now disabled automatically via centralized control system
  
  // Add pointer observer for terrain editing with higher priority
  if (renderStore.scene && !pointerObserver) {
    pointerObserver = renderStore.scene.onPointerObservable.add((pointerInfo) => {
      // Handle mouse movement for brush cursor
      if (pointerInfo.type === PointerEventTypes.POINTERMOVE) {
        handleMouseMove(pointerInfo);
        
        // Handle dragging for continuous terrain editing
        if (isMouseDown) {
          // Throttle brush application to prevent it from being too fast
          const now = Date.now();
          if (now - lastBrushTime >= brushThrottleMs) {
            console.log('POINTERMOVE during drag - isMouseDown:', isMouseDown, 'throttled');
            
            // In sculpting mode, always prevent default behavior during drag
            pointerInfo.event.preventDefault();
            pointerInfo.event.stopPropagation();
            pointerInfo.skipNextObservers = true;
            
            // Try to handle terrain editing
            const handled = handleTerrainEdit(pointerInfo);
            console.log('Terrain edit handled during drag:', handled);
            
            lastBrushTime = now;
          }
        }
      }
      
      // Handle mouse down events
      if (pointerInfo.type === PointerEventTypes.POINTERDOWN && pointerInfo.event && pointerInfo.event.button === 0) {
        console.log('POINTERDOWN detected - setting isMouseDown to true');
        isMouseDown = true;
        // Reset brush position for new stroke
        lastBrushPosition = null;
        
        // In sculpting mode, always prevent default selection behavior
        // This prevents object deselection when clicking on empty space
        pointerInfo.event.preventDefault();
        pointerInfo.event.stopPropagation();
        pointerInfo.skipNextObservers = true;
        
        // Try to handle terrain editing
        const handled = handleTerrainEdit(pointerInfo);
        console.log('Initial terrain edit handled:', handled);
      }
      
      // Handle mouse up events
      if (pointerInfo.type === PointerEventTypes.POINTERUP && pointerInfo.event && pointerInfo.event.button === 0) {
        console.log('POINTERUP detected - setting isMouseDown to false');
        isMouseDown = false;
        // End stroke - reset brush position
        lastBrushPosition = null;
      }
    }, -1); // Higher priority (negative value = higher priority)
  }
  
  // Add wheel event listener for brush size adjustment
  if (!wheelEventListener && renderStore.scene) {
    const canvas = renderStore.scene.getEngine().getRenderingCanvas();
    if (canvas) {
      wheelEventListener = (event) => handleWheelEvent(event);
      canvas.addEventListener('wheel', wheelEventListener, { passive: false });
      
      // Add mouse leave listener to hide brush cursor
      const handleMouseLeave = () => {
        if (brushCursorMesh) {
          brushCursorMesh.setEnabled(false);
        }
      };
      canvas.addEventListener('mouseleave', handleMouseLeave);
      
      // Store the listener so we can remove it later
      canvas._terrainMouseLeaveListener = handleMouseLeave;
    }
  }
  
  editorActions.addConsoleMessage(`Started terrain sculpting mode (Use Ctrl+Scroll to resize brush)`, 'info');
};

const stopTerrainEditMode = () => {
  console.log('Stopping terrain sculpting mode');
  
  // Restore the system cursor
  if (renderStore.scene) {
    const canvas = renderStore.scene.getEngine().getRenderingCanvas();
    if (canvas) {
      canvas.style.cursor = 'default';
    }
  }
  
  // Camera controls are now re-enabled automatically via centralized control system
  
  // Reset mouse state and brush position
  isMouseDown = false;
  lastBrushPosition = null;
  
  // Remove pointer observer
  if (pointerObserver && renderStore.scene) {
    renderStore.scene.onPointerObservable.remove(pointerObserver);
    pointerObserver = null;
  }
  
  // Remove wheel event listener and mouse leave listener
  if (wheelEventListener && renderStore.scene) {
    const canvas = renderStore.scene.getEngine().getRenderingCanvas();
    if (canvas) {
      canvas.removeEventListener('wheel', wheelEventListener);
      wheelEventListener = null;
      
      // Remove mouse leave listener
      if (canvas._terrainMouseLeaveListener) {
        canvas.removeEventListener('mouseleave', canvas._terrainMouseLeaveListener);
        canvas._terrainMouseLeaveListener = null;
      }
    }
  }
  
  // Hide brush cursor
  if (brushCursorMesh) {
    brushCursorMesh.setEnabled(false);
  }
  
  editorActions.addConsoleMessage('Stopped terrain sculpting mode', 'info');
};

const switchTerrainTool = (tool) => {
  setCurrentTool(tool);
  editorActions.addConsoleMessage(`Switched to terrain tool: ${tool}`, 'info');
};

// Handle terrain creation function
const handleCreateTerrain = async () => {
  console.log('Creating terrain...');
  
  const scene = renderStore.scene;
  if (!scene) {
    editorActions.addConsoleMessage('No active scene available', 'error');
    return;
  }

  try {
    // Create terrain mesh with high resolution for detailed editing
    const terrainSize = 64; // Good size for terrain editing
    const subdivisions = 128; // High subdivision for detailed sculpting (129x129 vertices = 16,641 vertices)
    
    // Initialize heightmap data for editing
    const verticesPerSide = subdivisions + 1;
    const initialHeightmap = new Array(verticesPerSide * verticesPerSide).fill(0);
    
    // Create terrain mesh using custom function for proper editing support
    const terrainMesh = createTerrainMesh('terrain', terrainSize, subdivisions, initialHeightmap, scene);
    
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
      heightmapData: initialHeightmap,
      brushSize: 8, // Larger default brush for high-res terrain
      brushStrength: 0.2, // Stronger default for visible effects
      brushFalloff: 'smooth'
    };
    
    
    // Add to scene hierarchy and select
    renderActions.addObject(terrainMesh);
    renderActions.selectObject(terrainMesh);
    editorActions.addConsoleMessage('Created terrain. Switch to Sculpting mode from the toolbar dropdown to start editing.', 'info');
    
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
  
  // Mark vertex data as updatable for terrain editing
  mesh.markVerticesDataAsUpdatable('position', true);
  mesh.markVerticesDataAsUpdatable('normal', true);
  
  console.log('Created terrain mesh with', positions.length / 3, 'vertices');
  
  return mesh;
};

// Export terrain editing functions for use by other components
export {
  switchTerrainTool,
  isTerrainEditMode,
  currentTool,
  updateTerrainHeightmap,
  updateTerrainMeshGeometry,
  createTerrainMesh
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
    
    // Listen for terrain tool switching
    document.addEventListener('engine:switch-terrain-tool', (e) => {
      switchTerrainTool(e.detail?.tool || 'raise');
    });
    
    // Listen for mode changes
    document.addEventListener('engine:mode-change', handleModeChange);
    
    // Check initial mode state
    const { editorStore } = await import('@/layout/stores/EditorStore.jsx');
    if (editorStore.ui.currentMode === 'sculpting') {
      currentSculptMode = true;
      startTerrainEditMode();
    }
    
    console.log('[TerrainPlugin] Started');
  },

  onUpdate() {
    // Update logic if needed
  },

  async onStop() {
    console.log('[TerrainPlugin] Stopping...');
    document.removeEventListener('engine:create-terrain', handleCreateTerrain);
    document.removeEventListener('engine:switch-terrain-tool', switchTerrainTool);
    document.removeEventListener('engine:mode-change', handleModeChange);
    
    // Clean up editing mode
    stopTerrainEditMode();
  },

  async onDispose() {
    console.log('[TerrainPlugin] Disposing...');
    
    // Remove event listeners
    document.removeEventListener('engine:mode-change', handleModeChange);
    
    stopTerrainEditMode();
    
    // Clean up brush cursor
    if (brushCursorMesh) {
      brushCursorMesh.dispose();
      brushCursorMesh = null;
    }
  }
});