<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='renadmin_world_editor_window' class='window window_bg' style='width: 350px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
      <button class="icon minimize_dark hint--left" aria-label="Minimise" data-minimize></button>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>World Editor</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>

        <div id="world_editor_tabs">
          <div id="tabs" class="flex border-b border-gray-300">
            <button class="tab text-gray-800" data-tab="tab1">Time</button>
            <button class="tab text-gray-800" data-tab="tab2">Weather</button>
            <button class="tab text-gray-800" data-tab="tab3">Lighting</button>
            <button class="tab text-gray-800" data-tab="tab4">Particles</button>
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab1">
            <p>Set Game Time:</p>
            <label for="hours">Hours:</label>
            <input type="range" id="hours" name="hours" min="0" max="23" value="7">
            <span id="hours_value">7</span>
            <label for="minutes">Minutes:</label>
            <input type="range" id="minutes" name="minutes" min="0" max="59" value="0">
            <span id="minutes_value">0</span>
            <label for="seconds">Seconds:</label>
            <input type="range" id="seconds" name="seconds" min="0" max="59" value="0">
            <span id="seconds_value">0</span>
            <label for="days">Days:</label>
            <input type="range" id="days" name="days" min="0" max="7" value="0">
            <span id="days_value">0</span>
            <label for="speed_multiplier">Speed Multiplier:</label>
            <input type="range" id="speed_multiplier" name="speed_multiplier" min="1" max="5000" value="1000">
            <span id="speed_multiplier_value">1000</span>
          </div>

          <div class="tab-content p-2 hidden" data-tab-content="tab2">
            <label for="toggle_snow">Snow:</label>
            <input type="checkbox" id="toggle_snow">
            <br />
            <label for="toggle_rain">Rain:</label>
            <input type="checkbox" id="toggle_rain">
            <br />
            <label for="toggle_fog">Fog:</label>
            <input type="checkbox" id="toggle_fog">
            <br />
            <label for="toggle_fireflys">FireFlys:</label>
            <input type="checkbox" id="toggle_fireflys">
            <br />
            <button id="set_weather_button" class="mt-4 green_button text-white font-bold py-1 px-2 rounded shadow-md">Set Weather</button>
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab3">
            <div id="lightTypeContainer"></div>
            <div class="form-group mt-3">
              <label for="compositeOperation">Composite</label>
              <select id="compositeOperation" class="form-control" onchange="renadmin_world_editor_window.updateCompositeOperation()">
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
              <label for="nightFilterOpacity">NightFilter Opacity</label>
              <input type="range" id="nightFilterOpacity" class="form-control-range" min="0" max="1" step="0.01" value="0.89" oninput="renadmin_world_editor_window.updateNightFilterAndLabel('nightFilterOpacityLabel', this.value)">
              <span id="nightFilterOpacityLabel"></span>
            </div>
            <div class="form-group">
              <label for="nightFilterColorR">NightFilter Color (R)</label>
              <input type="range" id="nightFilterColorR" class="form-control-range" min="0" max="255" value="0" oninput="renadmin_world_editor_window.updateNightFilterAndLabel('nightFilterColorRLabel', this.value)">
              <span id="nightFilterColorRLabel"></span>
            </div>
            <div class="form-group">
              <label for="nightFilterColorG">NightFilter Color (G)</label>
              <input type="range" id="nightFilterColorG" class="form-control-range" min="0" max="255" value="7" oninput="renadmin_world_editor_window.updateNightFilterAndLabel('nightFilterColorGLabel', this.value)">
              <span id="nightFilterColorGLabel"></span>
            </div>
            <div class="form-group">
              <label for="nightFilterColorB">NightFilter Color (B)</label>
              <input type="range" id="nightFilterColorB" class="form-control-range" min="0" max="255" value="25" oninput="renadmin_world_editor_window.updateNightFilterAndLabel('nightFilterColorBLabel', this.value)">
              <span id="nightFilterColorBLabel"></span>
            </div>
            <div class="form-group">
              <label for="toggleTimeBasedUpdates">Enable Time-based Updates</label>
              <input type="checkbox" id="toggleTimeBasedUpdates" checked onchange="renadmin_world_editor_window.toggleTimeBasedUpdates(this.checked)">
            </div>
            <div class="form-group">
              <label for="nightFilterCompositeOperation">NightFilter Composite</label>
              <select id="nightFilterCompositeOperation" class="form-control" onchange="renadmin_world_editor_window.updateNightFilterCompositeOperation()">
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
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab4">
            <select id="effectSelect" class="w-full mb-4 p-2 border rounded">
              <option value="">Select Effect</option>
            </select>

            <div id="effectControls" class="space-y-4" style="display: none;">
              <div class="slider-group">
                <label for="particleRepeat" class="flex justify-between">Repeat: <span id="particleRepeatValue"></span></label>
                <input type="checkbox" id="particleRepeat" class="w-full">
              </div>

              <div class="slider-group">
                <label for="particleCount" class="flex justify-between">Count: <span id="particleCountValue"></span></label>
                <input type="range" id="particleCount" min="1" max="100" step="1" class="w-full">
              </div>

              <div class="slider-group">
                <label for="particleSpeed" class="flex justify-between">Speed: <span id="particleSpeedValue"></span></label>
                <input type="range" id="particleSpeed" min="0.1" max="10" step="0.1" class="w-full">
              </div>

              <div class="slider-group">
                <label for="particleLife" class="flex justify-between">Life: <span id="particleLifeValue"></span></label>
                <input type="range" id="particleLife" min="1" max="500" step="1" class="w-full">
              </div>

              <div class="slider-group">
                <label for="particleSize" class="flex justify-between">Size: <span id="particleSizeValue"></span></label>
                <input type="range" id="particleSize" min="1" max="20" step="1" class="w-full">
              </div>
              
              <div class="slider-group">
                <label for="particleGlow" class="flex justify-between">Glow: <span id="particleGlowValue"></span></label>
                <input type="range" id="particleGlow" min="0" max="20" step="1" class="w-full">
              </div>
              
              <div class="slider-group">
                <label for="particleOpacity" class="flex justify-between">Opacity: <span id="particleOpacityValue"></span></label>
                <input type="range" id="particleOpacity" min="0.1" max="1" step="0.1" class="w-full">
              </div>
              
              <div class="slider-group">
                <label for="particleBlur" class="flex justify-between">Blur: <span id="particleBlurValue"></span></label>
                <input type="range" id="particleBlur" min="0" max="50" step="1" class="w-full">
              </div>

              <div class="slider-group">
                <label for="particleShape" class="flex justify-between">Shape: <span id="particleShapeValue"></span></label>
                <select id="particleShape" class="w-full p-2 border rounded">
                  <option value="circle">Circle</option>
                  <option value="square">Square</option>
                  <option value="triangle">Triangle</option>
                </select>
              </div>

              <div class="slider-group">
                <label for="particleColor1" class="flex justify-between">Color 1: <span id="particleColor1Value"></span></label>
                <input type="color" id="particleColor1" class="w-full">
              </div>
              
              <div class="slider-group">
                <label for="particleColor2" class="flex justify-between">Color 2: <span id="particleColor2Value"></span></label>
                <input type="color" id="particleColor2" class="w-full">
              </div>

              <div class="slider-group">
                <label for="particleColor3" class="flex justify-between">Color 3: <span id="particleColor3Value"></span></label>
                <input type="color" id="particleColor3" class="w-full">
              </div>

              <div class="slider-group">
                <label for="particleAngle" class="flex justify-between">Angle: <span id="particleAngleValue"></span></label>
                <input type="range" id="particleAngle" min="0" max="360" step="1" class="w-full">
              </div>
              
              <div class="slider-group">
                <label for="particleSpread" class="flex justify-between">Spread: <span id="particleSpreadValue"></span></label>
                <input type="range" id="particleSpread" min="0" max="360" step="1" class="w-full">
              </div>
            </div>
          </div>
        </div>

      </div>
    </div>

    <script>
      var renadmin_world_editor_window = {
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
        unmount: function() {
          ui.destroyTabs('world_editor_tabs');
          this.unbindEvents();
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
        },
        unbindEvents: function() {
          document.getElementById('set_weather_button').removeEventListener('click', this.setWeather.bind(this));
        },
        updateGameTime: function() {
          var hours = document.getElementById('hours').value;
          var minutes = document.getElementById('minutes').value;
          var seconds = document.getElementById('seconds').value;
          var days = document.getElementById('days').value;
          var speedMultiplier = document.getElementById('speed_multiplier').value;

          game.gameTime.hours = parseInt(hours);
          game.gameTime.minutes = parseInt(minutes);
          game.gameTime.seconds = parseInt(seconds);
          game.gameTime.days = parseInt(days);
          game.gameTime.speedMultiplier = parseInt(speedMultiplier);

          console.log('Game time set to: ', game.gameTime.display());
        },
        setWeather: function() {
          var snow = document.getElementById('toggle_snow').checked;
          var rain = document.getElementById('toggle_rain').checked;
          var fog = document.getElementById('toggle_fog').checked;
          var fireflys = document.getElementById('toggle_fireflys').checked;

          weather.snowActive = snow;
          weather.rainActive = rain;
          weather.fogActive = fog;
          weather.fireflysActive = fireflys;

          if (snow) weather.createSnow(0.5); else weather.stopSnow();
          if (rain) weather.createRain(0.7);
          if (fog) weather.createFog(0.1);
          if (fireflys) weather.createFireflys();

          console.log('Weather updated: FireFlys -', fireflys, ', Rain -', rain, ', Fog -', fog, ', FireFlys -', fireflys);
        },
        initLightTypes: function() {
          const lightTypeContainer = document.getElementById('lightTypeContainer');
          lightTypeContainer.innerHTML = '';

          const lightTypes = [...new Set(lighting.lights.map(light => light.type))];

          lightTypes.forEach((type, index) => {
            const lightControls = `
              <div class="light-item" style="background: #f0f0f0; padding: 10px; margin-bottom: 10px; border-radius: 5px;">
                <div class="form-group">
                  <label for="lightRadius_${index}">${type} Radius</label>
                  <input type="range" id="lightRadius_${index}" class="form-control-range" min="10" max="1000" value="${lighting.lights.find(light => light.type === type).baseRadius}" oninput="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightRadiusLabel_${index}', this.value, 'radius')">
                  <span id="lightRadiusLabel_${index}">${lighting.lights.find(light => light.type === type).baseRadius}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorR_${index}">${type} Color (R)</label>
                  <input type="range" id="lightColorR_${index}" class="form-control-range" min="0" max="255" value="${lighting.lights.find(light => light.type === type).color.r}" oninput="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightColorRLabel_${index}', this.value, 'colorR')">
                  <span id="lightColorRLabel_${index}">${lighting.lights.find(light => light.type === type).color.r}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorG_${index}">${type} Color (G)</label>
                  <input type="range" id="lightColorG_${index}" class="form-control-range" min="0" max="255" value="${lighting.lights.find(light => light.type === type).color.g}" oninput="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightColorGLabel_${index}', this.value, 'colorG')">
                  <span id="lightColorGLabel_${index}">${lighting.lights.find(light => light.type === type).color.g}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorB_${index}">${type} Color (B)</label>
                  <input type="range" id="lightColorB_${index}" class="form-control-range" min="0" max="255" value="${lighting.lights.find(light => light.type === type).color.b}" oninput="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightColorBLabel_${index}', this.value, 'colorB')">
                  <span id="lightColorBLabel_${index}">${lighting.lights.find(light => light.type === type).color.b}</span>
                </div>
                <div class="form-group">
                  <label for="lightIntensity_${index}">${type} Max Intensity</label>
                  <input type="range" id="lightIntensity_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${lighting.lights.find(light => light.type === type).maxIntensity}" oninput="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightIntensityLabel_${index}', this.value, 'intensity')">
                  <span id="lightIntensityLabel_${index}">${lighting.lights.find(light => light.type === type).maxIntensity}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlicker_${index}">${type} Flicker</label>
                  <input type="checkbox" id="lightFlicker_${index}" ${lighting.lights.find(light => light.type === type).flicker ? 'checked' : ''} onchange="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightFlickerLabel_${index}', this.checked, 'flicker')">
                  <span id="lightFlickerLabel_${index}">${lighting.lights.find(light => light.type === type).flicker}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerSpeed_${index}">${type} Flicker Speed</label>
                  <input type="range" id="lightFlickerSpeed_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${lighting.lights.find(light => light.type === type).flickerSpeed}" oninput="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightFlickerSpeedLabel_${index}', this.value, 'flickerSpeed')">
                  <span id="lightFlickerSpeedLabel_${index}">${lighting.lights.find(light => light.type === type).flickerSpeed}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerAmount_${index}">${type} Flicker Amount</label>
                  <input type="range" id="lightFlickerAmount_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${lighting.lights.find(light => light.type === type).flickerAmount}" oninput="renadmin_world_editor_window.updateLightTypeAndLabel('${type}', 'lightFlickerAmountLabel_${index}', this.value, 'flickerAmount')">
                  <span id="lightFlickerAmountLabel_${index}">${lighting.lights.find(light => light.type === type).flickerAmount}</span>
                </div>
              </div>
            `;
            lightTypeContainer.insertAdjacentHTML('beforeend', lightControls);
          });
        },
        initNightFilter: function() {
          const opacity = parseFloat(document.getElementById('nightFilterOpacity').value);
          const colorR = parseInt(document.getElementById('nightFilterColorR').value);
          const colorG = parseInt(document.getElementById('nightFilterColorG').value);
          const colorB = parseInt(document.getElementById('nightFilterColorB').value);
          const compositeOperation = document.getElementById('nightFilterCompositeOperation').value;

          if (lighting) {
            lighting.nightFilter = {
              opacity: opacity,
              color: { r: colorR, g: colorG, b: colorB },
              compositeOperation: compositeOperation
            };
          } else {
            console.error('Effects object not found');
          }
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
        updateNightFilterAndLabel: function(labelId, value) {
          document.getElementById(labelId).textContent = value;
          this.initNightFilter();
        },
        updateCompositeOperation: function() {
          const compositeOperation = document.getElementById('compositeOperation').value;
          if (lighting) {
            lighting.compositeOperation = compositeOperation;
          } else {
            console.error('Effects object not found');
          }
        },
        updateNightFilterCompositeOperation: function() {
          const compositeOperation = document.getElementById('nightFilterCompositeOperation').value;
          if (lighting) {
            lighting.nightFilter.compositeOperation = compositeOperation;
          } else {
            console.error('Effects object not found');
          }
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
        loadEffectData: function(event) {
          const effectId = event.target.value;
          const effect = particles.activeEffects[effectId];

          if (effect && effect.length > 0) {
            document.getElementById('effectControls').style.display = 'block';

            this.setInputValue('particleCount', effect.length);
            this.setInputValue('particleSpeed', effect[0].vx || 2);
            this.setInputValue('particleLife', effect[0].maxLife || 50);
            this.setInputValue('particleSize', effect[0].size || 2);
            this.setInputValue('particleGlow', effect[0].glow || 0);
            this.setInputValue('particleOpacity', effect[0].opacity || 1);
            this.setInputValue('particleBlur', effect[0].blur || 0);
            this.setInputValue('particleShape', effect[0].shape || 'circle');
            this.setInputValue('particleColor1', effect[0].color || 'rgba(255, 0, 0, 1)');
            this.setInputValue('particleAngle', effect[0].angle || 0);
            this.setInputValue('particleSpread', effect[0].spread || Math.PI * 2);

            this.setInputValue('particleColor2', effect[1]?.color || 'rgba(255, 0, 0, 1)');
            this.setInputValue('particleColor3', effect[2]?.color || 'rgba(255, 0, 0, 1)');
          } else {
            document.getElementById('effectControls').style.display = 'none';
          }
        },
        setInputValue: function(inputId, value) {
          const inputElement = document.getElementById(inputId);
          const valueElement = document.getElementById(inputId + 'Value');
          
          if (inputElement && valueElement) {
            if (inputElement.type === 'color') {
              inputElement.value = this.rgbToHex(value);
            } else {
              inputElement.value = value;
              valueElement.textContent = value;
            }
          }
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
                repeat: document.getElementById('particleRepeat').checked
              },
              effectId
            );
          } else {
            console.warn(`Effect ${effectId} is not defined or has no particles.`);
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

      renadmin_world_editor_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
