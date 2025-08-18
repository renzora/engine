import { useThemedClasses } from './hooks/useThemedClasses.js';

export default function Card({ 
  children, 
  variant = 'default',
  padding = 'md',
  hoverable = false,
  class: className = '',
  ...props 
}) {
  const { themed, combine } = useThemedClasses();
  
  const variants = {
    default: `${themed.bg.surface} border ${themed.border.surface}`,
    panel: `${themed.bg.panel} border ${themed.border.panel}`,
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
  
  const classes = combine(
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