export default function Stack({ 
  children, 
  direction = 'vertical',
  gap = 'md',
  align = 'start',
  justify = 'start',
  class: className = '',
  ...props 
}) {
  const combineClasses = (...classes) => classes.filter(Boolean).join(' ');
  
  const directionClasses = {
    vertical: 'flex-col',
    horizontal: 'flex-row'
  };
  
  const gapClasses = {
    none: 'gap-0',
    xs: 'gap-1',
    sm: 'gap-2',
    md: 'gap-4',
    lg: 'gap-6',
    xl: 'gap-8'
  };
  
  const alignClasses = {
    start: 'items-start',
    center: 'items-center',
    end: 'items-end',
    stretch: 'items-stretch'
  };
  
  const justifyClasses = {
    start: 'justify-start',
    center: 'justify-center',
    end: 'justify-end',
    between: 'justify-between',
    around: 'justify-around'
  };
  
  const classes = combineClasses(
    'flex',
    directionClasses[direction],
    gapClasses[gap],
    alignClasses[align],
    justifyClasses[justify],
    className
  );
  
  return (
    <div class={classes} {...props}>
      {children}
    </div>
  );
}