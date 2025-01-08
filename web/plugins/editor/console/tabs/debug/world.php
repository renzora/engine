<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if ($auth) {
?>
    <div class='text-white'>

        <div id="world_editor_tabs">
            <div id="tabs" class="flex border-b border-gray-700">
                <button class="tab text-white px-2 py-1 bg-gray-800 hover:bg-gray-700" data-tab="tab1">Time</button>
                <button class="tab text-white px-2 py-1 bg-gray-800 hover:bg-gray-700" data-tab="tab2">Weather</button>
                <button class="tab text-white px-2 py-1 bg-gray-800 hover:bg-gray-700" data-tab="tab3">Lighting</button>
                <button class="tab text-white px-2 py-1 bg-gray-800 hover:bg-gray-700" data-tab="tab4">Particles</button>
            </div>

            <div class="tab-content py-2 hidden" data-tab-content="tab1">
                <p class="mb-2">Set Game Time:</p>
                <label for="hours" class="block">Hours:</label>
                <input type="range" id="hours" name="hours" min="0" max="23" value="7" class="w-full mt-1 mb-2">
                <span id="hours_value" class="block text-sm text-gray-400">7</span>
                <label for="minutes" class="block">Minutes:</label>
                <input type="range" id="minutes" name="minutes" min="0" max="59" value="0" class="w-full mt-1 mb-2">
                <span id="minutes_value" class="block text-sm text-gray-400">0</span>
                <label for="seconds" class="block">Seconds:</label>
                <input type="range" id="seconds" name="seconds" min="0" max="59" value="0" class="w-full mt-1 mb-2">
                <span id="seconds_value" class="block text-sm text-gray-400">0</span>
                <label for="days" class="block">Days:</label>
                <input type="range" id="days" name="days" min="0" max="7" value="0" class="w-full mt-1 mb-2">
                <span id="days_value" class="block text-sm text-gray-400">0</span>
                <label for="speed_multiplier" class="block">Speed Multiplier:</label>
                <input type="range" id="speed_multiplier" name="speed_multiplier" min="1" max="5000" value="1000" class="w-full mt-1 mb-2">
                <span id="speed_multiplier_value" class="block text-sm text-gray-400">1000</span>
            </div>

            <div class="tab-content py-2 hidden" data-tab-content="tab2">
                <label for="toggle_snow" class="block">Snow:</label>
                <input type="checkbox" id="toggle_snow" class="mb-4">
                <label for="toggle_rain" class="block">Rain:</label>
                <input type="checkbox" id="toggle_rain" class="mb-4">
                <label for="toggle_fog" class="block">Fog:</label>
                <input type="checkbox" id="toggle_fog" class="mb-4">
                <label for="toggle_fireflys" class="block">FireFlys:</label>
                <input type="checkbox" id="toggle_fireflys" class="mb-4">
                <button id="set_weather_button" class="mt-4 bg-green-600 hover:bg-green-500 text-white font-bold py-2 px-4 rounded shadow-md">Set Weather</button>
            </div>

            <div class="tab-content py-2 hidden" data-tab-content="tab3">
    <div id="lightTypeContainer" class="space-y-4"></div>
    
    <div class="form-group mt-3">
        <label for="compositeOperation" class="block">Composite Operation</label>
        <select id="compositeOperation" class="form-control bg-gray-800 border-gray-600 text-white rounded mt-1" onchange="ui_console_tab_window.updateCompositeOperation()">
            <option value="source-over">source-over</option>
            <option value="source-in">source-in</option>
            <option value="source-out">source-out</option>
            <option value="source-atop">source-atop</option>
            <option value="destination-over">destination-over</option>
            <option value="destination-in">destination-in</option>
            <option value="destination-out">destination-out</option>
            <option value="destination-atop">destination-atop</option>
            <option value="lighter">lighter</option>
            <option value="copy">copy</option>
            <option value="xor">xor</option>
            <option value="multiply">multiply</option>
            <option value="screen">screen</option>
            <option value="overlay">overlay</option>
            <option value="darken">darken</option>
            <option value="lighten">lighten</option>
            <option value="color-dodge">color-dodge</option>
            <option value="color-burn">color-burn</option>
            <option value="hard-light">hard-light</option>
            <option value="soft-light">soft-light</option>
            <option value="difference">difference</option>
            <option value="exclusion">exclusion</option>
            <option value="hue">hue</option>
            <option value="saturation">saturation</option>
            <option value="color">color</option>
            <option value="luminosity">luminosity</option>
        </select>
    </div>

    <div class="form-group mt-3">
        <label for="nightFilterOpacity" class="block">Night Filter Opacity</label>
        <input type="range" id="nightFilterOpacity" class="form-control-range w-full mt-1 mb-2" min="0" max="1" step="0.01">
        <span id="nightFilterOpacityLabel" class="block text-sm text-gray-400"></span>
    </div>

    <div class="form-group">
        <label for="nightFilterColorR" class="block">Night Filter Color (R)</label>
        <input type="range" id="nightFilterColorR" class="form-control-range w-full mt-1 mb-2" min="0" max="255">
        <span id="nightFilterColorRLabel" class="block text-sm text-gray-400"></span>
    </div>
    <div class="form-group">
        <label for="nightFilterColorG" class="block">Night Filter Color (G)</label>
        <input type="range" id="nightFilterColorG" class="form-control-range w-full mt-1 mb-2" min="0" max="255">
        <span id="nightFilterColorGLabel" class="block text-sm text-gray-400"></span>
    </div>
    <div class="form-group">
        <label for="nightFilterColorB" class="block">Night Filter Color (B)</label>
        <input type="range" id="nightFilterColorB" class="form-control-range w-full mt-1 mb-2" min="0" max="255">
        <span id="nightFilterColorBLabel" class="block text-sm text-gray-400"></span>
    </div>

    <div class="form-group">
        <label for="toggleTimeBasedUpdates" class="block">Enable Time-based Updates</label>
        <input type="checkbox" id="toggleTimeBasedUpdates" class="mb-4">
    </div>

    <div class="form-group">
        <label for="nightFilterCompositeOperation" class="block">Night Filter Composite Operation</label>
        <select id="nightFilterCompositeOperation" class="form-control bg-gray-800 border-gray-600 text-white rounded mt-1" onchange="ui_console_tab_window.updateNightFilterCompositeOperation()">
            <option value="source-over">source-over</option>
            <option value="source-in">source-in</option>
            <option value="source-out">source-out</option>
            <option value="source-atop">source-atop</option>
            <option value="destination-over">destination-over</option>
            <option value="destination-in">destination-in</option>
            <option value="destination-out">destination-out</option>
            <option value="destination-atop">destination-atop</option>
            <option value="lighter">lighter</option>
            <option value="copy">copy</option>
            <option value="xor">xor</option>
            <option value="multiply">multiply</option>
            <option value="screen">screen</option>
            <option value="overlay">overlay</option>
            <option value="darken">darken</option>
            <option value="lighten">lighten</option>
            <option value="color-dodge">color-dodge</option>
            <option value="color-burn">color-burn</option>
            <option value="hard-light">hard-light</option>
            <option value="soft-light">soft-light</option>
            <option value="difference">difference</option>
            <option value="exclusion">exclusion</option>
            <option value="hue">hue</option>
            <option value="saturation">saturation</option>
            <option value="color">color</option>
            <option value="luminosity">luminosity</option>
        </select>
    </div>
    
    <div class="form-group mt-3">
        <label for="greyFilterOpacity" class="block">Grey Filter Opacity</label>
        <input type="range" id="greyFilterOpacity" class="form-control-range w-full mt-1 mb-2" min="0" max="1" step="0.01">
        <span id="greyFilterOpacityLabel" class="block text-sm text-gray-400"></span>
    </div>

    <div class="form-group">
        <label for="greyFilterColorR" class="block">Grey Filter Color (R)</label>
        <input type="range" id="greyFilterColorR" class="form-control-range w-full mt-1 mb-2" min="0" max="255">
        <span id="greyFilterColorRLabel" class="block text-sm text-gray-400"></span>
    </div>
    <div class="form-group">
        <label for="greyFilterColorG" class="block">Grey Filter Color (G)</label>
        <input type="range" id="greyFilterColorG" class="form-control-range w-full mt-1 mb-2" min="0" max="255">
        <span id="greyFilterColorGLabel" class="block text-sm text-gray-400"></span>
    </div>
    <div class="form-group">
        <label for="greyFilterColorB" class="block">Grey Filter Color (B)</label>
        <input type="range" id="greyFilterColorB" class="form-control-range w-full mt-1 mb-2" min="0" max="255">
        <span id="greyFilterColorBLabel" class="block text-sm text-gray-400"></span>
    </div>
</div>


            <div class="tab-content py-2 hidden" data-tab-content="tab4">
            <button onclick="ui_console_tab_window.createNewParticleEffect()" class="mt-4 bg-blue-600 hover:bg-blue-500 text-white font-bold py-2 px-4 rounded shadow-md">Add Particle</button>
                <select id="effectSelect" class="w-full mb-4 p-2 border rounded bg-gray-800 text-white">
                    <option value="">Select Effect</option>
                </select>

                <div id="effectControls" class="space-y-4" style="display: none;">
                <button onclick="ui_console_tab_window.removeParticleEffect()" class="mt-4 bg-red-600 hover:bg-red-500 text-white font-bold py-2 px-4 rounded shadow-md">Remove Particle</button>
                    <div class="slider-group">
                        <label for="particleRepeat" class="flex justify-between">Repeat: <span id="particleRepeatValue" class="text-sm text-gray-400"></span></label>
                        <input type="checkbox" id="particleRepeat" class="w-full">
                    </div>
                    <div class="slider-group">
                        <label for="particleX" class="flex justify-between">X Position: <span id="particleXValue" class="text-sm text-gray-400">100</span></label>
                        <input type="range" id="particleX" min="0" max="800" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleY" class="flex justify-between">Y Position: <span id="particleYValue" class="text-sm text-gray-400">100</span></label>
                        <input type="range" id="particleY" min="0" max="600" step="1" class="w-full mt-1 mb-2">
                    </div>

                    <div class="slider-group">
                        <label for="particleCount" class="flex justify-between">Count: <span id="particleCountValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleCount" min="1" max="100" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleSpeed" class="flex justify-between">Speed: <span id="particleSpeedValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleSpeed" min="0.1" max="10" step="0.1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleLife" class="flex justify-between">Life: <span id="particleLifeValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleLife" min="1" max="500" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleSize" class="flex justify-between">Size: <span id="particleSizeValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleSize" min="1" max="20" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleGlow" class="flex justify-between">Glow: <span id="particleGlowValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleGlow" min="0" max="20" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleOpacity" class="flex justify-between">Opacity: <span id="particleOpacityValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleOpacity" min="0.1" max="1" step="0.1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleBlur" class="flex justify-between">Blur: <span id="particleBlurValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleBlur" min="0" max="50" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleShape" class="flex justify-between">Shape: <span id="particleShapeValue" class="text-sm text-gray-400"></span></label>
                        <select id="particleShape" class="w-full p-2 border rounded bg-gray-800 text-white mt-1">
                            <option value="circle">Circle</option>
                            <option value="square">Square</option>
                            <option value="triangle">Triangle</option>
                        </select>
                    </div>
                    <div class="slider-group">
                        <label for="particleColor1" class="flex justify-between">Color 1: <span id="particleColor1Value" class="text-sm text-gray-400"></span></label>
                        <input type="color" id="particleColor1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleColor2" class="flex justify-between">Color 2: <span id="particleColor2Value" class="text-sm text-gray-400"></span></label>
                        <input type="color" id="particleColor2" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleColor3" class="flex justify-between">Color 3: <span id="particleColor3Value" class="text-sm text-gray-400"></span></label>
                        <input type="color" id="particleColor3" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleAngle" class="flex justify-between">Angle: <span id="particleAngleValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleAngle" min="0" max="360" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleSpread" class="flex justify-between">Spread: <span id="particleSpreadValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleSpread" min="0" max="360" step="1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleGravity" class="flex justify-between">Gravity: <span id="particleGravityValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleGravity" min="-10" max="10" step="0.1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleSway" class="flex justify-between">Sway: <span id="particleSwayValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleSway" min="0" max="10" step="0.1" class="w-full mt-1 mb-2">
                    </div>
                    <div class="slider-group">
                        <label for="particleTwinkle" class="flex justify-between">Twinkle Speed: <span id="particleTwinkleValue" class="text-sm text-gray-400"></span></label>
                        <input type="range" id="particleTwinkle" min="0" max="5" step="0.1" class="w-full mt-1 mb-2">
                    </div>
                </div>
            </div>
        </div>

    </div>

    <script>
var ui_console_tab_window = {
    start: function() {
        ui.initTabs('world_editor_tabs', 'tab1');
        this.initSliders();
        this.bindEvents();
        this.initLightTypes();
        this.initNightFilter();
        this.initTimeBasedUpdates();
        this.populateEffectList();
        this.initParticleEventListeners();
    },
    
    setNightFilterCompositeOperation: function() {
        var compositeOperation = lighting.nightFilter.compositeOperation;
        var selectElement = document.getElementById('nightFilterCompositeOperation');
        if (selectElement) {
            selectElement.value = compositeOperation;
        }
    },

    updateNightFilterCompositeOperation: function() {
        const compositeOperation = document.getElementById('nightFilterCompositeOperation').value;
        if (lighting && lighting.nightFilter) {
            lighting.nightFilter.compositeOperation = compositeOperation;
            console.log(`Night Filter Composite Operation set to: ${compositeOperation}`);
        } else {
            console.error('Lighting object or nightFilter not found');
        }
    },

    initNightFilter: function() {
        document.getElementById('nightFilterOpacity').value = lighting.nightFilter.opacity;
        document.getElementById('nightFilterColorR').value = lighting.nightFilter.color.r;
        document.getElementById('nightFilterColorG').value = lighting.nightFilter.color.g;
        document.getElementById('nightFilterColorB').value = lighting.nightFilter.color.b;

        this.setNightFilterCompositeOperation();

        document.getElementById('nightFilterOpacity').addEventListener('input', function() {
            const opacity = parseFloat(this.value);
            lighting.nightFilter.opacity = opacity;
            document.getElementById('nightFilterOpacityLabel').textContent = opacity;
        });

        document.getElementById('nightFilterColorR').addEventListener('input', function() {
            const r = parseInt(this.value);
            lighting.nightFilter.color.r = r;
            document.getElementById('nightFilterColorRLabel').textContent = r;
        });

        document.getElementById('nightFilterColorG').addEventListener('input', function() {
            const g = parseInt(this.value);
            lighting.nightFilter.color.g = g;
            document.getElementById('nightFilterColorGLabel').textContent = g;
        });

        document.getElementById('nightFilterColorB').addEventListener('input', function() {
            const b = parseInt(this.value);
            lighting.nightFilter.color.b = b;
            document.getElementById('nightFilterColorBLabel').textContent = b;
        });

        this.updateNightFilterCompositeOperation();
    },

    initSliders: function() {
        this.updateSliderValue('hours', 'hours_value');
        this.updateSliderValue('minutes', 'minutes_value');
        this.updateSliderValue('seconds', 'seconds_value');
        this.updateSliderValue('days', 'days_value');
        this.updateSliderValue('speed_multiplier', 'speed_multiplier_value');
    },

    updateSliderValue: function(sliderId, valueId) {
        var slider = document.getElementById(sliderId);
        var valueSpan = document.getElementById(valueId);
        var self = this;
        slider.addEventListener('input', function() {
            valueSpan.textContent = slider.value;
            self.updateGameTime();
        });
    },

    bindEvents: function() {
        document.getElementById('set_weather_button').addEventListener('click', this.setWeather.bind(this));
        document.getElementById('nightFilterCompositeOperation').addEventListener('change', this.updateNightFilterCompositeOperation.bind(this));
    },

    unbindEvents: function() {
        document.getElementById('set_weather_button').removeEventListener('click', this.setWeather.bind(this));
        document.getElementById('nightFilterCompositeOperation').removeEventListener('change', this.updateNightFilterCompositeOperation.bind(this));
    },

    updateGameTime: function() {
        var hours = document.getElementById('hours').value;
        var minutes = document.getElementById('minutes').value;
        var seconds = document.getElementById('seconds').value;
        var days = document.getElementById('days').value;
        var speedMultiplier = document.getElementById('speed_multiplier').value;

        utils.gameTime.hours = parseInt(hours);
        utils.gameTime.minutes = parseInt(minutes);
        utils.gameTime.seconds = parseInt(seconds);
        utils.gameTime.days = parseInt(days);
        utils.gameTime.speedMultiplier = parseInt(speedMultiplier);

        console.log('Game time set to: ', utils.gameTime.display());
    },

    setWeather: function() {
        var snow = document.getElementById('toggle_snow').checked;
        var rain = document.getElementById('toggle_rain').checked;
        var fog = document.getElementById('toggle_fog').checked;
        var fireflys = document.getElementById('toggle_fireflys').checked;

        weather.snow.active = snow;
        weather.rain.active = rain;
        weather.fog.active = fog;
        weather.fireflys.active = fireflys;

        if (snow) weather.snow.create(0.5); else weather.snow.stop();
        if (rain) weather.rain.create(0.7);
        if (fog) weather.fog.create(0.1);
        if (fireflys) weather.firewflys.create();

        console.log('Weather updated: Snow -', snow, ', Rain -', rain, ', Fog -', fog, ', FireFlys -', fireflys);
    },

    initLightTypes: function() {
        const lightTypeContainer = document.getElementById('lightTypeContainer');
        lightTypeContainer.innerHTML = '';

        const lightTypes = [...new Set(lighting.lights.map(light => light.type))];

        lightTypes.forEach((type, index) => {
            const lightControls = `
              <div class="light-item" style="background: #2D2D2D; padding: 10px; margin-bottom: 10px; border-radius: 5px;">
                <div class="form-group">
                  <label for="lightRadius_${index}">${type} Radius</label>
                  <input type="range" id="lightRadius_${index}" class="form-control-range w-full mt-1 mb-2" min="10" max="1000" value="${lighting.lights.find(light => light.type === type).baseRadius}" oninput="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightRadiusLabel_${index}', this.value, 'radius')">
                  <span id="lightRadiusLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).baseRadius}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorR_${index}">${type} Color (R)</label>
                  <input type="range" id="lightColorR_${index}" class="form-control-range w-full mt-1 mb-2" min="0" max="255" value="${lighting.lights.find(light => light.type === type).color.r}" oninput="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightColorRLabel_${index}', this.value, 'colorR')">
                  <span id="lightColorRLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).color.r}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorG_${index}">${type} Color (G)</label>
                  <input type="range" id="lightColorG_${index}" class="form-control-range w-full mt-1 mb-2" min="0" max="255" value="${lighting.lights.find(light => light.type === type).color.g}" oninput="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightColorGLabel_${index}', this.value, 'colorG')">
                  <span id="lightColorGLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).color.g}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorB_${index}">${type} Color (B)</label>
                  <input type="range" id="lightColorB_${index}" class="form-control-range w-full mt-1 mb-2" min="0" max="255" value="${lighting.lights.find(light => light.type === type).color.b}" oninput="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightColorBLabel_${index}', this.value, 'colorB')">
                  <span id="lightColorBLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).color.b}</span>
                </div>
                <div class="form-group">
                  <label for="lightIntensity_${index}">${type} Max Intensity</label>
                  <input type="range" id="lightIntensity_${index}" class="form-control-range w-full mt-1 mb-2" min="0" max="1" step="0.01" value="${lighting.lights.find(light => light.type === type).maxIntensity}" oninput="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightIntensityLabel_${index}', this.value, 'intensity')">
                  <span id="lightIntensityLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).maxIntensity}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlicker_${index}">${type} Flicker</label>
                  <input type="checkbox" id="lightFlicker_${index}" ${lighting.lights.find(light => light.type === type).flicker ? 'checked' : ''} onchange="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightFlickerLabel_${index}', this.checked, 'flicker')">
                  <span id="lightFlickerLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).flicker}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerSpeed_${index}">${type} Flicker Speed</label>
                  <input type="range" id="lightFlickerSpeed_${index}" class="form-control-range w-full mt-1 mb-2" min="0" max="1" step="0.01" value="${lighting.lights.find(light => light.type === type).flickerSpeed}" oninput="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightFlickerSpeedLabel_${index}', this.value, 'flickerSpeed')">
                  <span id="lightFlickerSpeedLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).flickerSpeed}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerAmount_${index}">${type} Flicker Amount</label>
                  <input type="range" id="lightFlickerAmount_${index}" class="form-control-range w-full mt-1 mb-2" min="0" max="1" step="0.01" value="${lighting.lights.find(light => light.type === type).flickerAmount}" oninput="ui_console_tab_window.updateLightTypeAndLabel('${type}', 'lightFlickerAmountLabel_${index}', this.value, 'flickerAmount')">
                  <span id="lightFlickerAmountLabel_${index}" class="block text-sm text-gray-400">${lighting.lights.find(light => light.type === type).flickerAmount}</span>
                </div>
              </div>
            `;
            lightTypeContainer.insertAdjacentHTML('beforeend', lightControls);
        });
    },

    updateLightTypeAndLabel: function(type, labelId, value, property) {
        if (labelId.includes('lightFlickerLabel')) {
            document.getElementById(labelId).textContent = value ? 'true' : 'false';
        } else {
            document.getElementById(labelId).textContent = value;
        }
        this.updateLightType(type, value, property);
    },

    updateLightType: function(type, value, property) {
        const lightsOfType = lighting.lights.filter(light => light.type === type);
        lightsOfType.forEach(light => {
            switch (property) {
                case 'radius':
                    light.baseRadius = parseInt(value);
                    break;
                case 'colorR':
                    light.color.r = parseInt(value);
                    break;
                case 'colorG':
                    light.color.g = parseInt(value);
                    break;
                case 'colorB':
                    light.color.b = parseInt(value);
                    break;
                case 'intensity':
                    light.maxIntensity = parseFloat(value);
                    break;
                case 'flicker':
                    light.flicker = value;
                    break;
                case 'flickerSpeed':
                    light.flickerSpeed = parseFloat(value);
                    break;
                case 'flickerAmount':
                    light.flickerAmount = parseFloat(value);
                    break;
                default:
                    break;
            }
        });

        const event = new CustomEvent('lightTypeUpdated', { detail: { type, property, value } });
        window.dispatchEvent(event);
    },

    populateEffectList: function() {
        const effectSelect = document.getElementById('effectSelect');
        effectSelect.innerHTML = '<option value="">Select Effect</option>';

        Object.keys(particles.activeEffects).forEach(effectId => {
            const option = document.createElement('option');
            option.value = effectId;
            option.text = effectId;
            effectSelect.appendChild(option);
        });
    },

    initParticleEventListeners: function() {
        document.getElementById('effectSelect').addEventListener('change', this.loadEffectData.bind(this));
        document.querySelectorAll('.slider-group input[type="range"]').forEach(slider => {
            slider.addEventListener('input', this.updateEffectData.bind(this, slider.id));
        });
        document.querySelectorAll('.slider-group select').forEach(select => {
            select.addEventListener('change', this.updateEffectData.bind(this, select.id));
        });
        document.querySelectorAll('.slider-group input[type="color"]').forEach(colorPicker => {
            colorPicker.addEventListener('input', this.updateEffectData.bind(this, colorPicker.id));
        });
        document.getElementById('particleRepeat').addEventListener('change', this.updateEffectData.bind(this, 'particleRepeat'));
    },

    // Function to load and display controls for the selected effect
    loadEffectData: function(event) {
        const effectId = event.target.value;
        const effect = particles.activeEffects[effectId];

        if (effect && effect.length > 0) {
            document.getElementById('effectControls').style.display = 'block';

            // Load and set control values here...
            // Set the controls based on the first particle in the effect (e.g., size, speed, color, etc.)
            this.setInputValue('particleCount', effect.length);
            this.setInputValue('particleSpeed', effect[0].vx);
            this.setInputValue('particleLife', effect[0].life);
            this.setInputValue('particleSize', effect[0].size);
            this.setInputValue('particleGlow', effect[0].glow);
            this.setInputValue('particleOpacity', effect[0].initialOpacity);
            this.setInputValue('particleBlur', effect[0].blur);
            this.setInputValue('particleShape', effect[0].shape);
            this.setInputValue('particleColor1', effect[0].color);
            this.setInputValue('particleX', effect[0].x);
            this.setInputValue('particleY', effect[0].y);
            this.setInputValue('particleGravity', effect[0].gravity);  // Added for gravity
            this.setInputValue('particleSway', effect[0].sway);  // Added for sway
            this.setInputValue('particleTwinkle', effect[0].twinkleSpeed);  // Added for twinkle

            console.log('Loaded controls for effect:', effectId);
        } else {
            document.getElementById('effectControls').style.display = 'none';
            console.warn(`Effect ${effectId} is not defined or has no particles.`);
        }
    },

    // Helper function to set input values
    setInputValue: function(inputId, value) {
        const inputElement = document.getElementById(inputId);
        const valueElement = document.getElementById(inputId + 'Value');

        if (inputElement && valueElement) {
            inputElement.value = value;
            valueElement.textContent = value;
        }
    },

    initTimeBasedUpdates: function() {
        const toggleElement = document.getElementById('toggleTimeBasedUpdates');
        if (toggleElement) {
            toggleElement.checked = lighting.timeBasedUpdatesEnabled;
            toggleElement.addEventListener('change', (event) => {
                this.toggleTimeBasedUpdates(event.target.checked);
            });
        }
    },

    toggleTimeBasedUpdates: function(enabled) {
        lighting.timeBasedUpdatesEnabled = enabled;
        console.log('Time-based updates enabled:', enabled);
    },

    updateEffectData: function(property, event) {
        const effectId = document.getElementById('effectSelect').value;
        const effect = particles.activeEffects[effectId];

        if (effect && effect.length > 0) {
            let value;
            if (property === 'particleRepeat') {
                value = event.target.checked;
            } else {
                value = event.target.value;
            }

            if (property === 'particleCount') {
                const newCount = parseInt(value);
                const currentCount = effect.length;

                if (newCount > currentCount) {
                    for (let i = currentCount; i < newCount; i++) {
                        const baseParticle = effect[0];
                        if (baseParticle) {
                            const newParticle = { ...baseParticle, life: baseParticle.maxLife, x: baseParticle.initialX, y: baseParticle.initialY };
                            effect.push(newParticle);
                            game.particles.push(newParticle);
                        }
                    }
                } else if (newCount < currentCount) {
                    const particlesToRemove = currentCount - newCount;
                    for (let i = 0; i < particlesToRemove; i++) {
                        const particle = effect.pop();
                        const index = game.particles.indexOf(particle);
                        if (index > -1) {
                            game.particles.splice(index, 1);
                        }
                    }
                }
            } else if (property.startsWith('particleColor')) {
                effect.forEach(particle => {
                    if (particle) {
                        particle.color = this.hexToRgb(value);
                    }
                });
            } else if (property === 'particleGravity') {  // Added for gravity
                effect.forEach(particle => {
                    particle.gravity = parseFloat(value);
                });
            } else if (property === 'particleSway') {  // Added for sway
                effect.forEach(particle => {
                    particle.sway = parseFloat(value);
                });
            } else if (property === 'particleTwinkle') {  // Added for twinkle
                effect.forEach(particle => {
                    particle.twinkleSpeed = parseFloat(value);
                });
            } else {
                effect.forEach(particle => {
                    if (particle) {
                        particle[property.replace('particle', '').toLowerCase()] = isNaN(value) ? value : parseFloat(value);
                    }
                });
            }

            const valueElement = document.getElementById(`${property}Value`);
            if (valueElement) {
                valueElement.textContent = value;
            }

            this.restartEffect(effectId);
            game.render();
        } else {
            console.warn(`Effect ${effectId} is not defined or has no particles.`);
        }
    },

    restartEffect: function(effectId) {
        const effect = particles.activeEffects[effectId];
        if (effect && effect.length > 0) {
            const initialX = effect[0].initialX;
            const initialY = effect[0].initialY;
            const repeat = effect[0].repeat;

            effect.forEach(particle => {
                const index = game.particles.indexOf(particle);
                if (index > -1) {
                    game.particles.splice(index, 1);
                }
            });

            effect.length = 0;
            particles.createParticles(
                initialX, initialY,
                {
                    count: parseInt(document.getElementById('particleCount').value),
                    speed: parseFloat(document.getElementById('particleSpeed').value),
                    life: parseInt(document.getElementById('particleLife').value),
                    size: parseInt(document.getElementById('particleSize').value),
                    glow: parseInt(document.getElementById('particleGlow').value),
                    opacity: parseFloat(document.getElementById('particleOpacity').value),
                    blur: parseInt(document.getElementById('particleBlur').value),
                    shape: document.getElementById('particleShape').value,
                    colors: [
                        document.getElementById('particleColor1').value,
                        document.getElementById('particleColor2').value,
                        document.getElementById('particleColor3').value
                    ],
                    angle: parseFloat(document.getElementById('particleAngle').value),
                    spread: parseFloat(document.getElementById('particleSpread').value),
                    gravity: parseFloat(document.getElementById('particleGravity').value),  // Added for gravity
                    sway: parseFloat(document.getElementById('particleSway').value),  // Added for sway
                    twinkle: parseFloat(document.getElementById('particleTwinkle').value),  // Added for twinkle
                    repeat: document.getElementById('particleRepeat').checked
                },
                effectId
            );
        } else {
            console.warn(`Effect ${effectId} is not defined or has no particles.`);
        }
    },

    createNewParticleEffect: function() {
        const effectId = 'new_effect_' + Date.now(); // Generate a unique effect ID based on the current timestamp

        // Get X and Y position values from the sliders
        const xPosition = parseInt(document.getElementById('particleX').value, 10);
        const yPosition = parseInt(document.getElementById('particleY').value, 10);

        // Define the particle options for the new effect
        const particleOptions = {
            color: [[106, 106, 106], [169, 169, 169], [192, 192, 192]], // Smoke colors
            count: 4,
            speed: 0.2,
            baseAngle: 260,
            spread: 90,
            frames: 200,
            size: 1,
            rotationSpeed: 0,
            HorAcc: 0,
            VerAcc: 0,
            Glow: 0,
            Opacity: 0.7,
            Blur: 20,
            Shape: "circle",
            repeat: true
        };

        // Initialize the effect in particles.activeEffects
        particles.activeEffects[effectId] = [];

        for (let i = 0; i < particleOptions.count; i++) {
            const randomSpread = (Math.random() - 0.5) * particleOptions.spread * (Math.PI / 180);
            const angle = particleOptions.baseAngle * (Math.PI / 180) + randomSpread;
            const speed = particleOptions.speed * (0.5 + Math.random() * 0.5);
            const color = `rgba(${particleOptions.color[Math.floor(Math.random() * particleOptions.color.length)].join(',')}, ${particleOptions.Opacity})`;

            const newParticle = {
                x: xPosition,  // Set X position based on slider
                y: yPosition,  // Set Y position based on slider
                initialX: xPosition,  // Set initial X position
                initialY: yPosition,  // Set initial Y position
                vx: Math.cos(angle) * speed,
                vy: Math.sin(angle) * speed,
                life: particleOptions.frames,
                maxLife: particleOptions.frames,
                color: color,
                size: particleOptions.size,
                type: 'smoke',
                repeat: particleOptions.repeat,
                glow: particleOptions.Glow,
                initialOpacity: particleOptions.Opacity,
                blur: particleOptions.Blur,
                shape: particleOptions.Shape
            };

            particles.activeEffects[effectId].push(newParticle);
            game.particles.push(newParticle);
        }

        // Add the new effect to the dropdown
        const effectSelect = document.getElementById('effectSelect');
        const option = document.createElement('option');
        option.value = effectId;
        option.text = `Effect ${effectId}`;
        effectSelect.appendChild(option);

        // Select the newly added effect
        effectSelect.value = effectId;

        // Trigger the display of controls for the newly added effect
        this.loadEffectData({ target: effectSelect });

        console.log('Created and activated new particle effect:', effectId);
    },

    // Function to remove the entire particle effect
    removeParticleEffect: function() {
        const effectId = document.getElementById('effectSelect').value;

        if (particles.activeEffects[effectId]) {
            // Remove particles associated with the effect from game.particles
            particles.activeEffects[effectId].forEach(particle => {
                const index = game.particles.indexOf(particle);
                if (index > -1) {
                    game.particles.splice(index, 1);
                }
            });

            // Delete the effect from activeEffects
            delete particles.activeEffects[effectId];

            // Remove the effect from the dropdown
            const effectSelect = document.getElementById('effectSelect');
            const optionToRemove = effectSelect.querySelector(`option[value="${effectId}"]`);
            if (optionToRemove) {
                optionToRemove.remove();
            }

            // Clear controls if no effect is selected
            effectSelect.value = '';
            document.getElementById('effectControls').style.display = 'none';

            console.log('Removed particle effect:', effectId);
        } else {
            console.warn(`Effect ${effectId} does not exist.`);
        }
    },

    hexToRgb: function(hex) {
        const bigint = parseInt(hex.slice(1), 16);
        const r = (bigint >> 16) & 255;
        const g = (bigint >> 8) & 255;
        const b = (bigint & 255);
        return `rgba(${r}, ${g}, ${b}, 1)`;
    },

    rgbToHex: function(rgb) {
        const [r, g, b] = rgb.match(/\d+/g).map(Number);
        return `#${((1 << 24) + (r << 16) + (g << 8) + b).toString(16).slice(1).toUpperCase()}`;
    }
};

// Call the start function to initialize everything
ui_console_tab_window.start();

    </script>
<?php
}
?>