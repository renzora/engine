const ColorPicker = ({ 
  value = '#ff0000',
  onChange,
  disabled = false,
  size = 'md',
  showValue = true,
  className = '',
  ...props 
}) => {
  const sizeClasses = {
    sm: 'w-8 h-8',
    md: 'w-10 h-10',
    lg: 'w-12 h-12'
  };

  const handleChange = (e) => {
    onChange?.(e.target.value);
  };

  return (
    <div className={`flex items-center gap-2 ${className}`}>
      <input 
        type="color" 
        value={value} 
        onChange={handleChange}
        disabled={disabled}
        className={`
          rounded-lg border border-slate-600 bg-slate-800 cursor-pointer
          disabled:opacity-50 disabled:cursor-not-allowed
          ${sizeClasses[size]}
        `}
        {...props}
      />
      
      {showValue && (
        <div className="flex-1 bg-slate-800/80 border border-slate-600 rounded-lg p-2">
          <div className="text-xs text-gray-300">{value.toUpperCase()}</div>
        </div>
      )}
    </div>
  );
};

export default ColorPicker;