import { Show } from 'solid-js'

const buttonSizes = {
  sm: 'px-2 py-1 text-sm',
  md: 'px-3 py-2 text-sm',
  lg: 'px-4 py-2',
  icon: 'p-2'
}

function Button({ 
  children, 
  variant = 'secondary', 
  size = 'md',
  loading = false,
  disabled = false,
  leftIcon,
  rightIcon,
  class: className = '',
  onClick,
  ...props 
}) {
  const isDisabled = () => disabled || loading
  
  const handleClick = (e) => {
    if (!isDisabled() && onClick) {
      onClick(e)
    }
  }
  
  const getVariantClasses = () => {
    const variants = {
      primary: 'bg-gradient-to-r from-blue-600 to-purple-600 hover:from-blue-700 hover:to-purple-700 text-white shadow-lg hover:shadow-xl transform hover:scale-105',
      secondary: 'bg-gray-700 hover:bg-gray-600 text-gray-100',
      outline: 'border border-gray-600 hover:border-gray-500 text-gray-100 hover:bg-gray-800',
      ghost: 'text-gray-400 hover:text-gray-100 hover:bg-gray-800',
      danger: 'bg-red-600 hover:bg-red-700 text-white',
      gradient: 'bg-gradient-to-br from-blue-500/20 to-purple-500/20 hover:from-blue-500/30 hover:to-purple-500/30 border-2 border-dashed border-white/15 hover:border-blue-400',
      success: 'bg-gradient-to-r from-green-600 to-emerald-600 hover:from-green-700 hover:to-emerald-700 text-white'
    };
    return variants[variant] || variants.secondary;
  }
  
  const combineClasses = (...classes) => classes.filter(Boolean).join(' ')
  
  return (
    <button
      class={combineClasses(
        'inline-flex items-center justify-center gap-2',
        'rounded-lg font-medium transition-all',
        'focus:outline-none focus:ring-2 focus:ring-blue-500',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        getVariantClasses(),
        buttonSizes[size],
        className
      )}
      disabled={isDisabled()}
      onClick={handleClick}
      {...props}
    >
      <Show when={loading}>
        <svg class="animate-spin w-4 h-4" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" class="opacity-25" />
          <path fill="currentColor" class="opacity-75" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
        </svg>
      </Show>
      
      {leftIcon && !loading && (
        <span class="w-4 h-4 flex-shrink-0">
          {leftIcon}
        </span>
      )}
      
      {children}
      
      {rightIcon && (
        <span class="w-4 h-4 flex-shrink-0">
          {rightIcon}
        </span>
      )}
    </button>
  )
}

export default Button
