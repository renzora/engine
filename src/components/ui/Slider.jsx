import { createSignal, createEffect, Show } from 'solid-js';

const Slider = (props) => {
  const [currentValue, setCurrentValue] = createSignal(props.value ?? props.min ?? 0);

  // Update internal state when props.value changes
  createEffect(() => {
    setCurrentValue(props.value ?? props.min ?? 0);
  });

  const handleChange = (e) => {
    const newValue = parseFloat(e.target.value);
    setCurrentValue(newValue);
    props.onChange?.(newValue);
  };

  const min = props.min ?? 0;
  const max = props.max ?? 100;
  const step = props.step ?? 1;
  const disabled = props.disabled || false;
  const size = props.size || 'md';
  const variant = props.variant || 'primary';
  const showValue = props.showValue ?? true;
  const className = props.className || '';

  const sizeClasses = {
    sm: 'h-1',
    md: 'h-2',
    lg: 'h-3'
  };

  const getTrackColor = () => {
    switch (variant) {
      case 'success': return 'accent-green-500';
      case 'warning': return 'accent-amber-400';
      case 'error': return 'accent-red-500';
      default: return 'accent-blue-500';
    }
  };

  return (
    <div class={`space-y-2 ${className}`}>
      <div class="flex items-center justify-between">
        <Show when={showValue}>
          <span class="text-xs text-gray-300">
            {currentValue()}
          </span>
        </Show>
      </div>
      
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={currentValue()}
        onInput={handleChange}
        disabled={disabled}
        class={`
          w-full appearance-none bg-transparent cursor-pointer
          ${sizeClasses[size]}
          ${getTrackColor()}
          ${disabled ? 'opacity-50 cursor-not-allowed' : ''}
          
          [&::-webkit-slider-track]:bg-slate-600 
          [&::-webkit-slider-track]:rounded-lg
          [&::-webkit-slider-track]:${sizeClasses[size]}
          
          [&::-webkit-slider-thumb]:appearance-none
          [&::-webkit-slider-thumb]:bg-white
          [&::-webkit-slider-thumb]:rounded-full
          [&::-webkit-slider-thumb]:w-4
          [&::-webkit-slider-thumb]:h-4
          [&::-webkit-slider-thumb]:cursor-pointer
          [&::-webkit-slider-thumb]:shadow-md
          [&::-webkit-slider-thumb]:border-2
          [&::-webkit-slider-thumb]:border-slate-400
          
          [&::-moz-range-track]:bg-slate-600
          [&::-moz-range-track]:rounded-lg
          [&::-moz-range-track]:${sizeClasses[size]}
          [&::-moz-range-track]:border-none
          
          [&::-moz-range-thumb]:bg-white
          [&::-moz-range-thumb]:rounded-full
          [&::-moz-range-thumb]:w-4
          [&::-moz-range-thumb]:h-4
          [&::-moz-range-thumb]:cursor-pointer
          [&::-moz-range-thumb]:shadow-md
          [&::-moz-range-thumb]:border-2
          [&::-moz-range-thumb]:border-slate-400
          [&::-moz-range-thumb]:appearance-none
        `}
        {...props}
      />
      
      <div class="flex justify-between text-xs text-gray-500">
        <span>{min}</span>
        <span>{max}</span>
      </div>
    </div>
  );
};

export default Slider;