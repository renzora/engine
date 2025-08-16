import { createSignal, createMemo, onCleanup, onMount, createEffect, For, Show } from 'solid-js';
import { IconChevronRight, IconBox, IconBulb, IconCamera, IconFolder, IconLayersLinked, IconEye, IconEyeOff, IconTrash, IconPencil, IconCode, IconX } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/plugins/editor/stores/EditorStore';
import { sceneStore, sceneActions, babylonScene } from '@/plugins/core/render/store';
import { viewportActions, objectPropertiesActions, objectPropertiesStore } from '@/plugins/editor/stores/ViewportStore';
import useSceneDnD from '@/plugins/editor/hooks/useSceneDnD';
import { CollapsibleSection } from '@/components/ui';


function Scene(props) {
  const { ui, selection } = editorStore;
  const sceneData = sceneStore;
  const bottomPanelHeight = () => ui.scenePropertiesHeight;
  const { setScenePropertiesHeight: setBottomPanelHeight } = editorActions;
  const [isResizing, setIsResizing] = createSignal(false);
  const { selectEntity: setSelectedEntity, setTransformMode } = editorActions;
  const { selectObject } = sceneActions;
  const { createNodeTab: createNodeEditorTab } = viewportActions;
  const { updateObjectProperty } = objectPropertiesActions;
  
  const [droppedItemId, setDroppedItemId] = createSignal(null);
  const [isDragOverScript, setIsDragOverScript] = createSignal(false);
  const [expandedItems, setExpandedItems] = createSignal({});
  const [renamingItemId, setRenamingItemId] = createSignal(null);
  const [renameValue, setRenameValue] = createSignal('');
  const [folderCounter, setFolderCounter] = createSignal(1);

  const { draggedItem, dragOverItem, dropPosition, handleDragStart, handleDragOver, handleDrop, handleDragEnd } = useSceneDnD(sceneData.hierarchy);

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
    
    const scene = babylonScene?.current;
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
      <label className="block text-xs text-gray-400 mb-1">{label}</label>
      <div className="grid grid-cols-3 gap-1">
        <For each={['X', 'Y', 'Z']}>
          {(axis, index) => (
            <div className="relative">
              <span className="absolute left-0 top-0 bottom-0 w-6 flex items-center justify-center text-[10px] text-gray-300 pointer-events-none font-medium bg-gray-700 border-t border-l border-b border-r border-gray-600 rounded-l">
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
                className={`w-full text-xs p-1.5 pl-7 pr-1.5 rounded text-center focus:outline-none focus:ring-1 focus:ring-blue-500 ${
                  isNodeControlled(`${propertyPath}.${index()}`) 
                    ? 'border-blue-500 bg-blue-900/20 text-blue-200' 
                    : 'border-gray-600 bg-gray-800 text-white'
                } border`}
                disabled={isNodeControlled(`${propertyPath}.${index()}`)}
              />
            </div>
          )}
        </For>
      </div>
      <Show when={isNodeControlled(propertyPath)}>
        <div className="text-xs text-blue-400 mt-1">Controlled by node</div>
      </Show>
    </div>
  );

  const hierarchyData = createMemo(() => {
    return sceneData.hierarchy;
  });

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
      renameObject(renamingItemId(), renameValue().trim());
      setRenamingItemId(null);
      setRenameValue('');
    }
  };
  
  const cancelRename = () => {
    setRenamingItemId(null);
    setRenameValue('');
  };
  
  const handleCreateFolder = () => {
    const folderName = `New Folder ${folderCounter()}`;
    const parentId = selection.entity && selection.entity !== 'scene-root' ? selection.entity : null;
    
    const folderId = createFolder(folderName, parentId);
    setFolderCounter(prev => prev + 1);
    
    selectObject(folderId);
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
    
    editorActions.refreshSceneData();
  };

  const getIcon = (type, lightType) => {
    switch (type) {
      case 'mesh': return IconBox;
      case 'light': 
        switch (lightType) {
          case 'directional': return IconBulb;
          case 'point': return IconBulb;
          case 'spot': return IconBulb;
          default: return IconBulb;
        }
      case 'camera': return IconCamera;
      case 'folder': return IconFolder;
      default: return IconLayersLinked;
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
          <div className="absolute top-0 left-0 right-0 h-0.5 bg-blue-500 z-10 pointer-events-none" />
        </Show>
        <div 
          className={`group flex items-center transition-all duration-200 text-xs relative overflow-hidden rounded ${
            isSelected() 
              ? 'bg-blue-600/25 text-white shadow-sm' 
              : 'hover:bg-slate-700/40 text-gray-300 hover:text-gray-100 active:bg-slate-700/60'
          } ${
            draggedItem?.id === item.id ? 'opacity-30' : ''
          } ${
            isFolderDrop() ? 'border-2 border-blue-500' : ''
          } ${
            droppedItemId() === item.id ? 'bg-green-500/50' : ''
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
              selectObject(item.id);
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
            <div className="absolute left-0 top-0 bottom-0 w-0.5 bg-blue-400 pointer-events-none" />
          </Show>
          
          <Show when={depth > 0}>
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
          </Show>
          
          <Show when={hasChildren}>
            <button 
              className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-slate-600/50"
              onClick={(e) => {
                e.stopPropagation();
                setExpandedItems(prev => ({ ...prev, [item.id]: !isExpanded() }));
              }}
            >
              <IconChevronRight class={`w-3 h-3 transition-all duration-200 ${
                isExpanded() 
                  ? 'rotate-90 text-blue-400' 
                  : 'text-gray-500 group-hover:text-gray-300'
              }`} />
            </button>
          </Show>
          
          <button 
            className="mr-1 p-0.5 rounded transition-all duration-200 hover:bg-slate-600/50"
            onClick={(e) => {
              e.stopPropagation();
              editorActions.updateSceneObjectProperty(item.id, 'visible', !item.visible);
            }}
          >
            <Show 
              when={item.visible}
              fallback={<IconEyeOff className="w-4 h-4 text-gray-600 hover:text-gray-400" />}
            >
              <IconEye className="w-4 h-4 text-gray-400 hover:text-gray-200" />
            </Show>
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
          
          <Show 
            when={renamingItemId() === item.id}
            fallback={<span className="flex-1 text-gray-200 truncate">{item.name}</span>}
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
              className="flex-1 bg-slate-700 text-gray-200 px-1 rounded text-xs border border-blue-400 focus:outline-none focus:border-blue-300"
              autofocus
              onFocus={(e) => e.target.select()}
            />
          </Show>
          
          <button 
            className="ml-2 p-0.5 rounded transition-colors opacity-0 group-hover:opacity-70 hover:opacity-100"
            onClick={(e) => handleDeleteObject(item.id, e)}
            title="Delete object"
          >
            <IconTrash className="w-4 h-4 text-gray-300 hover:text-red-400" />
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
          <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-500 z-10 pointer-events-none" />
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
      
      <div className="flex items-center justify-between px-2 py-1 border-t border-slate-700/60 bg-gradient-to-b from-slate-800/50 to-slate-900/80">
        <div className="flex items-center gap-1">
          <button
            onClick={handleCreateFolder}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95"
            title="Create Folder"
          >
            <IconFolder className="w-4 h-4" />
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
                const itemName = findItemName(hierarchyData(), selection.entity);
                startRename(selection.entity, itemName);
              }
            }}
            disabled={!selection.entity || selection.entity === 'scene-root'}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95 disabled:opacity-30 disabled:cursor-not-allowed"
            title="Rename Selected (F2)"
          >
            <IconPencil className="w-4 h-4" />
          </button>
        </div>
        
        <div className="flex items-center gap-1">
          <button
            onClick={expandAll}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95"
            title="Expand All"
          >
            <IconChevronRight className="w-4 h-4 rotate-90" />
          </button>
          
          <button
            onClick={collapseAll}
            className="p-1.5 rounded hover:bg-slate-700/50 text-gray-400 hover:text-gray-200 transition-all duration-150 active:bg-slate-600/50 active:scale-95"
            title="Collapse All"
          >
            <IconChevronRight className="w-4 h-4" />
          </button>
        </div>
      </div>
      
      <Show when={selection.entity}>
        <>
          <div
            className={`h-1 cursor-row-resize transition-colors ${isResizing() ? 'bg-blue-500/75' : 'bg-slate-700/50 hover:bg-blue-500/75'}`}
            onMouseDown={handleMouseDown}
          />
          <div className="overflow-y-auto scrollbar-thin" style={{ height: `${bottomPanelHeight()}px` }}>
            {(() => {
              let objectProps = objectPropertiesStore.objects[selection.entity];
              
              // Initialize properties if they don't exist for the selected object
              if (!objectProps && selectedObjectData()) {
                objectPropertiesActions.ensureDefaultComponents(selection.entity);
                objectProps = objectPropertiesStore.objects[selection.entity];
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
                        <label className="block text-xs text-gray-400 mb-2">Attached Scripts</label>
                        <div 
                          className="min-h-[80px] border-2 border-dashed border-gray-600 rounded-lg p-3 text-center transition-colors hover:border-blue-500 hover:bg-blue-900/10"
                          onDragOver={(e) => {
                            e.preventDefault();
                            e.currentTarget.classList.add('border-blue-500', 'bg-blue-900/20');
                          }}
                          onDragLeave={(e) => {
                            e.currentTarget.classList.remove('border-blue-500', 'bg-blue-900/20');
                          }}
                          onDrop={(e) => {
                            e.preventDefault();
                            e.currentTarget.classList.remove('border-blue-500', 'bg-blue-900/20');
                            
                            const droppedData = e.dataTransfer.getData('text/plain');
                            try {
                              const data = JSON.parse(droppedData);
                              if (data.type === 'asset' && data.fileType === 'script') {
                                // Check if file extension is .js, .jsx, .ts, or .tsx
                                const validExtensions = ['.js', '.jsx', '.ts', '.tsx'];
                                const fileExt = data.name.substring(data.name.lastIndexOf('.')).toLowerCase();
                                
                                if (validExtensions.includes(fileExt)) {
                                  // Initialize scripts array if it doesn't exist
                                  if (!objectProps.scripts) {
                                    objectPropertiesActions.addPropertySection(selection.entity, 'scripts', []);
                                  }
                                  
                                  // Add script to the array
                                  const currentScripts = objectProps.scripts || [];
                                  if (!currentScripts.find(s => s.path === data.path)) {
                                    const newScripts = [...currentScripts, { 
                                      path: data.path, 
                                      name: data.name,
                                      enabled: true 
                                    }];
                                    updateObjectProperty(selection.entity, 'scripts', newScripts);
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
                              <div className="text-gray-500 text-xs">
                                <p>Drop script files here</p>
                                <p className="text-gray-600 mt-1">(.js, .jsx, .ts, .tsx)</p>
                              </div>
                            }
                          >
                            <div className="space-y-2">
                              <For each={objectProps.scripts}>
                                {(script, index) => (
                                  <div className="flex items-center justify-between bg-gray-800 rounded px-2 py-1">
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
                                      <IconCode className="w-3 h-3 text-gray-400" />
                                      <span className="text-xs text-gray-300">{script.name}</span>
                                    </div>
                                    <button
                                      onClick={() => {
                                        const updatedScripts = objectProps.scripts.filter((_, i) => i !== index());
                                        updateObjectProperty(selection.entity, 'scripts', updatedScripts);
                                      }}
                                      className="p-0.5 hover:bg-gray-700 rounded"
                                    >
                                      <IconX className="w-3 h-3 text-gray-400 hover:text-red-400" />
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
                      <div className="text-gray-500 text-sm mb-2">
                        No properties configured
                      </div>
                      <div className="text-gray-600 text-xs">
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