const captionSizes = {
  xs: 'text-xs',
  sm: 'text-sm',
  md: 'text-base'
};

export default function Caption({ 
  children, 
  size = 'xs',
  class: className = '',
  ...props 
}) {
  const combineClasses = (...classes) => classes.filter(Boolean).join(' ');
  
  const classes = combineClasses(
    captionSizes[size],
    'font-semibold',
    'uppercase',
    'tracking-wider',
    'text-gray-500',
    className
  );
  
  return (
    <div class={classes} {...props}>
      {children}
    </div>
  );
}