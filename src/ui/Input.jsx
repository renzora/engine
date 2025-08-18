import { Show } from 'solid-js';

const Input = (props) => {
  const sizeClasses = {
    sm: 'text-xs p-2',
    md: 'text-xs p-2.5',
    lg: 'text-sm p-3'
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

  const type = props.type || 'text';
  const disabled = props.disabled || false;
  const size = props.size || 'md';
  const className = props.className || '';

  const baseClasses = `
    w-full bg-slate-800/80 text-white rounded-lg 
    focus:outline-none focus:ring-2 transition-all
    ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
    ${sizeClasses[size]}
    ${getVariantClasses()}
    ${className}
  `;

  return (
    <Show 
      when={props.prefix || props.suffix}
      fallback={
        <input
          type={type}
          value={props.value}
          onInput={(e) => props.onChange?.(e.target.value)}
          placeholder={props.placeholder}
          disabled={disabled}
          class={baseClasses}
          {...props}
        />
      }
    >
      <div class="relative">
        <Show when={props.prefix}>
          <div class="absolute left-2 top-1/2 -translate-y-1/2 text-xs text-gray-400 pointer-events-none">
            {props.prefix}
          </div>
        </Show>
        <input
          type={type}
          value={props.value}
          onInput={(e) => props.onChange?.(e.target.value)}
          placeholder={props.placeholder}
          disabled={disabled}
          class={`${baseClasses} ${props.prefix ? 'pl-8' : ''} ${props.suffix ? 'pr-8' : ''}`}
          {...props}
        />
        <Show when={props.suffix}>
          <div class="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-gray-400 pointer-events-none">
            {props.suffix}
          </div>
        </Show>
      </div>
    </Show>
  );
};

export default Input;
