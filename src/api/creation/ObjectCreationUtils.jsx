// Unified object creation utilities for folder-aware creation
import { renderStore, renderActions } from '@/render/store';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { CreateBox } from '@babylonjs/core/Meshes/Builders/boxBuilder';
import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder';
import { CreateCylinder } from '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import { CreateGround } from '@babylonjs/core/Meshes/Builders/groundBuilder';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight';
import { PointLight } from '@babylonjs/core/Lights/pointLight';
import { SpotLight } from '@babylonjs/core/Lights/spotLight';
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';

/**
 * Helper function to find items in hierarchy
 */
export const findItemInHierarchy = (items, id) => {
  for (const item of items) {
    if (item.id === id) return item;
    if (item.children) {
      const found = findItemInHierarchy(item.children, id);
      if (found) return found;
    }
  }
  return null;
};

/**
 * Unified function to handle folder-aware object creation
 * This function handles the common logic for adding objects to the scene hierarchy
 * with proper folder awareness
 * 
 * @param {Object} babylonObject - The Babylon.js object to add
 * @param {string} objectName - Display name for the object
 * @param {boolean} selectAfterCreation - Whether to select the object after creation (default: true)
 * @returns {string} The object ID
 */
export const addObjectToHierarchy = (babylonObject, objectName, selectAfterCreation = true) => {
  // Get current selection to determine folder context
  const currentSelection = editorStore.selection.entity;
  let selectedItem = null;
  
  if (currentSelection) {
    const hierarchy = renderStore.hierarchy;
    selectedItem = findItemInHierarchy(hierarchy, currentSelection);
    
    if (selectedItem && selectedItem.type === 'folder') {
      // If a folder is selected, set it as parent
      if (selectedItem.isVirtual) {
        // For virtual folders, objects stay at scene root in Babylon but are organized in hierarchy
        babylonObject.parent = null;
      } else if (selectedItem.babylonObject) {
        // For real Babylon folders (TransformNodes), set as parent
        babylonObject.parent = selectedItem.babylonObject;
      }
      
      console.log(`📁 Adding ${objectName} as child of folder: ${selectedItem.name}`);
    }
  }
  
  // Add to hierarchy
  renderActions.addObject(babylonObject);
  const objectId = babylonObject.uniqueId || babylonObject.name;
  
  // If we're adding to a virtual folder, we need to move it there after it's been added
  if (selectedItem && selectedItem.type === 'folder' && selectedItem.isVirtual) {
    // Move the object to the virtual folder in the hierarchy
    setTimeout(() => {
      renderActions.reorderObjectInHierarchy(objectId, currentSelection, 'inside');
      console.log(`📁 Moved ${objectName} into virtual folder: ${selectedItem.name}`);
    }, 100);
  }
  
  // Select the object if requested
  if (selectAfterCreation) {
    // Update both editor store and render store for proper synchronization
    if (typeof editorActions.selectEntity === 'function') {
      editorActions.selectEntity(objectId);
    }
    
    // Also update render store to ensure 3D highlighting and property tabs work
    renderActions.selectObject(babylonObject);
    
    // Set transform mode for non-scene objects
    if (objectId !== 'scene-root' && typeof editorActions.setTransformMode === 'function') {
      editorActions.setTransformMode('move');
    }
  }
  
  return objectId;
};

/**
 * Unified object creation function that creates Babylon objects with consistent parameters
 * 
 * @param {string} objectType - Type of object to create ('cube', 'sphere', 'cylinder', etc.)
 * @param {Object} scene - Babylon.js scene
 * @returns {Object} The created Babylon.js object
 */
export const createBabylonObject = (objectType, scene) => {

  const objectName = (() => {
    switch (objectType) {
      case 'cube': return 'Cube';
      case 'sphere': return 'Sphere';
      case 'cylinder': return 'Cylinder';
      case 'plane': return 'Plane';
      case 'hemispheric-light': return 'Hemispheric Light';
      case 'directional-light': return 'Directional Light';
      case 'point-light': return 'Point Light';
      case 'spot-light': return 'Spot Light';
      case 'camera': return 'Camera';
      default: return objectType.charAt(0).toUpperCase() + objectType.slice(1);
    }
  })();

  let newObject;

  switch (objectType) {
    case 'cube': {
      newObject = CreateBox(objectName, { size: 1 }, scene); // Standardize to size 1
      break;
    }
    case 'sphere': {
      newObject = CreateSphere(objectName, { diameter: 1 }, scene); // Standardize to diameter 1
      break;
    }
    case 'cylinder': {
      newObject = CreateCylinder(objectName, { height: 1, diameter: 1 }, scene); // Standardize sizes
      break;
    }
    case 'plane': {
      newObject = CreateGround(objectName, { width: 1, height: 1 }, scene); // Standardize to 1x1
      break;
    }
    case 'hemispheric-light': {
      newObject = new HemisphericLight(objectName, new Vector3(0, 1, 0), scene);
      newObject.intensity = 0.7;
      break;
    }
    case 'directional-light': {
      newObject = new DirectionalLight(objectName, new Vector3(-1, -1, -1), scene);
      newObject.intensity = 1.0;
      break;
    }
    case 'point-light': {
      newObject = new PointLight(objectName, new Vector3(0, 5, 0), scene);
      newObject.intensity = 1.0;
      newObject.range = 100;
      break;
    }
    case 'spot-light': {
      newObject = new SpotLight(objectName, new Vector3(0, 5, 0), new Vector3(0, -1, 0), Math.PI / 3, 2, scene);
      newObject.intensity = 1.0;
      newObject.range = 100;
      break;
    }
    case 'camera': {
      newObject = new UniversalCamera(objectName, new Vector3(0, 5, -10), scene);
      newObject.lookAt(Vector3.Zero());
      break;
    }
    default:
      throw new Error(`Unknown object type: ${objectType}`);
  }

  return { object: newObject, name: objectName };
};

/**
 * Generic function to handle object creation with standard positioning and materials
 * This provides a common interface for all object creation
 * 
 * @param {string} objectType - Type of object to create
 * @param {Object} scene - Babylon.js scene
 * @returns {string} The object ID
 */
export const createAndAddObject = (objectType, scene) => {
  const { object: babylonObject, name: objectName } = createBabylonObject(objectType, scene);
  
  // Position objects appropriately based on type
  if (babylonObject.position) {
    if (objectType.includes('light')) {
      // Position lights higher up
      babylonObject.position.x = Math.random() * 6 - 3;
      babylonObject.position.y = 3 + Math.random() * 3; // 3-6 units high
      babylonObject.position.z = Math.random() * 6 - 3;
    } else if (objectType === 'camera') {
      // Position cameras at a good viewing angle
      babylonObject.position.x = Math.random() * 8 - 4;
      babylonObject.position.y = 2 + Math.random() * 3; // 2-5 units high
      babylonObject.position.z = Math.random() * 8 - 4;
    } else {
      // Position meshes on the ground with slight offset
      babylonObject.position.x = Math.random() * 4 - 2;
      babylonObject.position.y = 0.5; // Consistent Y position for meshes
      babylonObject.position.z = Math.random() * 4 - 2;
    }
  }
  
  // Add standardized material for meshes with consistent color seed
  if (babylonObject.material !== undefined && !objectType.includes('light') && objectType !== 'camera') {
    const material = new StandardMaterial(objectName + "_material", scene);
    // Use a fixed seed based on object type for consistent colors
    const colorSeed = objectType.split('').reduce((acc, char) => acc + char.charCodeAt(0), 0) / 1000;
    material.diffuseColor = new Color3(
      0.3 + (colorSeed * 0.7) % 0.7,
      0.3 + ((colorSeed * 1.3) % 1) * 0.7,
      0.3 + ((colorSeed * 1.7) % 1) * 0.7
    );
    babylonObject.material = material;
  }
  
  // Use unified hierarchy addition
  const objectId = addObjectToHierarchy(babylonObject, objectName, true);
  
  // Focus canvas for immediate interaction
  setTimeout(() => {
    const canvas = document.querySelector('canvas');
    if (canvas) {
      canvas.focus();
    }
  }, 100);
  
  return objectId;
};

/**
 * Legacy function for backward compatibility
 * @deprecated Use createAndAddObject instead
 */
export const finalizeObjectCreation = async (babylonObject, objectType, objectName, scene) => {
  console.warn('finalizeObjectCreation is deprecated, use createAndAddObject instead');
  return addObjectToHierarchy(babylonObject, objectName, true);
};