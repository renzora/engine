<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='renadmin_lighting_window' class='window window_bg' style='width: 330px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Lighting</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <div id="lightsContainer"></div>
        <div class="form-group mt-3">
          <label for="compositeOperation">Composite</label>
          <select id="compositeOperation" class="form-control" onchange="renadmin_lighting_window.updateCompositeOperation()">
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
          <input type="range" id="nightFilterOpacity" class="form-control-range" min="0" max="1" step="0.01" value="0.89" oninput="renadmin_lighting_window.updateNightFilterAndLabel('nightFilterOpacityLabel', this.value)">
          <span id="nightFilterOpacityLabel"></span>
        </div>
        <div class="form-group">
          <label for="nightFilterColorR">NightFilter Color (R)</label>
          <input type="range" id="nightFilterColorR" class="form-control-range" min="0" max="255" value="0" oninput="renadmin_lighting_window.updateNightFilterAndLabel('nightFilterColorRLabel', this.value)">
          <span id="nightFilterColorRLabel"></span>
        </div>
        <div class="form-group">
          <label for="nightFilterColorG">NightFilter Color (G)</label>
          <input type="range" id="nightFilterColorG" class="form-control-range" min="0" max="255" value="7" oninput="renadmin_lighting_window.updateNightFilterAndLabel('nightFilterColorGLabel', this.value)">
          <span id="nightFilterColorGLabel"></span>
        </div>
        <div class="form-group">
          <label for="nightFilterColorB">NightFilter Color (B)</label>
          <input type="range" id="nightFilterColorB" class="form-control-range" min="0" max="255" value="25" oninput="renadmin_lighting_window.updateNightFilterAndLabel('nightFilterColorBLabel', this.value)">
          <span id="nightFilterColorBLabel"></span>
        </div>
        <div class="form-group">
          <label for="nightFilterCompositeOperation">NightFilter Composite</label>
          <select id="nightFilterCompositeOperation" class="form-control" onchange="renadmin_lighting_window.updateNightFilterCompositeOperation()">
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
    </div>

    <script>
      var renadmin_lighting_window = {
        start: function() {
          console.log('Lighting window started');
          this.setInitialValues(); // Set initial values
          this.initLights(); // Initialize the lights when the window starts
          this.initNightFilter(); // Initialize the night filter when the window starts
        },
        unmount: function() {
          console.log('Lighting window unmounted');
        },
        setInitialValues: function() {
          if (effects) {
            // Set initial values for composite operation and night filter
            document.getElementById('compositeOperation').value = effects.compositeOperation || 'soft-light';
            document.getElementById('nightFilterOpacity').value = effects.nightFilter.opacity;
            document.getElementById('nightFilterColorR').value = effects.nightFilter.color.r;
            document.getElementById('nightFilterColorG').value = effects.nightFilter.color.g;
            document.getElementById('nightFilterColorB').value = effects.nightFilter.color.b;
            document.getElementById('nightFilterOpacityLabel').textContent = effects.nightFilter.opacity;
            document.getElementById('nightFilterColorRLabel').textContent = effects.nightFilter.color.r;
            document.getElementById('nightFilterColorGLabel').textContent = effects.nightFilter.color.g;
            document.getElementById('nightFilterColorBLabel').textContent = effects.nightFilter.color.b;
            document.getElementById('nightFilterCompositeOperation').value = effects.nightFilter.compositeOperation || 'multiply';
          }
        },
        initLights: function() {
          const lightsContainer = document.getElementById('lightsContainer');
          lightsContainer.innerHTML = ''; // Clear existing content
          
          effects.lights.forEach((light, index) => {
            const lightControls = `
              <div class="light-item" style="background: #f0f0f0; padding: 10px; margin-bottom: 10px; border-radius: 5px;">
                <div class="form-group">
                  <label for="lightX_${index}">Light ${index + 1} X Position</label>
                  <input type="range" id="lightX_${index}" class="w-full form-control-range" min="0" max="640" value="${light.x}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightXLabel_${index}', this.value)">
                  <span id="lightXLabel_${index}">${light.x}</span>
                </div>
                <div class="form-group">
                  <label for="lightY_${index}">Light ${index + 1} Y Position</label>
                  <input type="range" id="lightY_${index}" class="w-full form-control-range" min="0" max="640" value="${light.y}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightYLabel_${index}', this.value)">
                  <span id="lightYLabel_${index}">${light.y}</span>
                </div>
                <div class="form-group">
                  <label for="lightRadius_${index}">Light ${index + 1} Radius</label>
                  <input type="range" id="lightRadius_${index}" class="form-control-range" min="10" max="1000" value="${light.baseRadius}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightRadiusLabel_${index}', this.value)">
                  <span id="lightRadiusLabel_${index}">${light.baseRadius}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorR_${index}">Light ${index + 1} Color (R)</label>
                  <input type="range" id="lightColorR_${index}" class="form-control-range" min="0" max="255" value="${light.color.r}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightColorRLabel_${index}', this.value)">
                  <span id="lightColorRLabel_${index}">${light.color.r}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorG_${index}">Light ${index + 1} Color (G)</label>
                  <input type="range" id="lightColorG_${index}" class="form-control-range" min="0" max="255" value="${light.color.g}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightColorGLabel_${index}', this.value)">
                  <span id="lightColorGLabel_${index}">${light.color.g}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorB_${index}">Light ${index + 1} Color (B)</label>
                  <input type="range" id="lightColorB_${index}" class="form-control-range" min="0" max="255" value="${light.color.b}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightColorBLabel_${index}', this.value)">
                  <span id="lightColorBLabel_${index}">${light.color.b}</span>
                </div>
                <div class="form-group">
                  <label for="lightIntensity_${index}">Light ${index + 1} Max Intensity</label>
                  <input type="range" id="lightIntensity_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${light.maxIntensity}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightIntensityLabel_${index}', this.value)">
                  <span id="lightIntensityLabel_${index}">${light.maxIntensity}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlicker_${index}">Light ${index + 1} Flicker</label>
                  <input type="checkbox" id="lightFlicker_${index}" ${light.flicker ? 'checked' : ''} onchange="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightFlickerLabel_${index}', this.checked)">
                  <span id="lightFlickerLabel_${index}">${light.flicker}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerSpeed_${index}">Light ${index + 1} Flicker Speed</label>
                  <input type="range" id="lightFlickerSpeed_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${light.flickerSpeed}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightFlickerSpeedLabel_${index}', this.value)">
                  <span id="lightFlickerSpeedLabel_${index}">${light.flickerSpeed}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerAmount_${index}">Light ${index + 1} Flicker Amount</label>
                  <input type="range" id="lightFlickerAmount_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${light.flickerAmount}" oninput="renadmin_lighting_window.updateLightAndLabel(${index}, 'lightFlickerAmountLabel_${index}', this.value)">
                  <span id="lightFlickerAmountLabel_${index}">${light.flickerAmount}</span>
                </div>
              </div>
            `;
            lightsContainer.insertAdjacentHTML('beforeend', lightControls);
          });
        },
        initNightFilter: function() {
          const opacity = parseFloat(document.getElementById('nightFilterOpacity').value);
          const colorR = parseInt(document.getElementById('nightFilterColorR').value);
          const colorG = parseInt(document.getElementById('nightFilterColorG').value);
          const colorB = parseInt(document.getElementById('nightFilterColorB').value);
          const compositeOperation = document.getElementById('nightFilterCompositeOperation').value;

          if (effects) {
            effects.nightFilter = {
              opacity: opacity,
              color: { r: colorR, g: colorG, b: colorB },
              compositeOperation: compositeOperation
            };
          } else {
            console.error('Effects object not found');
          }
        },
        updateLightAndLabel: function(index, labelId, value) {
          if (labelId.includes('lightFlickerLabel')) {
            document.getElementById(labelId).textContent = value ? 'true' : 'false';
          } else {
            document.getElementById(labelId).textContent = value;
          }
          this.updateLight(index); // Update the specific light with the given index
        },
        updateLight: function(index) {
          const light = effects.lights[index];
          if (light) {
            light.x = parseInt(document.getElementById(`lightX_${index}`).value);
            light.y = parseInt(document.getElementById(`lightY_${index}`).value);
            light.baseRadius = parseInt(document.getElementById(`lightRadius_${index}`).value);
            light.color.r = parseInt(document.getElementById(`lightColorR_${index}`).value);
            light.color.g = parseInt(document.getElementById(`lightColorG_${index}`).value);
            light.color.b = parseInt(document.getElementById(`lightColorB_${index}`).value);
            light.maxIntensity = parseFloat(document.getElementById(`lightIntensity_${index}`).value);
            light.flicker = document.getElementById(`lightFlicker_${index}`).checked;
            light.flickerSpeed = parseFloat(document.getElementById(`lightFlickerSpeed_${index}`).value);
            light.flickerAmount = parseFloat(document.getElementById(`lightFlickerAmount_${index}`).value);
            
            // Emit a custom event with updated light data
            const event = new CustomEvent('lightUpdated', { detail: { index, light } });
            window.dispatchEvent(event);
          }
        },
        updateNightFilterAndLabel: function(labelId, value) {
          document.getElementById(labelId).textContent = value;
          this.initNightFilter(); // Reinitialize the night filter with updated values
        },
        updateCompositeOperation: function() {
          const compositeOperation = document.getElementById('compositeOperation').value;
          if (effects) {
            effects.compositeOperation = compositeOperation;
          } else {
            console.error('Effects object not found');
          }
        },
        updateNightFilterCompositeOperation: function() {
          const compositeOperation = document.getElementById('nightFilterCompositeOperation').value;
          if (effects) {
            effects.nightFilter.compositeOperation = compositeOperation;
          } else {
            console.error('Effects object not found');
          }
        }
      };
      renadmin_lighting_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
