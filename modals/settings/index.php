<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if($auth) {
?>
  <div data-window='settings_window' class='window window_bg' style='width: 330px;'>
    <div data-part='handle' class='window_title window_border'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border'>Settings</div>
    </div>
    <div class='clearfix'></div>
    <div class='container text-light window_body p-2'>
      <div class='clearfix mt-3'></div>

      <div class="volume-control mt-4 mb-4">
        <label for="volumeControl" class="block text-white text-lg mb-2">Volume:</label>
        <input type="range" id="volumeControl" class="slider-thumb w-full h-2 rounded-lg cursor-pointer" min="0" max="1" step="0.01" value="0.5">
      </div>

      <div class="lerp-control mt-4 mb-4">
        <label for="lerpControl" class="block text-white text-lg mb-2">Camera Smoothness:</label>
        <input type="range" id="lerpControl" class="slider-thumb w-full h-2 rounded-lg cursor-pointer" min="0.05" max="0.4" step="0.01" value="0.1">
      </div>

      <div class="color-control mt-4 mb-4">
        <label for="colorControl" class="block text-white text-lg mb-2">Black & White:</label>
        <input type="checkbox" id="colorControl" class="cursor-pointer">
      </div>

      <div class="night-color-control mt-4 mb-4">
        <label for="nightColor" class="block text-white text-lg mb-2">Night Color Filter:</label>
        <input type="color" id="nightColor" value="#000032" class="cursor-pointer">
      </div>

      <div class="night-opacity-control mt-4 mb-4">
        <label for="nightOpacity" class="block text-white text-lg mb-2">Night Filter Opacity:</label>
        <input type="range" id="nightOpacity" class="slider-thumb w-full h-2 rounded-lg cursor-pointer" min="0" max="1" step="0.01" value="0.7">
      </div>

      <button data-close onclick='modal.load("auth/signout.php", "signout_window");'
        class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded-full w-full">
        Sign Out
      </button>
    </div>

    <script>
    var settings_window = {
      start: function() {
        var lerpControl = document.getElementById('lerpControl');
        var storedLerpFactor = localStorage.getItem('lerpFactor') || 0.1;
        lerpControl.value = storedLerpFactor;
        game.lerpFactor = parseFloat(storedLerpFactor);

        lerpControl.addEventListener('input', function() {
          var lerpFactor = parseFloat(lerpControl.value);
          localStorage.setItem('lerpFactor', lerpFactor);
          game.lerpFactor = lerpFactor;
        });

        var colorControl = document.getElementById('colorControl');
        var isBlackAndWhite = localStorage.getItem('blackAndWhite') === 'true';
        colorControl.checked = isBlackAndWhite;
        game.setBlackAndWhiteMode(isBlackAndWhite);

        colorControl.addEventListener('change', function() {
          var isBlackAndWhite = colorControl.checked;
          localStorage.setItem('blackAndWhite', isBlackAndWhite);
          game.setBlackAndWhiteMode(isBlackAndWhite);
        });

        var nightColorControl = document.getElementById('nightColor');
        var nightOpacityControl = document.getElementById('nightOpacity');

        var nightColor = localStorage.getItem('nightColor') || '#000032';
        var nightOpacity = localStorage.getItem('nightOpacity') || 0.7;

        nightColorControl.value = nightColor;
        nightOpacityControl.value = nightOpacity;

        weather.nightColor = nightColor;
        weather.nightOpacity = parseFloat(nightOpacity);

        nightColorControl.addEventListener('input', function() {
          var nightColor = nightColorControl.value;
          localStorage.setItem('nightColor', nightColor);
          weather.nightColor = nightColor;
        });

        nightOpacityControl.addEventListener('input', function() {
          var nightOpacity = parseFloat(nightOpacityControl.value);
          localStorage.setItem('nightOpacity', nightOpacity);
          weather.nightOpacity = nightOpacity;
        });
      },
      unmount: function() {
        // Any cleanup if necessary
      }
    };

    settings_window.start();
  </script>
  </div>
<?php
}
?>
