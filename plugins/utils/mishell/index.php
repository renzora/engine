<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if($auth) {
?>
  <div class='window window_bg' style='width: 330px; background: #000;'>
  
    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#103515 1px, transparent 0) !important;'>
    <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #000; color: #4cab5d;'>😊MiShell</div>
    </div>
    <div class='clearfix'></div>
    <div class='position-relative'>
      <div class='container text-light window_body p-2'>
        <input id="mishell_prompt" type="text" autocomplete="off" placeholder="Type command or help and press enter" class="w-full bg-black text-white border-0 outline-0" onkeyup="if(event.key === 'Enter' || event.keyCode === 13) { mishell_window.enter(); }" />
      </div>
    </div>
</div>

    <script>
mishell_window = {
        start: function() {
          document.getElementById('mishell_prompt').focus();
        },
        enter: function() {
        var mishellPrompt = document.getElementById('mishell_prompt');
      var prompt = mishellPrompt.value;
      var words = prompt.split(' ');

      if(words[0] === 'load') {
        plugin.load(words[1]);
      } else if(words[0] === 'debug') {
        plugin.load('debug');
      } else if(prompt === 'closeAll') {
        plugin.closeAll();
        plugin.load('mishell');
      } else if(words[0] === 'close') {
        plugin.close(words[1] + '_window');
      } else if(words[0] === 'reload') {
        plugin.close(words[1] + '_window');
        plugin.load(words[1]);
      } else if(prompt === 'new room') {
        plugin.load('createScene')
      } else if(words[0] === 'quit') {
        plugin.close('mishell_window');
      }

      mishellPrompt.value = '';
    },
        unmount: function() {
            
        }
      }
    </script>
<?php
}
?>