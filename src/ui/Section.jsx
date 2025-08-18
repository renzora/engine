import { createSignal, Show, createMemo } from 'solid-js';
import { ChevronDown } from '@/ui/icons';

const Section = (props) => {
  const [isOpen, setIsOpen] = createSignal(props.defaultOpen ?? true);

  const handleToggle = () => {
    if (props.collapsible ?? true) {
      setIsOpen(!isOpen());
    }
  };

  const getVariantStyles = createMemo(() => {
    const variant = props.variant || 'default';
    const index = props.index || 0;
    
    switch (variant) {
      case 'accent':
        return {
          container: 'bg-blue-900/20',
          header: isOpen() 
            ? 'bg-blue-600/20 text-blue-200 shadow-md shadow-blue-900/20 border-l-2 border-blue-500' 
            : 'text-gray-300 hover:bg-blue-800/20 hover:text-blue-200 hover:border-l-2 hover:border-blue-400 border-l-2 border-transparent',
        };
      case 'subtle':
        return {
          container: 'bg-slate-800/10',
          header: isOpen() 
            ? 'bg-slate-700/30 text-slate-200 border-l-2 border-slate-400' 
            : 'text-gray-300 hover:bg-slate-700/20 hover:text-gray-200 hover:border-l-2 hover:border-slate-400 border-l-2 border-transparent',
        };
      default:
        const isEven = index % 2 === 0;
        return {
          container: isEven ? 'bg-slate-800/20' : 'bg-slate-900/20',
          header: isOpen() 
            ? 'bg-blue-600/20 text-blue-200 shadow-md shadow-blue-900/20 border-l-2 border-blue-500' 
            : 'text-gray-300 hover:bg-slate-700/30 hover:text-gray-200 hover:border-l-2 hover:border-slate-400 border-l-2 border-transparent',
        };
    }
  });

  const collapsible = props.collapsible ?? true;
  const className = props.className || '';
  const headerClassName = props.headerClassName || '';
  const contentClassName = props.contentClassName || '';

  return (
    <div class={`${getVariantStyles().container} ${className}`} {...props}>
      <button
        onClick={handleToggle}
        disabled={!collapsible}
        class={`
          w-full flex items-center justify-between px-3 py-2.5 text-xs font-medium transition-all duration-200 group
          ${getVariantStyles().header}
          ${!collapsible ? 'cursor-default' : 'cursor-pointer'}
          ${headerClassName}
        `}
      >
        <div class="flex items-center gap-2">
          <Show when={props.icon}>
            <props.icon class="w-4 h-4" />
          </Show>
          <span class={`transition-transform duration-200 ${!isOpen() && collapsible ? 'group-hover:translate-x-1' : ''}`}>
            {props.title}
          </span>
        </div>
        
        <Show when={collapsible}>
          <ChevronDown class={`w-3 h-3 transition-transform ${isOpen() ? '' : '-rotate-90'}`} />
        </Show>
      </button>
      
      <Show when={isOpen()}>
        <div class={`px-3 pt-2 pb-3 ${contentClassName}`}>
          {props.children}
        </div>
      </Show>
    </div>
  );
};

export default Section;
