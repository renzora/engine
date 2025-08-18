import { useThemedClasses } from './hooks/useThemedClasses.js';

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
  const { themed, combine } = useThemedClasses();
  
  const classes = combine(
    captionSizes[size],
    'font-semibold',
    'uppercase',
    'tracking-wider',
    themed.text.disabled,
    className
  );
  
  return (
    <div class={classes} {...props}>
      {children}
    </div>
  );
}