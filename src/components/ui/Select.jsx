import { Show, For } from 'solid-js';

const Select = (props) => {
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

  const size = props.size || 'md';
  const disabled = props.disabled || false;
  const className = props.className || '';
  const options = props.options || [];

  return (
    <select
      value={props.value}
      onChange={(e) => props.onChange?.(e.target.value)}
      disabled={disabled}
      class={`
        w-full bg-slate-800/80 border text-white rounded-lg 
        focus:outline-none focus:ring-2 transition-all cursor-pointer
        ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
        ${sizeClasses[size]}
        ${getVariantClasses()}
        ${className}
      `}
      {...props}
    >
      <Show when={props.placeholder}>
        <option value="" disabled>
          {props.placeholder}
        </option>
      </Show>
      <For each={options}>
        {(option, index) => (
          <option 
            value={option.value || option}
          >
            {option.label || option}
          </option>
        )}
      </For>
    </select>
  );
};

export default Select;