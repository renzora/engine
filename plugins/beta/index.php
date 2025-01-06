<div class='fixed inset-0 flex items-center justify-center p-4 bg-black/50' style='pointer-events: auto;'>
    <div data-window='network_connect_window' class='window fade-in-scale pixel-corners shadow-lg' style='width: 500px; background: #122f5d;'>

      <div data-part='handle' class='window_title text-yellow-100 p-2 rounded-t' style="background: #122f5d;cursor: default;">
        <div data-part='title' class='title_bg window_border' style="color: #a1b3cf; background: #122f5d;">
          Alert from Renzora Staff
        </div>
      </div>

      <div class='clearfix'></div>

      <div class='relative'>
        <div class='container text-white p-4 text-center'>
          <p class="text-xl m-0">Welcome to the <span class="font-bold text-yellow-500">Renzora Beta</span>! This game is currently in development and will most likely contain issues such as crashes, frame rate drops, bugs, and glitches.</p>
          <p class="mt-4">You may encounter problems like data loss, unintended game logic or strange character behavior. This game is completely safe to play and will NOT in any way affect your computer system. We apologize for any inconvenience this may cause and we will work to address any problems as soon as possible.</p>
          <p class="mt-4">If you find any problems or you have any suggestions for new features, please feel free to message us on our Discord server:
            <a href="https://discord.gg/NudmMSxand" target="_blank" class="text-blue-300 underline hover:text-blue-500 transition">
              Renzora Official Server
            </a>
          </p>
          <p class="mt-4 font-semibold">Thanks for playing, and have fun!</p>
        </div>
      </div>

      <div class="flex justify-center p-4">
      <button 
  class="green_button px-2 py-2 bg-blue-400 hover:bg-blue-500 text-blue-900 font-bold rounded-lg transition duration-300 flex items-center justify-between w-auto" 
  data-close>
  <span class="ml-2 mr-4">Start Game</span>
  <div class="gamepad_button_x w-6 h-6 bg-gray-800/30 rounded-full mr-0"></div>
</button>

      </div>
    </div>
  </div>
  
  <script>
beta_window = {
      start: function() {
        gamepad.updateButtonImages();
        plugin.front('beta_window');
        plugin.close('main_title_window');
      },

      unmount: function() {
        game.plugin_init();
      },

      aButton: function() {
        plugin.close('beta_window');
      }
    };
  </script>