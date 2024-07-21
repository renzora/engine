<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='debug_gamepad_window' class='window window_bg' style='width: 500px; background: #333;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#444 1px, transparent 0) !important;'>
    <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #444; color: #fff;'>Gamepad Debug</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        
        <div id="gamepad_display" class="flex flex-col items-center">
          <div id="gamepad_visual" class="relative bg-gray-800 p-6 rounded-lg">
            <!-- Gamepad visual representation -->
            <div id="button_A" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 40%; left: 80%;"></div>
            <div id="button_B" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 30%; left: 85%;"></div>
            <div id="button_X" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 50%; left: 75%;"></div>
            <div id="button_Y" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 20%; left: 80%;"></div>
            <div id="button_LeftBumper" class="absolute w-12 h-6 bg-gray-600 rounded" style="top: 10%; left: 10%;"></div>
            <div id="button_RightBumper" class="absolute w-12 h-6 bg-gray-600 rounded" style="top: 10%; left: 75%;"></div>
            <div id="button_LeftTrigger" class="absolute w-8 h-12 bg-gray-600 rounded" style="top: 5%; left: 5%;"></div>
            <div id="button_RightTrigger" class="absolute w-8 h-12 bg-gray-600 rounded" style="top: 5%; left: 87%;"></div>
            <div id="button_Select" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 40%; left: 40%;"></div>
            <div id="button_Start" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 40%; left: 60%;"></div>
            <div id="button_LeftStick" class="absolute w-8 h-8 bg-gray-600 rounded-full" style="top: 60%; left: 20%;" data-stick></div>
            <div id="button_RightStick" class="absolute w-8 h-8 bg-gray-600 rounded-full" style="top: 60%; left: 70%;" data-stick></div>
            <div id="button_DPadUp" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 50%; left: 15%;"></div>
            <div id="button_DPadDown" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 60%; left: 15%;"></div>
            <div id="button_DPadLeft" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 55%; left: 10%;"></div>
            <div id="button_DPadRight" class="absolute w-6 h-6 bg-gray-600 rounded-full" style="top: 55%; left: 20%;"></div>
          </div>
          
          <div id="button_values" class="mt-4">
            <h3 class="text-white">Button Values</h3>
            <ul id="buttons_list">
              <!-- Button values will be appended here -->
            </ul>
          </div>
          <div id="axis_values" class="mt-4">
            <h3 class="text-white">Axis Values</h3>
            <ul id="axes_list">
              <!-- Axis values will be appended here -->
            </ul>
          </div>
          <div id="pressure_values" class="mt-4">
            <h3 class="text-white">Pressure Values</h3>
            <ul id="pressures_list">
              <!-- Pressure values will be appended here -->
            </ul>
          </div>
        </div>

      </div>
    </div>

    <script>
      var debug_gamepad_window = {
        start: function() {
          this.updateButtonList();
          this.gamepadAxesListener = (e) => {
            this.updateAxesValues(e.detail);
            this.updateSticks(e.detail);
          };
          this.gamepadConnectedListener = () => {
            this.updateButtonList();
          };
          this.gamepadDisconnectedListener = () => {
            this.updateButtonList();
          };

          window.addEventListener('gamepadAxes', this.gamepadAxesListener);
          window.addEventListener('gamepadConnected', this.gamepadConnectedListener);
          window.addEventListener('gamepadDisconnected', this.gamepadDisconnectedListener);

          // Add listeners for each button event
          this.buttonEvents = ['A', 'B', 'X', 'Y', 'LeftBumper', 'RightBumper', 'LeftTrigger', 'RightTrigger', 'Select', 'Start', 'LeftStick', 'RightStick', 'DPadUp', 'DPadDown', 'DPadLeft', 'DPadRight'].map(button => {
            const buttonDownEvent = `gamepad${button}Pressed`;
            const buttonUpEvent = `gamepad${button}Released`;
            const buttonListener = (e) => {
              this.updateButtonValue(button, e.detail.state, e.detail.pressure);
            };
            window.addEventListener(buttonDownEvent, buttonListener);
            window.addEventListener(buttonUpEvent, buttonListener);
            return { buttonDownEvent, buttonUpEvent, buttonListener };
          });
        },
        unmount: function() {
          window.removeEventListener('gamepadAxes', this.gamepadAxesListener);
          window.removeEventListener('gamepadConnected', this.gamepadConnectedListener);
          window.removeEventListener('gamepadDisconnected', this.gamepadDisconnectedListener);

          // Remove listeners for each button event
          this.buttonEvents.forEach(({ buttonDownEvent, buttonUpEvent, buttonListener }) => {
            window.removeEventListener(buttonDownEvent, buttonListener);
            window.removeEventListener(buttonUpEvent, buttonListener);
          });
        },
        updateButtonList: function() {
          const buttonNames = [
            'A', 'B', 'X', 'Y',
            'LeftBumper', 'RightBumper',
            'LeftTrigger', 'RightTrigger',
            'Select', 'Start',
            'LeftStick', 'RightStick',
            'DPadUp', 'DPadDown', 'DPadLeft', 'DPadRight'
          ];

          const buttonsList = document.getElementById('buttons_list');
          const pressuresList = document.getElementById('pressures_list');
          buttonsList.innerHTML = '';
          pressuresList.innerHTML = '';

          buttonNames.forEach(name => {
            const buttonItem = document.createElement('li');
            buttonItem.id = `button_${name}_value`;
            buttonItem.innerText = `${name}: Not Pressed`;
            buttonsList.appendChild(buttonItem);

            const pressureItem = document.createElement('li');
            pressureItem.id = `pressure_${name}`;
            pressureItem.innerText = `${name} Pressure: 0.00`;
            pressuresList.appendChild(pressureItem);
          });
        },
        updateButtonValue: function(name, state, pressure) {
          const buttonItem = document.getElementById(`button_${name}_value`);
          const pressureItem = document.getElementById(`pressure_${name}`);
          const buttonVisual = document.getElementById(`button_${name}`);

          if (buttonItem) {
            buttonItem.innerText = `${name}: ${state === 'down' ? 'Pressed' : 'Not Pressed'}`;
          }

          if (pressureItem) {
            pressureItem.innerText = `${name} Pressure: ${pressure.toFixed(2)}`;
          }

          if (buttonVisual) {
            buttonVisual.classList.toggle('bg-green-500', state === 'down');
            buttonVisual.classList.toggle('bg-gray-600', state !== 'down');
          }
        },
        updateAxesValues: function(axes) {
          const axesList = document.getElementById('axes_list');
          axesList.innerHTML = '';

          axes.forEach((value, index) => {
            const axisItem = document.createElement('li');
            axisItem.innerText = `Axis ${index + 1}: ${value.toFixed(2)}`;
            axesList.appendChild(axisItem);
          });
        },
        updateSticks: function(axes) {
          const leftStick = document.querySelector('[data-stick="left"]');
          const rightStick = document.querySelector('[data-stick="right"]');
          
          if (leftStick) {
            leftStick.style.transform = `translate(${axes[0] * 20}px, ${axes[1] * 20}px)`;
          }
          
          if (rightStick) {
            rightStick.style.transform = `translate(${axes[2] * 20}px, ${axes[3] * 20}px)`;
          }
        }
      }

      // Initialize the gamepad debug window
      debug_gamepad_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
