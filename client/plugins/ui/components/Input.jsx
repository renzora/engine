const Input = ({ 
  type = 'text',
  value,
  onChange,
  placeholder,
  disabled = false,
  size = 'md',
  variant = 'default',
  error = false,
  success = false,
  className = '',
  suffix,
  prefix,
  ...props 
}) => {
  const sizeClasses = {
    sm: 'text-xs p-2',
    md: 'text-xs p-2.5',
    lg: 'text-sm p-3'
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

  const baseClasses = `
    w-full bg-slate-800/80 text-white rounded-lg 
    focus:outline-none focus:ring-2 transition-all
    ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
    ${sizeClasses[size]}
    ${getVariantClasses()}
    ${className}
  `;

  if (prefix || suffix) {
    return (
      <div className="relative">
        {prefix && (
          <div className="absolute left-2 top-1/2 -translate-y-1/2 text-xs text-gray-400 pointer-events-none">
            {prefix}
          </div>
        )}
        <input
          type={type}
          value={value}
          onChange={onChange}
          placeholder={placeholder}
          disabled={disabled}
          className={`${baseClasses} ${prefix ? 'pl-8' : ''} ${suffix ? 'pr-8' : ''}`}
          {...props}
        />
        {suffix && (
          <div className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-gray-400 pointer-events-none">
            {suffix}
          </div>
        )}
      </div>
    );
  }

  return (
    <input
      type={type}
      value={value}
      onChange={onChange}
      placeholder={placeholder}
      disabled={disabled}
      className={baseClasses}
      {...props}
    />
  );
};

export default Input;