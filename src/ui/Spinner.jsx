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
  const combineClasses = (...classes) => classes.filter(Boolean).join(' ');
  
  const classes = combineClasses(
    spinnerSizes[size],
    'border-2 border-t-transparent rounded-full animate-spin',
    'border-blue-500',
    className
  );
  
  return (
    <div class={classes} {...props} />
  );
}