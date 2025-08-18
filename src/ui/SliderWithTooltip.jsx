import { createSignal, createEffect } from 'solid-js';

const SliderWithTooltip = ({ label, min, max, step, defaultValue, onChange }) => {
  const [value, setValue] = createSignal(defaultValue);
  const [tooltipVisible, setTooltipVisible] = createSignal(false);
  const [tooltipPosition, setTooltipPosition] = createSignal(0);
  let sliderRef;

  const handleInputChange = (e) => {
    const newValue = parseFloat(e.target.value);
    setValue(newValue);
    if (onChange) {
      onChange(newValue);
    }
  };

  const handleSliderChange = (e) => {
    const newValue = parseFloat(e.target.value);
    setValue(newValue);
    if (onChange) {
      onChange(newValue);
    }
    updateTooltipPosition(e.target);
  };

  const handleMouseDown = () => {
    setTooltipVisible(true);
  };

  const handleMouseUp = () => {
    setTooltipVisible(false);
  };

  const updateTooltipPosition = (target) => {
    if (target) {
      const minVal = parseFloat(target.min);
      const maxVal = parseFloat(target.max);
      const val = parseFloat(target.value);
      const percentage = (val - minVal) / (maxVal - minVal);
      const thumbWidth = 16;
      const trackWidth = target.offsetWidth - thumbWidth;
      const thumbPosition = percentage * trackWidth + thumbWidth / 2;
      setTooltipPosition(thumbPosition);
    }
  };

  createEffect(() => {
    if (sliderRef) {
      updateTooltipPosition(sliderRef);
    }
  });

  return (
    <div class="space-y-1 relative">
      <label class="text-xs text-gray-400">{label}</label>
      <div class="flex items-center space-x-2">
        <div class="relative flex-1">
          <input
            type="range"
            min={min}
            max={max}
            step={step}
            value={value()}
            onInput={handleSliderChange}
            onMouseDown={handleMouseDown}
            onMouseUp={handleMouseUp}
            ref={sliderRef}
            class="w-full h-1 bg-slate-700 rounded-lg appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-blue-500 [&::-webkit-slider-thumb]:shadow [&::-moz-range-thumb]:h-4 [&::-moz-range-thumb]:w-4 [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:bg-blue-500 [&::-moz-range-thumb]:shadow"
          />
          {tooltipVisible() && (
            <div
              class="absolute bg-blue-500 text-white text-xs rounded py-1 px-2 -mt-8 transform -translate-x-1/2 transition-opacity duration-100"
              style={{ 
                left: `${tooltipPosition()}px`, 
                opacity: tooltipVisible() ? 1 : 0 
              }}
            >
              {value().toFixed(1)}
              <div class="absolute left-1/2 transform -translate-x-1/2 border-t-4 border-t-blue-500 border-x-4 border-x-transparent bottom-[-4px]"></div>
            </div>
          )}
        </div>
        <input
          type="number"
          min={min}
          max={max}
          step={step}
          value={value()}
          onInput={handleInputChange}
          class="w-16 bg-slate-800 border border-slate-700 text-white text-xs p-1.5 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
      </div>
    </div>
  );
};

export default SliderWithTooltip;
