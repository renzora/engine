import { createSignal, Show } from 'solid-js'
import { IconChevronRight } from '@tabler/icons-solidjs'

function CollapsibleSection({ 
  title, 
  children, 
  defaultOpen = true,
  className = '',
  headerClassName = '',
  contentClassName = ''
}) {
  const [isOpen, setIsOpen] = createSignal(defaultOpen)
  
  const toggle = () => setIsOpen(!isOpen())
  
  return (
    <div className={`border-b border-gray-700/60 ${className}`}>
      <button
        onClick={toggle}
        className={`
          w-full pl-2 pr-4 py-3 text-left font-semibold text-sm 
          transition-all duration-200 flex items-center gap-2 group
          ${isOpen() 
            ? 'bg-slate-700/50 text-white' 
            : 'text-gray-300 hover:bg-slate-700/30 hover:text-gray-100 active:bg-slate-700/60'
          }
          ${headerClassName}
        `}
      >
        <IconChevronRight 
          className={`
            w-3.5 h-3.5 transition-all duration-200 
            ${isOpen() 
              ? 'rotate-90 text-blue-400' 
              : 'text-gray-400 group-hover:text-gray-300'
            }
          `} 
        />
        <span className="flex-1">{title}</span>
      </button>
      
      <Show when={isOpen()}>
        <div className={`bg-slate-800/20 ${contentClassName}`}>
          {children}
        </div>
      </Show>
    </div>
  )
}

export default CollapsibleSection