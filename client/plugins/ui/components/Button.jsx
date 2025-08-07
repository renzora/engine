const Button = ({ 
  children,
  onClick,
  disabled = false,
  variant = 'primary',
  size = 'md',
  type = 'button',
  className = '',
  ...props 
}) => {
  const sizeClasses = {
    sm: 'text-xs px-2.5 py-1.5',
    md: 'text-xs px-3 py-2',
    lg: 'text-sm px-4 py-2.5'
  };

  const getVariantClasses = () => {
    switch (variant) {
      case 'secondary':
        return 'bg-slate-700 hover:bg-slate-600 text-gray-200 border border-slate-600';
      case 'outline':
        return 'bg-transparent hover:bg-slate-800 text-gray-300 border border-slate-600';
      case 'ghost':
        return 'bg-transparent hover:bg-slate-700/50 text-gray-300';
      case 'success':
        return 'bg-green-600 hover:bg-green-700 text-white';
      case 'warning':
        return 'bg-amber-500 hover:bg-amber-600 text-white';
      case 'error':
        return 'bg-red-600 hover:bg-red-700 text-white';
      default:
        return 'bg-blue-600 hover:bg-blue-700 text-white';
    }
  };

  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      className={`
        inline-flex items-center justify-center rounded-lg font-medium
        transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-blue-500/50
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
        ${sizeClasses[size]}
        ${getVariantClasses()}
        ${className}
      `}
      {...props}
    >
      {children}
    </button>
  );
};

export default Button;