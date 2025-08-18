import { createSignal } from 'solid-js';
import { editorActions } from '@/layout/stores/EditorStore';
import useCustomDragPreview from './useCustomDragPreview';

const useSceneDnD = (hierarchyData) => {
  const [draggedItem, setDraggedItem] = createSignal(null);
  const [dragOverItem, setDragOverItem] = createSignal(null);
  const [dropPosition, setDropPosition] = createSignal(null);
  const { createPreview, updatePreviewPosition, removePreview } = useCustomDragPreview();

  const handleDragStart = (e, item) => {
    setDraggedItem(item);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', item.id);

    createPreview(e.currentTarget, e.clientX, e.clientY);
    document.body.style.cursor = 'grabbing';

    const emptyImg = document.createElement('img');
    emptyImg.src = 'data:image/gif;base64,R0lGODlhAQABAIAAAAUEBAAAACwAAAAAAQABAAACAkQBADs=';
    e.dataTransfer.setDragImage(emptyImg, 0, 0);
  };

  const handleDragOver = (e, item) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';

    const currentDraggedItem = draggedItem();
    if (!currentDraggedItem || currentDraggedItem.id === item.id) return;

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

    const currentDragOverItem = dragOverItem();
    const currentDropPosition = dropPosition();
    if (currentDragOverItem?.id !== item.id || currentDropPosition !== position) {
      setDragOverItem(item);
      setDropPosition(position);
    }
  };

  const handleDrop = (e, targetItem) => {
    e.preventDefault();
    const currentDraggedItem = draggedItem();
    const currentDropPosition = dropPosition();
    
    if (!currentDraggedItem || !targetItem || currentDraggedItem.id === targetItem.id) {
      removePreview();
      return;
    }

    if (currentDropPosition === 'inside' && targetItem.type !== 'folder') {
      editorActions.addConsoleMessage(`Cannot drop inside "${targetItem.name}". It is not a folder.`, 'warning');
      removePreview();
      return;
    }

    const success = editorActions.reorderObjectInHierarchy(currentDraggedItem.id, targetItem.id, currentDropPosition);

    if (success) {
      editorActions.addConsoleMessage(`Moved "${currentDraggedItem.name}" ${currentDropPosition} "${targetItem.name}"`, 'success');
    } else {
      editorActions.addConsoleMessage(`Failed to move "${currentDraggedItem.name}"`, 'error');
    }

    removePreview();
  };

  const handleDragEnd = () => {
    removePreview();
    setDraggedItem(null);
    setDragOverItem(null);
    setDropPosition(null);
    document.body.style.cursor = '';
  };

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
