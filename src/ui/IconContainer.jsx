const containerSizes = {
  sm: 'w-8 h-8',
  md: 'w-10 h-10',
  lg: 'w-12 h-12', 
  xl: 'w-16 h-16',
  xxl: 'w-20 h-20'
};

const iconSizes = {
  sm: 'w-4 h-4',
  md: 'w-5 h-5',
  lg: 'w-6 h-6',
  xl: 'w-8 h-8', 
  xxl: 'w-10 h-10'
};

export default function IconContainer({ 
  children,
  size = 'md',
  variant = 'primary',
  class: className = '',
  ...props 
}) {
  const combineClasses = (...classes) => classes.filter(Boolean).join(' ');
  
  const variants = {
    primary: 'bg-gradient-to-br from-blue-500 to-purple-600',
    secondary: 'bg-gray-700',
    surface: 'bg-gray-800 border border-gray-700',
    success: 'bg-gradient-to-br from-green-500 to-emerald-600',
    warning: 'bg-gradient-to-br from-orange-500 to-amber-600',
    danger: 'bg-gradient-to-br from-red-500 to-pink-600',
    glass: 'bg-gradient-to-br from-blue-500/20 to-purple-500/20 border border-white/10'
  };
  
  const classes = combineClasses(
    containerSizes[size],
    'rounded-xl flex items-center justify-center shadow-lg',
    variants[variant],
    className
  );
  
  return (
    <div class={classes} {...props}>
      <div class={combineClasses(iconSizes[size], 'text-white')}>
        {children}
      </div>
    </div>
  );
}