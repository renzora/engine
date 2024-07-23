<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='tileset_window' class='window window_bg' style='width: 900px; height: 500px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Tileset Manager</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2' style='height: 460px; overflow-y: hidden;'>

        <div id="tileset_window_tabs" style="height: 100%;">
          <div id="tabs" class="flex border-b border-gray-300">
            <button class="tab text-gray-800 p-2" data-tab="tab1">Upload</button>
          </div>

          <div class="tab-content mt-2 hidden" data-tab-content="tab1" style="height: calc(100% - 35px);">
            <div class="flex h-full">
              <div class="w-9/12 p-2 pl-0 flex flex-col">
                <!-- Left column content -->
                <div id="drop_zone" class="flex-grow w-full border border-gray-300 rounded overflow-auto" style="position: relative;">
                  <p id="drop_prompt">Drop an image here to upload</p>
                  <canvas id="uploaded_canvas" style="display: none;"></canvas>
                </div>
              </div>
              <div class="w-3/12 p-2" style="height: 100%; overflow-y: scroll;">
                <!-- Right column content -->
                <div class="mb-4">
                  <label for="name" class="block text-gray-700">Name:</label>
                  <input type="text" id="name" name="name" class="w-full p-2 border border-gray-300 rounded disabled:opacity-50" disabled>
                </div>
                <div class="mb-4">
                  <label for="description" class="block text-gray-700">Description:</label>
                  <textarea id="description" name="description" class="w-full p-2 border border-gray-300 rounded disabled:opacity-50" disabled></textarea>
                </div>
                <div class="mb-4">
                  <h4 class="font-bold text-gray-800">Animation:</h4>
                  <label for="duration" class="block text-gray-700">Duration:</label>
                  <input type="range" id="duration" name="duration" min="0" max="200" value="0" class="w-full disabled:opacity-50" disabled>
                </div>
                <div class="mb-4">
                  <h4 class="font-bold text-gray-800">Lighting:</h4>
                  <label for="color" class="block text-gray-700">Color:</label>
                  <input type="color" id="color" name="color" class="w-full disabled:opacity-50" disabled>
                  <label for="intensity" class="block text-gray-700 mt-2">Intensity:</label>
                  <input type="range" id="intensity" name="intensity" min="0" max="1" step="0.01" value="0" class="w-full disabled:opacity-50" disabled>
                  <label for="flicker_speed" class="block text-gray-700 mt-2">Flicker Speed:</label>
                  <input type="range" id="flicker_speed" name="flicker_speed" min="0" max="1" step="0.01" value="0" class="w-full disabled:opacity-50" disabled>
                  <label for="flicker_amount" class="block text-gray-700 mt-2">Flicker Amount:</label>
                  <input type="range" id="flicker_amount" name="flicker_amount" min="0" max="1" step="0.01" value="0" class="w-full disabled:opacity-50" disabled>
                </div>
              </div>
            </div>
          </div>

        </div>

      </div>
    </div>

    <style>
      #drop_zone {
        display: flex;
        justify-content: center;
        align-items: center;
      }
      #drop_zone.dropped {
        justify-content: flex-start;
        align-items: flex-start;
        overflow: auto;
      }
      #uploaded_canvas {
        width: 100%;
        height: auto;
      }
    </style>

    <script>
      var tileset_window = {
        start: function() {
          ui.initTabs('tileset_window_tabs', 'tab1');

          // Drag and drop functionality
          var dropZone = document.getElementById('drop_zone');
          var dropPrompt = document.getElementById('drop_prompt');
          var uploadCanvas = document.getElementById('uploaded_canvas');
          var ctx = uploadCanvas.getContext('2d');

          dropZone.addEventListener('dragover', function(e) {
            e.preventDefault();
            dropZone.style.borderColor = '#000';
          });

          dropZone.addEventListener('dragleave', function(e) {
            e.preventDefault();
            dropZone.style.borderColor = '#ccc';
          });

          dropZone.addEventListener('drop', function(e) {
            e.preventDefault();
            dropZone.style.borderColor = '#ccc';
            var files = e.dataTransfer.files;
            if (files.length > 0) {
              var reader = new FileReader();
              reader.onload = function(event) {
                var img = new Image();
                img.onload = function() {
                  // Set canvas size to image size
                  uploadCanvas.width = img.width;
                  uploadCanvas.height = img.height;
                  ctx.drawImage(img, 0, 0, img.width, img.height);
                  uploadCanvas.style.display = 'block';
                  dropPrompt.style.display = 'none';
                  dropZone.classList.add('dropped');
                }
                img.src = event.target.result;
              };
              reader.readAsDataURL(files[0]);
            }
          });
        },
        unmount: function() {
          ui.destroyTabs('tileset_window_tabs');
        }
      }
      tileset_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
