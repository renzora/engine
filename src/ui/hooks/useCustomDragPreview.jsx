import { createSignal } from 'solid-js';

const useCustomDragPreview = () => {
  const [preview, setPreview] = createSignal(null);

  const createPreview = (draggedElement, x, y) => {
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
  };

  const updatePreviewPosition = (x, y) => {
    const currentPreview = preview();
    if (currentPreview) {
      const rect = currentPreview.getBoundingClientRect();
      currentPreview.style.left = `${x - rect.width / 2}px`;
      currentPreview.style.top = `${y - rect.height / 2}px`;
    }
  };

  const removePreview = () => {
    const currentPreview = preview();
    if (currentPreview) {
      currentPreview.remove();
      setPreview(null);
    }
  };

  return { createPreview, updatePreviewPosition, removePreview };
};

export default useCustomDragPreview;
