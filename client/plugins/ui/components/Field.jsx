import Input from './Input';
import Textarea from './Textarea';
import Select from './Select';

const Field = ({ 
  label,
  description,
  error,
  success,
  required = false,
  type = 'input',
  orientation = 'vertical',
  className = '',
  labelClassName = '',
  children,
  ...props 
}) => {
  const renderInput = () => {
    const commonProps = {
      error: !!error,
      success: !!success,
      ...props
    };

    switch (type) {
      case 'textarea':
        return <Textarea {...commonProps} />;
      case 'select':
        return <Select {...commonProps} />;
      default:
        return <Input {...commonProps} />;
    }
  };

  const isHorizontal = orientation === 'horizontal';

  return (
    <div className={`${isHorizontal ? 'flex items-center gap-3' : 'space-y-2'} ${className}`}>
      {label && (
        <div className={isHorizontal ? 'flex-shrink-0' : ''}>
          <label className={`
            text-xs font-medium text-gray-300 uppercase tracking-wide
            ${required ? "after:content-['*'] after:text-red-400 after:ml-1" : ''}
            ${labelClassName}
          `}>
            {label}
          </label>
          {description && !isHorizontal && (
            <div className="text-xs text-gray-500 mt-0.5">{description}</div>
          )}
        </div>
      )}
      
      <div className={isHorizontal ? 'flex-1' : ''}>
        {children || renderInput()}
        
        {description && isHorizontal && (
          <div className="text-xs text-gray-500 mt-1">{description}</div>
        )}
        
        {error && (
          <div className="text-xs text-red-400 mt-1">{error}</div>
        )}
        
        {success && (
          <div className="text-xs text-green-400 mt-1">{success}</div>
        )}
      </div>
    </div>
  );
};

export default Field;