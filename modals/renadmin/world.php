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

          <div class="tab-content p-4 hidden" data-tab-content="tab2">
            <p>Toggle Weather Effects:</p>
            <label for="toggle_snow">Snow:</label>
            <input type="checkbox" id="toggle_snow">
            <label for="toggle_rain">Rain:</label>
            <input type="checkbox" id="toggle_rain">
            <label for="toggle_fog">Fog:</label>
            <input type="checkbox" id="toggle_fog">
            <label for="toggle_stars">Stars:</label>
            <input type="checkbox" id="toggle_stars">
            <button id="set_weather_button">Set Weather</button>
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
          var stars = document.getElementById('toggle_stars').checked;

          weather.snowActive = snow;
          weather.rainActive = rain;
          weather.fogActive = fog;
          weather.starsActive = stars;

          if (snow) weather.createSnow(0.5); else weather.stopSnow();
          if (rain) weather.createRain(0.7);
          if (fog) weather.createFog(0.1);
          if (stars) weather.createStars();

          console.log('Weather updated: Snow -', snow, ', Rain -', rain, ', Fog -', fog, ', Stars -', stars);
        }
      }
      renadmin_world_editor_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
