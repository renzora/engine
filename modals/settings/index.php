<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if($auth) {
?>
  <div data-window='settings_window' class='window window_bg' style='width: 400px;'>
    <div data-part='handle' class='window_title window_border'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border'>Settings</div>
    </div>
    <div class='clearfix'></div>
    <div class='container text-light window_body p-2'>
      <div class="flex border-b border-gray-300">
        <button class="tablinks py-2 px-4 text-white hover:text-blue-600 focus:outline-none" onclick="settings_window.openTab(event, 'Visual')">Visual</button>
        <button class="tablinks py-2 px-4 text-white hover:text-blue-600 focus:outline-none" onclick="settings_window.openTab(event, 'Audio')">Audio</button>
        <button class="tablinks py-2 px-4 text-white hover:text-blue-600 focus:outline-none" onclick="settings_window.openTab(event, 'Controller')">Controller</button>
        <button class="tablinks py-2 px-4 text-white hover:text-blue-600 focus:outline-none" onclick="settings_window.openTab(event, 'Keyboard')">Keyboard</button>
      </div>

      <div id="Visual" class="tabcontent mt-4">
        <div class="lerp-control mt-4 mb-4">
          <label for="lerpControl" class="block text-white text-lg mb-2">Camera Smoothness:</label>
          <input type="range" id="lerpControl" class="slider-thumb w-full h-2 rounded-lg cursor-pointer" min="0.05" max="0.4" step="0.01" value="0.1">
        </div>

        <div class="night-color-control mt-4 mb-4">
          <label for="nightColor" class="block text-white text-lg mb-2">Night Color Filter:</label>
          <input type="color" id="nightColor" value="#000032" class="cursor-pointer">
        </div>

        <div class="night-opacity-control mt-4 mb-4">
          <label for="nightOpacity" class="block text-white text-lg mb-2">Night Filter Opacity:</label>
          <input type="range" id="nightOpacity" class="slider-thumb w-full h-2 rounded-lg cursor-pointer" min="0" max="1" step="0.01" value="0.7">
        </div>
      </div>

      <div id="Audio" class="tabcontent mt-4" style="display:none;">
        <div class="volume-control mt-4 mb-4">
          <label for="volumeControl" class="block text-white text-lg mb-2">Volume:</label>
          <input type="range" id="volumeControl" class="slider-thumb w-full h-2 rounded-lg cursor-pointer" min="0" max="1" step="0.01" value="0.5">
        </div>
      </div>

      <div id="Controller" class="tabcontent mt-4" style="display:none;">
        <div class="controller-select mt-4 mb-4">
          <label for="defaultController" class="block text-white text-lg mb-2">Default Controller:</label>
          <select id="defaultController" class="w-full p-2 rounded-lg cursor-pointer">
            <option value="controller1">Controller 1</option>
            <option value="controller2">Controller 2</option>
            <option value="controller3">Controller 3</option>
          </select>
        </div>
      </div>

      <div id="Keyboard" class="tabcontent mt-4" style="display:none;">
        <!-- Keyboard settings can go here -->
      </div>

      <button data-close onclick='modal.load("auth/signout.php", "signout_window");'
        class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded-full w-full mt-4">
        Sign Out
      </button>
    </div>

    <script>
    var settings_window = {
      start: function() {
        this.initLerpControl();
        this.initColorControl();
        this.initNightControls();
        this.initVolumeControl();
        this.initControllerControl();
      },
      initLerpControl: function() {
        var lerpControl = document.getElementById('lerpControl');
        var storedLerpFactor = localStorage.getItem('lerpFactor') || 0.1;
        lerpControl.value = storedLerpFactor;
        game.lerpFactor = parseFloat(storedLerpFactor);

        this.lerpControlListener = function() {
          var lerpFactor = parseFloat(lerpControl.value);
          localStorage.setItem('lerpFactor', lerpFactor);
          game.lerpFactor = lerpFactor;
        };

        lerpControl.addEventListener('input', this.lerpControlListener);
      },
      initColorControl: function() {
        var colorControl = document.getElementById('colorControl');
        var isBlackAndWhite = localStorage.getItem('blackAndWhite') === 'true';
        colorControl.checked = isBlackAndWhite;
        game.setBlackAndWhiteMode(isBlackAndWhite);

        this.colorControlListener = function() {
          var isBlackAndWhite = colorControl.checked;
          localStorage.setItem('blackAndWhite', isBlackAndWhite);
          game.setBlackAndWhiteMode(isBlackAndWhite);
        };

        colorControl.addEventListener('change', this.colorControlListener);
      },
      initNightControls: function() {
        var nightColorControl = document.getElementById('nightColor');
        var nightOpacityControl = document.getElementById('nightOpacity');

        var nightColor = localStorage.getItem('nightColor') || '#000032';
        var nightOpacity = localStorage.getItem('nightOpacity') || 0.7;

        nightColorControl.value = nightColor;
        nightOpacityControl.value = nightOpacity;

        weather.nightColor = nightColor;
        weather.nightOpacity = parseFloat(nightOpacity);

        this.nightColorControlListener = function() {
          var nightColor = nightColorControl.value;
          localStorage.setItem('nightColor', nightColor);
          weather.nightColor = nightColor;
        };

        this.nightOpacityControlListener = function() {
          var nightOpacity = parseFloat(nightOpacityControl.value);
          localStorage.setItem('nightOpacity', nightOpacity);
          weather.nightOpacity = nightOpacity;
        };

        nightColorControl.addEventListener('input', this.nightColorControlListener);
        nightOpacityControl.addEventListener('input', this.nightOpacityControlListener);
      },
      initVolumeControl: function() {
        var volumeControl = document.getElementById('volumeControl');
        var storedVolume = localStorage.getItem('volume') || 0.5;
        volumeControl.value = storedVolume;

        this.volumeControlListener = function() {
          var volume = parseFloat(volumeControl.value);
          localStorage.setItem('volume', volume);
          game.setVolume(volume);
        };

        volumeControl.addEventListener('input', this.volumeControlListener);
      },
      initControllerControl: function() {
        var defaultController = document.getElementById('defaultController');
        var storedController = localStorage.getItem('defaultController') || 'controller1';
        defaultController.value = storedController;
        game.defaultController = storedController;

        this.defaultControllerListener = function() {
          var selectedController = defaultController.value;
          localStorage.setItem('defaultController', selectedController);
          game.defaultController = selectedController;
        };

        defaultController.addEventListener('change', this.defaultControllerListener);
      },
      openTab: function(evt, tabName) {
        var i, tabcontent, tablinks;
        tabcontent = document.getElementsByClassName("tabcontent");
        for (i = 0; i < tabcontent.length; i++) {
          tabcontent[i].style.display = "none";
        }
        tablinks = document.getElementsByClassName("tablinks");
        for (i = 0; i < tablinks.length; i++) {
          tablinks[i].className = tablinks[i].className.replace(" active", "");
        }
        document.getElementById(tabName).style.display = "block";
        evt.currentTarget.className += " active";
      },
      unmount: function() {
        document.getElementById('lerpControl').removeEventListener('input', this.lerpControlListener);
        document.getElementById('colorControl').removeEventListener('change', this.colorControlListener);
        document.getElementById('nightColor').removeEventListener('input', this.nightColorControlListener);
        document.getElementById('nightOpacity').removeEventListener('input', this.nightOpacityControlListener);
        document.getElementById('volumeControl').removeEventListener('input', this.volumeControlListener);
        document.getElementById('defaultController').removeEventListener('change', this.defaultControllerListener);
      }
    };

    settings_window.start();

    </script>
  </div>
<?php
}
?>
