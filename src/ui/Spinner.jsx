import { useThemedClasses } from './hooks/useThemedClasses.js';

const spinnerSizes = {
  sm: 'w-4 h-4',
  md: 'w-6 h-6', 
  lg: 'w-8 h-8',
  xl: 'w-12 h-12'
};

export default function Spinner({ 
  size = 'md',
  class: className = '',
  ...props 
}) {
  const { themed, combine } = useThemedClasses();
  
  const classes = combine(
    spinnerSizes[size],
    'border-2 border-t-transparent rounded-full animate-spin',
    `border-[rgb(var(--primary))]`,
    className
  );
  
  return (
    <div class={classes} {...props} />
  );
}