import { createSignal, createMemo, onCleanup, onMount, createEffect, For, Show, Switch, Match } from 'solid-js';
import { ChevronRight, Box, Lightbulb, Camera, Folder, Circle, Eye, EyeOff, Trash, Edit, Code, X } from '@/ui/icons';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
const getBabylonScene = () => window._cleanBabylonScene;
import { viewportActions, objectPropertiesActions, objectPropertiesStore } from '@/layout/stores/ViewportStore';
import { CollapsibleSection } from '@/ui';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { getScriptRuntime } from '@/api/script';

const buildHierarchyFromBabylon = (babylonObject, depth = 0) => {
  if (!babylonObject) return null;
  
  const objectId = babylonObject.uniqueId || babylonObject.name || `${babylonObject.getClassName()}-${Math.random()}`;
  
  let type = 'mesh';
  let lightType = null;
  
  const className = babylonObject.getClassName();
  if (className.includes('Light')) {
    type = 'light';
    lightType = className.toLowerCase().replace('light', '');
  } else if (className.includes('Camera')) {
    type = 'camera';
  } else if (className === 'TransformNode') {
    type = 'folder';
  }
  
  const children = [];
  if (babylonObject.getChildren) {
    const babylonChildren = babylonObject.getChildren();
    babylonChildren.forEach(child => {
      if (child.name && !child.name.startsWith('__') && !child.name.includes('gizmo')) {
        children.push(buildHierarchyFromBabylon(child, depth + 1));
      }
    });
  }
  
  return {
    id: objectId,
    name: babylonObject.name || `Unnamed ${className}`,
    type: type,
    lightType: lightType,
    visible: babylonObject.isVisible !== undefined ? babylonObject.isVisible : 
             (babylonObject.isEnabled ? babylonObject.isEnabled() : true),
    children: children.length > 0 ? children : undefined,
    expanded: depth < 2,
    babylonObject: babylonObject
  };
};

const getSceneHierarchy = () => {
  const scene = getBabylonScene();
  if (!scene) return [];
  
  const allObjects = [
    ...(scene.meshes || []),
    ...(scene.transformNodes || []),
    ...(scene.lights || []),
    ...(scene.cameras || [])
  ];
  
  const rootObjects = allObjects.filter(obj => {
    const isSystemObject = obj.name && (
      obj.name.startsWith('__') ||
      obj.name.includes('gizmo') ||
      obj.name.includes('helper') ||
      obj.name.includes('_internal_')
    );
    
    return !isSystemObject && !obj.parent;
  });
  
  const hierarchyItems = rootObjects.map(obj => buildHierarchyFromBabylon(obj));
  
  return [{
    id: 'scene-root',
    name: 'Clean Scene',
    type: 'scene',
    expanded: true,
    children: hierarchyItems
  }];
};

function Scene(props) {
  const { ui, selection } = editorStore;
  const bottomPanelHeight = () => ui.scenePropertiesHeight;
  const { setScenePropertiesHeight: setBottomPanelHeight } = editorActions;
  const [isResizing, setIsResizing] = createSignal(false);
  const { selectEntity: setSelectedEntity, setTransformMode } = editorActions;
  const { createNodeTab: createNodeEditorTab } = viewportActions;
  const { updateObjectProperty } = objectPropertiesActions;
  
  // Handle window resize to adjust properties panel height
  onMount(() => {
    const handleWindowResize = () => {
      const currentHeight = ui.scenePropertiesHeight;
      const maxHeight = Math.floor(window.innerHeight * 0.7);
      const minHeight = 300;
      if (currentHeight > maxHeight) {
        setBottomPanelHeight(maxHeight);
      } else if (currentHeight < minHeight) {
        setBottomPanelHeight(minHeight);
      }
    };
    
    window.addEventListener('resize', handleWindowResize);
    onCleanup(() => window.removeEventListener('resize', handleWindowResize));
  });
  
  const [droppedItemId, setDroppedItemId] = createSignal(null);
  const [isDragOverScript, setIsDragOverScript] = createSignal(false);
  const [expandedItems, setExpandedItems] = createSignal({});
  const [renamingItemId, setRenamingItemId] = createSignal(null);
  const [renameValue, setRenameValue] = createSignal('');
  const [folderCounter, setFolderCounter] = createSignal(1);
  const [sceneUpdateTrigger, setSceneUpdateTrigger] = createSignal(0);

  const hierarchyData = createMemo(() => {
    const trigger = sceneUpdateTrigger();
    
    if (!getBabylonScene()) return [];
    
    console.log('🌳 Scene Tree: Rebuilding hierarchy, trigger:', trigger);
    
    return getSceneHierarchy();
  });

  createEffect(() => {
    const handleSceneChange = (event) => {
      console.log('🌳 Scene Tree: Received scene change event:', event.detail);
      setSceneUpdateTrigger(prev => prev + 1);
    };
    
    window.addEventListener('babylonSceneChanged', handleSceneChange);
    
    const interval = setInterval(() => {
      const scene = getBabylonScene();
      if (scene) {
        const currentMeshCount = scene.meshes?.length || 0;
        if (currentMeshCount !== (window._lastMeshCount || 0)) {
          window._lastMeshCount = currentMeshCount;
          setSceneUpdateTrigger(prev => prev + 1);
          console.log('🌳 Scene Tree: Mesh count changed via polling, triggering update:', currentMeshCount);
        }
      }
    }, 2000);
    
    onCleanup(() => {
      window.removeEventListener('babylonSceneChanged', handleSceneChange);
      clearInterval(interval);
    });
  });

  // Inline drag and drop state and handlers
  const [draggedItem, setDraggedItem] = createSignal(null);
  const [dragOverItem, setDragOverItem] = createSignal(null);
  const [dropPosition, setDropPosition] = createSignal(null);

  const handleDragStart = (e, item) => {
    setDraggedItem(item);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', JSON.stringify({ type: 'scene-item', item }));
  };

  const handleDragOver = (e, item) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';

    const rect = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const height = rect.height;
    
    let position = 'inside';
    if (y < height * 0.25) {
      position = 'above';
    } else if (y > height * 0.75) {
      position = 'below';
    }

    setDragOverItem(item);
    setDropPosition(position);
  };

  const handleDrop = (e, targetItem) => {
    e.preventDefault();
    const draggedData = draggedItem();
    
    if (!draggedData || draggedData.id === targetItem.id) {
      return;
    }

    const scene = getBabylonScene();
    if (!scene) return;

    // Find the dragged object in Babylon scene
    const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
    const draggedBabylonObject = allObjects.find(obj => 
      (obj.uniqueId || obj.name) === draggedData.id
    );
    const targetBabylonObject = allObjects.find(obj => 
      (obj.uniqueId || obj.name) === targetItem.id
    );

    if (!draggedBabylonObject) return;

    const position = dropPosition();
    
    if (position === 'inside' && targetItem.type === 'folder') {
      // Parent to target folder
      draggedBabylonObject.parent = targetBabylonObject;
    } else if (position === 'above' || position === 'below') {
      // Parent to same parent as target
      draggedBabylonObject.parent = targetBabylonObject?.parent || null;
    }

    // Trigger scene update
    setSceneUpdateTrigger(prev => prev + 1);
  };

  const handleDragEnd = (e) => {
    setDraggedItem(null);
    setDragOverItem(null);
    setDropPosition(null);
  };

  let containerRef;

  const handleDropWithAnimation = (e, item) => {
    handleDrop(e, item);
    const draggedData = draggedItem();
    if (draggedData) {
      setDroppedItemId(draggedData.id);
      setTimeout(() => setDroppedItemId(null), 500);
    }
  };

  const selectedObjectData = createMemo(() => {
    if (!selection.entity) {
      return null;
    }
    
    const scene = getBabylonScene();
    if (!scene) {
      return null;
    }
    
    const babylonObjects = [];
    
    // Helper function to create object data
    const createObjectData = (obj, type) => {
      return {
        id: obj.uniqueId || obj.name,
        name: obj.name,
        type: type,
        position: obj.position ? [obj.position.x, obj.position.y, obj.position.z] : [0, 0, 0],
        rotation: obj.rotation ? [obj.rotation.x, obj.rotation.y, obj.rotation.z] : [0, 0, 0],
        scale: obj.scaling ? [obj.scaling.x, obj.scaling.y, obj.scaling.z] : [1, 1, 1],
        visible: obj.isVisible !== undefined ? obj.isVisible : (obj.isEnabled ? obj.isEnabled() : true),
        babylonObject: obj
      };
    };
    
    // Check all object types
    const allObjects = [
      ...(scene.meshes || []).map(obj => ({ obj, type: 'mesh' })),
      ...(scene.transformNodes || []).map(obj => ({ obj, type: 'transform' })),
      ...(scene.cameras || []).map(obj => ({ obj, type: 'camera' })),
      ...(scene.lights || []).map(obj => ({ obj, type: 'light' }))
    ];
    
    for (const { obj, type } of allObjects) {
      const objId = obj.uniqueId || obj.name;
      if (objId === selection.entity) {
        babylonObjects.push(createObjectData(obj, type));
        break;
      }
    }
    
    return babylonObjects.length > 0 ? babylonObjects[0] : null;
  });

  const handleAssetDrop = (e, propertyPath) => {
    e.preventDefault();
    const droppedData = e.dataTransfer.getData('text/plain');
    
    try {
      const data = JSON.parse(droppedData);
      if (data.type === 'asset' && data.fileType === 'texture') {
        updateObjectProperty(props.selectedObject, propertyPath, data.path);
      } else if (data.type === 'asset' && data.fileType === 'script') {
        updateObjectProperty(props.selectedObject, propertyPath, data.path);
      }
    } catch (err) {
      console.warn('Invalid drop data:', droppedData);
    }
  };

  const handleDragOverAsset = (e) => {
    e.preventDefault();
  };

  const isNodeControlled = (propertyPath) => {
    const objectProps = objectPropertiesStore.objects[selection.entity];
    return objectProps?.nodeBindings && objectProps.nodeBindings[propertyPath];
  };

  const renderVector3Input = (label, value, propertyPath) => (
    <div className="mb-3">
      <label className="block text-xs text-base-content/60 mb-1">{label}</label>
      <div className="grid grid-cols-3 gap-1">
        <For each={['X', 'Y', 'Z']}>
          {(axis, index) => (
            <div className="relative">
              <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-base-content/70 pointer-events-none font-medium bg-base-300 border-t border-l border-b border-r border-base-300 rounded-l">
                {axis}
              </span>
              <input
                type="number"
                step="0.1"
                value={value[index()] || 0}
                onChange={(e) => {
                  const newValue = [...value];
                  newValue[index()] = parseFloat(e.target.value) || 0;
                  updateObjectProperty(selection.entity, propertyPath, newValue);
                  editorActions.updateBabylonObjectFromProperties(selection.entity);
                }}
                className={`w-full text-xs p-1.5 pl-7 pr-1.5 rounded text-center focus:outline-none focus:ring-1 focus:ring-primary ${
                  isNodeControlled(`${propertyPath}.${index()}`) 
                    ? 'border-primary bg-primary/20 text-primary' 
                    : 'border-base-300 bg-base-200 text-base-content'
                } border`}
                disabled={isNodeControlled(`${propertyPath}.${index()}`)}
              />
            </div>
          )}
        </For>
      </div>
      <Show when={isNodeControlled(propertyPath)}>
        <div className="text-xs text-primary mt-1">Controlled by node</div>
      </Show>
    </div>
  );

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
    
    const allExpanded = expandAllNodes(hierarchyData());
    setExpandedItems(allExpanded);
  };
  
  const collapseAll = () => {
    setExpandedItems({});
  };
  
  const startRename = (itemId, currentName) => {
    setRenamingItemId(itemId);
    setRenameValue(currentName);
  };
  
  const confirmRename = () => {
    if (renamingItemId() && renameValue().trim()) {
      const scene = getBabylonScene();
      if (scene) {
        const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
        const objectToRename = allObjects.find(obj => 
          (obj.uniqueId || obj.name) === renamingItemId()
        );
        
        if (objectToRename) {
          objectToRename.name = renameValue().trim();
        }
      }
      
      setRenamingItemId(null);
      setRenameValue('');
    }
  };
  
  const cancelRename = () => {
    setRenamingItemId(null);
    setRenameValue('');
  };
  
  const handleCreateFolder = () => {
    const scene = getBabylonScene();
    if (!scene) return;
    
    const folderName = `New Folder ${folderCounter()}`;
    const folder = new TransformNode(folderName, scene);
    
    if (selection.entity && selection.entity !== 'scene-root') {
      const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
      const parentObject = allObjects.find(obj => 
        (obj.uniqueId || obj.name) === selection.entity
      );
      if (parentObject) {
        folder.parent = parentObject;
      }
    }
    
    const folderId = folder.uniqueId || folder.name;
    setFolderCounter(prev => prev + 1);
    setSelectedEntity(folderId);
    setTimeout(() => startRename(folderId, folderName), 100);
  };

  const handleKeyDown = (e, item) => {
    if (e.key === 'F2' && item && !renamingItemId()) {
      e.preventDefault();
      startRename(item.id, item.name);
    } else if (e.key === 'Escape' && renamingItemId()) {
      e.preventDefault();
      cancelRename();
    } else if (e.key === 'Enter' && renamingItemId()) {
      e.preventDefault();
      confirmRename();
    }
  };

  const handleMouseDown = (e) => {
    e.preventDefault();
    setIsResizing(true);
    
    const startY = e.clientY;
    const startHeight = bottomPanelHeight();
    
    const handleMouseMove = (e) => {
      const deltaY = startY - e.clientY;
      const maxHeight = Math.floor(window.innerHeight * 0.7);
      const minHeight = 200;
      const newHeight = Math.max(minHeight, Math.min(maxHeight, startHeight + deltaY));
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

  const handleDeleteObject = (item, e) => {
    e.stopPropagation();
    
    if (item.babylonObject && item.babylonObject.dispose) {
      const scene = getBabylonScene();
      const isCamera = item.babylonObject.getClassName && item.babylonObject.getClassName().includes('Camera');
      
      // Check if this is the last camera
      if (isCamera && scene && scene.cameras.length <= 1) {
        editorActions.addConsoleMessage('Cannot delete the last camera! At least one camera is required for rendering.', 'error');
        return;
      }
      
      item.babylonObject.dispose();
      
      // If we deleted a camera, ensure there's still an active camera
      if (isCamera && scene) {
        setTimeout(() => {
          if (scene.cameras.length > 0 && !scene.activeCamera) {
            scene.activeCamera = scene.cameras[0];
            scene._camera = scene.cameras[0];
            scene.cameras[0].attachControl(scene.getEngine().getRenderingCanvas(), true);
            editorActions.addConsoleMessage(`Switched to camera: ${scene.cameras[0].name}`, 'info');
          }
        }, 100);
      }
      
      if (selection.entity === item.id) {
        setSelectedEntity(null);
      }
    }
  };

  const getIcon = (type, lightType) => {
    switch (type) {
      case 'mesh': return Box;
      case 'light': 
        switch (lightType) {
          case 'directional': return Lightbulb;
          case 'point': return Lightbulb;
          case 'spot': return Lightbulb;
          default: return Lightbulb;
        }
      case 'camera': return Camera;
      case 'folder': return Folder;
      default: return Circle;
    }
  };

  const renderSceneItem = (item, depth = 0, index = 0, parent = null) => {
    if (!item) return null;
    
    const isSelected = () => selection.entity === item.id;
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = () => expandedItems().hasOwnProperty(item.id) ? expandedItems()[item.id] : (item.expanded || false);
    const Icon = getIcon(item.type, item.lightType);
    
    const isDraggedOver = () => dragOverItem()?.id === item.id;
    const isFolderDrop = () => isDraggedOver() && dropPosition() === 'inside' && item.type === 'folder';

    const showTopDivider = () => isDraggedOver() && dropPosition() === 'above';
    const showBottomDivider = () => isDraggedOver() && dropPosition() === 'below';

    return (
      <div className="select-none relative">
        <Show when={showTopDivider()}>
          <div className="absolute top-0 left-0 right-0 h-0.5 bg-primary z-10 pointer-events-none" />
        </Show>
        <div 
          className={`group flex items-center transition-all duration-200 text-xs relative overflow-hidden rounded ${
            isSelected() 
              ? 'bg-primary/25 text-primary-content shadow-sm' 
              : 'hover:bg-base-300/40 text-base-content/70 hover:text-base-content active:bg-base-300/60'
          } ${
            draggedItem()?.id === item.id ? 'opacity-30' : ''
          } ${
            isFolderDrop() ? 'border-2 border-primary' : ''
          } ${
            droppedItemId() === item.id ? 'bg-success/50' : ''
          }`}
          style={{ 
            'padding-left': `${8 + depth * 20}px`,
            'padding-right': '8px',
            'padding-top': '2px',
            'padding-bottom': '2px',
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
            if (renamingItemId() !== item.id) {
              setSelectedEntity(item.id);
            }
          }}
          onDoubleClick={() => {
            if (!renamingItemId()) {
              startRename(item.id, item.name);
            }
          }}
          onContextMenu={(e) => {
            props.onContextMenu(e, item, 'scene');
          }}
        >
          <Show when={isSelected()}>
            <div className="absolute left-0 top-0 bottom-0 w-0.5 bg-primary pointer-events-none" />
          </Show>
          
          <Show when={depth > 0}>
            <div className="absolute left-0 top-0 bottom-0 pointer-events-none">
              <div
                className="absolute top-0 bottom-0 w-px bg-base-content/25"
                style={{ left: `${8 + (depth - 1) * 20 + 10}px` }}
              />
              <div
                className="absolute top-1/2 w-3 h-px bg-base-content/25"
                style={{ left: `${8 + (depth - 1) * 20 + 10}px` }}
              />
            </div>
          </Show>
          
          <Show when={hasChildren}>
            <button 
              className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-base-200/50"
              onClick={(e) => {
                e.stopPropagation();
                setExpandedItems(prev => ({ ...prev, [item.id]: !isExpanded() }));
              }}
            >
              <ChevronRight class={`w-3 h-3 transition-all duration-200 ${
                isExpanded() 
                  ? 'rotate-90 text-primary' 
                  : 'text-base-content/50 group-hover:text-base-content/70'
              }`} />
            </button>
          </Show>
          
          <button 
            className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-base-200/50"
            onClick={(e) => {
              e.stopPropagation();
              if (item.babylonObject) {
                if (item.babylonObject.isVisible !== undefined) {
                  item.babylonObject.isVisible = !item.babylonObject.isVisible;
                } else if (item.babylonObject.isEnabled) {
                  item.babylonObject.setEnabled(!item.babylonObject.isEnabled());
                }
              }
            }}
          >
            <Show 
              when={item.visible}
              fallback={<EyeOff className="w-4 h-4 text-base-content/40 hover:text-base-content/60" />}
            >
              <Eye className="w-4 h-4 text-base-content/60 hover:text-base-content/80" />
            </Show>
          </button>
          
          <button 
            className="mr-2 p-0.5 rounded transition-colors opacity-70 hover:opacity-100 hover:bg-base-200"
            onClick={(e) => {
              e.stopPropagation();
              createNodeEditorTab(item.id, item.name);
            }}
            title="Open Node Editor"
          >
            <Icon className="w-4 h-4 text-base-content/60" />
          </button>
          
          <Show 
            when={renamingItemId() === item.id}
            fallback={<span className="flex-1 text-base-content/80 truncate">{item.name}</span>}
          >
            <input
              type="text"
              value={renameValue()}
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
              className="flex-1 bg-base-300 text-base-content px-1 rounded text-xs border border-primary focus:outline-none focus:border-primary/80"
              autofocus
              onFocus={(e) => e.target.select()}
            />
          </Show>
          
          <button 
            className="ml-2 p-0.5 rounded transition-colors opacity-0 group-hover:opacity-70 hover:opacity-100"
            onClick={(e) => handleDeleteObject(item, e)}
            title="Delete object"
          >
            <Trash className="w-4 h-4 text-base-content/70 hover:text-error" />
          </button>
        </div>
        
        <Show when={hasChildren && isExpanded()}>
          <div className="transition-all duration-300 ease-out">
            <For each={item.children}>
              {(child, i) => renderSceneItem(child, depth + 1, i(), item)}
            </For>
          </div>
        </Show>
        
        <Show when={showBottomDivider()}>
          <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary z-10 pointer-events-none" />
        </Show>
      </div>
    );
  };

  return (
    <div 
      ref={containerRef} 
      className="flex flex-col flex-1 overflow-hidden"
      onContextMenu={(e) => props.onContextMenu(e, null)}
    >
      <div
        className="flex-1 overflow-y-auto scrollbar-thin"
        onContextMenu={(e) => props.onContextMenu(e, null)}
      >
        <For each={hierarchyData()}>
          {(item, i) => renderSceneItem(item, 0, i(), hierarchyData())}
        </For>
      </div>
      
      <div className="flex items-center justify-between px-2 py-1 border-t border-base-300/60 bg-gradient-to-b from-base-200/50 to-base-300/80">
        <div className="flex items-center gap-1">
          <button
            onClick={handleCreateFolder}
            className="p-1.5 rounded hover:bg-base-300/50 text-base-content/60 hover:text-base-content transition-all duration-150 active:bg-base-200/50 active:scale-95"
            title="Create Folder"
          >
            <Folder className="w-4 h-4" />
          </button>
          
          <div className="w-px h-4 bg-base-content/40 mx-1" />
          
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
                const itemName = findItemName(hierarchyData(), selection.entity);
                startRename(selection.entity, itemName);
              }
            }}
            disabled={!selection.entity || selection.entity === 'scene-root'}
            className="p-1.5 rounded hover:bg-base-300/50 text-base-content/60 hover:text-base-content transition-all duration-150 active:bg-base-200/50 active:scale-95 disabled:opacity-30 disabled:cursor-not-allowed"
            title="Rename Selected (F2)"
          >
            <Edit className="w-4 h-4" />
          </button>
        </div>
        
        <div className="flex items-center gap-1">
          <button
            onClick={expandAll}
            className="p-1.5 rounded hover:bg-base-300/50 text-base-content/60 hover:text-base-content transition-all duration-150 active:bg-base-200/50 active:scale-95"
            title="Expand All"
          >
            <ChevronRight className="w-4 h-4 rotate-90" />
          </button>
          
          <button
            onClick={collapseAll}
            className="p-1.5 rounded hover:bg-base-300/50 text-base-content/60 hover:text-base-content transition-all duration-150 active:bg-base-200/50 active:scale-95"
            title="Collapse All"
          >
            <ChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>
      
      <Show when={selection.entity}>
        <>
          <div
            className={`h-1 cursor-row-resize transition-colors ${isResizing() ? 'bg-primary/75' : 'bg-base-300/50 hover:bg-primary/75'}`}
            onMouseDown={handleMouseDown}
          />
          <div className="overflow-y-auto scrollbar-thin" style={{ height: `${bottomPanelHeight()}px` }}>
            {(() => {
              let objectProps = objectPropertiesStore.objects[selection.entity];
              
              if (!objectProps && selectedObjectData()) {
                objectPropertiesActions.ensureDefaultComponents(selection.entity);
                objectProps = objectPropertiesStore.objects[selection.entity];
              }
              
              if (!objectProps) {
                return (
                  <div className="p-4 text-base-content/50 text-sm">
                    No object selected.
                  </div>
                );
              }
              
              return (
                <div className="space-y-0">
                  <CollapsibleSection title="Scripts" defaultOpen={true} index={0}>
                    <div className="p-4">
                      {/* Attached Scripts List - Outside the drop zone */}
                      <Show when={objectProps.scripts && objectProps.scripts.length > 0}>
                        <div className="mb-4 space-y-2">
                          <For each={objectProps.scripts}>
                            {(script, index) => (
                              <div className="flex items-center justify-between bg-base-200 border border-base-300 rounded-lg px-2 py-1.5 shadow-sm">
                                <div className="flex items-center gap-2 min-w-0 flex-1">
                                  <input
                                    type="checkbox"
                                    checked={(() => {
                                      const runtime = getScriptRuntime();
                                      try {
                                        return !runtime.isScriptPaused(selection.entity, script.path);
                                      } catch {
                                        return true;
                                      }
                                    })()}
                                    onChange={(e) => {
                                      const runtime = getScriptRuntime();
                                      const entityId = selection.entity;
                                      
                                      if (e.target.checked) {
                                        runtime.resumeScript(entityId, script.path);
                                        editorActions.addConsoleMessage(`Resumed script "${script.name}"`, 'success');
                                      } else {
                                        runtime.pauseScript(entityId, script.path);
                                        editorActions.addConsoleMessage(`Paused script "${script.name}"`, 'info');
                                      }
                                    }}
                                    className="toggle toggle-xs toggle-success flex-shrink-0"
                                    title={(() => {
                                      const runtime = getScriptRuntime();
                                      try {
                                        return runtime.isScriptPaused(selection.entity, script.path) ? "Resume script" : "Pause script";
                                      } catch {
                                        return "Toggle script";
                                      }
                                    })()}
                                  />
                                  <div className="flex flex-col min-w-0 flex-1">
                                    <span className={`text-sm font-medium truncate ${(() => {
                                      const runtime = getScriptRuntime();
                                      try {
                                        return runtime.isScriptPaused(selection.entity, script.path) ? 'text-base-content/40' : 'text-base-content';
                                      } catch {
                                        return 'text-base-content';
                                      }
                                    })()}`} title={script.name}>{script.name}</span>
                                    {(() => {
                                      const runtime = getScriptRuntime();
                                      try {
                                        return runtime.isScriptPaused(selection.entity, script.path) && (
                                          <span className="text-xs text-warning">Paused</span>
                                        );
                                      } catch {
                                        return null;
                                      }
                                    })()}
                                  </div>
                                </div>
                                <button
                                  onClick={() => {
                                    console.log('🔧 Removing script:', script.path, 'from', selection.entity);
                                    
                                    // Detach from runtime
                                    const runtime = getScriptRuntime();
                                    runtime.detachScript(selection.entity, script.path);
                                    
                                    // Update UI
                                    const updatedScripts = objectProps.scripts.filter((_, i) => i !== index());
                                    updateObjectProperty(selection.entity, 'scripts', updatedScripts);
                                    
                                    editorActions.addConsoleMessage(`Script "${script.name}" removed`, 'info');
                                  }}
                                  className="p-1.5 hover:bg-base-300 rounded transition-colors"
                                >
                                  <X className="w-4 h-4 text-base-content/60 hover:text-error" />
                                </button>
                              </div>
                            )}
                          </For>
                        </div>
                      </Show>

                      {/* Drop Zone */}
                      <div 
                        className="min-h-[60px] bg-base-300 border-2 border-dashed border-base-300 rounded-lg p-4 text-center"
                        onDragOver={(e) => {
                          e.preventDefault();
                          e.currentTarget.classList.add('border-primary', 'bg-primary/20');
                        }}
                        onDragLeave={(e) => {
                          e.currentTarget.classList.remove('border-primary', 'bg-primary/20');
                        }}
                        onDrop={async (e) => {
                          e.preventDefault();
                          e.currentTarget.classList.remove('border-primary', 'bg-primary/20');
                          
                          const droppedData = e.dataTransfer.getData('text/plain');
                          try {
                            const data = JSON.parse(droppedData);
                            if (data.type === 'asset' && data.fileType === 'script') {
                              const validExtensions = ['.js', '.jsx', '.ts', '.tsx', '.ren'];
                              const fileExt = data.name.substring(data.name.lastIndexOf('.')).toLowerCase();
                              
                              if (validExtensions.includes(fileExt)) {
                                if (!objectProps.scripts) {
                                  objectPropertiesActions.addPropertySection(selection.entity, 'scripts', []);
                                }
                                
                                const currentScripts = objectProps.scripts || [];
                                if (!currentScripts.find(s => s.path === data.path)) {
                                  console.log('🔧 Attaching script via drag and drop:', data.path, 'to', selection.entity);
                                  
                                  // Use script runtime to attach the script
                                  const runtime = getScriptRuntime();
                                  const success = await runtime.attachScript(selection.entity, data.path);
                                  
                                  if (success) {
                                    const newScripts = [...currentScripts, { 
                                      path: data.path, 
                                      name: data.name,
                                      enabled: true 
                                    }];
                                    updateObjectProperty(selection.entity, 'scripts', newScripts);
                                    editorActions.addConsoleMessage(`Script "${data.name}" attached successfully`, 'success');
                                  } else {
                                    // The error message from ScriptManager will be more specific about type mismatches
                                    editorActions.addConsoleMessage(`Cannot attach "${data.name}" - check console for details`, 'error');
                                  }
                                } else {
                                  console.log('🔧 Script already attached:', data.path);
                                }
                              }
                            }
                          } catch (err) {
                            console.warn('Invalid drop data:', droppedData);
                          }
                        }}
                      >
                        <div className="flex flex-col items-center gap-2">
                          <Code className="w-5 h-5 text-base-content/40" />
                          <div className="text-base-content/60 text-sm">drop scripts here</div>
                          <div className="text-xs text-base-content/40">.ren, .js, .jsx, .ts, .tsx</div>
                        </div>
                      </div>
                    </div>
                  </CollapsibleSection>

                  <Show when={objectProps.transform}>
                    <CollapsibleSection title="Transform" defaultOpen={true} index={1}>
                      <div className="p-4">
                        <Show when={objectProps.transform.position}>
                          {renderVector3Input('Position', objectProps.transform.position, 'transform.position')}
                        </Show>
                        <Show when={objectProps.transform.rotation}>
                          {renderVector3Input('Rotation', objectProps.transform.rotation, 'transform.rotation')}
                        </Show>
                        <Show when={objectProps.transform.scale}>
                          {renderVector3Input('Scale', objectProps.transform.scale, 'transform.scale')}
                        </Show>
                      </div>
                    </CollapsibleSection>
                  </Show>

                  {/* Script Properties Sections - Each script gets sections organized by props category */}
                  <Show when={objectProps.scripts && objectProps.scripts.length > 0}>
                    <For each={objectProps.scripts}>
                      {(script) => {
                        const runtime = getScriptRuntime();
                        
                        // Create reactive signals for script properties by section
                        const [scriptPropertiesBySection, setScriptPropertiesBySection] = createSignal({});
                        const [propertyValues, setPropertyValues] = createSignal({});
                        const [refreshTrigger, setRefreshTrigger] = createSignal(0);
                        
                        // Listen for live script property updates and script reloads
                        onMount(() => {
                          const propertyUpdateListener = (event) => {
                            const { scriptPath, properties } = event.detail;
                            
                            // Check if this is the script we're displaying
                            if (scriptPath === script.path) {
                              console.log('🔧 Scene.jsx: Received property update for script', scriptPath);
                              setRefreshTrigger(prev => prev + 1);
                            }
                          };
                          
                          const scriptReloadListener = (event) => {
                            const { scriptPath, action } = event.detail;
                            
                            // Check if this is the script we're displaying
                            if (scriptPath === script.path) {
                              console.log('🔄 Scene.jsx: Script fully reloaded, refreshing properties', scriptPath);
                              setRefreshTrigger(prev => prev + 1);
                            }
                          };

                          const scriptRemovedListener = (event) => {
                            const { scriptPath, affectedObjects, action } = event.detail;
                            
                            // Check if this script was removed and affects our current object
                            if (scriptPath === script.path && affectedObjects.includes(selection.entity)) {
                              console.log('🗑️ Scene.jsx: Script removed, clearing properties', scriptPath);
                              setScriptPropertiesBySection({});
                              setPropertyValues({});
                            }
                          };
                          
                          document.addEventListener('engine:script-properties-updated', propertyUpdateListener);
                          document.addEventListener('engine:script-reloaded', scriptReloadListener);
                          document.addEventListener('engine:script-removed', scriptRemovedListener);
                          
                          onCleanup(() => {
                            document.removeEventListener('engine:script-properties-updated', propertyUpdateListener);
                            document.removeEventListener('engine:script-reloaded', scriptReloadListener);
                            document.removeEventListener('engine:script-removed', scriptRemovedListener);
                          });
                        });
                        
                        // Initialize properties reactively
                        createEffect(() => {
                          // Force reactive update by accessing refreshTrigger
                          refreshTrigger();
                          try {
                            const scriptInstance = runtime.getScriptInstance(selection.entity, script.path);
                            if (scriptInstance && scriptInstance._scriptAPI && scriptInstance._scriptAPI.getScriptPropertiesBySection) {
                              const propsBySection = scriptInstance._scriptAPI.getScriptPropertiesBySection();
                              console.log('🔧 Scene.jsx: Updating properties for', script.path, 'sections:', Object.keys(propsBySection), 'total props:', Object.values(propsBySection).flat().length);
                              setScriptPropertiesBySection(propsBySection);
                              
                              // Get current values for all properties
                              const values = {};
                              Object.values(propsBySection).flat().forEach(prop => {
                                const currentValue = scriptInstance._scriptAPI.getScriptProperty(prop.name);
                                values[prop.name] = currentValue !== null && currentValue !== undefined 
                                  ? currentValue 
                                  : (prop.defaultValue || 0);
                              });
                              setPropertyValues(values);
                            }
                          } catch (error) {
                            console.warn('Failed to get script properties:', error);
                            console.log('🔧 Scene.jsx: Clearing properties due to error for', script.path);
                            setScriptPropertiesBySection({});
                            setPropertyValues({});
                          }
                        });
                        
                        const handlePropertyChange = (propertyName, newValue) => {
                          console.log(`Property change: ${propertyName} = ${newValue}`);
                          try {
                            const scriptInstance = runtime.getScriptInstance(selection.entity, script.path);
                            if (scriptInstance?._scriptAPI?.setScriptProperty) {
                              scriptInstance._scriptAPI.setScriptProperty(propertyName, newValue);
                              
                              // Force update reactive state with new object
                              setPropertyValues(prev => {
                                const updated = { ...prev };
                                updated[propertyName] = newValue;
                                console.log('Updated property values:', updated);
                                return updated;
                              });
                            }
                          } catch (error) {
                            console.error('Failed to set script property:', error);
                          }
                        };
                        
                        const renderPropertyInput = (property) => {
                          const currentValue = () => {
                            const val = propertyValues()[property.name];
                            // For booleans, don't use || operator as false is a valid value
                            if (property.type === 'boolean') {
                              return val !== undefined ? val : property.defaultValue;
                            }
                            return val !== undefined && val !== null ? val : (property.defaultValue || 0);
                          };
                          
                          return (
                            <div className="form-control">
                              <label className="label pb-2">
                                <span className="label-text text-sm font-medium capitalize">
                                  {property.name.replace(/_/g, ' ')}
                                </span>
                                <Show when={property.description && property.description !== 'null'}>
                                  <div className="tooltip tooltip-left" data-tip={property.description.replace(/"/g, '')}>
                                    <span className="text-xs text-base-content/50 cursor-help">?</span>
                                  </div>
                                </Show>
                              </label>
                              
                              <Switch>
                                <Match when={property.type === 'number' || property.type === 'float'}>
                                  <Show 
                                    when={property.min !== undefined && property.max !== undefined}
                                    fallback={
                                      <div className="join w-full">
                                        <input
                                          type="number"
                                          value={currentValue()}
                                          step={property.type === 'float' ? '0.1' : '1'}
                                          min={property.min}
                                          max={property.max}
                                          onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value) || 0)}
                                          className="input input-bordered input-sm join-item flex-1 text-sm"
                                          placeholder="0"
                                        />
                                      </div>
                                    }
                                  >
                                    <div className="flex items-center gap-2 w-full">
                                      <input
                                        type="range"
                                        min={property.min}
                                        max={property.max}
                                        step={property.type === 'float' ? '0.1' : '1'}
                                        value={currentValue()}
                                        onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value))}
                                        className="range range-primary range-xs flex-1"
                                      />
                                      <input
                                        type="number"
                                        value={currentValue()}
                                        step={property.type === 'float' ? '0.1' : '1'}
                                        min={property.min}
                                        max={property.max}
                                        onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value) || 0)}
                                        className="input input-bordered input-xs w-16 text-xs text-center"
                                      />
                                    </div>
                                  </Show>
                                </Match>
                                
                                <Match when={property.type === 'boolean'}>
                                  <div className="flex items-center justify-between">
                                    <div className="form-control">
                                      <label className="label cursor-pointer justify-start gap-3">
                                        <input
                                          type="checkbox"
                                          checked={!!currentValue()}
                                          onChange={(e) => handlePropertyChange(property.name, e.target.checked)}
                                          className="toggle toggle-secondary toggle-sm"
                                        />
                                        <span className="label-text text-sm">
                                          {currentValue() ? 'Enabled' : 'Disabled'}
                                        </span>
                                      </label>
                                    </div>
                                    <div className={`badge badge-sm ${currentValue() ? 'badge-success' : 'badge-ghost'}`}>
                                      {currentValue() ? 'ON' : 'OFF'}
                                    </div>
                                  </div>
                                </Match>
                                
                                <Match when={property.type === 'string'}>
                                  <input
                                    type="text"
                                    value={currentValue() || ''}
                                    onChange={(e) => handlePropertyChange(property.name, e.target.value)}
                                    className="input input-bordered input-sm w-full text-sm"
                                    placeholder={property.defaultValue?.replace(/"/g, '') || 'Enter text...'}
                                  />
                                </Match>
                                
                                <Match when={property.type === 'select' && property.options}>
                                  <select
                                    value={currentValue() || property.defaultValue}
                                    onChange={(e) => handlePropertyChange(property.name, e.target.value)}
                                    className="select select-bordered select-sm w-full text-sm"
                                  >
                                    <For each={property.options}>
                                      {(option) => (
                                        <option value={option}>{option}</option>
                                      )}
                                    </For>
                                  </select>
                                </Match>
                                
                                <Match when={property.type === 'range'}>
                                  <div className="space-y-3">
                                    <div className="flex items-center gap-3">
                                      <input
                                        type="range"
                                        value={currentValue()}
                                        min={property.min || 0}
                                        max={property.max || 100}
                                        step={0.01}
                                        onChange={(e) => handlePropertyChange(property.name, parseFloat(e.target.value))}
                                        className="range range-secondary range-sm flex-1"
                                      />
                                      <div className="badge badge-secondary badge-outline font-mono text-xs min-w-[4rem]">
                                        {parseFloat(currentValue()).toFixed(2)}
                                      </div>
                                    </div>
                                    <div className="flex justify-between text-xs text-base-content/60">
                                      <span className="badge badge-ghost badge-xs">{property.min || 0}</span>
                                      <span className="badge badge-ghost badge-xs">{property.max || 100}</span>
                                    </div>
                                  </div>
                                </Match>
                                
                                <Match when={true}>
                                  <div className="join w-full">
                                    <input
                                      type="text"
                                      value={currentValue() || ''}
                                      onChange={(e) => handlePropertyChange(property.name, e.target.value)}
                                      className="input input-bordered input-sm join-item flex-1 text-sm"
                                      placeholder={`${property.type} value`}
                                    />
                                    <span className="btn btn-sm join-item btn-outline btn-disabled text-xs">
                                      {property.type}
                                    </span>
                                  </div>
                                </Match>
                              </Switch>
                            </div>
                          );
                        };
                        
                        return (
                          <Show when={Object.keys(scriptPropertiesBySection()).length > 0}>
                            <For each={Object.entries(scriptPropertiesBySection())}>
                              {([sectionName, properties]) => (
                                <CollapsibleSection
                                  title={sectionName}
                                  icon={<Code className="w-4 h-4 text-secondary" />}
                                  defaultExpanded={true}
                                >
                                  <div className="space-y-6 p-4">
                                    <For each={properties}>
                                      {(property) => renderPropertyInput(property)}
                                    </For>
                                  </div>
                                </CollapsibleSection>
                              )}
                            </For>
                          </Show>
                        );
                      }}
                    </For>
                  </Show>

                  <Show when={!objectProps.transform && !objectProps.material && !objectProps.components}>
                    <div className="p-4 text-center">
                      <div className="text-base-content/50 text-sm mb-2">
                        No properties configured
                      </div>
                      <div className="text-base-content/40 text-xs">
                        Open the node editor and connect nodes to output nodes to create property sections
                      </div>
                    </div>
                  </Show>
                </div>
              );
            })()}
          </div>
        </>
      </Show>
    </div>
  );
}

export default Scene;