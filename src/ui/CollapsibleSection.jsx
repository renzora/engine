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
  
  const getDynamicHeaderStyle = (title) => {
    const lowerTitle = title.toLowerCase()
    if (lowerTitle.includes('script')) {
      return 'bg-base-300/90 text-base-content border-accent/15'
    } else if (lowerTitle.includes('transform')) {
      return 'bg-base-300/90 text-base-content border-primary/15'
    } else if (lowerTitle.includes('prop') || lowerTitle.includes('properties')) {
      return 'bg-base-300/90 text-base-content border-info/15'
    }
    return 'bg-base-300/90 text-base-content border-secondary/15'
  }
  
  return (
    <div className={`${className}`}>
      <button
        onClick={toggle}
        className={`
          w-full pl-2 pr-4 py-3 text-left font-semibold text-sm 
          transition-all duration-200 flex items-center gap-2 group border-b border-base-content/10
          ${isOpen() 
            ? getDynamicHeaderStyle(title) 
            : 'text-base-content/70 hover:bg-gradient-to-r hover:from-base-300/40 hover:to-base-200/60 hover:text-base-content active:bg-base-300/70'
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
        <div className={`${contentClassName}`}>
          {children}
        </div>
      </Show>
    </div>
  )
}

export default CollapsibleSection
