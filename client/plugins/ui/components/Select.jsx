const Select = ({ 
  value,
  onChange,
  options = [],
  placeholder,
  disabled = false,
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
    <select
      value={value}
      onChange={onChange}
      disabled={disabled}
      className={`
        w-full bg-slate-800/80 border text-white rounded-lg 
        focus:outline-none focus:ring-2 transition-all cursor-pointer
        ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
        ${sizeClasses[size]}
        ${getVariantClasses()}
        ${className}
      `}
      {...props}
    >
      {placeholder && (
        <option value="" disabled>
          {placeholder}
        </option>
      )}
      {options.map((option, index) => (
        <option 
          key={option.value || index} 
          value={option.value || option}
        >
          {option.label || option}
        </option>
      ))}
    </select>
  );
};

export default Select;