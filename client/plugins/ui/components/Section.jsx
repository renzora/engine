import { useState } from 'react';
import { Icons } from '@/plugins/editor/components/Icons';

const Section = ({ 
  title, 
  children, 
  defaultOpen = true, 
  index = 0,
  collapsible = true,
  icon: IconComponent,
  className = '',
  headerClassName = '',
  contentClassName = '',
  variant = 'default',
  ...props 
}) => {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  const handleToggle = () => {
    if (collapsible) {
      setIsOpen(!isOpen);
    }
  };

  const getVariantStyles = () => {
    switch (variant) {
      case 'accent':
        return {
          container: 'bg-blue-900/20',
          header: isOpen 
            ? 'bg-blue-600/20 text-blue-200 shadow-md shadow-blue-900/20 border-l-2 border-blue-500' 
            : 'text-gray-300 hover:bg-blue-800/20 hover:text-blue-200 hover:border-l-2 hover:border-blue-400 border-l-2 border-transparent',
        };
      case 'subtle':
        return {
          container: 'bg-slate-800/10',
          header: isOpen 
            ? 'bg-slate-700/30 text-slate-200 border-l-2 border-slate-400' 
            : 'text-gray-300 hover:bg-slate-700/20 hover:text-gray-200 hover:border-l-2 hover:border-slate-400 border-l-2 border-transparent',
        };
      default:
        const isEven = index % 2 === 0;
        return {
          container: isEven ? 'bg-slate-800/20' : 'bg-slate-900/20',
          header: isOpen 
            ? 'bg-blue-600/20 text-blue-200 shadow-md shadow-blue-900/20 border-l-2 border-blue-500' 
            : 'text-gray-300 hover:bg-slate-700/30 hover:text-gray-200 hover:border-l-2 hover:border-slate-400 border-l-2 border-transparent',
        };
    }
  };

  const styles = getVariantStyles();

  return (
    <div className={`${styles.container} ${className}`} {...props}>
      <button
        onClick={handleToggle}
        disabled={!collapsible}
        className={`
          w-full flex items-center justify-between px-3 py-2.5 text-xs font-medium transition-all duration-200 group
          ${styles.header}
          ${!collapsible ? 'cursor-default' : 'cursor-pointer'}
          ${headerClassName}
        `}
      >
        <div className="flex items-center gap-2">
          {IconComponent && <IconComponent className="w-4 h-4" />}
          <span className={`transition-transform duration-200 ${!isOpen && collapsible ? 'group-hover:translate-x-1' : ''}`}>
            {title}
          </span>
        </div>
        
        {collapsible && (
          <Icons.ChevronDown className={`w-3 h-3 transition-transform ${isOpen ? '' : '-rotate-90'}`} />
        )}
      </button>
      
      {isOpen && (
        <div className={`px-3 pt-2 pb-3 ${contentClassName}`}>
          {children}
        </div>
      )}
    </div>
  );
};

export default Section;