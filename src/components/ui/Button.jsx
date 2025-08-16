import { Show } from 'solid-js'

const buttonVariants = {
  primary: 'bg-blue-600 hover:bg-blue-700 text-white',
  secondary: 'bg-slate-600 hover:bg-slate-700 text-white',
  outline: 'border border-gray-600 hover:border-gray-500 text-gray-300 hover:text-gray-200 hover:bg-slate-800/50',
  ghost: 'text-gray-400 hover:text-gray-200 hover:bg-slate-700/50',
  danger: 'bg-red-600 hover:bg-red-700 text-white'
}

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
  
  return (
    <button
      class={`
        inline-flex items-center justify-center gap-2 
        rounded-lg font-medium transition-all
        focus:outline-none focus:ring-2 focus:ring-blue-500/50
        disabled:opacity-50 disabled:cursor-not-allowed
        ${buttonVariants[variant]} 
        ${buttonSizes[size]}
        ${className}
      `}
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