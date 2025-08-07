import { useState, useCallback } from 'react';

const useCustomDragPreview = () => {
  const [preview, setPreview] = useState(null);

  const createPreview = useCallback((draggedElement, x, y) => {
    const nameElement = draggedElement.querySelector('span.truncate');
    const name = nameElement ? nameElement.textContent : 'Item';

    const element = document.createElement('div');
    element.id = 'custom-drag-preview';
    element.textContent = name;
    element.style.position = 'fixed';
    element.style.pointerEvents = 'none';
    element.style.zIndex = '1000';
    element.style.backgroundColor = '#3B82F6';
    element.style.color = 'white';
    element.style.padding = '4px 12px';
    element.style.borderRadius = '6px';
    element.style.fontSize = '14px';
    element.style.boxShadow = '0 5px 15px rgba(0, 0, 0, 0.3)';
    element.style.opacity = '0.95';
    element.style.border = 'none';

    document.body.appendChild(element);

    const rect = element.getBoundingClientRect();
    element.style.left = `${x - rect.width / 2}px`;
    element.style.top = `${y - rect.height / 2}px`;

    setPreview(element);
  }, []);

  const updatePreviewPosition = useCallback((x, y) => {
    if (preview) {
      const rect = preview.getBoundingClientRect();
      preview.style.left = `${x - rect.width / 2}px`;
      preview.style.top = `${y - rect.height / 2}px`;
    }
  }, [preview]);

  const removePreview = useCallback(() => {
    if (preview) {
      preview.remove();
      setPreview(null);
    }
  }, [preview]);

  return { createPreview, updatePreviewPosition, removePreview };
};

export default useCustomDragPreview;
