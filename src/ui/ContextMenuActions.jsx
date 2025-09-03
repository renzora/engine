import { IconEdit, IconCopy, IconTrash, IconFolder, IconBox, IconCode, IconArchive, IconMountain, IconBrush, IconPalette, IconX, IconCirclePlus, IconCircle, IconRectangle, IconGrid3x3, IconBulb, IconVideo, IconClipboard, IconArrowBackUp, IconArrowForwardUp, IconMaximize, IconSearch, IconRotate, IconArrowUp, IconArrowRight, IconArrowDown, IconPointer, IconCircleMinus } from '@tabler/icons-solidjs';

export const createContextMenuActions = (editorActions) => {
  const {
    addSceneObject, removeSceneObject, selectEntity: setSelectedEntity, setTransformMode,
    setSelectedTool: setSelectedRightTool, setSelectedBottomTab: setActiveTab,
    unpackMesh, packMesh
  } = editorActions;

  const getContextMenuItems = (item, context) => {
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
      const baseGeneralItems = [
        { label: 'Create Object', action: () => {}, icon: <IconCirclePlus class="w-4 h-4" />, submenu: [
          { label: 'Cube', action: () => handleCreateObject('cube'), icon: <IconBox class="w-4 h-4" /> },
          { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <IconCircle class="w-4 h-4" /> },
          { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <IconRectangle class="w-4 h-4" /> },
          { label: 'Plane', action: () => handleCreateObject('plane'), icon: <IconGrid3x3 class="w-4 h-4" /> },
          { separator: true },
          { label: 'Light', action: () => handleCreateObject('light'), icon: <IconBulb class="w-4 h-4" /> },
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <IconVideo class="w-4 h-4" /> },
        ]},
        { separator: true },
        { label: 'Paste', action: () => handlePaste(), icon: <IconClipboard class="w-4 h-4" /> },
        { separator: true },
        { label: 'Undo', action: () => handleUndo(), icon: <IconArrowBackUp class="w-4 h-4" /> },
        { label: 'Redo', action: () => handleRedo(), icon: <IconArrowForwardUp class="w-4 h-4" /> },
      ];

      if (context === 'viewport') {
        return [
          ...baseGeneralItems,
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
        return [
          ...baseGeneralItems,
          { separator: true },
          { label: 'Select All', action: () => handleSelectAll(), icon: <IconPointer class="w-4 h-4" /> },
          { label: 'Expand All', action: () => handleExpandAll(), icon: <IconCirclePlus class="w-4 h-4" /> },
          { label: 'Collapse All', action: () => handleCollapseAll(), icon: <IconCircleMinus class="w-4 h-4" /> },
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
    removeSceneObject(itemId);
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
    const newObject = {
      name: type.charAt(0).toUpperCase() + type.slice(1),
      type: 'mesh',
      position: [Math.random() * 4 - 2, 0, Math.random() * 4 - 2],
      rotation: [0, 0, 0],
      scale: [1, 1, 1],
      geometry: type === 'cube' ? 'box' : type,
      material: { 
        color: `hsl(${Math.random() * 360}, 70%, 50%)`
      },
      visible: true
    };
    
    const objectWithId = addSceneObject(newObject);
    setSelectedEntity(objectWithId.id);
    setTransformMode('move');
    
    setTimeout(() => {
      const canvas = document.querySelector('canvas');
      if (canvas) {
        canvas.focus();
      }
    }, 100);
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
    unpackMesh(itemId);
  };

  const handlePackMesh = (itemId) => {
    console.log('Pack Mesh', itemId);
    packMesh(itemId);
  };

  const handleAddToNewFolder = (itemId) => {
    const event = new CustomEvent('contextMenuAddToNewFolder', { detail: { itemId } });
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
    handleAddToNewFolder
  };
};
