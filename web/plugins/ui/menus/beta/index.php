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
window[id] = {
  id: id,
      start: function() {
        plugin.front('beta_window');
        plugin.close('main_title_window');
        gamepad.updateButtonImages();
      },

      unmount: function() {
        this.plugin_init();
      },

      aButton: function() {
        plugin.close('beta_window');
      },

      plugin_init: function() {

if (utils.isMobileDevice()) {
    plugin.load({ id: 'joypad_window', url: 'utils/joypad/index.php', name: 'joypad', drag: false, reload: true, hidden: false });
    plugin.load({ id: 'auth_window', url: 'auth/index.php', drag: true, reload: true });
    plugin.load({ id: 'ui_overlay_window', url: 'ui/overlay/index.php', drag: false, reload: false });
    plugin.load({ id: 'ui_inventory_window', url: 'ui/inventory/index.php', drag: false, reload: false });
    plugin.load({ id: 'speech_window', url: 'ui/speech/index.php', drag: false, reload: true, hidden: true });
    plugin.load({ id: 'keyboard_window', url: 'utils/keyboard/index.html', drag: false, reload: true, hidden: true });
    plugin.load({ id: 'weather_plugin', url: 'effects/weather/index.js', drag: false, reload: true });

    utils.fullScreen();

} else {

    plugin.preload([
        { priority: 1, options: { id: 'console_window', url: 'editor/console/index.php', drag: false, reload: false } },
        { priority: 2, options: { id: 'context_menu_window', url: 'ui/menus/context_menu/index.php', drag: true, reload: false } },
        { priority: 4, options: { id: 'pie_menu_window', url: 'ui/menus/pie/index.php', drag: false, reload: false, hidden: false } },
        { priority: 5, options: { id: 'ui_overlay_window', url: 'ui/hud/index.php', drag: false, reload: false } },
        { priority: 6, options: { id: 'speech_window', url: 'ui/speech/index.php', drag: false, reload: true, hidden: true } },
        { priority: 7, options: { id: 'ui_inventory_window', url: 'ui/inventory/index.php', drag: false, reload: false } },
        { priority: 8, options: { id: 'weather_plugin', url: 'effects/weather/index.js', drag: false, reload: true } },
        { priority: 9, options: { id: 'auth_window', url: 'auth/index.php', drag: true, reload: true } }
    ]);

    camera.panning = false;
    camera.manual = false;
    camera.activeCamera = true;
    camera.cutsceneMode = false;
    utils.gameTime.hours = 22;
    game.timeActive = true;

    const playerOptions = {
      id: game.playerid,  
      x: game.x,
      y: game.y,
      isPlayer: true,
      topSpeed: 100,
      canShoot: true,
      animalType: 'female-01',
      targetAim: false,
      maxRange: 200,
      health: 100,
      energy: 100,
      handOffsetX: 8,
      handOffsetY: -5
    };
              
    sprite.create(playerOptions);

    game.mainSprite = game.sprites[game.playerid];

}

}
    };
  </script>