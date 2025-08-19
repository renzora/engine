const subtitleSizes = {
  sm: 'text-sm',
  md: 'text-base',
  lg: 'text-lg',
  xl: 'text-xl'
};

export default function Subtitle({ 
  children, 
  size = 'lg',
  class: className = '',
  ...props 
}) {
  const combineClasses = (...classes) => classes.filter(Boolean).join(' ');
  
  const classes = combineClasses(
    subtitleSizes[size],
    'text-gray-400',
    'leading-relaxed',
    className
  );
  
  return (
    <p class={classes} {...props}>
      {children}
    </p>
  );
}