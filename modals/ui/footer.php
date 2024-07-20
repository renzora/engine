<div data-window='ui_footer_window' data-close="false">
  <div class='fixed bottom-0 right-2 z-10 text-sm mb-1 flex space-x-4 tracking-tight'>
    <span class="text-white rounded-md">Renzora v0.0.7</span>
    <span class="text-white" id="input_method">Input: keyboard</span>
    <span class="text-white rounded-md" id="gameFps"></span>
    <span id="tiles_rendered" class="text-white rounded-md"></span>
    <span id="game_time" class="text-white rounded-md">00:00</span>
    <span id="lights_rendered" class="text-white rounded-md"></span>
    <span id="effects_rendered" class="text-white rounded-md"></span>
    <button onclick="ui_footer_window.load();">Minimap</button>
    <button onclick="modal.load('debug', null, 'Debugger', true);">debug</button>
    <button onclick="modal.load('debug/utils.php', 'debug_utils_window', 'Debugger', true);">utils</button>
  </div>
</div>

<script>
  var ui_footer_window = {
    load: function(modalName) {
      audio.playAudio("walkAudio", assets.load('walkAudio'), 'sfx');
    }
  };
</script>
