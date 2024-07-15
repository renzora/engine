<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='renadmin_particles_window' class='window window_bg' style='width: 330px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Particle Editor</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        
        <div id="renadmin_particles_window_tabs">
          <div id="tabs" class="flex border-b border-gray-300">
            <button class="tab text-gray-800" data-tab="tab1">Particles</button>
          </div>

          <div class="tab-content p-4" data-tab-content="tab1">
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
var renadmin_particles_window = {
    start: function() {
        ui.initTabs('renadmin_particles_window_tabs', 'tab1');
        this.populateEffectList();
        this.initEventListeners();
    },
    unmount: function() {
        ui.destroyTabs('renadmin_particles_window_tabs');
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
    initEventListeners: function() {
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

            // For the color inputs, handle cases where there may be fewer than three particles
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
                // Add more particles
                for (let i = currentCount; i < newCount; i++) {
                    const baseParticle = effect[0]; // Copy properties from the first particle
                    if (baseParticle) {
                        const newParticle = { ...baseParticle, life: baseParticle.maxLife, x: baseParticle.initialX, y: baseParticle.initialY };
                        effect.push(newParticle);
                        game.particles.push(newParticle);
                    }
                }
            } else if (newCount < currentCount) {
                // Remove extra particles
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

        this.restartEffect(effectId); // Restart the effect after updating

        // Force re-render
        game.render();
    } else {
        console.warn(`Effect ${effectId} is not defined or has no particles.`);
    }
},
restartEffect: function(effectId) {
    const effect = particles.activeEffects[effectId];
    if (effect && effect.length > 0) {
        // Store initial coordinates before clearing the array
        const initialX = effect[0].initialX;
        const initialY = effect[0].initialY;
        const repeat = effect[0].repeat; // Store repeat option

        // Remove current particles
        effect.forEach(particle => {
            const index = game.particles.indexOf(particle);
            if (index > -1) {
                game.particles.splice(index, 1);
            }
        });

        // Reinitialize particles with updated properties
        effect.length = 0; // Clear the current effect array
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
                repeat: document.getElementById('particleRepeat').checked // Set repeat option based on checkbox
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

renadmin_particles_window.start();
</script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>