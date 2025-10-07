import { createEffect, onCleanup } from 'solid-js';

const PanelResizer = ({ 
  type,
  isResizing,
  onResizeStart,
  onResizeEnd,
  onResize,
  position,
  className = ''
}) => {
  createEffect(() => {
    const resizing = typeof isResizing === 'function' ? isResizing() : isResizing;
    if (!resizing) return;

    const handleMouseMove = (e) => {
      e.preventDefault();
      onResize(e);
    };

    const handleMouseUp = (e) => {
      e.preventDefault();
      onResizeEnd();
    };

    document.addEventListener('mousemove', handleMouseMove, { passive: false });
    document.addEventListener('mouseup', handleMouseUp, { passive: false });

    onCleanup(() => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    });
  });

  const handleMouseDown = (e) => {
    e.preventDefault();
    onResizeStart();
  };

  const cursorClass = type === 'bottom' ? 'cursor-row-resize' : 'cursor-col-resize';
  const dimensionClass = (position.width || position.height) ? '' : (type === 'bottom' ? 'h-2' : 'w-2');
  const resizing = typeof isResizing === 'function' ? isResizing() : isResizing;

  return (
    <div
      class={`absolute pointer-events-auto z-50 ${cursorClass} ${dimensionClass} ${
        resizing 
          ? 'bg-primary/75 opacity-100 transition-none' 
          : 'bg-base-300/30 opacity-30 hover:opacity-100 hover:bg-primary/75 transition-all duration-200'
      } ${className}`}
      style={position}
      onMouseDown={handleMouseDown}
    />
  );
};

export default PanelResizer;
