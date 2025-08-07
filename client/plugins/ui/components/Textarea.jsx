const Textarea = ({ 
  value,
  onChange,
  placeholder,
  disabled = false,
  rows = 3,
  resize = 'none',
  size = 'md',
  variant = 'default',
  error = false,
  success = false,
  className = '',
  ...props 
}) => {
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
    if (error) {
      return 'border-red-500 focus:border-red-500 focus:ring-red-500/50';
    }
    if (success) {
      return 'border-green-500 focus:border-green-500 focus:ring-green-500/50';
    }
    return 'border-slate-600 focus:border-blue-500 focus:ring-blue-500/50';
  };

  return (
    <textarea
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      disabled={disabled}
      rows={rows}
      className={`
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