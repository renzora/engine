import { createSignal, createEffect, onCleanup } from 'solid-js';

const CustomColorPicker = ({ 
  color = '#ffffff', 
  onChange, 
  onClose,
  position = { x: 0, y: 0 }
}) => {
  const [hue, setHue] = createSignal(0);
  const [saturation, setSaturation] = createSignal(0);
  const [lightness, setLightness] = createSignal(100);
  const [hex, setHex] = createSignal(color);
  
  let satLightRef;
  let hueRef;
  let isDragging = { satLight: false, hue: false };
  let isUserInteracting = false;

  const hexToHsl = (hex) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;

    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    let h, s, l = (max + min) / 2;

    if (max === min) {
      h = s = 0;
    } else {
      const d = max - min;
      s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
      switch (max) {
        case r: h = (g - b) / d + (g < b ? 6 : 0); break;
        case g: h = (b - r) / d + 2; break;
        case b: h = (r - g) / d + 4; break;
      }
      h /= 6;
    }

    return [Math.round(h * 360), Math.round(s * 100), Math.round(l * 100)];
  };

  const hslToHex = (h, s, l) => {
    h /= 360;
    s /= 100;
    l /= 100;

    const hue2rgb = (p, q, t) => {
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1/6) return p + (q - p) * 6 * t;
      if (t < 1/2) return q;
      if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
      return p;
    };

    let r, g, b;
    if (s === 0) {
      r = g = b = l;
    } else {
      const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
      const p = 2 * l - q;
      r = hue2rgb(p, q, h + 1/3);
      g = hue2rgb(p, q, h);
      b = hue2rgb(p, q, h - 1/3);
    }

    const toHex = (c) => {
      const hexVal = Math.round(c * 255).toString(16);
      return hexVal.length === 1 ? '0' + hexVal : hexVal;
    };

    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
  };

  createEffect(() => {
    if (color && color !== hex() && !isUserInteracting) {
      const [h, s, l] = hexToHsl(color);
      setHue(h);
      setSaturation(s);
      setLightness(l);
      setHex(color);
      console.log('🎨 ColorPicker: Updated from external color:', color, 'HSL:', h, s, l);
    }
  });

  createEffect(() => {
    const newHex = hslToHex(hue(), saturation(), lightness());
    if (newHex !== hex() && newHex !== color) {
      console.log('🎨 ColorPicker: HSL changed, updating hex:', newHex, 'from HSL:', hue(), saturation(), lightness());
      setHex(newHex);
      onChange?.(newHex);
    }
  });

  const handleSatLightMouseDown = (e) => {
    e.preventDefault();
    e.stopPropagation();
    isDragging.satLight = true;
    isUserInteracting = true;
    
    const rect = satLightRef.getBoundingClientRect();
    const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    setSaturation(Math.round(x * 100));
    setLightness(Math.round((1 - y) * 100));
    
    const tempHandleMouseMove = (e) => {
      if (isDragging.satLight) {
        const rect = satLightRef.getBoundingClientRect();
        const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setSaturation(Math.round(x * 100));
        setLightness(Math.round((1 - y) * 100));
      }
      
      if (isDragging.hue) {
        const rect = hueRef.getBoundingClientRect();
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setHue(Math.round(y * 360));
      }
    };
    
    const tempHandleMouseUp = () => {
      isDragging.satLight = false;
      isDragging.hue = false;
      document.removeEventListener('mousemove', tempHandleMouseMove, { capture: true });
      document.removeEventListener('mouseup', tempHandleMouseUp, { capture: true });
      
      setTimeout(() => {
        isUserInteracting = false;
      }, 100);
    };
    
    document.addEventListener('mousemove', tempHandleMouseMove, { capture: true });
    document.addEventListener('mouseup', tempHandleMouseUp, { capture: true });
  };

  const handleHueMouseDown = (e) => {
    e.preventDefault();
    e.stopPropagation();
    isDragging.hue = true;
    isUserInteracting = true;
    
    const rect = hueRef.getBoundingClientRect();
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    setHue(Math.round(y * 360));
    
    const tempHandleMouseMove = (e) => {
      if (isDragging.satLight) {
        const rect = satLightRef.getBoundingClientRect();
        const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setSaturation(Math.round(x * 100));
        setLightness(Math.round((1 - y) * 100));
      }
      
      if (isDragging.hue) {
        const rect = hueRef.getBoundingClientRect();
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setHue(Math.round(y * 360));
      }
    };
    
    const tempHandleMouseUp = () => {
      isDragging.satLight = false;
      isDragging.hue = false;
      document.removeEventListener('mousemove', tempHandleMouseMove, { capture: true });
      document.removeEventListener('mouseup', tempHandleMouseUp, { capture: true });
      
      setTimeout(() => {
        isUserInteracting = false;
      }, 100);
    };
    
    document.addEventListener('mousemove', tempHandleMouseMove, { capture: true });
    document.addEventListener('mouseup', tempHandleMouseUp, { capture: true });
  };

  onCleanup(() => {
    console.log('🧹 ColorPicker unmounting, ensuring complete cleanup');
    isDragging.satLight = false;
    isDragging.hue = false;
    isUserInteracting = false;
  });

  createEffect(() => {
    const handleKeyDown = (e) => {
      if (e.key === 'Escape') {
        onClose?.();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    onCleanup(() => document.removeEventListener('keydown', handleKeyDown));
  });

  const handleHexInputChange = (e) => {
    const value = e.target.value;
    if (/^#[0-9A-Fa-f]{0,6}$/.test(value)) {
      isUserInteracting = true;
      setHex(value);
      if (value.length === 7) {
        onChange?.(value);
        const [h, s, l] = hexToHsl(value);
        setHue(h);
        setSaturation(s);
        setLightness(l);
        setTimeout(() => {
          isUserInteracting = false;
        }, 100);
      }
    }
  };

  return (
    <div 
      class="fixed z-50 bg-gray-800 border border-gray-600 rounded-lg shadow-xl p-4 select-none color-picker"
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
        width: '280px'
      }}
    >
      <div class="flex justify-between items-center mb-3">
        <h3 class="text-sm font-medium text-gray-200">Color Picker</h3>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onClose?.();
          }}
          class="text-gray-400 hover:text-gray-200 text-lg leading-none"
        >
          ×
        </button>
      </div>

      <div class="flex gap-3">
        <div class="relative w-48 h-48 rounded cursor-crosshair">
          <div
            ref={satLightRef}
            class="w-full h-full rounded"
            style={{
              background: `linear-gradient(to right, #ffffff, hsl(${hue()}, 100%, 50%))`,
            }}
            onMouseDown={handleSatLightMouseDown}
          >
            <div
              class="w-full h-full rounded"
              style={{
                background: 'linear-gradient(to bottom, transparent, #000000)',
              }}
            >
              <div
                class="absolute w-3 h-3 border-2 border-white rounded-full transform -translate-x-1/2 -translate-y-1/2 pointer-events-none"
                style={{
                  left: `${saturation()}%`,
                  top: `${100 - lightness()}%`,
                  'box-shadow': '0 0 0 1px rgba(0,0,0,0.3)'
                }}
              />
            </div>
          </div>
        </div>

        <div class="relative w-6 h-48 rounded cursor-pointer">
          <div
            ref={hueRef}
            class="w-full h-full rounded"
            style={{
              background: 'linear-gradient(to bottom, #ff0000 0%, #ffff00 17%, #00ff00 33%, #00ffff 50%, #0000ff 67%, #ff00ff 83%, #ff0000 100%)'
            }}
            onMouseDown={handleHueMouseDown}
          >
            <div
              class="absolute w-full h-1 border border-white transform -translate-y-1/2 pointer-events-none"
              style={{
                top: `${(hue() / 360) * 100}%`,
                'background-color': `hsl(${hue()}, 100%, 50%)`,
                'box-shadow': '0 0 0 1px rgba(0,0,0,0.3)'
              }}
            />
          </div>
        </div>
      </div>

      <div class="flex items-center gap-3 mt-3">
        <div
          class="w-8 h-8 rounded border border-gray-600"
          style={{ 'background-color': hex() }}
        />
        <input
          type="text"
          value={hex()}
          onInput={handleHexInputChange}
          onClick={(e) => e.stopPropagation()}
          class="flex-1 px-2 py-1 text-sm bg-gray-700 border border-gray-600 rounded text-white focus:outline-none focus:border-blue-500"
          placeholder="#ffffff"
        />
      </div>

      <div class="text-xs text-gray-400 mt-2">
        HSL({hue()}, {saturation()}%, {lightness()}%)
      </div>
    </div>
  );
};

export default CustomColorPicker;