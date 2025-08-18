import { Edit, Copy, Trash, Folder, Box, Code, Archive, Mountain, Palette, X, Plus, Circle, Square, Grid3x3, Lightbulb, Video, Clipboard, ArrowLeft, ArrowRight, Maximize, Search, RotateCcw, ArrowUp, ArrowDown, MousePointer, Minus } from '@/ui/icons';

export const createContextMenuActions = (editorActions) => {
  const {
    addSceneObject, removeSceneObject, selectEntity: setSelectedEntity, setTransformMode,
    setSelectedTool: setSelectedRightTool, setSelectedBottomTab: setActiveTab,
    unpackMesh, packMesh
  } = editorActions;

  const getContextMenuItems = (item, context) => {
    if (item) {
      const baseItems = [
        { label: 'Rename', action: () => handleRename(item.id), icon: Edit },
        { separator: true },
        { label: 'Copy', action: () => handleCopy(item.id), icon: Copy },
        { label: 'Duplicate', action: () => handleDuplicate(item.id), icon: Copy },
        { label: 'Delete', action: () => handleDelete(item.id), icon: Trash },
        { label: 'Add to New Folder', action: () => handleAddToNewFolder(item.id), icon: Folder },
        { separator: true },
      ];

      const typeSpecificItems = [];
      
      if (item.type === 'model' || item.type === 'mesh') {
        typeSpecificItems.push(
          { label: 'Add Material', action: () => handleAddMaterial(item.id), icon: Box },
          { label: 'Add Script', action: () => handleAddScript(item.id), icon: Code },
        );

        if (item.hasChildMeshes) {
          typeSpecificItems.push(
            { separator: true },
            item.isUnpacked 
              ? { label: 'Pack Mesh', action: () => handlePackMesh(item.id), icon: Archive }
              : { label: 'Unpack Mesh', action: () => handleUnpackMesh(item.id), icon: Folder }
          );
        }
      }

      if (item.type === 'terrain') {
        typeSpecificItems.push(
          { label: 'Edit Terrain', action: () => handleEditTerrain(item.id), icon: Mountain },
          { label: 'Paint Texture', action: () => handlePaintTexture(item.id), icon: Palette },
        );
      }

      const colorItems = [
        { separator: true },
        { label: 'Color Code', action: () => {}, icon: Palette, submenu: [
          { label: 'Red', action: () => handleColorCode(item.id, 'red'), color: '#ef4444' },
          { label: 'Orange', action: () => handleColorCode(item.id, 'orange'), color: '#f97316' },
          { label: 'Yellow', action: () => handleColorCode(item.id, 'yellow'), color: '#eab308' },
          { label: 'Green', action: () => handleColorCode(item.id, 'green'), color: '#22c55e' },
          { label: 'Blue', action: () => handleColorCode(item.id, 'blue'), color: '#3b82f6' },
          { label: 'Purple', action: () => handleColorCode(item.id, 'purple'), color: '#a855f7' },
          { label: 'Clear', action: () => handleColorCode(item.id, null), icon: X },
        ]},
      ];

      return [...baseItems, ...typeSpecificItems, ...colorItems];
    } else {
      const baseGeneralItems = [
        { label: 'Create Object', action: () => {}, icon: Plus, submenu: [
          { label: 'Cube', action: () => handleCreateObject('cube'), icon: Box },
          { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: Circle },
          { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: Square },
          { label: 'Plane', action: () => handleCreateObject('plane'), icon: Grid3x3 },
          { separator: true },
          { label: 'Light', action: () => handleCreateObject('light'), icon: Lightbulb },
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: Video },
        ]},
        { separator: true },
        { label: 'Paste', action: () => handlePaste(), icon: Clipboard },
        { separator: true },
        { label: 'Undo', action: () => handleUndo(), icon: ArrowLeft },
        { label: 'Redo', action: () => handleRedo(), icon: ArrowRight },
      ];

      if (context === 'viewport') {
        return [
          ...baseGeneralItems,
          { separator: true },
          { label: 'Frame All', action: () => handleFrameAll(), icon: Maximize },
          { label: 'Frame Selected', action: () => handleFocusSelected(), icon: Search },
          { separator: true },
          { label: 'Reset View', action: () => handleResetView(), icon: RotateCcw },
          { label: 'Top View', action: () => handleSetView('top'), icon: ArrowUp },
          { label: 'Front View', action: () => handleSetView('front'), icon: ArrowRight },
          { label: 'Right View', action: () => handleSetView('right'), icon: ArrowDown },
        ];
      } else {
        return [
          ...baseGeneralItems,
          { separator: true },
          { label: 'Select All', action: () => handleSelectAll(), icon: MousePointer },
          { label: 'Expand All', action: () => handleExpandAll(), icon: Plus },
          { label: 'Collapse All', action: () => handleCollapseAll(), icon: Minus },
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