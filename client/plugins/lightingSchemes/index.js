lightingSchemes = {
    
    start() {
        this.createPluginWindow();
        this.addEventListeners();
    },
    
    createPluginWindow() {
        const container = document.createElement('div');
        container.className = 'window window_bg text-white';
        container.innerHTML = `
            <div class="window_title p-2">
                <span>Lighting Schemes</span>
            </div>
            <div class="container window_body p-2">
                <select id="schemeSelect" class="p-2 mb-2">
                    <option value="normal">Normal</option>
                    <option value="dystopian">Dystopian</option>
                    <option value="cyberpunk">Cyberpunk</option>
                    <option value="wasteland">Wasteland</option>
                </select>
                
                <div class="color-inputs mt-2">
                    <div class="mb-2">
                        <label>Day Color:</label>
                        <input type="color" id="dayColorPicker" class="ml-2">
                    </div>
                    <div class="mb-2">
                        <label>Night Color:</label>
                        <input type="color" id="nightColorPicker" class="ml-2">
                    </div>
                    <div class="mb-2">
                        <label>Day Brightness:</label>
                        <input type="range" id="dayBrightness" min="0" max="3" step="0.1" class="ml-2">
                        <span id="dayBrightnessValue"></span>
                    </div>
                    <div class="mb-2">
                        <label>Day Saturation:</label>
                        <input type="range" id="daySaturation" min="0" max="3" step="0.1" class="ml-2">
                        <span id="daySaturationValue"></span>
                    </div>
                </div>
            </div>
        `;
        
        document.body.appendChild(container);
    },
    
    addEventListeners() {
        const schemeSelect = document.getElementById('schemeSelect');
        const dayColorPicker = document.getElementById('dayColorPicker');
        const nightColorPicker = document.getElementById('nightColorPicker');
        const dayBrightness = document.getElementById('dayBrightness');
        const daySaturation = document.getElementById('daySaturation');
        const dayBrightnessValue = document.getElementById('dayBrightnessValue');
        const daySaturationValue = document.getElementById('daySaturationValue');
        
        const updateScheme = () => {
            const schemeName = schemeSelect.value;
            const scheme = lighting.colorSchemes[schemeName];
            
            const dayColor = this.hexToRgb(dayColorPicker.value);
            const nightColor = this.hexToRgb(nightColorPicker.value);
            const brightness = parseFloat(dayBrightness.value);
            const saturation = parseFloat(daySaturation.value);
            
            dayBrightnessValue.textContent = brightness.toFixed(1);
            daySaturationValue.textContent = saturation.toFixed(1);
            
            scheme.dayColor = dayColor;
            scheme.nightColor = nightColor;
            scheme.dayBrightness = brightness;
            scheme.daySaturation = saturation;
            
            lighting.nightFilter.dayColor = dayColor;
            lighting.nightFilter.nightColor = nightColor;
            lighting.nightFilter.dayBrightness = brightness;
            lighting.nightFilter.daySaturation = saturation;
            
            const processedColor = lighting.applyBrightnessSaturation(dayColor, brightness, saturation);
            lighting.nightFilter.dayColor = processedColor;
            lighting.lastBaseNightFilterColor = null;
            lighting.lastProcessedNightFilterColor = null;
        };
        
        schemeSelect.addEventListener('change', () => {
            const selectedScheme = lighting.colorSchemes[schemeSelect.value];
            this.updateInputs(selectedScheme);
            updateScheme();
        });
        
        dayColorPicker.addEventListener('input', updateScheme);
        nightColorPicker.addEventListener('input', updateScheme);
        dayBrightness.addEventListener('input', updateScheme);
        daySaturation.addEventListener('input', updateScheme);
        
        this.updateInputs(lighting.colorSchemes[schemeSelect.value]);
    },
    
    updateInputs(scheme) {
        const dayColorPicker = document.getElementById('dayColorPicker');
        const nightColorPicker = document.getElementById('nightColorPicker');
        const dayBrightness = document.getElementById('dayBrightness');
        const daySaturation = document.getElementById('daySaturation');
        const dayBrightnessValue = document.getElementById('dayBrightnessValue');
        const daySaturationValue = document.getElementById('daySaturationValue');
        
        dayColorPicker.value = this.rgbToHex(scheme.dayColor);
        nightColorPicker.value = this.rgbToHex(scheme.nightColor);
        dayBrightness.value = scheme.dayBrightness;
        daySaturation.value = scheme.daySaturation;
        dayBrightnessValue.textContent = scheme.dayBrightness.toFixed(1);
        daySaturationValue.textContent = scheme.daySaturation.toFixed(1);
    },
    
    hexToRgb(hex) {
        const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
        return {
            r: parseInt(result[1], 16),
            g: parseInt(result[2], 16),
            b: parseInt(result[3], 16)
        };
    },
    
    rgbToHex(color) {
        return '#' + [color.r, color.g, color.b]
            .map(x => {
                const hex = x.toString(16);
                return hex.length === 1 ? '0' + hex : hex;
            })
            .join('');
    },
    
    unmount() {
        const container = document.querySelector('.window');
        if (container) {
            container.remove();
        }
    }
};