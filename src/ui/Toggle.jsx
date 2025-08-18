import { createSignal, createEffect, Show } from 'solid-js';

const Toggle = (props) => {
  const [isChecked, setIsChecked] = createSignal(props.checked || false);

  createEffect(() => {
    setIsChecked(props.checked || false);
  });

  const handleToggle = () => {
    if (props.disabled) return;
    const newValue = !isChecked();
    setIsChecked(newValue);
    props.onChange?.(newValue);
  };

  const size = props.size || 'md';
  const variant = props.variant || 'primary';
  const disabled = props.disabled || false;
  const className = props.className || '';

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

  const translateClasses = () => ({
    sm: isChecked() ? 'translate-x-4' : 'translate-x-0.5',
    md: isChecked() ? 'translate-x-6' : 'translate-x-1',
    lg: isChecked() ? 'translate-x-7' : 'translate-x-1'
  })[size];

  const getBackgroundColor = () => {
    if (disabled) return 'bg-gray-600';
    if (isChecked()) {
      switch (variant) {
        case 'success': return 'bg-green-500 shadow-lg shadow-green-500/30';
        case 'warning': return 'bg-amber-400 shadow-lg shadow-amber-400/30';
        case 'error': return 'bg-red-500 shadow-lg shadow-red-500/30';
        default: return 'bg-blue-500 shadow-lg shadow-blue-500/30';
      }
    }
    return 'bg-slate-600';
  };

  return (
    <Show
      when={props.label}
      fallback={
        <button
          onClick={handleToggle}
          disabled={disabled}
          type="button"
          role="switch"
          aria-checked={isChecked()}
          class={`
            relative inline-flex items-center rounded-full transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:ring-offset-2 focus:ring-offset-slate-900
            ${sizeClasses[size]}
            ${getBackgroundColor()}
            ${disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}
            ${className}
          `}
          {...props}
        >
          <span
            class={`
              inline-block transform rounded-full bg-white transition-transform duration-200 shadow-sm
              ${thumbSizeClasses[size]}
              ${translateClasses()}
            `}
          />
        </button>
      }
    >
      <div class={`flex items-center justify-between ${className}`}>
        <div class="flex-1 mr-3">
          <div class="text-xs font-medium text-gray-300">{props.label}</div>
          <Show when={props.description}>
            <div class="text-xs text-gray-500 mt-0.5">{props.description}</div>
          </Show>
        </div>
        
        <button
          onClick={handleToggle}
          disabled={disabled}
          type="button"
          role="switch"
          aria-checked={isChecked()}
          class={`
            relative inline-flex items-center rounded-full transition-all duration-200 focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:ring-offset-2 focus:ring-offset-slate-900
            ${sizeClasses[size]}
            ${getBackgroundColor()}
            ${disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}
          `}
        >
          <span
            class={`
              inline-block transform rounded-full bg-white transition-transform duration-200 shadow-sm
              ${thumbSizeClasses[size]}
              ${translateClasses()}
            `}
          />
        </button>
      </div>
    </Show>
  );
};

export default Toggle;
