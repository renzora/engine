import { useState, useRef, useEffect, useCallback } from 'react';

const CustomColorPicker = ({ 
  color = '#ffffff', 
  onChange, 
  onClose,
  position = { x: 0, y: 0 }
}) => {
  const [hue, setHue] = useState(0);
  const [saturation, setSaturation] = useState(0);
  const [lightness, setLightness] = useState(100);
  const [hex, setHex] = useState(color);
  
  const satLightRef = useRef(null);
  const hueRef = useRef(null);
  const isDraggingRef = useRef({ satLight: false, hue: false });
  const isUserInteractingRef = useRef(false);

  // Convert hex to HSL
  const hexToHsl = useCallback((hex) => {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;

    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    let h, s, l = (max + min) / 2;

    if (max === min) {
      h = s = 0; // achromatic
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
  }, []);

  // Convert HSL to hex
  const hslToHex = useCallback((h, s, l) => {
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
      r = g = b = l; // achromatic
    } else {
      const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
      const p = 2 * l - q;
      r = hue2rgb(p, q, h + 1/3);
      g = hue2rgb(p, q, h);
      b = hue2rgb(p, q, h - 1/3);
    }

    const toHex = (c) => {
      const hex = Math.round(c * 255).toString(16);
      return hex.length === 1 ? '0' + hex : hex;
    };

    return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
  }, []);

  // Initialize HSL from color prop and update when color prop changes (only when not interacting)
  useEffect(() => {
    if (color && color !== hex && !isUserInteractingRef.current) {
      const [h, s, l] = hexToHsl(color);
      setHue(h);
      setSaturation(s);
      setLightness(l);
      setHex(color);
      console.log('🎨 ColorPicker: Updated from external color:', color, 'HSL:', h, s, l);
    }
  }, [color, hexToHsl]); // Removed 'hex' from dependencies to avoid conflicts

  // Update hex when HSL changes (only when user interacts, not from external changes)
  useEffect(() => {
    const newHex = hslToHex(hue, saturation, lightness);
    if (newHex !== hex && newHex !== color) {
      console.log('🎨 ColorPicker: HSL changed, updating hex:', newHex, 'from HSL:', hue, saturation, lightness);
      setHex(newHex);
      onChange?.(newHex);
    }
  }, [hue, saturation, lightness, hex, color, hslToHex, onChange]);

  const handleSatLightMouseDown = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    isDraggingRef.current.satLight = true;
    isUserInteractingRef.current = true;
    
    const rect = satLightRef.current.getBoundingClientRect();
    const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    setSaturation(Math.round(x * 100));
    setLightness(Math.round((1 - y) * 100));
    
    // Define temporary handlers for this drag session
    const tempHandleMouseMove = (e) => {
      if (isDraggingRef.current.satLight) {
        const rect = satLightRef.current.getBoundingClientRect();
        const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setSaturation(Math.round(x * 100));
        setLightness(Math.round((1 - y) * 100));
      }
      
      if (isDraggingRef.current.hue) {
        const rect = hueRef.current.getBoundingClientRect();
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setHue(Math.round(y * 360));
      }
    };
    
    const tempHandleMouseUp = () => {
      isDraggingRef.current.satLight = false;
      isDraggingRef.current.hue = false;
      
      // Remove listeners immediately
      document.removeEventListener('mousemove', tempHandleMouseMove, { capture: true });
      document.removeEventListener('mouseup', tempHandleMouseUp, { capture: true });
      
      // Set a small delay before allowing external updates again
      setTimeout(() => {
        isUserInteractingRef.current = false;
      }, 100);
    };
    
    // Add listeners dynamically
    document.addEventListener('mousemove', tempHandleMouseMove, { capture: true });
    document.addEventListener('mouseup', tempHandleMouseUp, { capture: true });
  }, []);

  const handleHueMouseDown = useCallback((e) => {
    e.preventDefault();
    e.stopPropagation();
    isDraggingRef.current.hue = true;
    isUserInteractingRef.current = true;
    
    const rect = hueRef.current.getBoundingClientRect();
    const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
    setHue(Math.round(y * 360));
    
    // Define temporary handlers for this drag session
    const tempHandleMouseMove = (e) => {
      if (isDraggingRef.current.satLight) {
        const rect = satLightRef.current.getBoundingClientRect();
        const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setSaturation(Math.round(x * 100));
        setLightness(Math.round((1 - y) * 100));
      }
      
      if (isDraggingRef.current.hue) {
        const rect = hueRef.current.getBoundingClientRect();
        const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));
        setHue(Math.round(y * 360));
      }
    };
    
    const tempHandleMouseUp = () => {
      isDraggingRef.current.satLight = false;
      isDraggingRef.current.hue = false;
      
      // Remove listeners immediately
      document.removeEventListener('mousemove', tempHandleMouseMove, { capture: true });
      document.removeEventListener('mouseup', tempHandleMouseUp, { capture: true });
      
      // Set a small delay before allowing external updates again
      setTimeout(() => {
        isUserInteractingRef.current = false;
      }, 100);
    };
    
    // Add listeners dynamically
    document.addEventListener('mousemove', tempHandleMouseMove, { capture: true });
    document.addEventListener('mouseup', tempHandleMouseUp, { capture: true });
  }, []);

  // Cleanup on unmount - ensure no lingering states
  useEffect(() => {
    return () => {
      console.log('🧹 ColorPicker unmounting, ensuring complete cleanup');
      // Ensure all drag states are reset on cleanup
      isDraggingRef.current.satLight = false;
      isDraggingRef.current.hue = false;
      isUserInteractingRef.current = false;
    };
  }, []);

  // Close on escape key
  useEffect(() => {
    const handleKeyDown = (e) => {
      if (e.key === 'Escape') {
        onClose?.();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);


  const handleHexInputChange = (e) => {
    const value = e.target.value;
    if (/^#[0-9A-Fa-f]{0,6}$/.test(value)) {
      isUserInteractingRef.current = true;
      setHex(value);
      if (value.length === 7) {
        onChange?.(value);
        const [h, s, l] = hexToHsl(value);
        setHue(h);
        setSaturation(s);
        setLightness(l);
        // Allow external updates after a delay
        setTimeout(() => {
          isUserInteractingRef.current = false;
        }, 100);
      }
    }
  };

  return (
    <div 
      className="fixed z-50 bg-gray-800 border border-gray-600 rounded-lg shadow-xl p-4 select-none color-picker"
      style={{
        left: position.x,
        top: position.y,
        width: '280px'
      }}
    >
      {/* Header */}
      <div className="flex justify-between items-center mb-3">
        <h3 className="text-sm font-medium text-gray-200">Color Picker</h3>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onClose?.();
          }}
          className="text-gray-400 hover:text-gray-200 text-lg leading-none"
        >
          ×
        </button>
      </div>

      <div className="flex gap-3">
        {/* Saturation/Lightness Area */}
        <div className="relative w-48 h-48 rounded cursor-crosshair">
          <div
            ref={satLightRef}
            className="w-full h-full rounded"
            style={{
              background: `linear-gradient(to right, #ffffff, hsl(${hue}, 100%, 50%))`,
            }}
            onMouseDown={handleSatLightMouseDown}
          >
            <div
              className="w-full h-full rounded"
              style={{
                background: 'linear-gradient(to bottom, transparent, #000000)',
              }}
            >
              {/* Crosshair indicator */}
              <div
                className="absolute w-3 h-3 border-2 border-white rounded-full transform -translate-x-1/2 -translate-y-1/2 pointer-events-none"
                style={{
                  left: `${saturation}%`,
                  top: `${100 - lightness}%`,
                  boxShadow: '0 0 0 1px rgba(0,0,0,0.3)'
                }}
              />
            </div>
          </div>
        </div>

        {/* Hue Slider */}
        <div className="relative w-6 h-48 rounded cursor-pointer">
          <div
            ref={hueRef}
            className="w-full h-full rounded"
            style={{
              background: 'linear-gradient(to bottom, #ff0000 0%, #ffff00 17%, #00ff00 33%, #00ffff 50%, #0000ff 67%, #ff00ff 83%, #ff0000 100%)'
            }}
            onMouseDown={handleHueMouseDown}
          >
            {/* Hue indicator */}
            <div
              className="absolute w-full h-1 border border-white transform -translate-y-1/2 pointer-events-none"
              style={{
                top: `${(hue / 360) * 100}%`,
                backgroundColor: `hsl(${hue}, 100%, 50%)`,
                boxShadow: '0 0 0 1px rgba(0,0,0,0.3)'
              }}
            />
          </div>
        </div>
      </div>

      {/* Color Preview and Hex Input */}
      <div className="flex items-center gap-3 mt-3">
        <div
          className="w-8 h-8 rounded border border-gray-600"
          style={{ backgroundColor: hex }}
        />
        <input
          type="text"
          value={hex}
          onChange={handleHexInputChange}
          onClick={(e) => e.stopPropagation()}
          className="flex-1 px-2 py-1 text-sm bg-gray-700 border border-gray-600 rounded text-white focus:outline-none focus:border-blue-500"
          placeholder="#ffffff"
        />
      </div>

      {/* HSL Values (for reference) */}
      <div className="text-xs text-gray-400 mt-2">
        HSL({hue}, {saturation}%, {lightness}%)
      </div>
    </div>
  );
};

export default CustomColorPicker;