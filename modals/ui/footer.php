<div data-window='ui_footer_window' data-close="false">
  <div class='fixed bottom-0 right-2 z-10 text-sm mb-1 flex space-x-4 tracking-tight'>
    <a href="https://github.com/renzora/web/commit/39818fa47c6679e837eb921481013a72f04cf712" 
       target="_blank" 
       class="text-white rounded-md">Renzora v0.3.0+dev-build-99</a>
    <span class="text-white" id="input_method">Input: keyboard</span>
    <span class="text-white rounded-md cursor-pointer" id="gameFps" onclick="modal.load({ id: 'fps_monitor_window', url: 'debug/fps.php', name: 'FPS monitor', drag: true, reload: true });"></span>
    <span id="tiles_rendered" class="text-white rounded-md"></span>
    <span id="lights_rendered" class="text-white rounded-md"></span>
    <span id="effects_rendered" class="text-white rounded-md"></span>
  </div>
</div>

<script>
  var ui_footer_window = {

  };
</script>
