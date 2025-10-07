import { createSignal, onCleanup, onMount, For, Show } from 'solid-js';
import { IconBox, IconBulb, IconChairDirector, IconFolder, IconFolderOpen, IconCircle, IconEye, IconEyeOff, IconTrash, IconVideo, IconChevronRight, IconChevronDown, IconMountain, IconSphere } from '@tabler/icons-solidjs';
import { editorStore, editorActions } from '@/layout/stores/EditorStore';
import { viewportActions, viewportStore } from '@/layout/stores/ViewportStore';
import { Color3 } from '@babylonjs/core/Maths/math.color';
import { renderStore, renderActions, setRenderStore } from '@/render/store';


function Scene(props) {
  const { ui, selection } = editorStore;
  const bottomPanelHeight = () => ui.scenePropertiesHeight;
  const { setScenePropertiesHeight: setBottomPanelHeight } = editorActions;
  const [_isResizing, setIsResizing] = createSignal(false);
  const { selectEntity: setSelectedEntity } = editorActions;
  const { addViewportTab: _addViewportTab, setActiveViewportTab: _setActiveViewportTab } = viewportActions;
  const _tabs = () => viewportStore.tabs;
  
  // Ref for the main container
  let containerRef = null;
  
  // Helper function to update render store with multi-selection
  const updateRenderStoreSelection = (primaryEntityId, selectedEntityIds) => {
    const scene = renderStore.scene;
    if (!scene) return;

    // Convert entity IDs to Babylon objects
    const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
    const selectedObjects = [];
    let primaryObject = null;

    // Handle scene root selection
    if (primaryEntityId === 'scene-root' || primaryEntityId === scene.uniqueId) {
      primaryObject = scene;
      selectedObjects.push(scene);
    } else {
      // Find primary object
      primaryObject = allObjects.find(obj => 
        (obj.uniqueId || obj.name) === primaryEntityId
      );
      
      // Find all selected objects
      for (const entityId of selectedEntityIds) {
        if (entityId === 'scene-root' || entityId === scene.uniqueId) {
          selectedObjects.push(scene);
        } else {
          const babylonObject = allObjects.find(obj => 
            (obj.uniqueId || obj.name) === entityId
          );
          if (babylonObject) {
            selectedObjects.push(babylonObject);
          }
        }
      }
    }

    // Update render store with multi-selection
    if (primaryObject && selectedObjects.length > 0) {
      console.log('🎯 Updating render store with multi-selection:', {
        primaryObject: primaryObject.name,
        selectedCount: selectedObjects.length,
        selectedNames: selectedObjects.map(obj => obj.name)
      });
      
      // Set both selectedObject and selectedObjects manually
      setRenderStore('selectedObject', primaryObject);
      setRenderStore('selectedObjects', selectedObjects);
      
      // Apply highlighting using a persistent function that can be re-called
      applyPersistentHighlighting(selectedObjects, primaryObject);
    }
  };
  
  // Set up persistent highlighting monitoring
  onMount(() => {
    let highlightingTimer;
    
    // Monitor for highlighting being cleared and re-apply if needed
    const monitorHighlighting = () => {
      const persistentState = renderStore._persistentHighlighting;
      const highlightLayer = renderStore.highlightLayer;
      const currentSelection = selection;
      
      if (persistentState && highlightLayer) {
        // Check if the current selection has changed from the persistent state
        const currentSelectedIds = currentSelection.entities || [];
        const persistentIds = persistentState.selectedObjects.map(obj => obj.uniqueId || obj.name);
        
        // Compare current selection with persistent selection
        const selectionChanged = 
          currentSelectedIds.length !== persistentIds.length ||
          !currentSelectedIds.every(id => persistentIds.includes(id)) ||
          currentSelection.entity !== (persistentState.primaryObject?.uniqueId || persistentState.primaryObject?.name);
        
        if (selectionChanged) {
          console.log('🧹 Clearing persistent highlighting - selection changed:', {
            currentIds: currentSelectedIds,
            persistentIds: persistentIds,
            currentPrimary: currentSelection.entity,
            persistentPrimary: persistentState.primaryObject?.uniqueId || persistentState.primaryObject?.name
          });
          
          // Clear persistent state and let normal selection handling take over
          setRenderStore('_persistentHighlighting', null);
          return;
        }
        
        // Only re-apply highlighting if selection hasn't changed and we have multi-selection
        if (persistentState.selectedObjects.length > 1) {
          // Check if highlighting has been cleared (no meshes in highlight layer)
          const hasHighlights = highlightLayer._meshes && highlightLayer._meshes.size > 0;
          
          if (!hasHighlights) {
            console.log('🔄 Re-applying cleared highlighting for folder selection');
            // Re-apply the highlighting
            applyPersistentHighlighting(persistentState.selectedObjects, persistentState.primaryObject);
          }
        }
      }
    };
    
    // Check every 100ms for cleared highlighting
    highlightingTimer = setInterval(monitorHighlighting, 100);
    
    onCleanup(() => {
      if (highlightingTimer) {
        clearInterval(highlightingTimer);
      }
    });
  });

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
    
    // Context menu event listeners
    const handleContextMenuCreateFolder = () => {
      handleCreateFolder();
    };
    
    const handleContextMenuRename = (e) => {
      const { itemId } = e.detail;
      const hierarchyItem = findItemInHierarchy(itemId, hierarchyData());
      if (hierarchyItem) {
        startRename(itemId, hierarchyItem.name);
      }
    };
    
    const handleContextMenuAddToNewFolder = (e) => {
      const { itemId: _itemId } = e.detail;
      // Create a new folder and move the item to it
      handleCreateFolder();
      // Note: This would need additional logic to move the item after folder creation
    };
    
    const handleContextMenuColorCode = (e) => {
      const { itemId, color } = e.detail;
      console.log('🎨 Scene: Setting color code for item:', itemId, 'color:', color);
      setItemColors(prev => {
        const updated = { ...prev };
        if (color === null) {
          delete updated[itemId];
        } else {
          updated[itemId] = color;
        }
        console.log('🎨 Scene: Updated itemColors:', updated);
        return updated;
      });
      
      // Mark scene as modified
      import('@/api/scene/SceneManager.js').then(({ sceneManager }) => {
        sceneManager.markAsModified();
      });
    };

    const handleGetSceneColorCodes = (_e) => {
      console.log('🔄 Scene: Received request for color codes, current itemColors:', itemColors());
      // Respond with current color codes
      const response = new CustomEvent('sceneColorCodesResponse', {
        detail: { colorCodes: itemColors() }
      });
      document.dispatchEvent(response);
    };


    const handleRestoreSceneColorCodes = (e) => {
      const { colorCodes } = e.detail;
      console.log('🔄 Scene: Restoring color codes:', colorCodes);
      console.log('🔄 Scene: Current itemColors before restore:', itemColors());
      
      if (colorCodes && Object.keys(colorCodes).length > 0) {
        setItemColors(colorCodes);
        console.log('🔄 Scene: itemColors after restore:', itemColors());
        
        // Force a re-render by updating the signal
        setTimeout(() => {
          console.log('🔄 Scene: Final itemColors state:', itemColors());
        }, 100);
      } else {
        console.log('🔄 Scene: No color codes to restore or empty object');
      }
    };

    const handleContextMenuDeleteVirtualFolder = (e) => {
      const { itemId } = e.detail;
      // Find the virtual folder in hierarchy
      const hierarchy = hierarchyData();
      const findVirtualFolderInHierarchy = (items, id) => {
        for (const item of items) {
          if (item.id === id) return item;
          if (item.children) {
            const found = findVirtualFolderInHierarchy(item.children, id);
            if (found) return found;
          }
        }
        return null;
      };
      
      const folderToDelete = findVirtualFolderInHierarchy(hierarchy, itemId);
      if (folderToDelete) {
        handleDeleteVirtualFolder(folderToDelete);
      }
    };
    
    window.addEventListener('resize', handleWindowResize);
    document.addEventListener('contextMenuCreateFolder', handleContextMenuCreateFolder);
    document.addEventListener('contextMenuRename', handleContextMenuRename);
    document.addEventListener('contextMenuAddToNewFolder', handleContextMenuAddToNewFolder);
    document.addEventListener('contextMenuColorCode', handleContextMenuColorCode);
    document.addEventListener('contextMenuDeleteVirtualFolder', handleContextMenuDeleteVirtualFolder);
    document.addEventListener('getSceneColorCodes', handleGetSceneColorCodes);
    document.addEventListener('restoreSceneColorCodes', handleRestoreSceneColorCodes);
    
    // Global keyboard shortcuts
    const handleGlobalKeyDown = (e) => {
      // Only handle if focus is in the scene panel or no input is focused
      const focusedElement = document.activeElement;
      const isInputFocused = focusedElement && (
        focusedElement.tagName === 'INPUT' || 
        focusedElement.tagName === 'TEXTAREA' ||
        focusedElement.contentEditable === 'true'
      );
      
      if (!isInputFocused) {
        if (e.key === 'a' && (e.ctrlKey || e.metaKey)) {
          e.preventDefault();
          handleSelectAll();
        } else if (e.key === 'Delete' || e.key === 'Backspace') {
          e.preventDefault();
          handleDeleteSelected();
        }
      }
    };
    
    document.addEventListener('keydown', handleGlobalKeyDown);
    
    // Debug: Watch for any other selection changes
    const _handleSelectionChange = () => {
      console.log('Selection changed externally:', {
        entity: selection.entity,
        entities: selection.entities
      });
    };
    
    // This won't work as editorStore isn't directly observable
    // but let's see if we can detect changes another way
    
    onCleanup(() => {
      window.removeEventListener('resize', handleWindowResize);
      document.removeEventListener('contextMenuCreateFolder', handleContextMenuCreateFolder);
      document.removeEventListener('contextMenuRename', handleContextMenuRename);
      document.removeEventListener('contextMenuAddToNewFolder', handleContextMenuAddToNewFolder);
      document.removeEventListener('contextMenuColorCode', handleContextMenuColorCode);
      document.removeEventListener('contextMenuDeleteVirtualFolder', handleContextMenuDeleteVirtualFolder);
      document.removeEventListener('getSceneColorCodes', handleGetSceneColorCodes);
      document.removeEventListener('restoreSceneColorCodes', handleRestoreSceneColorCodes);
      document.removeEventListener('keydown', handleGlobalKeyDown);
    });
    
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
  const [itemColors, setItemColors] = createSignal({});
  
  // Global counter for alternating backgrounds
  let globalRowCounter = { value: 0 };
  
  // Use hierarchy from render store
  const hierarchyData = () => renderStore.hierarchy;


  // Inline drag and drop state and handlers
  const [draggedItem, setDraggedItem] = createSignal(null);
  const [dragOverItem, setDragOverItem] = createSignal(null);
  const [dropPosition, setDropPosition] = createSignal(null);

  const handleDragStart = (e, item) => {
    // Check if this item is part of a multi-selection
    const selectedItems = selection.entities || [];
    const isMultiSelection = selectedItems.length > 1 && selectedItems.includes(item.id);
    
    if (isMultiSelection) {
      // Dragging multiple items
      setDraggedItem({ 
        ...item, 
        isMultiDrag: true, 
        draggedIds: selectedItems 
      });
      
      // Create safe items for all selected items
      const hierarchy = hierarchyData();
      const safeItems = selectedItems.map(id => {
        const foundItem = findItemInHierarchy(id, hierarchy);
        return foundItem ? {
          id: foundItem.id,
          name: foundItem.name,
          type: foundItem.type,
          lightType: foundItem.lightType,
          visible: foundItem.visible
        } : null;
      }).filter(Boolean);
      
      e.dataTransfer.setData('text/plain', JSON.stringify({ 
        type: 'scene-items-multi', 
        items: safeItems,
        count: safeItems.length
      }));
      
      console.log(`Multi-drag started: ${safeItems.length} items`);
    } else {
      // Single item drag (existing logic)
      setDraggedItem(item);
      const safeItem = {
        id: item.id,
        name: item.name,
        type: item.type,
        lightType: item.lightType,
        visible: item.visible
      };
      e.dataTransfer.setData('text/plain', JSON.stringify({ type: 'scene-item', item: safeItem }));
    }
    
    e.dataTransfer.effectAllowed = 'move';
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

    // Only allow "inside" drop for folders, force "below" for non-folders
    if (position === 'inside' && item.type !== 'folder') {
      position = 'below';
    }

    setDragOverItem(item);
    setDropPosition(position);
  };

  const handleDrop = (e, targetItem) => {
    e.preventDefault();
    const draggedData = draggedItem();
    
    if (!draggedData) {
      return;
    }
    
    // Handle multi-item drag
    if (draggedData.isMultiDrag && draggedData.draggedIds) {
      console.log('Multi-drop detected:', { 
        draggedIds: draggedData.draggedIds, 
        targetItem: targetItem.id 
      });
      
      // Don't allow dropping if target is one of the dragged items
      if (draggedData.draggedIds.includes(targetItem.id)) {
        console.log('Cannot drop items onto themselves');
        return;
      }
      
      handleMultiItemDrop(draggedData.draggedIds, targetItem);
      return;
    }
    
    // Single item drop (existing logic)
    if (draggedData.id === targetItem.id) {
      return;
    }

    const scene = renderStore.scene;
    if (!scene) return;

    // Handle virtual folders separately from Babylon objects
    const isVirtualFolder = (item) => {
      return item.isVirtual ||
             (item.type === 'folder' && !item.babylonObject) ||
             (typeof item.id === 'string' && item.id.startsWith('virtual-folder-'));
    };

    // Check if we're dragging a virtual folder
    const isDraggingVirtualFolder = isVirtualFolder(draggedData);

    // Find the dragged object in Babylon scene (if it's not a virtual folder)
    const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
    const draggedBabylonObject = isDraggingVirtualFolder ? null : allObjects.find(obj => 
      (obj.uniqueId || obj.name) === draggedData.id
    );
    const targetBabylonObject = allObjects.find(obj => 
      (obj.uniqueId || obj.name) === targetItem.id
    );

    // Exit early only if we're not dragging a virtual folder and can't find the Babylon object
    if (!isDraggingVirtualFolder && !draggedBabylonObject) return;

    const position = dropPosition();
    
    // Only allow dropping inside folders
    if (position === 'inside' && targetItem.type !== 'folder') {
      console.warn('Cannot drop inside non-folder objects');
      return;
    }

    // Only update Babylon parent relationships for actual Babylon objects (not virtual folders)
    if (!isDraggingVirtualFolder && position === 'inside' && targetItem.type === 'folder') {
      if (isVirtualFolder(targetItem)) {
        // For virtual folders, just clear the parent (move to scene root)
        draggedBabylonObject.parent = null;
      } else if (targetBabylonObject) {
        // For user-created folders (TransformNodes), we need to ensure proper parenting
        try {
          // Ensure both target and dragged objects have proper isEnabled method if they're TransformNodes
          if (targetBabylonObject.getClassName && targetBabylonObject.getClassName() === 'TransformNode') {
            // Make sure the TransformNode has the isEnabled method
            if (typeof targetBabylonObject.isEnabled !== 'function') {
              targetBabylonObject.isEnabled = () => true;
            }
          }
          
          if (draggedBabylonObject.getClassName && draggedBabylonObject.getClassName() === 'TransformNode') {
            // Make sure the dragged TransformNode also has the isEnabled method
            if (typeof draggedBabylonObject.isEnabled !== 'function') {
              draggedBabylonObject.isEnabled = () => true;
            }
          }
          
          if (targetBabylonObject.getClassName && 
              (targetBabylonObject.getClassName() === 'TransformNode' || 
               targetBabylonObject.getClassName() === 'Mesh')) {
            draggedBabylonObject.parent = targetBabylonObject;
          }
        } catch (error) {
          console.warn('Failed to set parent:', error);
          // As a fallback, try to set parent to null and update hierarchy only
          try {
            draggedBabylonObject.parent = null;
          } catch (e) {
            console.warn('Could not even clear parent:', e);
          }
        }
      }
    } else if (!isDraggingVirtualFolder && (position === 'above' || position === 'below')) {
      // Parent to same parent as target, but validate the parent first
      let newParent = null;
      
      if (isVirtualFolder(targetItem)) {
        // If target is a virtual folder, parent to scene root
        newParent = null;
      } else if (targetBabylonObject) {
        newParent = targetBabylonObject.parent || null;
      }
      
      // Only set parent if it's safe to do so
      try {
        // Ensure parent has proper isEnabled method if it's a TransformNode
        if (newParent && newParent.getClassName && newParent.getClassName() === 'TransformNode') {
          if (typeof newParent.isEnabled !== 'function') {
            newParent.isEnabled = () => true;
          }
        }
        
        // Ensure dragged object has proper isEnabled method if it's a TransformNode
        if (draggedBabylonObject.getClassName && draggedBabylonObject.getClassName() === 'TransformNode') {
          if (typeof draggedBabylonObject.isEnabled !== 'function') {
            draggedBabylonObject.isEnabled = () => true;
          }
        }
        
        if (newParent === null || 
            (newParent && newParent.getClassName && 
             (newParent.getClassName() === 'TransformNode' || 
              newParent.getClassName() === 'Mesh'))) {
          draggedBabylonObject.parent = newParent;
        }
      } catch (error) {
        console.warn('Failed to set parent:', error);
        // As a fallback, try to set parent to null
        try {
          draggedBabylonObject.parent = null;
        } catch (e) {
          console.warn('Could not even clear parent:', e);
        }
      }
    }

    // Handle UI hierarchy updates for all cases
    renderActions.reorderObjectInHierarchy(draggedData.id, targetItem.id, position);
    
    // If we dropped into a folder, make sure it's expanded to show the new child
    if (position === 'inside' && targetItem.type === 'folder') {
      setExpandedItems(prev => ({ ...prev, [targetItem.id]: true }));
    }
    
    // Hierarchy will update automatically since we're changing Babylon parent relationships
  };

  const handleMultiItemDrop = (draggedIds, targetItem) => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    const position = dropPosition();
    
    // Only allow dropping inside folders for multi-item drops
    if (position !== 'inside' || targetItem.type !== 'folder') {
      console.warn('Multi-item drop only allowed inside folders');
      return;
    }
    
    console.log('Processing multi-item drop:', { 
      count: draggedIds.length, 
      targetFolder: targetItem.id 
    });
    
    // Check if we're dropping into a virtual folder or real folder
    const isVirtualFolder = (item) => {
      return item.isVirtual ||
             (item.type === 'folder' && !item.babylonObject) ||
             (typeof item.id === 'string' && item.id.startsWith('virtual-folder-'));
    };
    
    const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
    
    // Process each dragged item
    draggedIds.forEach(draggedId => {
      const draggedBabylonObject = allObjects.find(obj => 
        (obj.uniqueId || obj.name) === draggedId
      );
      
      if (draggedBabylonObject) {
        if (isVirtualFolder(targetItem)) {
          // For virtual folders, just clear the parent (move to scene root)
          draggedBabylonObject.parent = null;
        } else {
          // For real folders, set the parent
          const targetBabylonObject = allObjects.find(obj => 
            (obj.uniqueId || obj.name) === targetItem.id
          );
          
          if (targetBabylonObject) {
            try {
              // Ensure target has proper isEnabled method
              if (targetBabylonObject.getClassName && targetBabylonObject.getClassName() === 'TransformNode') {
                if (typeof targetBabylonObject.isEnabled !== 'function') {
                  targetBabylonObject.isEnabled = () => true;
                }
              }
              
              // Ensure dragged object has proper isEnabled method
              if (draggedBabylonObject.getClassName && draggedBabylonObject.getClassName() === 'TransformNode') {
                if (typeof draggedBabylonObject.isEnabled !== 'function') {
                  draggedBabylonObject.isEnabled = () => true;
                }
              }
              
              draggedBabylonObject.parent = targetBabylonObject;
            } catch (error) {
              console.warn('Failed to set parent for multi-item:', error);
              draggedBabylonObject.parent = null;
            }
          }
        }
      }
      
      // Update UI hierarchy for each item
      renderActions.reorderObjectInHierarchy(draggedId, targetItem.id, position);
    });
    
    // Expand the target folder to show the new children
    setExpandedItems(prev => ({ ...prev, [targetItem.id]: true }));
    
    console.log(`Successfully moved ${draggedIds.length} items to ${targetItem.name}`);
  };

  const handleDragEnd = (_e) => {
    setDraggedItem(null);
    setDragOverItem(null);
    setDropPosition(null);
  };


  const handleDropWithAnimation = (e, item) => {
    handleDrop(e, item);
    const draggedData = draggedItem();
    if (draggedData) {
      setDroppedItemId(draggedData.id);
      setTimeout(() => setDroppedItemId(null), 500);
    }
  };


  const _expandAll = () => {
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
  
  const _collapseAll = () => {
    setExpandedItems({});
  };
  
  const startRename = (itemId, currentName) => {
    setRenamingItemId(itemId);
    setRenameValue(currentName);
  };
  
  const confirmRename = () => {
    if (renamingItemId() && renameValue().trim()) {
      const newName = renameValue().trim();
      
      // Check if this is a virtual folder
      if (typeof renamingItemId() === 'string' && renamingItemId().startsWith('virtual-folder-')) {
        // For virtual folders, just update the hierarchy
        renderActions.updateObjectName(renamingItemId(), newName);
      } else {
        // For Babylon objects, update both the object and hierarchy
        const scene = renderStore.scene;
        if (scene) {
          const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
          const objectToRename = allObjects.find(obj => 
            (obj.uniqueId || obj.name) === renamingItemId()
          );
          
          if (objectToRename) {
            objectToRename.name = newName;
            // Update only this object's name in the hierarchy (much more efficient)
            renderActions.updateObjectName(renamingItemId(), newName);
          }
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
  
  const findItemInHierarchy = (itemId, hierarchy) => {
    for (const item of hierarchy) {
      if (item.id === itemId) {
        return item;
      }
      if (item.children) {
        const found = findItemInHierarchy(itemId, item.children);
        if (found) return found;
      }
    }
    return null;
  };

  // Helper function to get all items in hierarchy in display order (only visible/expanded items)
  const getAllItemsInOrder = (hierarchy) => {
    const items = [];
    
    const traverse = (nodes, depth = 0) => {
      for (const node of nodes) {
        items.push(node.id);
        if (node.children && node.children.length > 0 && expandedItems()[node.id]) {
          traverse(node.children, depth + 1);
        }
      }
    };
    
    traverse(hierarchy);
    return items;
  };

  // Helper function to get ALL items in hierarchy regardless of expansion state (for range selection)
  const getAllItemsFlat = (hierarchy) => {
    const items = [];
    
    const traverse = (nodes) => {
      for (const node of nodes) {
        items.push(node.id);
        if (node.children && node.children.length > 0) {
          traverse(node.children);
        }
      }
    };
    
    traverse(hierarchy);
    return items;
  };

  // Helper function to get selection range between two items
  const getSelectionRange = (hierarchy, fromId, toId) => {
    const allItems = getAllItemsFlat(hierarchy);
    const fromIndex = allItems.indexOf(fromId);
    const toIndex = allItems.indexOf(toId);
    
    if (fromIndex === -1 || toIndex === -1) {
      return [toId]; // Fallback to just the clicked item
    }
    
    const startIndex = Math.min(fromIndex, toIndex);
    const endIndex = Math.max(fromIndex, toIndex);
    return allItems.slice(startIndex, endIndex + 1);
  };

  // Helper function to get all object IDs within a folder (recursively)
  const getAllObjectsInFolder = (folderItem, _hierarchy) => {
    const objectIds = [];
    
    const collectObjects = (items) => {
      for (const item of items) {
        if (item.type === 'folder') {
          // Recursively collect from subfolders
          if (item.children) {
            collectObjects(item.children);
          }
        } else {
          // Add non-folder items (meshes, lights, cameras, etc.)
          objectIds.push(item.id);
        }
      }
    };
    
    // Start collecting from the folder's children
    if (folderItem.children) {
      collectObjects(folderItem.children);
    }
    
    return objectIds;
  };

  // Persistent highlighting function that can be re-called during transforms
  const applyPersistentHighlighting = (selectedObjects, primaryObject) => {
    const _scene = renderStore.scene;
    const gizmoManager = renderStore.gizmoManager;
    const highlightLayer = renderStore.highlightLayer;
    
    console.log('🎯 Applying persistent highlighting:', {
      selectedCount: selectedObjects.length,
      primaryObject: primaryObject?.name
    });
    
    // Clear previous highlights
    if (highlightLayer) {
      highlightLayer.removeAllMeshes();
    }
    
    // Highlight all selected objects
    if (highlightLayer && selectedObjects.length > 0) {
      selectedObjects.forEach((selectedObj) => {
        try {
          if (selectedObj.getClassName && selectedObj.getClassName() === 'Mesh') {
            highlightLayer.addMesh(selectedObj, new Color3(0.3, 0.6, 1.0));
          } else if (selectedObj.getChildMeshes) {
            const childMeshes = selectedObj.getChildMeshes();
            childMeshes.forEach(mesh => {
              if (mesh.getClassName() === 'Mesh' && mesh.isVisible) {
                highlightLayer.addMesh(mesh, new Color3(0.3, 0.6, 1.0));
              }
            });
          }
        } catch (error) {
          console.warn('Failed to highlight object:', selectedObj.name, error);
        }
      });
    }
    
    // Attach gizmo to primary object
    if (gizmoManager && primaryObject) {
      gizmoManager.attachToMesh(primaryObject);
    }
    
    // Store highlighting state for re-application
    setRenderStore('_persistentHighlighting', {
      selectedObjects,
      primaryObject,
      timestamp: Date.now()
    });
  };

  const handleCreateFolder = () => {
    const scene = renderStore.scene;
    if (!scene) return;
    
    const folderName = `New Folder ${folderCounter()}`;
    
    // All user-created folders are virtual organizational folders
    const virtualFolder = {
      id: `virtual-folder-${Date.now()}`,
      name: folderName,
      type: 'folder',
      visible: true,
      expanded: true,
      children: [],
      isVirtual: true
    };
    
    // Add virtual folder directly to hierarchy
    renderActions.addVirtualFolder(virtualFolder);
    
    setFolderCounter(prev => prev + 1);
    setSelectedEntity(virtualFolder.id);
    setTimeout(() => startRename(virtualFolder.id, folderName), 100);
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
    } else if (e.key === 'a' && (e.ctrlKey || e.metaKey) && !renamingItemId()) {
      e.preventDefault();
      handleSelectAll();
    } else if (e.key === 'Delete' && !renamingItemId()) {
      e.preventDefault();
      handleDeleteSelected();
    }
  };

  const handleSelectAll = () => {
    const allItems = getAllItemsInOrder(hierarchyData());
    if (allItems.length > 0) {
      setSelectedEntity(allItems[allItems.length - 1], allItems);
    }
  };

  const handleDeleteSelected = () => {
    const selectedItems = selection.entities || [];
    if (selectedItems.length === 0) return;

    const scene = renderStore.scene;
    if (!scene) return;

    const allObjects = [...scene.meshes, ...scene.transformNodes, ...scene.lights, ...scene.cameras];
    
    // Check if any cameras would be deleted
    const camerasToDelete = selectedItems.filter(id => {
      const obj = allObjects.find(o => (o.uniqueId || o.name) === id);
      return obj && obj.getClassName && obj.getClassName().includes('Camera');
    });

    // Prevent deleting all cameras
    if (camerasToDelete.length > 0 && scene.cameras.length <= camerasToDelete.length) {
      editorActions.addConsoleMessage('Cannot delete all cameras! At least one camera is required for rendering.', 'error');
      return;
    }

    // Delete all selected objects
    selectedItems.forEach(id => {
      const objectToDelete = allObjects.find(obj => (obj.uniqueId || obj.name) === id);
      if (objectToDelete) {
        renderActions.removeObject(objectToDelete);
      }
    });

    // Clear selection
    setSelectedEntity(null, []);
    
    // Ensure there's still an active camera if cameras were deleted
    if (camerasToDelete.length > 0 && scene) {
      setTimeout(() => {
        if (scene.cameras.length > 0 && !scene.activeCamera) {
          scene.activeCamera = scene.cameras[0];
          scene._camera = scene.cameras[0];
          scene.cameras[0].attachControl(scene.getEngine().getRenderingCanvas(), true);
          editorActions.addConsoleMessage(`Switched to camera: ${scene.cameras[0].name}`, 'info');
        }
      }, 100);
    }
  };

  const _handleMouseDown = (e) => {
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
    
    const isVirtualFolder = (item) => {
      return item.isVirtual ||
             (item.type === 'folder' && !item.babylonObject) ||
             (typeof item.id === 'string' && item.id.startsWith('virtual-folder-'));
    };
    
    if (isVirtualFolder(item)) {
      // Handle virtual folder deletion
      handleDeleteVirtualFolder(item);
    } else if (item.babylonObject && item.babylonObject.dispose) {
      const scene = renderStore.scene;
      const isCamera = item.babylonObject.getClassName && item.babylonObject.getClassName().includes('Camera');
      
      // Check if this is the last camera
      if (isCamera && scene && scene.cameras.length <= 1) {
        editorActions.addConsoleMessage('Cannot delete the last camera! At least one camera is required for rendering.', 'error');
        return;
      }
      
      // Detach scripts before deleting the object
      handleDetachAllScripts(item.babylonObject);
      
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

  const handleDeleteVirtualFolder = (folderItem) => {
    console.log('🗑️ Deleting virtual folder:', folderItem.name);
    
    // First, collect all objects inside this virtual folder recursively
    const collectAllObjectsInFolder = (folder, allObjects = []) => {
      if (folder.children) {
        folder.children.forEach(child => {
          if (child.isVirtual || (child.type === 'folder' && !child.babylonObject)) {
            // Recursively collect from nested virtual folders
            collectAllObjectsInFolder(child, allObjects);
          } else if (child.babylonObject) {
            // This is a real Babylon object
            allObjects.push(child);
          }
        });
      }
      return allObjects;
    };
    
    const objectsToDelete = collectAllObjectsInFolder(folderItem);
    console.log('🗑️ Found objects to delete in folder:', objectsToDelete.map(obj => obj.name));
    
    // Check if any cameras would be deleted
    const scene = renderStore.scene;
    if (scene) {
      const camerasToDelete = objectsToDelete.filter(obj => 
        obj.babylonObject?.getClassName?.()?.includes('Camera')
      );
      
      // Prevent deleting all cameras
      if (camerasToDelete.length > 0 && scene.cameras.length <= camerasToDelete.length) {
        editorActions.addConsoleMessage('Cannot delete virtual folder! It contains all remaining cameras. At least one camera is required for rendering.', 'error');
        return;
      }
    }
    
    // Delete all objects in the folder (with script cleanup)
    objectsToDelete.forEach(obj => {
      if (obj.babylonObject) {
        console.log('🗑️ Deleting object from folder:', obj.name);
        
        // Detach scripts before deleting
        handleDetachAllScripts(obj.babylonObject);
        
        // Remove the Babylon object
        renderActions.removeObject(obj.babylonObject);
      }
    });
    
    // Remove the virtual folder from hierarchy
    renderActions.removeVirtualFolder(folderItem.id);
    
    // Clear selection if this folder was selected
    if (selection.entity === folderItem.id) {
      setSelectedEntity(null);
    }
    
    // Ensure there's still an active camera if cameras were deleted
    if (scene && objectsToDelete.some(obj => obj.babylonObject?.getClassName?.()?.includes('Camera'))) {
      setTimeout(() => {
        if (scene.cameras.length > 0 && !scene.activeCamera) {
          scene.activeCamera = scene.cameras[0];
          scene._camera = scene.cameras[0];
          scene.cameras[0].attachControl(scene.getEngine().getRenderingCanvas(), true);
          editorActions.addConsoleMessage(`Switched to camera: ${scene.cameras[0].name}`, 'info');
        }
      }, 100);
    }
    
    editorActions.addConsoleMessage(`Deleted virtual folder "${folderItem.name}" and ${objectsToDelete.length} objects`, 'info');
  };

  const handleDetachAllScripts = async (babylonObject) => {
    try {
      const objectId = babylonObject.uniqueId || babylonObject.name;
      
      // Get script runtime to detach scripts
      const { getScriptRuntime } = await import('@/api/script');
      const runtime = getScriptRuntime();
      
      if (runtime && runtime.detachAllScriptsFromObject) {
        await runtime.detachAllScriptsFromObject(objectId);
        console.log('🗑️ Detached all scripts from object:', babylonObject.name);
      }
      
      // Also clean up objectProperties store
      const { objectPropertiesActions } = await import('@/layout/stores/ViewportStore.jsx');
      if (objectPropertiesActions.removeObject) {
        objectPropertiesActions.removeObject(objectId);
        console.log('🗑️ Cleaned up object properties for:', babylonObject.name);
      }
    } catch (error) {
      console.warn('⚠️ Failed to detach scripts from object:', babylonObject.name, error);
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
      case 'terrain': return IconMountain;
      case 'skybox': return IconSphere;
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
      case 'terrain': return '#84cc16'; // lime-500 (earthy green)
      case 'skybox': return '#38bdf8'; // sky-400 (sky blue)
      default: return '#9ca3af'; // gray-400
    }
  };

  const renderSceneItem = (item, depth = 0, index = 0, parent = null, globalCounter = { value: 0 }) => {
    if (!item) return null;
    
    const currentIndex = globalCounter.value++;
    
    const isSelected = () => {
      // Check if this item is in the multi-selection
      const entities = selection.entities || [];
      return entities.includes(item.id) || selection.entity === item.id;
    };
    
    const isPrimarySelection = () => {
      // Check if this is the primary selected item (last clicked)
      return selection.entity === item.id;
    };
    
    // Check if this item is a child of the selected folder
    const isChildOfSelectedFolder = () => {
      if (!parent || !selection.entity) return false;
      return parent.id === selection.entity;
    };

    // Check if this item will be a sibling when drag completes
    const isInTargetFolder = () => {
      const dragOver = dragOverItem();
      const dropPos = dropPosition();
      
      if (!dragOver || !dropPos || !parent) return false;
      
      // If dropping inside a folder, highlight all current children of that folder
      if (dropPos === 'inside' && dragOver.type === 'folder') {
        return parent.id === dragOver.id;
      }
      
      return false;
    };
    
    const hasChildren = item.children && item.children.length > 0;
    const isExpanded = () => expandedItems().hasOwnProperty(item.id) ? expandedItems()[item.id] : (item.expanded || false);
    const Icon = getIcon(item.type, item.lightType, hasChildren, isExpanded());
    const iconColor = getIconColor(item.type, item.lightType);
    
    // Scene root is always visible and active
    const isSceneRoot = item.type === 'scene';
    const itemVisible = isSceneRoot ? true : (item.visible !== undefined ? item.visible : true);
    
    const isDraggedOver = () => dragOverItem()?.id === item.id;
    const isFolderDrop = () => isDraggedOver() && dropPosition() === 'inside' && item.type === 'folder';
    const isInvalidDrop = () => isDraggedOver() && dropPosition() === 'inside' && item.type !== 'folder';

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
          className={`group flex items-center py-0.5 pr-2 text-xs cursor-pointer transition-colors relative overflow-hidden focus:outline-none ${
            isPrimarySelection() 
              ? 'bg-primary/80 text-primary-content' 
              : isSelected()
                ? 'bg-primary/50 text-primary-content'
                : 'text-base-content/70 hover:bg-primary/20 hover:text-base-content'
          } ${
            draggedItem()?.id === item.id ? 'opacity-30' : 
            (draggedItem()?.isMultiDrag && draggedItem()?.draggedIds?.includes(item.id)) ? 'opacity-30' : ''
          } ${
            isFolderDrop() ? 'bg-primary/20' : ''
          } ${
            isInvalidDrop() ? 'border-2 border-error bg-error/10' : ''
          } ${
            isInTargetFolder() ? 'bg-yellow-200/30' : ''
          } ${
            droppedItemId() === item.id ? 'bg-success/50' : ''
          }`}
          style={{ 
            'padding-left': `${6 + depth * 24}px`,
            cursor: 'pointer'
          }}
          draggable={!props.isResizing}
          onDragStart={(e) => !props.isResizing && handleDragStart(e, item)}
          onDragOver={(e) => !props.isResizing && handleDragOver(e, item)}
          onDrop={(e) => !props.isResizing && handleDropWithAnimation(e, item)}
          onDragEnd={handleDragEnd}
          tabIndex={0}
          onKeyDown={(e) => handleKeyDown(e, item)}
          onClick={(e) => {
            if (renamingItemId() !== item.id) {
              // Handle multi-selection with Ctrl+click and Shift+click
              const isCtrlClick = e.ctrlKey || e.metaKey; // Support both Ctrl and Cmd (Mac)
              const isShiftClick = e.shiftKey;
              
              console.log('CLICK EVENT:', {
                itemId: item.id,
                ctrlKey: e.ctrlKey,
                metaKey: e.metaKey,
                shiftKey: e.shiftKey,
                isCtrlClick,
                isShiftClick
              });
              
              if (isCtrlClick) {
                // Ctrl+click: Toggle selection of this item
                const currentSelection = selection.entities || [];
                const isAlreadySelected = currentSelection.includes(item.id);
                
                console.log('CTRL+CLICK DEBUG:', {
                  itemId: item.id,
                  currentSelection,
                  currentSelectionLength: currentSelection.length,
                  currentSelectionItems: [...currentSelection],
                  isAlreadySelected,
                  selectionEntity: selection.entity,
                  selectionEntities: selection.entities,
                  selectionEntitiesLength: (selection.entities || []).length
                });
                
                if (isAlreadySelected) {
                  // Remove from selection
                  const newSelection = currentSelection.filter(id => id !== item.id);
                  const newPrimary = newSelection.length > 0 ? newSelection[newSelection.length - 1] : null;
                  console.log('REMOVING from selection:', { newSelection, newPrimary });
                  setSelectedEntity(newPrimary, newSelection);
                  
                  // Update render store with multi-selection
                  updateRenderStoreSelection(newPrimary, newSelection);
                } else {
                  // Add to selection
                  const newSelection = [...currentSelection, item.id];
                  console.log('ADDING to selection:', { newSelection, newPrimary: item.id });
                  setSelectedEntity(item.id, newSelection);
                  
                  // Update render store with multi-selection
                  updateRenderStoreSelection(item.id, newSelection);
                  
                  // Clear persistent highlighting state for manual multi-selection
                  setRenderStore('_persistentHighlighting', null);
                  
                  // Check if the state actually changed
                  setTimeout(() => {
                    console.log('After setSelectedEntity:', {
                      entities: selection.entities,
                      entity: selection.entity
                    });
                  }, 10);
                }
                
              } else if (isShiftClick) {
                // Shift+click: Select range from last selected to this item
                const currentSelection = selection.entities || [];
                const hierarchy = hierarchyData();
                
                console.log('SHIFT+CLICK DEBUG:', {
                  itemId: item.id,
                  currentSelection,
                  currentSelectionLength: currentSelection.length,
                  currentSelectionItems: [...currentSelection],
                  selectionEntity: selection.entity
                });
                
                if (currentSelection.length > 0) {
                  const lastSelected = selection.entity || currentSelection[currentSelection.length - 1];
                  const rangeSelection = getSelectionRange(hierarchy, lastSelected, item.id);
                  console.log('SHIFT RANGE SELECTION:', { 
                    from: lastSelected, 
                    to: item.id, 
                    rangeSelection 
                  });
                  setSelectedEntity(item.id, rangeSelection);
                  
                  // Update render store with multi-selection
                  updateRenderStoreSelection(item.id, rangeSelection);
                  
                  // Clear persistent highlighting state for manual range selection
                  setRenderStore('_persistentHighlighting', null);
                } else {
                  // No previous selection, just select this item
                  console.log('SHIFT FALLBACK: No previous selection');
                  setSelectedEntity(item.id, [item.id]);
                  
                  // Update render store with single selection
                  updateRenderStoreSelection(item.id, [item.id]);
                }
                
              } else {
                // Normal click: Check if it's a folder
                if (item.type === 'folder') {
                  // Folder click: Select all objects within the folder
                  const hierarchy = hierarchyData();
                  const objectsInFolder = getAllObjectsInFolder(item, hierarchy);
                  
                  console.log('FOLDER CLICK:', {
                    folderName: item.name,
                    objectsInFolder: objectsInFolder
                  });
                  
                  if (objectsInFolder.length > 0) {
                    // Select the first object as primary, all as multi-selection
                    const primaryObjectId = objectsInFolder[0];
                    setSelectedEntity(primaryObjectId, objectsInFolder);
                    
                    // Update render store with folder's contents
                    updateRenderStoreSelection(primaryObjectId, objectsInFolder);
                  } else {
                    // Empty folder - just select the folder itself
                    setSelectedEntity(item.id, [item.id]);
                  }
                } else {
                  // Normal object click: Single selection
                  setSelectedEntity(item.id, [item.id]);
                  
                  // Clear persistent highlighting state for single selections
                  setRenderStore('_persistentHighlighting', null);
                  
                  // Only call renderActions.selectObjectById for single selections
                  const success = renderActions.selectObjectById(item.id);
                  if (!success) {
                    // Fallback for non-Babylon objects like folders
                  }
                }
              }
              
              // Only call onObjectSelect for single selections to avoid triggering render store override
              if (props.onObjectSelect && !isCtrlClick && !isShiftClick) {
                props.onObjectSelect(item.id);
              }
              // Note: Removed automatic toggling - now handled by chevron button
            }
          }}
          onContextMenu={(e) => {
            props.onContextMenu(e, item, 'scene');
          }}
        >
          <Show when={isPrimarySelection()}>
            <div className="absolute left-0 top-0 bottom-0 w-0.5 bg-primary pointer-events-none" />
          </Show>
          
          <Show when={isSelected() && !isPrimarySelection()}>
            <div className="absolute left-0 top-0 bottom-0 w-0.5 bg-primary/60 pointer-events-none" />
          </Show>
          
          <Show when={depth > 0}>
            <div className="absolute left-0 top-0 bottom-0 pointer-events-none">
              <div
                className={`absolute top-0 bottom-0 w-px ${isChildOfSelectedFolder() ? 'bg-yellow-400/70' : 'bg-base-content/30'}`}
                style={{ left: `${6 + (depth - 1) * 24 + 8}px` }}
              />
              <div
                className={`absolute top-1/2 w-2 h-px ${isChildOfSelectedFolder() ? 'bg-yellow-400/70' : 'bg-base-content/30'}`}
                style={{ left: `${6 + (depth - 1) * 24 + 8}px` }}
              />
            </div>
          </Show>
          
          <div class="relative flex items-center">
            {/* Chevron for expandable items */}
            <Show when={hasChildren}>
              <button
                className="w-4 h-4 mr-0.5 flex items-center justify-center hover:bg-base-content/10 rounded transition-colors"
                onClick={(e) => {
                  e.stopPropagation();
                  setExpandedItems(prev => ({ ...prev, [item.id]: !isExpanded() }));
                }}
                title={isExpanded() ? 'Collapse' : 'Expand'}
              >
                {isExpanded() ? (
                  <IconChevronDown class="w-3 h-3 text-base-content/60" />
                ) : (
                  <IconChevronRight class="w-3 h-3 text-base-content/60" />
                )}
              </button>
            </Show>
            
            <Icon 
              class="w-4 h-4 mr-1 cursor-pointer hover:opacity-70 transition-opacity" 
              style={{ 
                color: iconColor,
                fill: item.type === 'folder' && hasChildren && !isExpanded() ? iconColor : 'none'
              }}
            />
            
            <Show when={itemColors()[item.id]}>
              <div 
                class="w-3 h-3 rounded-full mr-1 border border-base-300/50 flex-shrink-0" 
                style={{ 'background-color': itemColors()[item.id] }}
                title={`Color: ${itemColors()[item.id]}`}
              />
            </Show>
            
            <Show when={draggedItem()?.isMultiDrag && draggedItem()?.id === item.id}>
              <div 
                class="w-4 h-4 rounded-full bg-primary text-primary-content text-xs font-bold flex items-center justify-center mr-1 flex-shrink-0" 
                title={`Dragging ${draggedItem()?.draggedIds?.length || 0} items`}
              >
                {draggedItem()?.draggedIds?.length || 0}
              </div>
            </Show>
          </div>
          
          <Show when={item.type !== 'scene'}>
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
          </Show>
          
          <Show 
            when={renamingItemId() === item.id}
            fallback={
              <span 
                className="flex-1 text-base-content/80 text-xs cursor-pointer block"
                style={{
                  "white-space": "nowrap",
                  "overflow": "hidden",
                  "text-overflow": "ellipsis",
                  "min-width": "0",
                  "max-width": "160px"
                }}
                title={item.name}
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
          
          <Show when={item.type !== 'scene'}>
            <button 
              className="ml-auto p-0.5 rounded transition-colors opacity-0 group-hover:opacity-70 hover:opacity-100 cursor-pointer flex-shrink-0"
              onClick={(e) => handleDeleteObject(item, e)}
              title="Delete object"
            >
              <IconTrash className="w-4 h-4 text-base-content/70 hover:text-error" />
            </button>
          </Show>
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
          Scene Panel
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