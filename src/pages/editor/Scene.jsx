import { createSignal, createMemo, onCleanup, onMount, createEffect, For, Show } from 'solid-js';
import { ChevronRight, Box, Lightbulb, Camera, Folder, Circle, Eye, EyeOff, Trash, Edit, Code, X } from '@/ui/icons';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
const getBabylonScene = () => window._cleanBabylonScene;
import { viewportActions, objectPropertiesActions, objectPropertiesStore } from '@/layout/stores/ViewportStore';
import useSceneDnD from '@/ui/hooks/useSceneDnD.jsx';
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

  const { draggedItem, dragOverItem, dropPosition, handleDragStart, handleDragOver, handleDrop, handleDragEnd } = useSceneDnD(hierarchyData());

  let containerRef;

  const handleDropWithAnimation = (e, item) => {
    handleDrop(e, item);
    setDroppedItemId(draggedItem.id);
    setTimeout(() => setDroppedItemId(null), 500);
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

  const handleDeleteObject = (item, e) => {
    e.stopPropagation();
    
    if (item.babylonObject && item.babylonObject.dispose) {
      item.babylonObject.dispose();
      
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
    
    const isDraggedOver = () => dragOverItem?.id === item.id;
    const isFolderDrop = () => isDraggedOver() && dropPosition === 'inside' && item.type === 'folder';

    const showTopDivider = () => isDraggedOver() && dropPosition === 'above';
    const showBottomDivider = () => isDraggedOver() && dropPosition === 'below';

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
            draggedItem?.id === item.id ? 'opacity-30' : ''
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
                  <Show when={objectProps.transform}>
                    <CollapsibleSection title="Transform" defaultOpen={true} index={0}>
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

                  <CollapsibleSection title="Scripts" defaultOpen={true} index={1}>
                    <div className="p-4">
                      <div className="mb-3">
                        <label className="block text-xs text-base-content/60 mb-2">Attached Scripts</label>
                        <div 
                          className="min-h-[80px] border-2 border-dashed border-base-300 rounded-lg p-3 text-center transition-colors hover:border-primary hover:bg-primary/10"
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
                                const validExtensions = ['.js', '.jsx', '.ts', '.tsx'];
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
                                      editorActions.addConsoleMessage(`Failed to attach script "${data.name}"`, 'error');
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
                          <Show 
                            when={objectProps.scripts && objectProps.scripts.length > 0}
                            fallback={
                              <div className="text-base-content/50 text-xs">
                                <p>Drop script files here</p>
                                <p className="text-base-content/40 mt-1">(.js, .jsx, .ts, .tsx)</p>
                              </div>
                            }
                          >
                            <div className="space-y-2">
                              <For each={objectProps.scripts}>
                                {(script, index) => (
                                  <div className="flex items-center justify-between bg-base-200 rounded px-2 py-1">
                                    <div className="flex items-center gap-2">
                                      <input
                                        type="checkbox"
                                        checked={script.enabled}
                                        onChange={(e) => {
                                          const updatedScripts = [...objectProps.scripts];
                                          updatedScripts[index()] = { ...script, enabled: e.target.checked };
                                          updateObjectProperty(selection.entity, 'scripts', updatedScripts);
                                        }}
                                        className="w-3 h-3"
                                      />
                                      <Code className="w-3 h-3 text-base-content/60" />
                                      <span className="text-xs text-base-content/70">{script.name}</span>
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
                                      className="p-0.5 hover:bg-base-300 rounded"
                                    >
                                      <X className="w-3 h-3 text-base-content/60 hover:text-error" />
                                    </button>
                                  </div>
                                )}
                              </For>
                            </div>
                          </Show>
                        </div>
                      </div>
                    </div>
                  </CollapsibleSection>

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
