import { MeshBuilder } from '@babylonjs/core/Meshes/meshBuilder.js';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial.js';
import { Color3 } from '@babylonjs/core/Maths/math.color.js';

// Function to create visual helpers for lights (shared between toolbar creation and scene loading)
export const createLightVisualHelper = (mainContainer, lightType, lightPosition, scene) => {
  const lightName = mainContainer.name;
  console.log('Creating light visual helper:', { lightName, lightType, lightPosition });
  
  // Helper materials
  const iconMaterial = new StandardMaterial(`${lightName}_icon_material`, scene);
  iconMaterial.emissiveColor = new Color3(1, 0.8, 0.2); // Bright orange
  iconMaterial.diffuseColor = new Color3(1, 0.8, 0.2);
  iconMaterial.disableLighting = true;
  iconMaterial.alpha = 1.0;
  
  const wireMaterial = new StandardMaterial(`${lightName}_wire_material`, scene);
  wireMaterial.emissiveColor = new Color3(1, 0.8, 0.2); // Same orange
  wireMaterial.wireframe = true;
  wireMaterial.disableLighting = true;
  wireMaterial.alpha = 0.8;
  
  const directionMaterial = new StandardMaterial(`${lightName}_direction_material`, scene);
  directionMaterial.emissiveColor = new Color3(0.8, 0.8, 1.0); // Light blue
  directionMaterial.diffuseColor = new Color3(0.8, 0.8, 1.0);
  directionMaterial.disableLighting = true;
  directionMaterial.alpha = 0.7;
  
  let mainIcon;
  
  switch (lightType) {
    case 'directional':
      // Unity/Unreal style sun icon with directional arrow
      mainIcon = MeshBuilder.CreateSphere(`${lightName}_icon`, { diameter: 0.4 }, scene);
      mainIcon.material = iconMaterial;
      
      // Add sun rays around the sphere
      for (let i = 0; i < 8; i++) {
        const angle = (i / 8) * Math.PI * 2;
        const ray = MeshBuilder.CreateBox(`${lightName}_ray_${i}`, { 
          width: 0.06, 
          height: 0.4, 
          depth: 0.06 
        }, scene);
        ray.position.x = Math.cos(angle) * 0.3;
        ray.position.z = Math.sin(angle) * 0.3;
        ray.rotation.y = angle;
        ray.material = iconMaterial;
        ray.parent = mainContainer;
        ray.isPickable = false;
        ray.metadata = { isLightHelper: true };
      }
      
      // Add large directional arrow pointing in light direction
      const dirArrow = MeshBuilder.CreateCylinder(`${lightName}_dir_arrow`, { 
        height: 3.0, 
        diameterTop: 0.0, 
        diameterBottom: 0.4 
      }, scene);
      dirArrow.position.set(-1.5, -1.5, -1.5); // Default direction
      dirArrow.rotation.x = Math.PI / 2;
      dirArrow.material = directionMaterial;
      dirArrow.parent = mainContainer;
      dirArrow.isPickable = false;
      dirArrow.metadata = { isLightHelper: true };
      break;
      
    case 'point':
      // Unity/Unreal style point light - simple sphere icon only
      mainIcon = MeshBuilder.CreateSphere(`${lightName}_icon`, { diameter: 0.5 }, scene);
      mainIcon.material = iconMaterial;
      break;
      
    case 'spot':
      // Unity/Unreal style spot light - cone with minimal lines
      mainIcon = MeshBuilder.CreateSphere(`${lightName}_icon`, { diameter: 0.4 }, scene);
      mainIcon.material = iconMaterial;
      
      // Create just 3 cone outline lines instead of full wireframe
      const coneHeight = 4.0;
      const coneRadius = 2.0;
      
      // Center direction line
      const centerLine = MeshBuilder.CreateBox(`${lightName}_center_line`, { 
        width: 0.03, 
        height: coneHeight, 
        depth: 0.03 
      }, scene);
      centerLine.position.y = -coneHeight / 2;
      centerLine.material = directionMaterial;
      centerLine.parent = mainContainer;
      centerLine.isPickable = false;
      centerLine.metadata = { isLightHelper: true };
      
      // Two edge lines to show cone shape
      for (let i = 0; i < 2; i++) {
        const angle = i * Math.PI; // 0° and 180°
        const edgeLine = MeshBuilder.CreateBox(`${lightName}_edge_line_${i}`, { 
          width: 0.02, 
          height: Math.sqrt(coneHeight * coneHeight + coneRadius * coneRadius), 
          depth: 0.02 
        }, scene);
        
        // Position at cone edge
        const edgeX = Math.cos(angle) * coneRadius;
        const edgeZ = Math.sin(angle) * coneRadius;
        edgeLine.position.set(edgeX / 2, -coneHeight / 2, edgeZ / 2);
        
        // Rotate to point from center to edge
        const rotationAngle = Math.atan2(coneRadius, coneHeight);
        edgeLine.rotation.z = Math.cos(angle) * rotationAngle;
        edgeLine.rotation.x = Math.sin(angle) * rotationAngle;
        
        edgeLine.material = directionMaterial;
        edgeLine.parent = mainContainer;
        edgeLine.isPickable = false;
        edgeLine.metadata = { isLightHelper: true };
      }
      break;
      
    case 'hemispheric':
      // Unity/Unreal style ambient light - clean hemisphere
      mainIcon = MeshBuilder.CreateSphere(`${lightName}_icon`, { 
        diameter: 0.5,
        slice: 0.5 
      }, scene);
      mainIcon.material = iconMaterial;
      
      // Add just a simple ground circle indicator instead of wireframe
      const groundCircle = MeshBuilder.CreateTorus(`${lightName}_ground_circle`, { 
        diameter: 3.0, 
        thickness: 0.05 
      }, scene);
      groundCircle.rotation.x = Math.PI / 2;
      groundCircle.position.y = 0;
      groundCircle.material = directionMaterial;
      groundCircle.parent = mainContainer;
      groundCircle.isPickable = false;
      groundCircle.metadata = { isLightHelper: true };
      break;
      
    case 'rectArea':
      // Unity/Unreal style area light - clean rectangle outline
      mainIcon = MeshBuilder.CreatePlane(`${lightName}_icon`, { 
        width: 0.6, 
        height: 0.6 
      }, scene);
      mainIcon.material = iconMaterial;
      
      // Add simple rectangle outline with 4 edge lines instead of wireframe
      const rectSize = 2.0;
      const edgeThickness = 0.03;
      
      // Top edge
      const topEdge = MeshBuilder.CreateBox(`${lightName}_top_edge`, { 
        width: rectSize, 
        height: edgeThickness, 
        depth: edgeThickness 
      }, scene);
      topEdge.position.y = rectSize / 2;
      topEdge.material = directionMaterial;
      topEdge.parent = mainContainer;
      topEdge.isPickable = false;
      topEdge.metadata = { isLightHelper: true };
      
      // Bottom edge
      const bottomEdge = MeshBuilder.CreateBox(`${lightName}_bottom_edge`, { 
        width: rectSize, 
        height: edgeThickness, 
        depth: edgeThickness 
      }, scene);
      bottomEdge.position.y = -rectSize / 2;
      bottomEdge.material = directionMaterial;
      bottomEdge.parent = mainContainer;
      bottomEdge.isPickable = false;
      bottomEdge.metadata = { isLightHelper: true };
      
      // Left edge
      const leftEdge = MeshBuilder.CreateBox(`${lightName}_left_edge`, { 
        width: edgeThickness, 
        height: rectSize, 
        depth: edgeThickness 
      }, scene);
      leftEdge.position.x = -rectSize / 2;
      leftEdge.material = directionMaterial;
      leftEdge.parent = mainContainer;
      leftEdge.isPickable = false;
      leftEdge.metadata = { isLightHelper: true };
      
      // Right edge
      const rightEdge = MeshBuilder.CreateBox(`${lightName}_right_edge`, { 
        width: edgeThickness, 
        height: rectSize, 
        depth: edgeThickness 
      }, scene);
      rightEdge.position.x = rectSize / 2;
      rightEdge.material = directionMaterial;
      rightEdge.parent = mainContainer;
      rightEdge.isPickable = false;
      rightEdge.metadata = { isLightHelper: true };
      break;
      
    default:
      // Default fallback
      mainIcon = MeshBuilder.CreateSphere(`${lightName}_icon`, { diameter: 0.4 }, scene);
      mainIcon.material = iconMaterial;
      break;
  }
  
  if (mainIcon) {
    mainIcon.parent = mainContainer;
    mainIcon.isPickable = false;
    mainIcon.metadata = { isLightHelper: true };
  }
  
  // Create an invisible pickable sphere at the center for easier selection
  const selectionHelper = MeshBuilder.CreateSphere(`${lightName}_selection`, { diameter: 0.8 }, scene);
  const invisibleMaterial = new StandardMaterial(`${lightName}_invisible_material`, scene);
  invisibleMaterial.alpha = 0.0; // Completely transparent
  invisibleMaterial.disableLighting = true;
  selectionHelper.material = invisibleMaterial;
  selectionHelper.parent = mainContainer;
  selectionHelper.isPickable = true; // This one IS pickable
  selectionHelper.metadata = { isLightSelectionHelper: true };
};