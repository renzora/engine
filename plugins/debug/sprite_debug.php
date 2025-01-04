<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='sprite_debug_window' class='window window_bg' style='width: 400px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Sprite Debugger</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <div class="grid grid-cols-2 gap-4 mb-4">
          <div>
            <label for="sprite-id" class="block text-sm font-medium text-gray-700">Sprite ID:</label>
            <input type="text" id="sprite-id" name="sprite-id" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
          <div>
            <label for="sprite-x" class="block text-sm font-medium text-gray-700">X Position:</label>
            <input type="number" id="sprite-x" name="sprite-x" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
          <div>
            <label for="sprite-y" class="block text-sm font-medium text-gray-700">Y Position:</label>
            <input type="number" id="sprite-y" name="sprite-y" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
          <div>
            <label for="sprite-outfit" class="block text-sm font-medium text-gray-700">Outfit:</label>
            <select id="sprite-outfit" name="sprite-outfit" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
              <option value="0">Outfit 1</option>
              <option value="1">Outfit 2</option>
              <option value="2">Outfit 3</option>
              <!-- Add more options as needed -->
            </select>
          </div>
          <div>
            <label for="sprite-hair" class="block text-sm font-medium text-gray-700">Hair:</label>
            <select id="sprite-hair" name="sprite-hair" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
            <?php 
                for($i=0; $i <= 29; $i++) {
                    ?>
              <option value="<?php echo $i; ?>">Hair <?php echo $i + 1; ?></option>
              <?php 
                }
                ?>
              <!-- Add more options as needed -->
            </select>
          </div>
          <div>
            <label for="sprite-duration" class="block text-sm font-medium text-gray-700">Duration (s):</label>
            <input type="number" id="sprite-duration" name="sprite-duration" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
        </div>

        <div class="grid grid-cols-2 gap-4 mb-4">
          <div>
            <label for="sprite-health" class="block text-sm font-medium text-gray-700">Health:</label>
            <input type="number" id="sprite-health" name="sprite-health" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
          <div>
            <label for="sprite-attack" class="block text-sm font-medium text-gray-700">Attack:</label>
            <input type="number" id="sprite-attack" name="sprite-attack" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
          <div>
            <label for="sprite-defense" class="block text-sm font-medium text-gray-700">Defense:</label>
            <input type="number" id="sprite-defense" name="sprite-defense" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
          <div>
            <label for="sprite-intensity" class="block text-sm font-medium text-gray-700">Intensity:</label>
            <input type="number" id="sprite-intensity" name="sprite-intensity" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-indigo-500 focus:border-indigo-500 sm:text-sm">
          </div>
        </div>

        <div class="mb-4">
          <label class="inline-flex items-center mt-2">
            <input type="checkbox" id="sprite-is-enemy" name="sprite-is-enemy" class="form-checkbox h-5 w-5 text-indigo-600">
            <span class="ml-2 text-gray-700">Is Enemy</span>
          </label>
        </div>

        <div class="mb-4">
          <button onclick="sprite_debug_window.createNPCSprite()" class="bg-green-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Create Sprite</button>
        </div>
        <div class="grid grid-cols-2 gap-4">
          <button onclick="sprite_debug_window.moveSprite('up')" class="bg-blue-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Up</button>
          <button onclick="sprite_debug_window.moveSprite('down')" class="bg-blue-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Down</button>
          <button onclick="sprite_debug_window.moveSprite('left')" class="bg-blue-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Left</button>
          <button onclick="sprite_debug_window.moveSprite('right')" class="bg-blue-500 text-white font-bold py-3 px-4 rounded w-full mt-2 shadow-md">Right</button>
        </div>
      </div>
    </div>

    <script>
      var sprite_debug_window = {
        createNPCSprite: function() {
          var id = document.getElementById('sprite-id').value;
          var x = parseInt(document.getElementById('sprite-x').value) || 0;
          var y = parseInt(document.getElementById('sprite-y').value) || 0;
          var outfit = parseInt(document.getElementById('sprite-outfit').value) || 0;
          var hair = parseInt(document.getElementById('sprite-hair').value) || 0;
          var health = parseInt(document.getElementById('sprite-health').value) || 100;
          var attack = parseInt(document.getElementById('sprite-attack').value) || 10;
          var defense = parseInt(document.getElementById('sprite-defense').value) || 5;
          var intensity = parseInt(document.getElementById('sprite-intensity').value) || 1;
          var isEnemy = document.getElementById('sprite-is-enemy').checked;

          var options = {
            id: id, 
            x: x, 
            y: y, 
            hairstyle: hair, 
            outfit: outfit, 
            health: health, 
            maxHealth: health, 
            attack: attack, 
            defense: defense, 
            intensity: intensity, 
            isEnemy: isEnemy 
          };

          sprite.createSprite(options);

          console.log(`Created sprite with ID: ${id}, X: ${x}, Y: ${y}, Outfit: ${outfit}, Hair: ${hair}, Health: ${health}, Attack: ${attack}, Defense: ${defense}, Intensity: ${intensity}, IsEnemy: ${isEnemy}`);
        },

        moveSprite: function(direction) {
          var id = document.getElementById('sprite-id').value;
          var duration = parseInt(document.getElementById('sprite-duration').value) || 1;
          var spriteInstance = game.sprites[id];
          if (spriteInstance) {
            sprite.moveSprite(spriteInstance, direction, duration);
          } else {
            console.log(`Sprite with ID: ${id} not found`);
          }
        },

        start: function() {
          // Initialize sprite debugger
        },

        unmount: function() {
          // Cleanup if necessary
        }
      }
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
