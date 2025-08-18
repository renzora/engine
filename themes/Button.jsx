import { splitProps } from 'solid-js';
import { useComponentTheme } from '../src/ui/hooks/useComponentTheme.js';

export function Button(props) {
  const [local, others] = splitProps(props, [
    'size',
    'children', 
    'className',
    'disabled',
    'loading'
  ]);
  
  const { style, onMouseEnter, onMouseLeave, onFocus, onBlur } = useComponentTheme('Button');
  
  const sizeClasses = {
    xs: 'px-2 py-1 text-xs',
    sm: 'px-3 py-1.5 text-sm',
    md: 'px-4 py-2 text-sm',
    lg: 'px-6 py-3 text-base',
    icon: 'p-2'
  };
  
  const sizeClass = sizeClasses[local.size || 'md'];
  
  return (
    <button
      className={`
        ${sizeClass}
        rounded-md font-medium transition-all duration-150 
        focus:outline-none focus:ring-2 focus:ring-offset-2
        disabled:opacity-50 disabled:cursor-not-allowed
        ${local.className || ''}
      `}
      style={style}
      disabled={local.disabled || local.loading}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onFocus={onFocus}
      onBlur={onBlur}
      {...others}
    >
      {local.loading ? (
        <div className="flex items-center">
          <svg className="animate-spin -ml-1 mr-2 h-4 w-4" fill="none" viewBox="0 0 24 24">
            <circle 
              className="opacity-25" 
              cx="12" 
              cy="12" 
              r="10" 
              stroke="currentColor" 
              strokeWidth="4"
            />
            <path 
              className="opacity-75" 
              fill="currentColor" 
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
            />
          </svg>
          {local.children}
        </div>
      ) : (
        local.children
      )}
    </button>
  );
}
