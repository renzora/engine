import { useEffect } from 'react';

const PanelResizer = ({ 
  type, // 'bottom' or 'right'
  isResizing,
  onResizeStart,
  onResizeEnd,
  onResize,
  position,
  className = '',
  isLeftPanel = false // New prop to indicate if panel is on left
}) => {
  useEffect(() => {
    if (!isResizing) return;

    const handleMouseMove = (e) => {
      e.preventDefault();
      onResize(e);
    };

    const handleMouseUp = () => {
      onResizeEnd();
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isResizing, onResize, onResizeEnd]);

  const handleMouseDown = (e) => {
    e.preventDefault();
    onResizeStart();
  };

  const cursorClass = type === 'bottom' ? 'cursor-row-resize' : 'cursor-col-resize';
  const dimensionClass = type === 'bottom' ? 'h-1' : 'w-1';

  return (
    <div
      className={`absolute pointer-events-auto z-50 ${cursorClass} ${dimensionClass} ${
        isResizing 
          ? 'bg-blue-500/75 opacity-100 transition-none' 
          : 'bg-slate-700/50 opacity-0 hover:opacity-100 hover:bg-blue-500/75 transition-all duration-200'
      } ${className}`}
      style={position}
      onMouseDown={handleMouseDown}
      suppressHydrationWarning
    />
  );
};

export default PanelResizer;