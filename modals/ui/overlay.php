<div data-window="ui_overlay_window" data-close="false">
  <div id="ui_overlay_window" class="w-72 fixed top-2 right-2 z-10 flex rounded flex-col bg-gray-800 text-white p-4 shadow-lg">
    <!-- Time Display -->
    <div class="flex items-center justify-between mb-4">
      <span id="game_time" class="text-white rounded-md">00:00</span>
    </div>

    <!-- Health Bar -->
    <div class="mb-4">
      <span class="text-lg font-semibold">Health:</span>
      <div class="w-full bg-gray-600 rounded-full h-4 mt-2">
        <div id="health-bar" class="bg-red-500 h-4 rounded-full" style="width: 75%;"></div>
      </div>
    </div>

    <!-- Energy -->
    <div class="mb-4">
      <span class="text-lg font-semibold">Energy:</span>
      <div class="w-full bg-gray-600 rounded-full h-4 mt-2">
        <div id="energy-bar" class="bg-blue-500 h-4 rounded-full" style="width: 50%;"></div>
      </div>
    </div>

    <!-- Coins -->
    <div class="flex items-center justify-between">
      <span class="text-lg font-semibold">Coins:</span>
      <span id="player-coins" class="text-lg font-bold">100</span>
    </div>
  </div>

  <script>
    var ui_overlay_window = {
      start: function () {
      },
      unmount: function () {
        // Cleanup logic
      }
    };

    ui_overlay_window.start();
  </script>
</div>
