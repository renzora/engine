import { IconEdit, IconCopy, IconTrash, IconFolder, IconBox, IconCode, IconArchive, IconMountain, IconBrush, IconPalette, IconX, IconCirclePlus, IconCircle, IconRectangle, IconGrid3x3, IconBulb, IconVideo, IconClipboard, IconArrowBackUp, IconArrowForwardUp, IconMaximize, IconSearch, IconRotate, IconArrowUp, IconArrowRight, IconArrowDown, IconPointer, IconCircleMinus, IconSun, IconSphere, IconPlane } from '@tabler/icons-solidjs';
import { renderStore, renderActions } from '@/render/store';
import { CreateBox } from '@babylonjs/core/Meshes/Builders/boxBuilder';
import { CreateSphere } from '@babylonjs/core/Meshes/Builders/sphereBuilder';
import { CreateCylinder } from '@babylonjs/core/Meshes/Builders/cylinderBuilder';
import { CreateGround } from '@babylonjs/core/Meshes/Builders/groundBuilder';
import { HemisphericLight } from '@babylonjs/core/Lights/hemisphericLight';
import { DirectionalLight } from '@babylonjs/core/Lights/directionalLight';
import { PointLight } from '@babylonjs/core/Lights/pointLight';
import { SpotLight } from '@babylonjs/core/Lights/spotLight';
import { UniversalCamera } from '@babylonjs/core/Cameras/universalCamera';
import { StandardMaterial } from '@babylonjs/core/Materials/standardMaterial';
import { Vector3 } from '@babylonjs/core/Maths/math.vector';
import { Color3 } from '@babylonjs/core/Maths/math.color';

export const createContextMenuActions = (editorActions) => {
  // Provide fallback functions if editorActions is undefined
  const {
    selectEntity: setSelectedEntity = () => {}, 
    setTransformMode = () => {},
    setSelectedTool: setSelectedRightTool = () => {}, 
    setSelectedBottomTab: setActiveTab = () => {}
  } = editorActions || {};

  const getContextMenuItems = (item, context, currentPath = '') => {
    if (item) {
      const baseItems = [
        { label: 'Rename', action: () => handleRename(item.id), icon: <IconEdit class="w-4 h-4" /> },
        { separator: true },
        { label: 'Copy', action: () => handleCopy(item.id), icon: <IconCopy class="w-4 h-4" /> },
        { label: 'Duplicate', action: () => handleDuplicate(item.id), icon: <IconCopy class="w-4 h-4" /> },
        { label: 'Delete', action: () => handleDelete(item.id), icon: <IconTrash class="w-4 h-4" /> },
        { label: 'Add to New Folder', action: () => handleAddToNewFolder(item.id), icon: <IconFolder class="w-4 h-4" /> },
        { separator: true },
      ];

      const typeSpecificItems = [];
      
      if (item.type === 'model' || item.type === 'mesh') {
        typeSpecificItems.push(
          { label: 'Add Material', action: () => handleAddMaterial(item.id), icon: <IconBox class="w-4 h-4" /> },
          { label: 'Add Script', action: () => handleAddScript(item.id), icon: <IconCode class="w-4 h-4" /> },
        );

        if (item.hasChildMeshes) {
          typeSpecificItems.push(
            { separator: true },
            item.isUnpacked 
              ? { label: 'Pack Mesh', action: () => handlePackMesh(item.id), icon: <IconArchive class="w-4 h-4" /> }
              : { label: 'Unpack Mesh', action: () => handleUnpackMesh(item.id), icon: <IconFolder class="w-4 h-4" /> }
          );
        }
      }

      if (item.type === 'terrain') {
        typeSpecificItems.push(
          { label: 'Edit Terrain', action: () => handleEditTerrain(item.id), icon: <IconMountain class="w-4 h-4" /> },
          { label: 'Paint Texture', action: () => handlePaintTexture(item.id), icon: <IconBrush class="w-4 h-4" /> },
        );
      }

      const colorItems = [
        { separator: true },
        { label: 'Color Code', action: () => {}, icon: <IconPalette class="w-4 h-4" />, submenu: [
          { label: 'Red', action: () => handleColorCode(item.id, 'red'), color: '#ef4444' },
          { label: 'Orange', action: () => handleColorCode(item.id, 'orange'), color: '#f97316' },
          { label: 'Yellow', action: () => handleColorCode(item.id, 'yellow'), color: '#eab308' },
          { label: 'Green', action: () => handleColorCode(item.id, 'green'), color: '#22c55e' },
          { label: 'Blue', action: () => handleColorCode(item.id, 'blue'), color: '#3b82f6' },
          { label: 'Purple', action: () => handleColorCode(item.id, 'purple'), color: '#a855f7' },
          { label: 'Clear', action: () => handleColorCode(item.id, null), icon: <IconX class="w-3 h-3" /> },
        ]},
      ];

      return [...baseItems, ...typeSpecificItems, ...colorItems];
    } else {
      // Context-specific empty space menu items
      if (context === 'scene') {
        return [
          { label: 'Create Folder', action: () => handleCreateFolder(), icon: <IconFolder class="w-4 h-4" /> },
          { separator: true },
          { label: 'Create Object', action: () => {}, icon: <IconCirclePlus class="w-4 h-4" />, submenu: [
            { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconBox class="w-4 h-4" /> },
            { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
            { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconRectangle class="w-4 h-4" /> },
            { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconGrid3x3 class="w-4 h-4" /> },
          ]},
          { label: 'Light', action: () => {}, icon: <IconBulb class="w-4 h-4" />, submenu: [
            { label: 'Point Light', action: () => handleCreateObject('point-light'), icon: <IconBulb class="w-4 h-4" /> },
            { label: 'Spot Light', action: () => handleCreateObject('spot-light'), icon: <IconBulb class="w-4 h-4" /> },
            { label: 'Hemispheric Light', action: () => handleCreateObject('hemispheric-light'), icon: <IconSun class="w-4 h-4" /> },
            { label: 'Directional Light', action: () => handleCreateObject('directional-light'), icon: <IconSun class="w-4 h-4" /> },
          ]},
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <IconVideo class="w-4 h-4" /> },
          { separator: true },
          { label: 'Environment', action: () => {}, icon: <IconSphere class="w-4 h-4" />, submenu: [
            { label: 'Skybox', action: () => handleCreateObject('skybox'), icon: <IconSphere class="w-4 h-4" /> },
            { label: 'Terrain', action: () => handleCreateObject('terrain'), icon: <IconMountain class="w-4 h-4" /> },
          ]},
          { separator: true },
          { label: 'Paste', action: () => handlePaste(), icon: <IconClipboard class="w-4 h-4" /> },
          { separator: true },
          { label: 'Select All', action: () => handleSelectAll(), icon: <IconPointer class="w-4 h-4" /> },
          { label: 'Expand All', action: () => handleExpandAll(), icon: <IconCirclePlus class="w-4 h-4" /> },
          { label: 'Collapse All', action: () => handleCollapseAll(), icon: <IconCircleMinus class="w-4 h-4" /> },
        ];
      } else if (context === 'bottom-panel') {
        return [
          { label: 'Create Folder', action: () => handleCreateAssetFolder(currentPath), icon: <IconFolder class="w-4 h-4" /> },
          { label: 'Create RenScript File', action: () => handleCreateRenScript(currentPath), icon: <IconCode class="w-4 h-4" /> },
          { separator: true },
          { label: 'Paste', action: () => handlePaste(), icon: <IconClipboard class="w-4 h-4" /> },
          { separator: true },
          { label: 'Refresh', action: () => handleRefreshAssets(), icon: <IconRotate class="w-4 h-4" /> },
        ];
      } else if (context === 'viewport') {
        return [
          { label: 'Create Object', action: () => {}, icon: <IconCirclePlus class="w-4 h-4" />, submenu: [
            { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconBox class="w-4 h-4" /> },
            { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
            { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconRectangle class="w-4 h-4" /> },
            { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconGrid3x3 class="w-4 h-4" /> },
          ]},
          { label: 'Light', action: () => {}, icon: <IconBulb class="w-4 h-4" />, submenu: [
            { label: 'Point Light', action: () => handleCreateObject('point-light'), icon: <IconBulb class="w-4 h-4" /> },
            { label: 'Spot Light', action: () => handleCreateObject('spot-light'), icon: <IconBulb class="w-4 h-4" /> },
            { label: 'Hemispheric Light', action: () => handleCreateObject('hemispheric-light'), icon: <IconSun class="w-4 h-4" /> },
            { label: 'Directional Light', action: () => handleCreateObject('directional-light'), icon: <IconSun class="w-4 h-4" /> },
          ]},
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <IconVideo class="w-4 h-4" /> },
          { separator: true },
          { label: 'Paste', action: () => handlePaste(), icon: <IconClipboard class="w-4 h-4" /> },
          { separator: true },
          { label: 'Frame All', action: () => handleFrameAll(), icon: <IconMaximize class="w-4 h-4" /> },
          { label: 'Frame Selected', action: () => handleFocusSelected(), icon: <IconSearch class="w-4 h-4" /> },
          { separator: true },
          { label: 'Reset View', action: () => handleResetView(), icon: <IconRotate class="w-4 h-4" /> },
          { label: 'Top View', action: () => handleSetView('top'), icon: <IconArrowUp class="w-4 h-4" /> },
          { label: 'Front View', action: () => handleSetView('front'), icon: <IconArrowRight class="w-4 h-4" /> },
          { label: 'Right View', action: () => handleSetView('right'), icon: <IconArrowDown class="w-4 h-4" /> },
        ];
      } else {
        // Default fallback for other contexts
        return [
          { label: 'Create Object', action: () => {}, icon: <IconCirclePlus class="w-4 h-4" />, submenu: [
            { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconBox class="w-4 h-4" /> },
            { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
            { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconRectangle class="w-4 h-4" /> },
            { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconGrid3x3 class="w-4 h-4" /> },
          ]},
          { label: 'Light', action: () => {}, icon: <IconBulb class="w-4 h-4" />, submenu: [
            { label: 'Point Light', action: () => handleCreateObject('point-light'), icon: <IconBulb class="w-4 h-4" /> },
            { label: 'Spot Light', action: () => handleCreateObject('spot-light'), icon: <IconBulb class="w-4 h-4" /> },
            { label: 'Hemispheric Light', action: () => handleCreateObject('hemispheric-light'), icon: <IconSun class="w-4 h-4" /> },
            { label: 'Directional Light', action: () => handleCreateObject('directional-light'), icon: <IconSun class="w-4 h-4" /> },
          ]},
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <IconVideo class="w-4 h-4" /> },
          { separator: true },
          { label: 'Paste', action: () => handlePaste(), icon: <IconClipboard class="w-4 h-4" /> },
          { separator: true },
          { label: 'Select All', action: () => handleSelectAll(), icon: <IconPointer class="w-4 h-4" /> },
        ];
      }
    }
  };

  const handleRename = (itemId) => {
    const event = new CustomEvent('contextMenuRename', { detail: { itemId } });
    document.dispatchEvent(event);
  };

  const handleCopy = (itemId) => {
    console.log('Copy', itemId);
  };

  const handleDuplicate = (itemId) => {
    console.log('Duplicate', itemId);
  };

  const handleDelete = (itemId) => {
    console.log('Delete', itemId);
    
    // Find the object in the render store
    const scene = renderStore.scene;
    if (scene) {
      const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
      const objectToDelete = allObjects.find(obj => 
        (obj.uniqueId || obj.name) === itemId
      );
      
      if (objectToDelete) {
        renderActions.removeObject(objectToDelete);
      }
    }
  };

  const handleAddMaterial = (itemId) => {
    console.log('Add Material', itemId);
    setSelectedRightTool('materials');
  };

  const handleAddScript = (itemId) => {
    console.log('Add Script', itemId);
    setActiveTab('scripts');
  };


  const handleEditTerrain = (itemId) => {
    console.log('Edit Terrain', itemId);
    setSelectedRightTool('terrain');
  };

  const handlePaintTexture = (itemId) => {
    console.log('Paint Texture', itemId);
  };

  const handleColorCode = (itemId, color) => {
    console.log('Color Code', itemId, color);
  };

  const handleCreateObject = (type) => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    try {
      let newObject;
      const objectName = (() => {
        switch (type) {
          case 'cube': return 'Cube';
          case 'sphere': return 'Sphere';
          case 'cylinder': return 'Cylinder';
          case 'plane': return 'Plane';
          case 'hemispheric-light': return 'Hemispheric Light';
          case 'directional-light': return 'Directional Light';
          case 'point-light': return 'Point Light';
          case 'spot-light': return 'Spot Light';
          case 'camera': return 'Camera';
          case 'skybox': return 'Skybox';
          case 'terrain': return 'Terrain';
          default: return type.charAt(0).toUpperCase() + type.slice(1);
        }
      })();
      
      switch (type) {
        case 'cube': {
          newObject = CreateBox(objectName, { size: 2 }, scene);
          break;
        }
        case 'sphere': {
          newObject = CreateSphere(objectName, { diameter: 2 }, scene);
          break;
        }
        case 'cylinder': {
          newObject = CreateCylinder(objectName, { height: 3, diameter: 2 }, scene);
          break;
        }
        case 'plane': {
          newObject = CreateGround(objectName, { width: 6, height: 6 }, scene);
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
        case 'skybox': {
          // Reuse existing skybox creation from menu plugin
          document.dispatchEvent(new CustomEvent('engine:create-skybox'));
          return; // Exit early since event-based creation doesn't return an object
        }
        case 'terrain': {
          // Reuse existing terrain creation from menu plugin
          document.dispatchEvent(new CustomEvent('engine:create-terrain'));
          return; // Exit early since event-based creation doesn't return an object
        }
      }
      
      if (newObject) {
        // Position objects appropriately
        if (newObject.position) {
          // Different positioning for different object types
          if (type.includes('light')) {
            // Position lights higher up
            newObject.position.x = Math.random() * 6 - 3;
            newObject.position.y = 3 + Math.random() * 3; // 3-6 units high
            newObject.position.z = Math.random() * 6 - 3;
          } else if (type === 'camera') {
            // Position cameras at a good viewing angle
            newObject.position.x = Math.random() * 8 - 4;
            newObject.position.y = 2 + Math.random() * 3; // 2-5 units high
            newObject.position.z = Math.random() * 8 - 4;
          } else {
            // Position meshes on the ground
            newObject.position.x = Math.random() * 4 - 2;
            newObject.position.z = Math.random() * 4 - 2;
          }
        }
        
        // Add material for meshes
        if (newObject.material !== undefined && !type.includes('light') && type !== 'camera') {
          const material = new StandardMaterial(objectName + "_material", scene);
          material.diffuseColor = new Color3(
            Math.random(),
            Math.random(),
            Math.random()
          );
          newObject.material = material;
        }
        
        // Add to hierarchy and select
        renderActions.addObject(newObject);
        const objectId = newObject.uniqueId || newObject.name;
        setSelectedEntity(objectId);
        setTransformMode('move');
        
        setTimeout(() => {
          const canvas = document.querySelector('canvas');
          if (canvas) {
            canvas.focus();
          }
        }, 100);
      }
    } catch (error) {
      console.error('Error creating object:', error);
    }
  };

  const handlePaste = () => {
    console.log('Paste');
  };

  const handleUndo = () => {
    console.log('Undo');
  };

  const handleRedo = () => {
    console.log('Redo');
  };

  const handleSelectAll = () => {
    console.log('Select All');
  };

  const handleFocusSelected = () => {
    console.log('Focus Selected');
  };

  const handleFrameAll = () => {
    console.log('Frame All');
  };

  const handleResetView = () => {
    console.log('Reset View');
  };

  const handleSetView = (view) => {
    console.log('Set View', view);
  };

  const handleExpandAll = () => {
    console.log('Expand All');
  };

  const handleCollapseAll = () => {
    console.log('Collapse All');
  };

  const handleUnpackMesh = (itemId) => {
    console.log('Unpack Mesh', itemId);
    // TODO: Implement mesh unpacking functionality
  };

  const handlePackMesh = (itemId) => {
    console.log('Pack Mesh', itemId);
    // TODO: Implement mesh packing functionality
  };

  const handleAddToNewFolder = (itemId) => {
    const event = new CustomEvent('contextMenuAddToNewFolder', { detail: { itemId } });
    document.dispatchEvent(event);
  };

  const handleCreateFolder = () => {
    const event = new CustomEvent('contextMenuCreateFolder');
    document.dispatchEvent(event);
  };

  const handleCreateAssetFolder = (currentPath) => {
    const event = new CustomEvent('contextMenuCreateAssetFolder', { detail: { currentPath } });
    document.dispatchEvent(event);
  };

  const handleCreateRenScript = (currentPath) => {
    const event = new CustomEvent('contextMenuCreateRenScript', { detail: { currentPath } });
    document.dispatchEvent(event);
  };

  const handleRefreshAssets = () => {
    const event = new CustomEvent('contextMenuRefreshAssets');
    document.dispatchEvent(event);
  };

  return {
    getContextMenuItems,
    handleCreateObject,
    handleDelete,
    handleRename,
    handleCopy,
    handleDuplicate,
    handleAddMaterial,
    handleAddScript,
    handleEditTerrain,
    handlePaintTexture,
    handleColorCode,
    handlePaste,
    handleUndo,
    handleRedo,
    handleSelectAll,
    handleFocusSelected,
    handleFrameAll,
    handleResetView,
    handleSetView,
    handleExpandAll,
    handleCollapseAll,
    handleUnpackMesh,
    handlePackMesh,
    handleAddToNewFolder,
    handleCreateFolder,
    handleCreateAssetFolder,
    handleCreateRenScript,
    handleRefreshAssets
  };
};
