import { createSignal, Show } from 'solid-js'
import { ChevronRight } from '@/ui/icons'

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
    <div className={`border-b border-base-300/60 ${className}`}>
      <button
        onClick={toggle}
        className={`
          w-full pl-2 pr-4 py-3 text-left font-semibold text-sm 
          transition-all duration-200 flex items-center gap-2 group
          ${isOpen() 
            ? 'bg-base-300/50 text-base-content' 
            : 'text-base-content/70 hover:bg-base-300/30 hover:text-base-content active:bg-base-300/60'
          }
          ${headerClassName}
        `}
      >
        <ChevronRight 
          class={`
            w-3.5 h-3.5 transition-all duration-200 
            ${isOpen() 
              ? 'rotate-90 text-primary' 
              : 'text-base-content/60 group-hover:text-base-content/80'
            }
          `} 
        />
        <span className="flex-1">{title}</span>
      </button>
      
      <Show when={isOpen()}>
        <div className={`bg-base-200/20 ${contentClassName}`}>
          {children}
        </div>
      </Show>
    </div>
  )
}

export default CollapsibleSection
