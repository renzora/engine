import { IconEdit, IconCopy, IconTrash, IconFolder, IconBox, IconCode, IconArchive, IconMountain, IconBrush, IconPalette, IconX, IconCirclePlus, IconCircle, IconCylinder, IconSquare, IconBulb, IconVideo, IconClipboard, IconArrowBackUp, IconArrowForwardUp, IconMaximize, IconSearch, IconRotate, IconArrowUp, IconArrowRight, IconArrowDown, IconPointer, IconCircleMinus, IconSun, IconSphere, IconPlane } from '@tabler/icons-solidjs';
import { renderStore, renderActions } from '@/render/store';
import { editorStore } from '@/layout/stores/EditorStore';
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
import { createAndAddObject } from '@/api/creation/ObjectCreationUtils.jsx';

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
      // Different base items depending on whether this is the scene root
      const baseItems = item.type === 'scene' ? [
        { label: 'Rename', action: () => handleRename(item.id), icon: <IconEdit class="w-4 h-4" /> },
        { separator: true },
      ] : [
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
          { label: 'Object', action: () => {}, icon: <IconCirclePlus class="w-4 h-4" />, submenu: [
            { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconBox class="w-4 h-4" /> },
            { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
            { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconCylinder class="w-4 h-4" /> },
            { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconSquare class="w-4 h-4" /> },
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
          { label: 'New Material', action: () => handleCreateNewMaterial(currentPath), icon: <IconPalette class="w-4 h-4" /> },
          { separator: true },
          { label: 'Paste', action: () => handlePaste(), icon: <IconClipboard class="w-4 h-4" /> },
          { separator: true },
          { label: 'Refresh', action: () => handleRefreshAssets(), icon: <IconRotate class="w-4 h-4" /> },
        ];
      } else if (context === 'viewport') {
        return [
          { label: 'Object', action: () => {}, icon: <IconCirclePlus class="w-4 h-4" />, submenu: [
            { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconBox class="w-4 h-4" /> },
            { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
            { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconCylinder class="w-4 h-4" /> },
            { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconSquare class="w-4 h-4" /> },
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
          { label: 'Object', action: () => {}, icon: <IconCirclePlus class="w-4 h-4" />, submenu: [
            { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconBox class="w-4 h-4" /> },
            { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
            { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconCylinder class="w-4 h-4" /> },
            { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconSquare class="w-4 h-4" /> },
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
    
    // Check if this is a virtual folder
    const isVirtualFolder = typeof itemId === 'string' && itemId.startsWith('virtual-folder-');
    
    if (isVirtualFolder) {
      // Dispatch event to Scene component to handle virtual folder deletion
      const event = new CustomEvent('contextMenuDeleteVirtualFolder', { 
        detail: { itemId } 
      });
      document.dispatchEvent(event);
    } else {
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
    const event = new CustomEvent('contextMenuColorCode', { 
      detail: { itemId, color } 
    });
    document.dispatchEvent(event);
  };

  const handleCreateObject = async (type) => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    try {
      // Handle special cases that use events
      if (type === 'skybox') {
        document.dispatchEvent(new CustomEvent('engine:create-skybox'));
        return;
      }
      if (type === 'terrain') {
        document.dispatchEvent(new CustomEvent('engine:create-terrain'));
        return;
      }
      
      // Use unified creation system for consistent sizes and colors
      const objectId = createAndAddObject(type, scene);
      setSelectedEntity(objectId);
      setTransformMode('move');
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

  const handleCreateNewMaterial = (currentPath) => {
    // Use the event-driven approach like other file creation functions
    const event = new CustomEvent('contextMenuCreateNewMaterial', { detail: { currentPath } });
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
    handleRefreshAssets,
    handleCreateNewMaterial
  };
};
