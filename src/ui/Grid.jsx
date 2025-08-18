import { useThemedClasses } from './hooks/useThemedClasses.js';

export default function Grid({ 
  children, 
  cols = 3,
  gap = 'md',
  class: className = '',
  ...props 
}) {
  const { combine } = useThemedClasses();
  
  const colClasses = {
    1: 'grid-cols-1',
    2: 'grid-cols-2', 
    3: 'grid-cols-3',
    4: 'grid-cols-4',
    6: 'grid-cols-6',
    auto: 'grid-cols-auto-fit'
  };
  
  const gapClasses = {
    sm: 'gap-2',
    md: 'gap-4',
    lg: 'gap-6',
    xl: 'gap-8'
  };
  
  const classes = combine(
    'grid',
    colClasses[cols],
    gapClasses[gap],
    className
  );
  
  return (
    <div class={classes} {...props}>
      {children}
    </div>
  );
}