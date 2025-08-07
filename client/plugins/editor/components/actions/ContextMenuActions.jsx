import { Icons } from '@/plugins/editor/components/Icons';

export const useContextMenuActions = (editorActions) => {
  const {
    addSceneObject, removeSceneObject, setSelectedEntity, setTransformMode,
    setSelectedTool: setSelectedRightTool, setSelectedBottomTab: setActiveTab,
    unpackMesh, packMesh
  } = editorActions;

  const getContextMenuItems = (item, context) => {
    if (item) {
      const baseItems = [
        { label: 'Rename', action: () => handleRename(item.id), icon: <Icons.Pencil className="w-4 h-4" /> },
        { separator: true },
        { label: 'Copy', action: () => handleCopy(item.id), icon: <Icons.Copy className="w-4 h-4" /> },
        { label: 'Duplicate', action: () => handleDuplicate(item.id), icon: <Icons.DocumentDuplicate className="w-4 h-4" /> },
        { label: 'Delete', action: () => handleDelete(item.id), icon: <Icons.Trash className="w-4 h-4" /> },
        { label: 'Add to New Folder', action: () => handleAddToNewFolder(item.id), icon: <Icons.Folder className="w-4 h-4" /> },
        { separator: true },
      ];

      const typeSpecificItems = [];
      
      if (item.type === 'model' || item.type === 'mesh') {
        typeSpecificItems.push(
          { label: 'Add Material', action: () => handleAddMaterial(item.id), icon: <Icons.Cube className="w-4 h-4" /> },
          { label: 'Add Script', action: () => handleAddScript(item.id), icon: <Icons.CodeBracket className="w-4 h-4" /> },
        );

        if (item.hasChildMeshes) {
          typeSpecificItems.push(
            { separator: true },
            item.isUnpacked 
              ? { label: 'Pack Mesh', action: () => handlePackMesh(item.id), icon: <Icons.Archive className="w-4 h-4" /> }
              : { label: 'Unpack Mesh', action: () => handleUnpackMesh(item.id), icon: <Icons.Folder className="w-4 h-4" /> }
          );
        }
      }

      if (item.type === 'terrain') {
        typeSpecificItems.push(
          { label: 'Edit Terrain', action: () => handleEditTerrain(item.id), icon: <Icons.Mountain className="w-4 h-4" /> },
          { label: 'Paint Texture', action: () => handlePaintTexture(item.id), icon: <Icons.PaintBrush className="w-4 h-4" /> },
        );
      }

      const colorItems = [
        { separator: true },
        { label: 'Color Code', action: () => {}, icon: <Icons.ColorSwatch className="w-4 h-4" />, submenu: [
          { label: 'Red', action: () => handleColorCode(item.id, 'red'), color: '#ef4444' },
          { label: 'Orange', action: () => handleColorCode(item.id, 'orange'), color: '#f97316' },
          { label: 'Yellow', action: () => handleColorCode(item.id, 'yellow'), color: '#eab308' },
          { label: 'Green', action: () => handleColorCode(item.id, 'green'), color: '#22c55e' },
          { label: 'Blue', action: () => handleColorCode(item.id, 'blue'), color: '#3b82f6' },
          { label: 'Purple', action: () => handleColorCode(item.id, 'purple'), color: '#a855f7' },
          { label: 'Clear', action: () => handleColorCode(item.id, null), icon: <Icons.XMark className="w-3 h-3" /> },
        ]},
      ];

      return [...baseItems, ...typeSpecificItems, ...colorItems];
    } else {
      const baseGeneralItems = [
        { label: 'Create Object', action: () => {}, icon: <Icons.PlusCircle className="w-4 h-4" />, submenu: [
          { label: 'Cube', action: () => handleCreateObject('cube'), icon: <Icons.Cube className="w-4 h-4" /> },
          { label: 'Sphere', action: () => handleCreateObject('sphere'), icon: <Icons.Circle className="w-4 h-4" /> },
          { label: 'Cylinder', action: () => handleCreateObject('cylinder'), icon: <Icons.Rectangle className="w-4 h-4" /> },
          { label: 'Plane', action: () => handleCreateObject('plane'), icon: <Icons.Square2Stack className="w-4 h-4" /> },
          { separator: true },
          { label: 'Light', action: () => handleCreateObject('light'), icon: <Icons.LightBulb className="w-4 h-4" /> },
          { label: 'Camera', action: () => handleCreateObject('camera'), icon: <Icons.Video className="w-4 h-4" /> },
        ]},
        { separator: true },
        { label: 'Paste', action: () => handlePaste(), icon: <Icons.Clipboard className="w-4 h-4" /> },
        { separator: true },
        { label: 'Undo', action: () => handleUndo(), icon: <Icons.Undo className="w-4 h-4" /> },
        { label: 'Redo', action: () => handleRedo(), icon: <Icons.Redo className="w-4 h-4" /> },
      ];

      if (context === 'viewport') {
        return [
          ...baseGeneralItems,
          { separator: true },
          { label: 'Frame All', action: () => handleFrameAll(), icon: <Icons.ArrowsPointingOut className="w-4 h-4" /> },
          { label: 'Frame Selected', action: () => handleFocusSelected(), icon: <Icons.MagnifyingGlass className="w-4 h-4" /> },
          { separator: true },
          { label: 'Reset View', action: () => handleResetView(), icon: <Icons.ArrowPath className="w-4 h-4" /> },
          { label: 'Top View', action: () => handleSetView('top'), icon: <Icons.ArrowUp className="w-4 h-4" /> },
          { label: 'Front View', action: () => handleSetView('front'), icon: <Icons.ArrowRight className="w-4 h-4" /> },
          { label: 'Right View', action: () => handleSetView('right'), icon: <Icons.ArrowDown className="w-4 h-4" /> },
        ];
      } else {
        return [
          ...baseGeneralItems,
          { separator: true },
          { label: 'Select All', action: () => handleSelectAll(), icon: <Icons.CursorArrowRays className="w-4 h-4" /> },
          { label: 'Expand All', action: () => handleExpandAll(), icon: <Icons.PlusCircle className="w-4 h-4" /> },
          { label: 'Collapse All', action: () => handleCollapseAll(), icon: <Icons.MinusCircle className="w-4 h-4" /> },
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