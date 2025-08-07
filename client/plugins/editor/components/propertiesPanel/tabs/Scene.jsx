import { useState, useEffect, useRef, useMemo } from 'react';
import { Icons } from '@/plugins/editor/components/Icons.jsx';
// import CollapsibleSection from '@/plugins/editor/components/ui/CollapsibleSection.jsx';
import { globalStore, actions, babylonScene } from "@/store.js";
import { useSnapshot } from 'valtio';
import useSceneDnD from '@/plugins/editor/hooks/useSceneDnD.js';

// Improved CollapsibleSection component with chevron on left and no border conflicts
const CollapsibleSection = ({ title, children, defaultOpen = true, index = 0 }) => {
  const [isOpen, setIsOpen] = useState(defaultOpen);
  
  return (
    <div className="border-b border-gray-700/60">
      <button
        onClick={() => setIsOpen(!isOpen)}
        className={`w-full pl-2 pr-4 py-3 text-left font-semibold text-sm transition-all duration-200 flex items-center gap-2 group ${
          isOpen 
            ? 'bg-slate-700/50 text-white' 
            : 'text-gray-300 hover:bg-slate-700/30 hover:text-gray-100 active:bg-slate-700/60'
        }`}
      >
        <Icons.ChevronRight className={`w-3.5 h-3.5 transition-all duration-200 ${
          isOpen 
            ? 'rotate-90 text-blue-400' 
            : 'text-gray-400 group-hover:text-gray-300'
        }`} />
        <span className="flex-1">{title}</span>
      </button>
      {isOpen && (
        <div className="bg-slate-800/20">
          {children}
        </div>
      )}
    </div>
  );
};

function Scene({ selectedObject, onObjectSelect, onContextMenu }) {
  const ui = useSnapshot(globalStore.editor.ui);
  const { scenePropertiesHeight: bottomPanelHeight } = ui;
  const { setScenePropertiesHeight: setBottomPanelHeight } = actions.editor;
  const [isResizing, setIsResizing] = useState(false);
  
  const settings = useSnapshot(globalStore.editor.settings);
  const sceneData = useSnapshot(globalStore.editor.scene);
  const objectProperties = useSnapshot(globalStore.editor.objectProperties);
  const selection = useSnapshot(globalStore.editor.selection);
  const { setSelectedEntity, selectObject, setTransformMode, createNodeEditorTab, updateObjectProperty, updateBabylonObjectFromProperties, createFolder, renameObject, moveObjectToFolder } = actions.editor;
  
  const [droppedItemId, setDroppedItemId] = useState(null);
  const [isDragOverScript, setIsDragOverScript] = useState(false);

  const { draggedItem, dragOverItem, dropPosition, handleDragStart, handleDragOver, handleDrop, handleDragEnd } = useSceneDnD(sceneData.hierarchy);

  const handleDropWithAnimation = (e, item) => {
    handleDrop(e, item);
    setDroppedItemId(draggedItem.id);
    setTimeout(() => setDroppedItemId(null), 500);
  };

  // Get selected object data from Babylon.js scene using external reference
  const selectedObjectData = useMemo(() => {
    if (!selection.entity) {
      return null;
    }
    
    const scene = babylonScene?.current;
    if (!scene) {
      return null;
    }
    
    const babylonObjects = [];
    
    // Check meshes
    if (scene.meshes) {
      scene.meshes.forEach(mesh => {
        const meshId = mesh.uniqueId || mesh.name;
        if (meshId === selection.entity) {
          babylonObjects.push({
            id: mesh.uniqueId || mesh.name,
            name: mesh.name,
            type: 'mesh',
            position: mesh.position ? [mesh.position.x, mesh.position.y, mesh.position.z] : [0, 0, 0],
            rotation: mesh.rotation ? [mesh.rotation.x, mesh.rotation.y, mesh.rotation.z] : [0, 0, 0],
            scale: mesh.scaling ? [mesh.scaling.x, mesh.scaling.y, mesh.scaling.z] : [1, 1, 1],
            visible: mesh.isVisible !== false,
            babylonObject: mesh
          });
        }
      });
    }
    
    // Check transform nodes
    if (scene.transformNodes) {
      scene.transformNodes.forEach(node => {
        const nodeId = node.uniqueId || node.name;
        if (nodeId === selection.entity) {
          babylonObjects.push({
            id: node.uniqueId || node.name,
            name: node.name,
            type: 'mesh', // Show transform nodes as meshes in UI
            position: node.position ? [node.position.x, node.position.y, node.position.z] : [0, 0, 0],
            rotation: node.rotation ? [node.rotation.x, node.rotation.y, node.rotation.z] : [0, 0, 0],
            scale: node.scaling ? [node.scaling.x, node.scaling.y, node.scaling.z] : [1, 1, 1],
            visible: true,
            babylonObject: node
          });
        }
      });
    }
    
    // Check lights
    if (scene.lights) {
      scene.lights.forEach(light => {
        const lightId = light.uniqueId || light.name;
        if (lightId === selection.entity) {
          babylonObjects.push({
            id: light.uniqueId || light.name,
            name: light.name,
            type: 'light',
            position: light.position ? [light.position.x, light.position.y, light.position.z] : [0, 0, 0],
            rotation: light.rotation ? [light.rotation.x, light.rotation.y, light.rotation.z] : [0, 0, 0],
            lightType: light._lightType || 'directional',
            intensity: light.intensity || 1,
            color: light.diffuse ? `#${light.diffuse.toHexString()}` : '#ffffff',
            castShadow: light.shadowEnabled || false,
            visible: light.isEnabled ? light.isEnabled() : true,
            babylonObject: light
          });
        }
      });
    }
    
    // Check cameras
    if (scene.cameras) {
      scene.cameras.forEach(camera => {
        const cameraId = camera.uniqueId || camera.name;
        if (cameraId === selection.entity) {
          babylonObjects.push({
            id: camera.uniqueId || camera.name,
            name: camera.name,
            type: 'camera',
            position: camera.position ? [camera.position.x, camera.position.y, camera.position.z] : [0, 0, 0],
            rotation: camera.rotation ? [camera.rotation.x, camera.rotation.y, camera.rotation.z] : [0, 0, 0],
            cameraType: 'perspective',
            fov: camera.fov ? (camera.fov * 180 / Math.PI) : 60,
            near: camera.minZ || 0.1,
            far: camera.maxZ || 1000,
            visible: true,
            babylonObject: camera
          });
        }
      });
    }
    
    return babylonObjects.length > 0 ? babylonObjects[0] : null;
  }, [selection.entity, babylonScene]);

  // Object Properties helper functions
  const handleAssetDrop = (e, propertyPath) => {
    e.preventDefault();
    const droppedData = e.dataTransfer.getData('text/plain');
    
    try {
      const data = JSON.parse(droppedData);
      if (data.type === 'asset' && data.fileType === 'texture') {
        updateObjectProperty(selectedObject, propertyPath, data.path);
      } else if (data.type === 'asset' && data.fileType === 'script') {
        updateObjectProperty(selectedObject, propertyPath, data.path);
      }
    } catch (err) {
      console.warn('Invalid drop data:', droppedData);
    }
  };

  const handleDragOverAsset = (e) => {
    e.preventDefault();
  };

  const isNodeControlled = (propertyPath) => {
    const objectProps = objectProperties.objects[selection.entity];
    return objectProps?.nodeBindings && objectProps.nodeBindings[propertyPath];
  };

  const renderVector3Input = (label, value, propertyPath) => (
    <div className="mb-3">
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <div className="grid grid-cols-3 gap-1">
        {['X', 'Y', 'Z'].map((axis, index) => (
          <div key={axis} className="relative">
            <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-gray-300 pointer-events-none font-medium bg-gray-700 border-t border-l border-b border-r border-gray-600 rounded-l">
              {axis}
            </span>
            <input
              type="number"
              step="0.1"
              value={value[index] || 0}
              onChange={(e) => {
                const newValue = [...value];
                newValue[index] = parseFloat(e.target.value) || 0;
                updateObjectProperty(selection.entity, propertyPath, newValue);
                actions.editor.updateBabylonObjectFromProperties(selection.entity);
              }}
              className={`w-full text-xs p-1.5 pl-7 pr-1.5 rounded text-center focus:outline-none focus:ring-1 focus:ring-blue-500 ${
                isNodeControlled(`${propertyPath}.${index}`) 
                  ? 'border-blue-500 bg-blue-900/20 text-blue-200' 
                  : 'border-gray-600 bg-gray-800 text-white'
              } border`}
              disabled={isNodeControlled(`${propertyPath}.${index}`)}
            />
          </div>
        ))}
      </div>
      {isNodeControlled(propertyPath) && (
        <div className="text-xs text-blue-400 mt-1">Controlled by node</div>
      )}
    </div>
  );

  const renderColorInput = (label, value, propertyPath) => (
    <div className="mb-3">
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <div className="flex items-center gap-1">
        <input
          type="color"
          value={value || '#ffffff'}
          onChange={(e) => {
            updateObjectProperty(selection.entity, propertyPath, e.target.value);
            actions.editor.updateBabylonObjectFromProperties(selection.entity);
          }}
          className="w-6 h-6 rounded border border-gray-600 bg-gray-800 cursor-pointer"
          disabled={isNodeControlled(propertyPath)}
        />
        <div className={`flex-1 rounded px-1.5 py-1 border ${
          isNodeControlled(propertyPath) 
            ? 'border-blue-500 bg-blue-900/20' 
            : 'border-gray-600 bg-gray-800'
        }`}>
          <div className={`text-xs ${isNodeControlled(propertyPath) ? 'text-blue-200' : 'text-gray-300'}`}>
            {(value || '#ffffff').toUpperCase()}
          </div>
        </div>
      </div>
      {isNodeControlled(propertyPath) && (
        <div className="text-xs text-blue-400 mt-1">Controlled by node</div>
      )}
    </div>
  );

  const renderSliderInput = (label, value, propertyPath, min = 0, max = 1, step = 0.01) => (
    <div className="mb-3">
      <label className="block text-xs text-gray-400 mb-1">
        {label} <span className="text-gray-500">({(value || 0).toFixed(2)})</span>
      </label>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value || 0}
        onChange={(e) => {
          updateObjectProperty(selection.entity, propertyPath, parseFloat(e.target.value));
          actions.editor.updateBabylonObjectFromProperties(selection.entity);
        }}
        className={`w-full h-1.5 bg-gray-600 rounded-lg appearance-none cursor-pointer ${
          isNodeControlled(propertyPath) ? 'opacity-50' : ''
        }`}
        disabled={isNodeControlled(propertyPath)}
      />
      {isNodeControlled(propertyPath) && (
        <div className="text-xs text-blue-400 mt-1">Controlled by node</div>
      )}
    </div>
  );

  const renderTextureSlot = (label, value, propertyPath) => (
    <div className="mb-3">
      <label className="block text-sm font-medium text-gray-300 mb-1">{label}</label>
      <div
        className="border-2 border-dashed border-gray-600 rounded-lg p-4 text-center hover:border-gray-500 transition-colors"
        onDrop={(e) => handleAssetDrop(e, propertyPath)}
        onDragOver={handleDragOverAsset}
      >
        {value ? (
          <div className="flex items-center justify-between">
            <span className="text-sm text-gray-300 truncate">{value.split('/').pop()}</span>
            <button
              onClick={() => updateObjectProperty(selection.entity, propertyPath, null)}
              className="text-red-400 hover:text-red-300 ml-2"
            >
              ×
            </button>
          </div>
        ) : (
          <div className="text-gray-500 text-sm">
            Drop texture here or click to browse
          </div>
        )}
      </div>
    </div>
  );

  // Scene hierarchy data
  const hierarchyData = useMemo(() => {
    return sceneData.hierarchy;
  }, [sceneData.hierarchy]);

  // Container ref for resize functionality
  const containerRef = useRef(null);
  
  // State for expanded items
  const [expandedItems, setExpandedItems] = useState({});
  
  // Rename state
  const [renamingItemId, setRenamingItemId] = useState(null);
  const [renameValue, setRenameValue] = useState('');
  
  // Folder creation counter
  const [folderCounter, setFolderCounter] = useState(1);

  // Listen for context menu events
  useEffect(() => {
    const handleContextMenuRename = (event) => {
      const { itemId } = event.detail;
      const item = sceneData.nodes.find(node => node.id === itemId);
      if (item) {
        startRename(itemId, item.name);
      }
    };

    const handleContextMenuAddToNewFolder = (event) => {
      const { itemId } = event.detail;
      const previousSelection = selection.entity;
      selectObject(itemId);
      
      setTimeout(() => {
        handleAddToNewFolder();
        if (previousSelection !== itemId) {
          selectObject(previousSelection);
        }
      }, 10);
    };

    document.addEventListener('contextMenuRename', handleContextMenuRename);
    document.addEventListener('contextMenuAddToNewFolder', handleContextMenuAddToNewFolder);

    return () => {
      document.removeEventListener('contextMenuRename', handleContextMenuRename);
      document.removeEventListener('contextMenuAddToNewFolder', handleContextMenuAddToNewFolder);
    };
  }, [sceneData.nodes, selection.entity, selectObject]);
  
  // Expand/collapse functions
  const expandAll = () => {
    const expandAllNodes = (nodes) => {
      const newExpanded = {};
      nodes.forEach(node => {
        if (node.children && node.children.length > 0) {
          newExpanded[node.id] = true;
          Object.assign(newExpanded, expandAllNodes(node.children));
        }
      });
      return newExpanded;
    };
    
    const allExpanded = expandAllNodes(hierarchyData);
    setExpandedItems(allExpanded);
  };
  
  const collapseAll = () => {
    setExpandedItems({});
  };
  
  // Rename functions
  const startRename = (itemId, currentName) => {
    setRenamingItemId(itemId);
    setRenameValue(currentName);
  };
  
  const confirmRename = () => {
    if (renamingItemId && renameValue.trim()) {
      renameObject(renamingItemId, renameValue.trim());
      setRenamingItemId(null);
      setRenameValue('');
    }
  };
  
  const cancelRename = () => {
    setRenamingItemId(null);
    setRenameValue('');
  };
  
  // Folder creation
  const handleCreateFolder = () => {
    const folderName = `New Folder ${folderCounter}`;
    const parentId = selection.entity && selection.entity !== 'scene-root' ? selection.entity : null;
    
    const folderId = createFolder(folderName, parentId);
    setFolderCounter(prev => prev + 1);
    
    selectObject(folderId);
    setTimeout(() => startRename(folderId, folderName), 100);
  };
  
  // Add selected object to new folder
  const handleAddToNewFolder = () => {
    if (!selection.entity || selection.entity === 'scene-root') return;
    
    const folderName = `New Folder ${folderCounter}`;
    const folderId = createFolder(folderName, null);
    setFolderCounter(prev => prev + 1);
    
    setTimeout(() => {
      const success = moveObjectToFolder(selection.entity, folderId);
      if (success) {
        selectObject(folderId);
        setTimeout(() => startRename(folderId, folderName), 100);
      }
    }, 50);
  };
  
  // Keyboard event handler
  const handleKeyDown = (e, item) => {
    if (e.key === 'F2' && item && !renamingItemId) {
      e.preventDefault();
      startRename(item.id, item.name);
    } else if (e.key === 'Escape' && renamingItemId) {
      e.preventDefault();
      cancelRename();
    } else if (e.key === 'Enter' && renamingItemId) {
      e.preventDefault();
      confirmRename();
    }
  };

  // Resizing logic
  const handleMouseDown = (e) => {
    e.preventDefault();
    setIsResizing(true);
    
    const startY = e.clientY;
    const startHeight = bottomPanelHeight;
    
    const handleMouseMove = (e) => {
      const deltaY = startY - e.clientY;
      const newHeight = Math.max(200, Math.min(600, startHeight + deltaY));
      setBottomPanelHeight(newHeight);
    };
    
    const handleMouseUp = () => {
      setIsResizing(false);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
    
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  // Handle object deletion
  const handleDeleteObject = (objectId, e) => {
    e.stopPropagation();
    
    const scene = babylonScene?.current;
    if (!scene) return;
    
    const allObjects = [...scene.meshes, ...scene.lights, ...scene.cameras];
    const objectToDelete = allObjects.find(obj => 
      (obj.uniqueId || obj.name) === objectId && obj.name !== 'ground' && obj.name !== 'skybox'
    );
    
    if (objectToDelete) {
      objectToDelete.dispose();
      
      if (selection.entity === objectId) {
        setSelectedEntity(null);
      }
    }
    
    actions.editor.refreshSceneData();
  };

  // Get icon for object type
  const getIcon = (type, lightType) => {
    switch (type) {
      case 'mesh': return Icons.Cube3D;
      case 'light': 
        switch (lightType) {
          case 'directional': return Icons.LightDirectional;
          case 'point': return Icons.LightPoint;
          case 'spot': return Icons.LightSpot;
          default: return Icons.LightDirectional;
        }
      case 'camera': return Icons.Video;
      case 'folder': return Icons.Folder;
      default: return Icons.Scene;
    }
  };

  // Render scene hierarchy item
  const renderSceneItem = (item, depth = 0, index = 0, parent = null) => {
    if (!item) return null;
    
    const isSelected = selection.entity === item.id;
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = expandedItems.hasOwnProperty(item.id) ? expandedItems[item.id] : (item.expanded || false);
    const Icon = getIcon(item.type, item.lightType);
    
    const isDraggedOver = dragOverItem?.id === item.id;
    const isFolderDrop = isDraggedOver && dropPosition === 'inside' && item.type === 'folder';

    const showTopDivider = isDraggedOver && dropPosition === 'above';
    const showBottomDivider = isDraggedOver && dropPosition === 'below';

    return (
      <div key={item.id} className="select-none relative">
        {showTopDivider && (
          <div className="absolute top-0 left-0 right-0 h-0.5 bg-blue-500 z-10 pointer-events-none" />
        )}
        <div 
          className={`group flex items-center transition-all duration-200 text-xs relative overflow-hidden rounded ${
            isSelected 
              ? 'bg-blue-600/25 text-white shadow-sm' 
              : 'hover:bg-slate-700/40 text-gray-300 hover:text-gray-100 active:bg-slate-700/60'
          } ${
            draggedItem?.id === item.id ? 'opacity-30' : ''
          } ${
            isFolderDrop ? 'border-2 border-blue-500' : ''
          } ${
            droppedItemId === item.id ? 'bg-green-500/50' : ''
          }`}
          style={{ 
            paddingLeft: `${8 + depth * 20}px`,
            paddingRight: '8px',
            paddingTop: '2px',
            paddingBottom: '2px',
            cursor: 'grab'
          }}
          draggable="true"
          onDragStart={(e) => handleDragStart(e, item)}
          onDragOver={(e) => handleDragOver(e, item)}
          onDrop={(e) => handleDropWithAnimation(e, item)}
          onDragEnd={handleDragEnd}
          tabIndex={0}
          onKeyDown={(e) => handleKeyDown(e, item)}
          onClick={() => {
            if (renamingItemId !== item.id) {
              selectObject(item.id);
            }
          }}
          onDoubleClick={() => {
            if (!renamingItemId) {
              startRename(item.id, item.name);
            }
          }}
          onContextMenu={(e) => {
            onContextMenu(e, item, 'scene');
          }}
        >
          {/* Selection indicator */}
          {isSelected && (
            <div className="absolute left-0 top-0 bottom-0 w-0.5 bg-blue-400 pointer-events-none" />
          )}
          
          {/* Hierarchy lines */}
          {depth > 0 && (
            <div className="absolute left-0 top-0 bottom-0 pointer-events-none">
              <div
                className="absolute top-0 bottom-0 w-px bg-slate-600/40"
                style={{ left: `${8 + (depth - 1) * 20 + 10}px` }}
              />
              <div
                className="absolute top-1/2 w-3 h-px bg-slate-600/40"
                style={{ left: `${8 + (depth - 1) * 20 + 10}px` }}
              />
            </div>
          )}
          
          {hasChildren && (
            <button 
              className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-slate-600/50"
              onClick={(e) => {
                e.stopPropagation();
                setExpandedItems(prev => ({ ...prev, [item.id]: !isExpanded }));
              }}
            >
              <Icons.ChevronRight className={`w-3 h-3 transition-all duration-200 ${
                isExpanded 
                  ? 'rotate-90 text-blue-400' 
                  : 'text-gray-500 group-hover:text-gray-300'
              }`} />
            </button>
          )}
          
          <button 
            className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-slate-600/50"
            onClick={(e) => {
              e.stopPropagation();
              actions.editor.updateSceneObjectProperty(item.id, 'visible', !item.visible);
            }}
          >
            {item.visible ? (
              <Icons.Eye className="w-4 h-4 text-gray-400 hover:text-gray-200" />
            ) : (
              <Icons.EyeSlash className="w-4 h-4 text-gray-600 hover:text-gray-400" />
            )}
          </button>
          
          <button 
            className="mr-2 p-0.5 rounded transition-colors opacity-70 hover:opacity-100 hover:bg-slate-600"
            onClick={(e) => {
              e.stopPropagation();
              createNodeEditorTab(item.id, item.name);
            }}
            title="Open Node Editor"
          >
            <Icon className="w-4 h-4 text-gray-400" />
          </button>
          {renamingItemId === item.id ? (
            <input
              type="text"
              value={renameValue}
              onChange={(e) => setRenameValue(e.target.value)}
              onBlur={confirmRename}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === 'Enter') {
                  e.preventDefault();
                  confirmRename();
                } else if (e.key === 'Escape') {
                  e.preventDefault();
                  cancelRename();
                }
              }}
              className="flex-1 bg-slate-700 text-gray-200 px-1 rounded text-xs border border-blue-400 focus:outline-none focus:border-blue-300"
              autoFocus
              onFocus={(e) => e.target.select()}
            />
          ) : (
            <span className="flex-1 text-gray-200 truncate">{item.name}</span>
          )}
          
          <button 
            className="ml-2 p-0.5 rounded transition-colors opacity-0 group-hover:opacity-70 hover:opacity-100"
            onClick={(e) => handleDeleteObject(item.id, e)}
            title="Delete object"
          >
            <Icons.Trash className="w-4 h-4 text-gray-300 hover:text-red-400" />
          </button>
        </div>
        
        {hasChildren && isExpanded && (
          <div className="transition-all duration-300 ease-out">
            {item.children.map((child, i) => 
              renderSceneItem(child, depth + 1, i, item)
            )}
          </div>
        )}
        {showBottomDivider && (
          <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500 z-10 pointer-events-none" />
        )}
      </div>
    );
  };

  return (
    <div 
      ref={containerRef} 
      className="flex flex-col flex-1 overflow-hidden"
      onContextMenu={(e) => onContextMenu(e, null)}
    >
      
      <div
        className="flex-1 overflow-y-auto scrollbar-thin"
        onContextMenu={(e) => onContextMenu(e, null)}
      >
        {hierarchyData.map((item, i) => renderSceneItem(item, 0, i, hierarchyData))}
      </div>
      
      {/* Photoshop-style bottom toolbar */}
      <div className="flex items-center justify-between px-2 py-1 border-t border-slate-700/60 bg-gradient-to-b from-slate-800/50 to-slate-900/80">
        <div className="flex items-center gap-1">
          <button
            onClick={handleCreateFolder}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95"
            title="Create Folder"
          >
            <Icons.Folder className="w-4 h-4" />
          </button>
          
          <div className="w-px h-4 bg-slate-600/60 mx-1" />
          
          <button
            onClick={() => {
              if (selection.entity && selection.entity !== 'scene-root') {
                const findItemName = (nodes, targetId) => {
                  for (let node of nodes) {
                    if (node.id === targetId) return node.name;
                    if (node.children) {
                      const childName = findItemName(node.children, targetId);
                      if (childName) return childName;
                    }
                  }
                  return targetId;
                };
                const itemName = findItemName(hierarchyData, selection.entity);
                startRename(selection.entity, itemName);
              }
            }}
            disabled={!selection.entity || selection.entity === 'scene-root'}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95 disabled:opacity-30 disabled:cursor-not-allowed"
            title="Rename Selected (F2)"
          >
            <Icons.Pencil className="w-4 h-4" />
          </button>
          
          <button
            onClick={() => {
              if (selection.entity && selection.entity !== 'scene-root') {
                handleDeleteObject(selection.entity, { stopPropagation: () => {} });
              }
            }}
            disabled={!selection.entity || selection.entity === 'scene-root'}
            className="p-1.5 rounded hover:bg-red-600/20 text-gray-400 hover:text-red-400 transition-all duration-150 active:bg-red-600/30 active:scale-95 disabled:opacity-30 disabled:cursor-not-allowed"
            title="Delete Selected"
          >
            <Icons.Trash className="w-4 h-4" />
          </button>
        </div>
        
        <div className="flex items-center gap-1">
          <button
            onClick={expandAll}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95"
            title="Expand All"
          >
            <Icons.ChevronRight className="w-4 h-4 rotate-90" />
          </button>
          
          <button
            onClick={collapseAll}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95"
            title="Collapse All"
          >
            <Icons.ChevronRight className="w-4 h-4" />
          </button>
          
          <div className="w-px h-4 bg-slate-600/60 mx-1" />
          
          <button
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95"
            title="Refresh Scene"
            onClick={() => actions.editor.refreshSceneData()}
          >
            <Icons.RotateCcw className="w-4 h-4" />
          </button>
        </div>
      </div>
      
      {selection.entity && (
        <>
          <div
            className={`h-1 cursor-row-resize transition-colors ${isResizing ? 'bg-blue-500/75' : 'bg-slate-700/50 hover:bg-blue-500/75'}`}
            onMouseDown={handleMouseDown}
          />
          <div className="overflow-y-auto scrollbar-thin" style={{ height: `${bottomPanelHeight}px` }}>
            {(() => {
              const objectProps = objectProperties.objects[selection.entity];
              
              if (!objectProps && selectedObjectData) {
                return (
                  <div className="p-4">
                    <div className="text-gray-500 text-sm mb-4">
                      Open the node editor to add advanced properties and components.
                    </div>
                    <div className="p-3 bg-gray-800 rounded">
                      <h4 className="text-gray-300 font-medium mb-2">Basic Info</h4>
                      <p className="text-sm text-gray-400">Name: {selectedObjectData.name}</p>
                      <p className="text-sm text-gray-400">Type: {selectedObjectData.type}</p>
                      {selectedObjectData.position && (
                        <p className="text-sm text-gray-400">Position: [{selectedObjectData.position.map(v => v.toFixed(2)).join(', ')}]</p>
                      )}
                    </div>
                  </div>
                );
              }
              
              if (!objectProps) {
                return (
                  <div className="p-4 text-gray-500 text-sm">
                    No object selected.
                  </div>
                );
              }
              
              return (
                <div className="space-y-0">
                  {objectProps.transform && (
                    <CollapsibleSection title="Transform" defaultOpen={true} index={0}>
                      <div className="p-4">
                        {objectProps.transform.position && renderVector3Input('Position', objectProps.transform.position, 'transform.position')}
                        {objectProps.transform.rotation && renderVector3Input('Rotation', objectProps.transform.rotation, 'transform.rotation')}
                        {objectProps.transform.scale && renderVector3Input('Scale', objectProps.transform.scale, 'transform.scale')}
                      </div>
                    </CollapsibleSection>
                  )}

                  {objectProps.material && (
                    <CollapsibleSection title="Material" defaultOpen={true} index={1}>
                      <div className="p-4">
                        {objectProps.material.baseColor !== undefined && renderColorInput('Base Color', objectProps.material.baseColor, 'material.baseColor')}
                        {objectProps.material.roughness !== undefined && renderSliderInput('Roughness', objectProps.material.roughness, 'material.roughness')}
                        {objectProps.material.metallic !== undefined && renderSliderInput('Metallic', objectProps.material.metallic, 'material.metallic')}
                        {objectProps.material.alpha !== undefined && renderSliderInput('Alpha', objectProps.material.alpha, 'material.alpha')}
                      </div>
                    </CollapsibleSection>
                  )}

                  {objectProps.components && (
                    <CollapsibleSection title="Scripting" defaultOpen={true} index={2}>
                      <div className="p-4">
                        {/* Scripting Component */}
                        {objectProps.components.scripting && (
                          <div className="mb-4">
                            <div
                              className={`border-2 border-dashed rounded-lg p-3 text-center transition-colors ${
                                isDragOverScript 
                                  ? 'border-blue-500 bg-blue-500/10' 
                                  : 'border-gray-600 hover:border-gray-500'
                              }`}
                              onDrop={(e) => {
                                e.preventDefault();
                                setIsDragOverScript(false);
                                
                                console.log('Drop event triggered. Available types:', Array.from(e.dataTransfer.types));
                                
                                // Handle asset drops from asset library (JSON data)
                                const jsonData = e.dataTransfer.getData('application/json');
                                console.log('JSON data retrieved:', jsonData);
                                if (jsonData) {
                                  try {
                                    const data = JSON.parse(jsonData);
                                    if (data.type === 'asset' && data.path && data.path.match(/\.(js|ts|jsx|tsx)$/i)) {
                                      console.log('Dropping script asset:', data);
                                      // Auto-enable scripting and add script to array
                                      const currentScripts = objectProps.components.scripting.scriptFiles || [];
                                      const newScript = {
                                        id: Date.now() + Math.random(),
                                        path: data.path,
                                        name: data.path.split('/').pop(),
                                        enabled: true
                                      };
                                      
                                      // Check if script is already added
                                      if (!currentScripts.some(script => script.path === data.path)) {
                                        updateObjectProperty(selection.entity, 'components.scripting.enabled', true);
                                        updateObjectProperty(selection.entity, 'components.scripting.scriptFiles', [...currentScripts, newScript]);
                                        actions.editor.updateBabylonObjectFromProperties(selection.entity);
                                      }
                                      return;
                                    }
                                  } catch (err) {
                                    console.warn('Failed to parse JSON drag data:', err);
                                  }
                                }
                                
                                // Fallback: Handle asset drops from asset library (text/plain)
                                const textData = e.dataTransfer.getData('text/plain');
                                if (textData) {
                                  try {
                                    const data = JSON.parse(textData);
                                    if (data.type === 'asset' && data.path && data.path.match(/\.(js|ts|jsx|tsx)$/i)) {
                                      console.log('Dropping script asset (text fallback):', data);
                                      // Auto-enable scripting and add script to array
                                      const currentScripts = objectProps.components.scripting.scriptFiles || [];
                                      const newScript = {
                                        id: Date.now() + Math.random(),
                                        path: data.path,
                                        name: data.path.split('/').pop(),
                                        enabled: true
                                      };
                                      
                                      // Check if script is already added
                                      if (!currentScripts.some(script => script.path === data.path)) {
                                        updateObjectProperty(selection.entity, 'components.scripting.enabled', true);
                                        updateObjectProperty(selection.entity, 'components.scripting.scriptFiles', [...currentScripts, newScript]);
                                        actions.editor.updateBabylonObjectFromProperties(selection.entity);
                                      }
                                      return;
                                    }
                                  } catch (err) {
                                    // Not JSON data, try file drops
                                  }
                                }
                                
                                // Handle file drops from system
                                const files = e.dataTransfer.files;
                                if (files.length > 0) {
                                  const file = files[0];
                                  if (file.name.match(/\.(js|ts|jsx|tsx)$/)) {
                                    // Auto-enable scripting and add script to array
                                    const currentScripts = objectProps.components.scripting.scriptFiles || [];
                                    const newScript = {
                                      id: Date.now() + Math.random(),
                                      path: file.path || file.name,
                                      name: file.name,
                                      enabled: true
                                    };
                                    
                                    // Check if script is already added
                                    if (!currentScripts.some(script => script.path === (file.path || file.name))) {
                                      updateObjectProperty(selection.entity, 'components.scripting.enabled', true);
                                      updateObjectProperty(selection.entity, 'components.scripting.scriptFiles', [...currentScripts, newScript]);
                                      actions.editor.updateBabylonObjectFromProperties(selection.entity);
                                    }
                                  }
                                }
                              }}
                              onDragOver={(e) => {
                                e.preventDefault();
                                setIsDragOverScript(true);
                                
                                // Debug: Check what data types are available
                                console.log('Drag over script zone. Available types:', Array.from(e.dataTransfer.types));
                              }}
                              onDragLeave={(e) => {
                                e.preventDefault();
                                setIsDragOverScript(false);
                              }}
                            >
                              <div className="text-gray-500 text-xs">
                                Drop JavaScript files here (.js, .ts, .jsx, .tsx)
                              </div>
                            </div>
                            
                            {/* Scripts List */}
                            {objectProps.components.scripting.scriptFiles && objectProps.components.scripting.scriptFiles.length > 0 && (
                              <div className="mt-3 space-y-2">
                                {objectProps.components.scripting.scriptFiles.map((script, index) => (
                                  <div
                                    key={script.id || index}
                                    className="flex items-center gap-2 p-2 bg-slate-800/60 rounded-lg border border-slate-700/60 hover:border-slate-600 transition-colors group"
                                  >
                                    {/* Script Icon */}
                                    <div className="flex-shrink-0">
                                      <Icons.Code className="w-4 h-4 text-blue-400" />
                                    </div>
                                    
                                    {/* Script Name */}
                                    <div className="flex-1 min-w-0">
                                      <div className="text-xs text-gray-200 truncate font-medium">
                                        {script.name}
                                      </div>
                                      <div className="text-xs text-gray-500 truncate">
                                        {script.path}
                                      </div>
                                    </div>
                                    
                                    {/* Enable/Disable Toggle */}
                                    <button
                                      onClick={() => {
                                        const updatedScripts = objectProps.components.scripting.scriptFiles.map((s, i) =>
                                          i === index ? { ...s, enabled: !s.enabled } : s
                                        );
                                        updateObjectProperty(selection.entity, 'components.scripting.scriptFiles', updatedScripts);
                                        actions.editor.updateBabylonObjectFromProperties(selection.entity);
                                      }}
                                      className={`flex-shrink-0 w-6 h-6 rounded flex items-center justify-center transition-colors ${
                                        script.enabled
                                          ? 'text-green-400 hover:text-green-300 hover:bg-green-400/10'
                                          : 'text-gray-500 hover:text-gray-400 hover:bg-gray-400/10'
                                      }`}
                                      title={script.enabled ? 'Disable script' : 'Enable script'}
                                    >
                                      {script.enabled ? (
                                        <Icons.Eye className="w-4 h-4" />
                                      ) : (
                                        <Icons.EyeSlash className="w-4 h-4" />
                                      )}
                                    </button>
                                    
                                    {/* Remove Script Button */}
                                    <button
                                      onClick={() => {
                                        const updatedScripts = objectProps.components.scripting.scriptFiles.filter((_, i) => i !== index);
                                        updateObjectProperty(selection.entity, 'components.scripting.scriptFiles', updatedScripts);
                                        
                                        // Disable scripting if no scripts left
                                        if (updatedScripts.length === 0) {
                                          updateObjectProperty(selection.entity, 'components.scripting.enabled', false);
                                        }
                                        
                                        actions.editor.updateBabylonObjectFromProperties(selection.entity);
                                      }}
                                      className="flex-shrink-0 w-6 h-6 rounded flex items-center justify-center text-gray-500 hover:text-red-400 hover:bg-red-400/10 transition-colors opacity-0 group-hover:opacity-100"
                                      title="Remove script"
                                    >
                                      <Icons.Trash className="w-4 h-4" />
                                    </button>
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    </CollapsibleSection>
                  )}

                  {!objectProps.transform && !objectProps.material && !objectProps.components && (
                    <div className="p-4 text-center">
                      <div className="text-gray-500 text-sm mb-2">
                        No properties configured
                      </div>
                      <div className="text-gray-600 text-xs">
                        Open the node editor and connect nodes to output nodes to create property sections
                      </div>
                    </div>
                  )}
                </div>
              );
            })()}
          </div>
        </>
      )}
    </div>
  );
}

export default Scene;