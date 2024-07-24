<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='tileset_window' class='window window_bg' style='width: 1200px;height: 700px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Tileset Manager</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2' style='height: 660px; overflow-y: hidden;'>

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

      var shiftPressed = false;
      var isDragging = false;
      var startX, startY, scrollLeft, scrollTop;

      var ctrlPressed = false;
      var middleMousePressed = false;
      var scale = 1;

      document.addEventListener('keydown', function(e) {
        if (e.key === 'Shift') {
          shiftPressed = true;
        }
        if (e.key === 'Control') {
          ctrlPressed = true;
        }
      });

      document.addEventListener('keyup', function(e) {
        if (e.key === 'Shift') {
          shiftPressed = false;
        }
        if (e.key === 'Control') {
          ctrlPressed = false;
        }
      });

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
              
              // Draw the 16x16 grid
              drawGrid(ctx, img.width, img.height);
            }
            img.src = event.target.result;
          };
          reader.readAsDataURL(files[0]);
        }
      });

      function drawGrid(ctx, width, height) {
        ctx.strokeStyle = '#000000'; // Grid line color
        ctx.lineWidth = 0.5; // Grid line width

        for (var x = 0; x <= width; x += 16) {
          ctx.moveTo(x, 0);
          ctx.lineTo(x, height);
        }

        for (var y = 0; y <= height; y += 16) {
          ctx.moveTo(0, y);
          ctx.lineTo(width, y);
        }

        ctx.stroke();
      }

      uploadCanvas.addEventListener('mousedown', function(e) {
        if (e.button === 1) { // middle mouse button
          middleMousePressed = true;
          startX = e.pageX - uploadCanvas.offsetLeft;
          startY = e.pageY - uploadCanvas.offsetTop;
          scrollLeft = dropZone.scrollLeft;
          scrollTop = dropZone.scrollTop;
          e.preventDefault(); // prevent default middle mouse scroll
        } else if (e.button === 0 && shiftPressed) { // left mouse button with shift
          isDragging = true;
          startX = e.pageX - uploadCanvas.offsetLeft;
          startY = e.pageY - uploadCanvas.offsetTop;
          scrollLeft = dropZone.scrollLeft;
          scrollTop = dropZone.scrollTop;
        }
      });

      uploadCanvas.addEventListener('mouseleave', function() {
        isDragging = false;
        middleMousePressed = false;
      });

      uploadCanvas.addEventListener('mouseup', function(e) {
        if (e.button === 1) {
          middleMousePressed = false;
        } else if (e.button === 0 && shiftPressed) {
          isDragging = false;
        }
      });

      uploadCanvas.addEventListener('mousemove', function(e) {
        if (middleMousePressed) {
          e.preventDefault();
          var x = e.pageX - uploadCanvas.offsetLeft;
          var y = e.pageY - uploadCanvas.offsetTop;
          var walkX = (x - startX);
          var walkY = (y - startY);
          dropZone.scrollLeft = scrollLeft - walkX;
          dropZone.scrollTop = scrollTop - walkY;
        } else if (isDragging) {
          e.preventDefault();
          var x = e.pageX - uploadCanvas.offsetLeft;
          var y = e.pageY - uploadCanvas.offsetTop;
          var walkX = (x - startX);
          var walkY = (y - startY);
          dropZone.scrollLeft = scrollLeft - walkX;
          dropZone.scrollTop = scrollTop - walkY;
        }
      });

      dropZone.addEventListener('wheel', function(e) {
        if (ctrlPressed) {
          e.preventDefault();
          var rect = uploadCanvas.getBoundingClientRect();
          var offsetX = e.clientX - rect.left; // cursor X position on canvas
          var offsetY = e.clientY - rect.top; // cursor Y position on canvas

          var delta = e.deltaY > 0 ? -0.2 : 0.2; // Increased zoom rate
          var previousScale = scale;
          scale += delta;
          if (scale < 0.5) scale = 0.5; // minimum scale set to 0.5
          if (scale > 2) scale = 2; // maximum scale set to 2
          uploadCanvas.style.transform = `scale(${scale})`;
          uploadCanvas.style.transformOrigin = 'top left';

          // Calculate the new scroll positions
          var newScrollLeft = (offsetX * scale / previousScale) - offsetX;
          var newScrollTop = (offsetY * scale / previousScale) - offsetY;

          dropZone.scrollLeft += newScrollLeft;
          dropZone.scrollTop += newScrollTop;

          updateScrollbarHeight();
        }
      });

      function updateScrollbarHeight() {
        dropZone.style.height = `${uploadCanvas.height * scale}px`;
      }
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
