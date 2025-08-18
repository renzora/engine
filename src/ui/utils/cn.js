export function cn(...inputs) {
  const classes = [];
  
  for (const input of inputs) {
    if (!input) continue;
    
    if (typeof input === 'string') {
      classes.push(input);
    } else if (typeof input === 'object') {
      for (const [key, value] of Object.entries(input)) {
        if (value) {
          classes.push(key);
        }
      }
    }
  }
  
  return classes.join(' ');
}

export function cva(base, config = {}) {
  return (props = {}) => {
    const { variants = {}, defaultVariants = {} } = config;
    const variantClasses = [];
    
    if (base) {
      variantClasses.push(base);
    }
    
    for (const [variantKey, variantValue] of Object.entries(variants)) {
      const value = props[variantKey] ?? defaultVariants[variantKey];
      
      if (value && variantValue[value]) {
        variantClasses.push(variantValue[value]);
      }
    }
    
    if (props.className) {
      variantClasses.push(props.className);
    }
    
    return cn(...variantClasses);
  };
}