const Textarea = (props) => {
  const sizeClasses = {
    sm: 'text-xs p-2',
    md: 'text-xs p-2.5',
    lg: 'text-sm p-3'
  };

  const resizeClasses = {
    none: 'resize-none',
    both: 'resize',
    horizontal: 'resize-x',
    vertical: 'resize-y'
  };

  const getVariantClasses = () => {
    if (props.error) {
      return 'border-red-500 focus:border-red-500 focus:ring-red-500/50';
    }
    if (props.success) {
      return 'border-green-500 focus:border-green-500 focus:ring-green-500/50';
    }
    return 'border-slate-600 focus:border-blue-500 focus:ring-blue-500/50';
  };

  const disabled = props.disabled || false;
  const rows = props.rows || 3;
  const resize = props.resize || 'none';
  const size = props.size || 'md';
  const className = props.className || '';

  return (
    <textarea
      value={props.value}
      onInput={(e) => props.onChange?.(e.target.value)}
      placeholder={props.placeholder}
      disabled={disabled}
      rows={rows}
      class={`
        w-full bg-slate-800/80 border text-white rounded-lg 
        focus:outline-none focus:ring-2 transition-all
        ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
        ${sizeClasses[size]}
        ${resizeClasses[resize]}
        ${getVariantClasses()}
        ${className}
      `}
      {...props}
    />
  );
};

export default Textarea;