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
  
  const getDynamicHeaderStyle = (title) => {
    return 'bg-base-300 text-base-content'
  }
  
  return (
    <div className={`${className} rounded-lg overflow-hidden bg-base-300`}>
      <button
        onClick={toggle}
        className={`
          w-full pl-2 pr-4 py-2 text-left font-semibold text-sm 
          transition-all duration-200 flex items-center gap-2 group
          focus:outline-none focus:ring-0 active:outline-none
          ${isOpen() 
            ? getDynamicHeaderStyle(title) 
            : 'bg-base-300 text-base-content/70 hover:text-base-content active:bg-base-300'
          }
          ${headerClassName}
        `}
      >
        <IconChevronRight 
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
        <div className={`${contentClassName}`}>
          {children}
        </div>
      </Show>
    </div>
  )
}

export default CollapsibleSection
