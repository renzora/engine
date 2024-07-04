<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='renadmin_lighting_window' class='window window_bg' style='width: 330px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
      <button class="icon minimize_dark hint--left" aria-label="Minimise" data-minimize></button>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Lighting</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <div id="lightTypeContainer"></div>
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
    <label for="toggleTimeBasedUpdates">Enable Time-based Updates</label>
    <input type="checkbox" id="toggleTimeBasedUpdates" checked onchange="renadmin_lighting_window.toggleTimeBasedUpdates(this.checked)">
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
          this.initLightTypes(); // Initialize the light types when the window starts
          this.initNightFilter(); // Initialize the night filter when the window starts
          this.initTimeBasedUpdates();
        },
        initTimeBasedUpdates: function() {
        const checkbox = document.getElementById('toggleTimeBasedUpdates');
        checkbox.checked = effects.timeBasedUpdatesEnabled;
    },
    toggleTimeBasedUpdates: function(isChecked) {
        effects.timeBasedUpdatesEnabled = isChecked;
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
        initLightTypes: function() {
          const lightTypeContainer = document.getElementById('lightTypeContainer');
          lightTypeContainer.innerHTML = ''; // Clear existing content

          const lightTypes = [...new Set(effects.lights.map(light => light.type))];
          
          lightTypes.forEach((type, index) => {
            const lightControls = `
              <div class="light-item" style="background: #f0f0f0; padding: 10px; margin-bottom: 10px; border-radius: 5px;">
                <div class="form-group">
                  <label for="lightRadius_${index}">${type} Radius</label>
                  <input type="range" id="lightRadius_${index}" class="form-control-range" min="10" max="1000" value="${effects.lights.find(light => light.type === type).baseRadius}" oninput="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightRadiusLabel_${index}', this.value, 'radius')">
                  <span id="lightRadiusLabel_${index}">${effects.lights.find(light => light.type === type).baseRadius}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorR_${index}">${type} Color (R)</label>
                  <input type="range" id="lightColorR_${index}" class="form-control-range" min="0" max="255" value="${effects.lights.find(light => light.type === type).color.r}" oninput="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightColorRLabel_${index}', this.value, 'colorR')">
                  <span id="lightColorRLabel_${index}">${effects.lights.find(light => light.type === type).color.r}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorG_${index}">${type} Color (G)</label>
                  <input type="range" id="lightColorG_${index}" class="form-control-range" min="0" max="255" value="${effects.lights.find(light => light.type === type).color.g}" oninput="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightColorGLabel_${index}', this.value, 'colorG')">
                  <span id="lightColorGLabel_${index}">${effects.lights.find(light => light.type === type).color.g}</span>
                </div>
                <div class="form-group">
                  <label for="lightColorB_${index}">${type} Color (B)</label>
                  <input type="range" id="lightColorB_${index}" class="form-control-range" min="0" max="255" value="${effects.lights.find(light => light.type === type).color.b}" oninput="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightColorBLabel_${index}', this.value, 'colorB')">
                  <span id="lightColorBLabel_${index}">${effects.lights.find(light => light.type === type).color.b}</span>
                </div>
                <div class="form-group">
                  <label for="lightIntensity_${index}">${type} Max Intensity</label>
                  <input type="range" id="lightIntensity_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${effects.lights.find(light => light.type === type).maxIntensity}" oninput="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightIntensityLabel_${index}', this.value, 'intensity')">
                  <span id="lightIntensityLabel_${index}">${effects.lights.find(light => light.type === type).maxIntensity}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlicker_${index}">${type} Flicker</label>
                  <input type="checkbox" id="lightFlicker_${index}" ${effects.lights.find(light => light.type === type).flicker ? 'checked' : ''} onchange="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightFlickerLabel_${index}', this.checked, 'flicker')">
                  <span id="lightFlickerLabel_${index}">${effects.lights.find(light => light.type === type).flicker}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerSpeed_${index}">${type} Flicker Speed</label>
                  <input type="range" id="lightFlickerSpeed_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${effects.lights.find(light => light.type === type).flickerSpeed}" oninput="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightFlickerSpeedLabel_${index}', this.value, 'flickerSpeed')">
                  <span id="lightFlickerSpeedLabel_${index}">${effects.lights.find(light => light.type === type).flickerSpeed}</span>
                </div>
                <div class="form-group">
                  <label for="lightFlickerAmount_${index}">${type} Flicker Amount</label>
                  <input type="range" id="lightFlickerAmount_${index}" class="form-control-range" min="0" max="1" step="0.01" value="${effects.lights.find(light => light.type === type).flickerAmount}" oninput="renadmin_lighting_window.updateLightTypeAndLabel('${type}', 'lightFlickerAmountLabel_${index}', this.value, 'flickerAmount')">
                  <span id="lightFlickerAmountLabel_${index}">${effects.lights.find(light => light.type === type).flickerAmount}</span>
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
        updateLightTypeAndLabel: function(type, labelId, value, property) {
          if (labelId.includes('lightFlickerLabel')) {
            document.getElementById(labelId).textContent = value ? 'true' : 'false';
          } else {
            document.getElementById(labelId).textContent = value;
          }
          this.updateLightType(type, value, property); // Update the specific light type
        },
        updateLightType: function(type, value, property) {
          const lightsOfType = effects.lights.filter(light => light.type === type);
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
          
          // Emit a custom event with updated light data
          const event = new CustomEvent('lightTypeUpdated', { detail: { type, property, value } });
          window.dispatchEvent(event);
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
