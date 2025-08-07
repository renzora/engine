import { useState } from 'react';
import { Icons } from '@/plugins/editor/components/Icons.jsx';

const CollapsibleSection = ({ 
  title, 
  children, 
  defaultOpen = false, 
  className = "",
  titleClassName = "",
  contentClassName = "",
  showToggleButton = true,
  rightElement = null,
  ...props 
}) => {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className={`bg-slate-800/50 border border-slate-700/50 ${className}`} {...props}>
      <div 
        className={`flex items-center justify-between px-3 py-2 bg-slate-700/30 border-b border-slate-700/50 cursor-pointer hover:bg-slate-700/50 transition-colors ${titleClassName}`}
        onClick={() => showToggleButton && setIsOpen(!isOpen)}
      >
        <div className="flex items-center gap-2">
          {showToggleButton && (
            <Icons.ChevronRight 
              className={`w-3 h-3 text-gray-400 transition-transform ${isOpen ? 'rotate-90' : ''}`} 
            />
          )}
          <span className="text-sm font-medium text-gray-200 uppercase tracking-wide">{title}</span>
        </div>
        {rightElement}
      </div>
      
      {isOpen && (
        <div className={`p-3 ${contentClassName}`}>
          {children}
        </div>
      )}
    </div>
  );
};

export default CollapsibleSection;