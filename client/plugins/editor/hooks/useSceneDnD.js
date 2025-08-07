import { useState, useCallback } from 'react';
import { actions } from '@/store.js';
import useCustomDragPreview from './useCustomDragPreview';

const useSceneDnD = (hierarchyData) => {
  const [draggedItem, setDraggedItem] = useState(null);
  const [dragOverItem, setDragOverItem] = useState(null);
  const [dropPosition, setDropPosition] = useState(null); // 'above', 'below', 'inside'
  const { createPreview, updatePreviewPosition, removePreview } = useCustomDragPreview();

  const handleDragStart = useCallback((e, item) => {
    setDraggedItem(item);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', item.id);

    createPreview(e.currentTarget, e.clientX, e.clientY);
    document.body.style.cursor = 'grabbing';

    // Hide default drag preview
    const emptyImg = document.createElement('img');
    emptyImg.src = 'data:image/gif;base64,R0lGODlhAQABAIAAAAUEBAAAACwAAAAAAQABAAACAkQBADs=';
    e.dataTransfer.setDragImage(emptyImg, 0, 0);
  }, [createPreview]);

  const handleDragOver = useCallback((e, item) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';

    if (!draggedItem || draggedItem.id === item.id) return;

    updatePreviewPosition(e.clientX, e.clientY);

    const rect = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const height = rect.height;

    let position;
    if (y < height * 0.3) {
      position = 'above';
    } else if (y > height * 0.7) {
      position = 'below';
    } else {
      position = 'inside';
    }

    if (dragOverItem?.id !== item.id || dropPosition !== position) {
      setDragOverItem(item);
      setDropPosition(position);
    }
  }, [draggedItem, dragOverItem, dropPosition, updatePreviewPosition]);

  const handleDrop = useCallback((e, targetItem) => {
    e.preventDefault();
    if (!draggedItem || !targetItem || draggedItem.id === targetItem.id) {
      removePreview();
      return;
    }

    if (dropPosition === 'inside' && targetItem.type !== 'folder') {
      actions.editor.addConsoleMessage(`Cannot drop inside "${targetItem.name}". It is not a folder.`, 'warning');
      removePreview();
      return;
    }

    const success = actions.editor.reorderObjectInHierarchy(draggedItem.id, targetItem.id, dropPosition);

    if (success) {
      actions.editor.addConsoleMessage(`Moved "${draggedItem.name}" ${dropPosition} "${targetItem.name}"`, 'success');
    } else {
      actions.editor.addConsoleMessage(`Failed to move "${draggedItem.name}"`, 'error');
    }

    removePreview();
  }, [draggedItem, dropPosition, removePreview]);

  const handleDragEnd = useCallback(() => {
    removePreview();
    setDraggedItem(null);
    setDragOverItem(null);
    setDropPosition(null);
    document.body.style.cursor = '';
  }, [removePreview]);

  return {
    draggedItem,
    dragOverItem,
    dropPosition,
    handleDragStart,
    handleDragOver,
    handleDrop,
    handleDragEnd,
  };
};

export default useSceneDnD;
