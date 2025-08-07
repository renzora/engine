import { useState } from 'react';

const Toggle = ({ 
  checked = false, 
  onChange, 
  disabled = false, 
  size = 'md',
  variant = 'primary',
  className = '',
  label,
  description,
  ...props 
}) => {
  const [isChecked, setIsChecked] = useState(checked);

  const handleToggle = () => {
    if (disabled) return;
    const newValue = !isChecked;
    setIsChecked(newValue);
    onChange?.(newValue);
  };

  const sizeClasses = {
    sm: 'h-4 w-8',
    md: 'h-6 w-11',
    lg: 'h-8 w-14'
  };

  const thumbSizeClasses = {
    sm: 'h-3 w-3',
    md: 'h-4 w-4', 
    lg: 'h-6 w-6'
  };

  const translateClasses = {
    sm: isChecked ? 'translate-x-4' : 'translate-x-0.5',
    md: isChecked ? 'translate-x-6' : 'translate-x-1',
    lg: isChecked ? 'translate-x-7' : 'translate-x-1'
  };

  const getBackgroundColor = () => {
    if (disabled) return 'bg-gray-600';
    if (isChecked) {
      switch (variant) {
        case 'success': return 'bg-green-500 shadow-lg shadow-green-500/30';
        case 'warning': return 'bg-amber-400 shadow-lg shadow-amber-400/30';
        case 'error': return 'bg-red-500 shadow-lg shadow-red-500/30';
        default: return 'bg-blue-500 shadow-lg shadow-blue-500/30';
      }
    }
    return 'bg-slate-600';
  };

  const WrapperComponent = label ? 'div' : 'button';
  const wrapperProps = label ? {} : {
    onClick: handleToggle,
    disabled,
    type: 'button',
    role: 'switch',
    'aria-checked': isChecked,
    ...props
  };

  return (
    <WrapperComponent 
      className={label ? `flex items-center justify-between ${className}` : ''}
      {...wrapperProps}
    >
      {label && (
        <div className="flex-1 mr-3">
          <div className="text-xs font-medium text-gray-300">{label}</div>
          {description && (
            <div className="text-xs text-gray-500 mt-0.5">{description}</div>
          )}
        </div>
      )}
      
      <button
        onClick={label ? handleToggle : undefined}
        disabled={disabled}
        type="button"
        role="switch"
        aria-checked={isChecked}
        className={`
          relative inline-flex items-center rounded-full transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:ring-offset-2 focus:ring-offset-slate-900
          ${sizeClasses[size]}
          ${getBackgroundColor()}
          ${disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}
          ${!label ? className : ''}
        `}
        {...(label ? {} : props)}
      >
        <span
          className={`
            inline-block transform rounded-full bg-white transition-transform duration-200 shadow-sm
            ${thumbSizeClasses[size]}
            ${translateClasses[size]}
          `}
        />
      </button>
    </WrapperComponent>
  );
};

export default Toggle;