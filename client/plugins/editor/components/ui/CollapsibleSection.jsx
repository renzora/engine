import { useState } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';

const CollapsibleSection = ({ title, children, defaultOpen = true, index = 0 }) => {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  const isEven = index % 2 === 0;
  
  return (
    <div className={`${isEven ? 'bg-slate-800/20' : 'bg-slate-900/20'}`}>
      <button
        onClick={() => setIsOpen(!isOpen)}
        className={`w-full flex items-center justify-between px-3 py-2.5 text-xs font-medium transition-all duration-200 group ${
          isOpen 
            ? 'bg-blue-600/20 text-blue-200 shadow-md shadow-blue-900/20 border-l-2 border-blue-500' 
            : 'text-gray-300 hover:bg-slate-700/30 hover:text-gray-200 hover:border-l-2 hover:border-slate-400 border-l-2 border-transparent'
        }`}
      >
        <span className={`transition-transform duration-200 ${!isOpen ? 'group-hover:translate-x-1' : ''}`}>{title}</span>
        <Icons.ChevronDown className={`w-3 h-3 transition-transform ${isOpen ? '' : '-rotate-90'}`} />
      </button>
      {isOpen && (
        <div className="px-3 pt-2 pb-3">
          {children}
        </div>
      )}
    </div>
  );
};

export default CollapsibleSection;