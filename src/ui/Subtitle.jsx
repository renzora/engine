import { useThemedClasses } from './hooks/useThemedClasses.js';

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
  const { themed, combine } = useThemedClasses();
  
  const classes = combine(
    subtitleSizes[size],
    themed.text.secondary,
    'leading-relaxed',
    className
  );
  
  return (
    <p class={classes} {...props}>
      {children}
    </p>
  );
}