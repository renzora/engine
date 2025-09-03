import { createSignal, onCleanup, onMount, For, Show } from 'solid-js';
import { ChevronRight, Box, Lightbulb, Camera, Folder, Circle, Eye, EyeOff, Trash, Edit } from '@/ui/icons';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { viewportActions } from '@/layout/stores/ViewportStore';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { renderStore, renderActions } from '@/render/store';


function Scene(props) {
  const { ui, selection } = editorStore;
  const bottomPanelHeight = () => ui.scenePropertiesHeight;
  const { setScenePropertiesHeight: setBottomPanelHeight } = editorActions;
  const [isResizing, setIsResizing] = createSignal(false);
  const { selectEntity: setSelectedEntity, setTransformMode } = editorActions;
  const { createNodeTab: createNodeEditorTab } = viewportActions;
  
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
  const [expandedItems, setExpandedItems] = createSignal({});
  const [renamingItemId, setRenamingItemId] = createSignal(null);
  const [renameValue, setRenameValue] = createSignal('');
  const [folderCounter, setFolderCounter] = createSignal(1);
  
  // Use hierarchy from render store
  const hierarchyData = () => renderStore.hierarchy;


  // Inline drag and drop state and handlers
  const [draggedItem, setDraggedItem] = createSignal(null);
  const [dragOverItem, setDragOverItem] = createSignal(null);
  const [dropPosition, setDropPosition] = createSignal(null);

  const handleDragStart = (e, item) => {
    setDraggedItem(item);
    e.dataTransfer.effectAllowed = 'move';
    // Only serialize safe properties to avoid circular references
    const safeItem = {
      id: item.id,
      name: item.name,
      type: item.type,
      lightType: item.lightType,
      visible: item.visible
    };
    e.dataTransfer.setData('text/plain', JSON.stringify({ type: 'scene-item', item: safeItem }));
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

    const scene = renderStore.scene;
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

    // Hierarchy will update automatically since we're changing Babylon parent relationships
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
      const scene = renderStore.scene;
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
    const scene = renderStore.scene;
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
    
    // Use render actions to add the folder to hierarchy
    renderActions.addObject(folder);
    
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
      const scene = renderStore.scene;
      const isCamera = item.babylonObject.getClassName && item.babylonObject.getClassName().includes('Camera');
      
      // Check if this is the last camera
      if (isCamera && scene && scene.cameras.length <= 1) {
        editorActions.addConsoleMessage('Cannot delete the last camera! At least one camera is required for rendering.', 'error');
        return;
      }
      
      // Use render actions to remove the object
      renderActions.removeObject(item.babylonObject);
      
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

  const getIconColor = (type, lightType) => {
    switch (type) {
      case 'mesh': return '#60a5fa'; // blue-400
      case 'light': 
        switch (lightType) {
          case 'directional': return '#facc15'; // yellow-400
          case 'point': return '#fb923c'; // orange-400
          case 'spot': return '#fbbf24'; // amber-400
          default: return '#fde047'; // yellow-300
        }
      case 'camera': return '#a78bfa'; // purple-400
      case 'folder': return '#eab308'; // yellow-500
      default: return '#9ca3af'; // gray-400
    }
  };

  const renderSceneItem = (item, depth = 0, index = 0, parent = null, globalIndex = 0) => {
    if (!item) return null;
    
    const isSelected = () => selection.entity === item.id;
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = () => expandedItems().hasOwnProperty(item.id) ? expandedItems()[item.id] : (item.expanded || false);
    const Icon = getIcon(item.type, item.lightType);
    const iconColor = getIconColor(item.type, item.lightType);
    
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
          className={`group flex items-center py-0.5 pr-2 text-xs cursor-pointer transition-colors relative overflow-hidden ${
            isSelected() 
              ? 'bg-primary text-primary-content' 
              : 'text-base-content/70 hover:bg-base-200 hover:text-base-content'
          } ${
            draggedItem()?.id === item.id ? 'opacity-30' : ''
          } ${
            isFolderDrop() ? 'border-2 border-primary' : ''
          } ${
            droppedItemId() === item.id ? 'bg-success/50' : ''
          }`}
          style={{ 
            'padding-left': `${6 + depth * 16}px`,
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
              // Use shared selection by ID
              const success = renderActions.selectObjectById(item.id);
              if (!success) {
                // Fallback for non-Babylon objects (like folders)
                setSelectedEntity(item.id);
              }
              // Call the parent's onObjectSelect to switch to object properties tab
              if (props.onObjectSelect) {
                props.onObjectSelect(item.id);
              }
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
                className="absolute top-0 bottom-0 w-px bg-base-content/30"
                style={{ left: `${6 + (depth - 1) * 16 + 8}px` }}
              />
              <div
                className="absolute top-1/2 w-2 h-px bg-base-content/30"
                style={{ left: `${6 + (depth - 1) * 16 + 8}px` }}
              />
            </div>
          </Show>
          
          <div class="relative flex items-center">
            <Icon class="w-3 h-3 mr-0.5 cursor-pointer" style={{ color: iconColor }} />
            <Show when={hasChildren}>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setExpandedItems(prev => ({ ...prev, [item.id]: !isExpanded() }));
                }}
                class="absolute -left-0.5 p-0.5 rounded transition-all duration-200 hover:bg-base-200/50"
              >
                <ChevronRight 
                  class={`w-2.5 h-2.5 transition-all duration-200 ${
                    hasChildren && isExpanded() 
                      ? 'rotate-90 text-primary' 
                      : hasChildren
                        ? 'text-base-content/50 hover:text-base-content/70'
                        : 'text-base-content/20'
                  }`} 
                />
              </button>
            </Show>
          </div>
          
          <button 
            className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-base-200/50 cursor-pointer"
            onClick={(e) => {
              e.stopPropagation();
              if (item.babylonObject) {
                const newVisibility = !item.babylonObject.isVisible;
                item.babylonObject.isVisible = newVisibility;
                
                // Update the hierarchy item's visibility directly to trigger reactive update
                renderActions.updateObjectVisibility(item.id, newVisibility);
              }
            }}
          >
            <Show 
              when={item.visible}
              fallback={<EyeOff class="w-3 h-3 cursor-pointer" style={{ color: '#ef4444' }} />}
            >
              <Eye class="w-3 h-3 cursor-pointer" style={{ color: '#9ca3af' }} />
            </Show>
          </button>
          
          <Show 
            when={renamingItemId() === item.id}
            fallback={<span className="flex-1 text-base-content/80 truncate text-xs">{item.name}</span>}
          >
            <input
              id={`rename-input-${renamingItemId() || 'unknown'}`}
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
            className="ml-1 p-0.5 rounded transition-colors opacity-0 group-hover:opacity-70 hover:opacity-100"
            onClick={(e) => handleDeleteObject(item, e)}
            title="Delete object"
          >
            <Trash className="w-3 h-3 text-base-content/70 hover:text-error" />
          </button>
        </div>
        
        <Show when={hasChildren && isExpanded()}>
          <div className="transition-all duration-300 ease-out">
            <For each={item.children}>
              {(child, i) => renderSceneItem(child, depth + 1, i(), item, globalIndex + i() + 1)}
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
      className="flex flex-col h-full overflow-hidden"
      onContextMenu={(e) => props.onContextMenu(e, null)}
    >
      {/* Scene header - fixed */}
      <div className="flex-shrink-0 px-3 py-2">
        <div className="text-xs text-base-content/60 uppercase tracking-wide">
          Scene
        </div>
      </div>
      
      <div
        className="flex-1 overflow-y-auto scrollbar-thin"
        onContextMenu={(e) => props.onContextMenu(e, null)}
      >
        <For each={hierarchyData()}>
          {(item, i) => renderSceneItem(item, 0, i(), hierarchyData(), i())}
        </For>
      </div>
      
      <div className="flex-shrink-0 flex items-center justify-between px-2 py-1 border-t border-base-content/5">
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
      
    </div>
  );
}

export default Scene;