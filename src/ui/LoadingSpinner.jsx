import { Show } from 'solid-js'
import { Refresh } from '@/ui/icons'

function LoadingSpinner({ 
  size = 'md', 
  message,
  className = '',
  iconClassName = '',
  messageClassName = ''
}) {
  const sizeClasses = {
    sm: 'w-4 h-4',
    md: 'w-6 h-6',
    lg: 'w-8 h-8',
    xl: 'w-12 h-12'
  }
  
  return (
    <div className={`flex items-center justify-center ${className}`}>
      <div className="text-center">
        <Refresh 
          class={`
            animate-spin text-blue-500 mx-auto
            ${sizeClasses[size]}
            ${iconClassName}
          `} 
        />
        
        <Show when={message}>
          <p className={`mt-2 text-gray-400 ${messageClassName}`}>
            {message}
          </p>
        </Show>
      </div>
    </div>
  )
}

export default LoadingSpinner
