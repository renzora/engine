export default function Card({ 
  children, 
  variant = 'default',
  padding = 'md',
  hoverable = false,
  class: className = '',
  ...props 
}) {
  const variants = {
    default: 'bg-gray-800 border border-gray-700',
    panel: 'bg-gray-900 border border-gray-800',
    glass: 'bg-black/90 backdrop-blur-xl border border-white/30',
    gradient: 'bg-gradient-to-br from-black/90 to-black/85 border border-blue-400/20'
  };
  
  const paddings = {
    none: '',
    sm: 'p-3',
    md: 'p-4', 
    lg: 'p-6',
    xl: 'p-8'
  };
  
  const hoverClasses = hoverable ? 'hover:shadow-xl transition-all duration-300 cursor-pointer' : '';
  
  const combineClasses = (...classes) => classes.filter(Boolean).join(' ');
  
  const classes = combineClasses(
    'rounded-xl',
    variants[variant],
    paddings[padding],
    hoverClasses,
    className
  );
  
  return (
    <div class={classes} {...props}>
      {children}
    </div>
  );
}