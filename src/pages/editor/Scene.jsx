import { createSignal, onCleanup, onMount, For, Show } from 'solid-js';
import { IconBox, IconBulb, IconChairDirector, IconFolder, IconFolderOpen, IconCircle, IconEye, IconEyeOff, IconTrash, IconEdit, IconVideo } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { viewportActions, viewportStore } from '@/layout/stores/ViewportStore';
import { TransformNode } from '@babylonjs/core/Meshes/transformNode';
import { renderStore, renderActions } from '@/render/store';


function Scene(props) {
  const { ui, selection } = editorStore;
  const bottomPanelHeight = () => ui.scenePropertiesHeight;
  const { setScenePropertiesHeight: setBottomPanelHeight } = editorActions;
  const [isResizing, setIsResizing] = createSignal(false);
  const { selectEntity: setSelectedEntity, setTransformMode } = editorActions;
  const { addViewportTab, setActiveViewportTab } = viewportActions;
  const tabs = () => viewportStore.tabs;
  
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
    
    // Select scene root by default on load
    if (!selection.entity && props.onObjectSelect) {
      props.onObjectSelect('scene-root');
    }
  });
  
  const [droppedItemId, setDroppedItemId] = createSignal(null);
  const [expandedItems, setExpandedItems] = createSignal({});
  const [renamingItemId, setRenamingItemId] = createSignal(null);
  const [renameValue, setRenameValue] = createSignal('');
  const [folderCounter, setFolderCounter] = createSignal(1);
  
  // Global counter for alternating backgrounds
  let globalRowCounter = { value: 0 };
  
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
          const newName = renameValue().trim();
          objectToRename.name = newName;
          
          // Update only this object's name in the hierarchy (much more efficient)
          renderActions.updateObjectName(renamingItemId(), newName);
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

  const getIcon = (type, lightType, hasChildren, isExpanded) => {
    switch (type) {
      case 'mesh': return IconBox;
      case 'light': 
        switch (lightType) {
          case 'directional': return IconBulb;
          case 'point': return IconBulb;
          case 'spot': return IconBulb;
          default: return IconBulb;
        }
      case 'camera': return IconVideo;
      case 'folder': return (hasChildren && isExpanded) ? IconFolderOpen : IconFolder;
      case 'scene': return IconChairDirector;
      default: return IconCircle;
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

  const renderSceneItem = (item, depth = 0, index = 0, parent = null, globalCounter = { value: 0 }) => {
    if (!item) return null;
    
    const currentIndex = globalCounter.value++;
    
    const isSelected = () => selection.entity === item.id;
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = () => expandedItems().hasOwnProperty(item.id) ? expandedItems()[item.id] : (item.expanded || false);
    const Icon = getIcon(item.type, item.lightType, hasChildren, isExpanded());
    const iconColor = getIconColor(item.type, item.lightType);
    
    // Scene root is always visible and active
    const isSceneRoot = item.type === 'scene';
    const itemVisible = isSceneRoot ? true : (item.visible !== undefined ? item.visible : true);
    
    const isDraggedOver = () => dragOverItem()?.id === item.id;
    const isFolderDrop = () => isDraggedOver() && dropPosition() === 'inside' && item.type === 'folder';

    const showTopDivider = () => isDraggedOver() && dropPosition() === 'above';
    const showBottomDivider = () => isDraggedOver() && dropPosition() === 'below';

    return (
      <div className={`select-none relative ${
        isSelected() 
          ? '' 
          : currentIndex % 2 === 0 
            ? 'bg-base-100/80'
            : 'bg-base-200/60'
      }`}>
        <Show when={showTopDivider()}>
          <div className="absolute top-0 left-0 right-0 h-0.5 bg-primary z-10 pointer-events-none" />
        </Show>
        <div 
          className={`group flex items-center py-0.5 pr-2 text-xs cursor-pointer transition-colors relative overflow-hidden ${
            isSelected() 
              ? 'bg-primary/80 text-primary-content' 
              : 'text-base-content/70 hover:bg-primary/20 hover:text-base-content'
          } ${
            draggedItem()?.id === item.id ? 'opacity-30' : ''
          } ${
            isFolderDrop() ? 'border-2 border-primary' : ''
          } ${
            droppedItemId() === item.id ? 'bg-success/50' : ''
          }`}
          style={{ 
            'padding-left': `${6 + depth * 16}px`,
            cursor: 'pointer'
          }}
          draggable={!props.isResizing}
          onDragStart={(e) => !props.isResizing && handleDragStart(e, item)}
          onDragOver={(e) => !props.isResizing && handleDragOver(e, item)}
          onDrop={(e) => !props.isResizing && handleDropWithAnimation(e, item)}
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
              // Toggle folder expansion
              if (hasChildren) {
                setExpandedItems(prev => ({ ...prev, [item.id]: !isExpanded() }));
              }
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
            <Icon 
              class="w-4 h-4 mr-0.5 cursor-pointer hover:opacity-70 transition-opacity" 
              style={{ 
                color: iconColor,
                fill: item.type === 'folder' && hasChildren && !isExpanded() ? iconColor : 'none'
              }}
            />
          </div>
          
          <button 
            className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-base-200/50 cursor-pointer"
            onClick={(e) => {
              e.stopPropagation();
              if (isSceneRoot) {
                // Scene root visibility toggle - could control overall scene visibility
                const newVisibility = !itemVisible;
                renderActions.updateObjectVisibility(item.id, newVisibility);
              } else if (item.babylonObject) {
                const newVisibility = !item.babylonObject.isVisible;
                item.babylonObject.isVisible = newVisibility;
                
                // Update the hierarchy item's visibility directly to trigger reactive update
                renderActions.updateObjectVisibility(item.id, newVisibility);
              }
            }}
          >
            <Show 
              when={itemVisible}
              fallback={<IconEyeOff class="w-4 h-4 cursor-pointer" style={{ color: '#ef4444' }} />}
            >
              <IconEye class="w-4 h-4 cursor-pointer" style={{ color: '#9ca3af' }} />
            </Show>
          </button>
          
          <Show 
            when={renamingItemId() === item.id}
            fallback={
              <span 
                className="flex-1 text-base-content/80 truncate text-xs cursor-pointer"
                onDoubleClick={(e) => {
                  console.log('Double click triggered on span:', item.name);
                  e.preventDefault();
                  e.stopPropagation();
                  console.log('Current renamingItemId:', renamingItemId());
                  if (!renamingItemId()) {
                    console.log('Starting rename for:', item.id, item.name);
                    startRename(item.id, item.name);
                  } else {
                    console.log('Already renaming another item');
                  }
                }}
              >
                {item.name}
              </span>
            }
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
              onFocus={(e) => {
                setTimeout(() => e.target.select(), 0);
              }}
            />
          </Show>
          
          <button 
            className="ml-1 p-0.5 rounded transition-colors opacity-0 group-hover:opacity-70 hover:opacity-100 cursor-pointer"
            onClick={(e) => handleDeleteObject(item, e)}
            title="Delete object"
          >
            <IconTrash className="w-4 h-4 text-base-content/70 hover:text-error" />
          </button>
        </div>
        
        <Show when={hasChildren && isExpanded()}>
          <div className="transition-all duration-300 ease-out">
            <For each={item.children}>
              {(child, i) => renderSceneItem(child, depth + 1, i(), item, globalCounter)}
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
      className="flex flex-col h-full overflow-hidden bg-base-100"
      onContextMenu={(e) => props.onContextMenu(e, null)}
    >
      {/* Scene header - fixed */}
      <div className="flex-shrink-0 px-3 py-2 flex items-center justify-between">
        <div className="text-xs text-base-content/60 uppercase tracking-wide">
          Scene
        </div>
      </div>
      
      <div
        className="flex-1 overflow-y-auto scrollbar-thin"
        onContextMenu={(e) => props.onContextMenu(e, null)}
      >
        <For each={hierarchyData()}>
          {(item, i) => {
            if (i() === 0) globalRowCounter.value = 0; // Only reset on first item
            return renderSceneItem(item, 0, i(), hierarchyData(), globalRowCounter);
          }}
        </For>
      </div>
      
      
    </div>
  );
}

export default Scene;