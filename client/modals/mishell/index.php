<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if($auth) {
?>
  <div data-window='mishell_window' class='window window_bg' style='width: 330px; background: #000;'>
  
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

    <script>
      var mishell_window = {
        start: function() {
          document.getElementById('mishell_prompt').focus();
        },
        enter: function() {
        var mishellPrompt = document.getElementById('mishell_prompt');
      var prompt = mishellPrompt.value;
      var words = prompt.split(' ');

      if(words[0] === 'load') {
        modal.load(words[1]);
      } else if(words[0] === 'debug') {
        modal.load('debug');
      } else if(prompt === 'closeAll') {
        modal.closeAll();
        modal.load('mishell');
      } else if(words[0] === 'close') {
        modal.close(words[1] + '_window');
      } else if(words[0] === 'reload') {
        modal.close(words[1] + '_window');
        modal.load(words[1]);
      } else if(prompt === 'new room') {
        modal.load('createScene')
      } else if(words[0] === 'quit') {
        modal.close('mishell_window');
      }

      mishellPrompt.value = '';
    },
        unmount: function() {
            
        }
      }
      mishell_window.start();
    </script>
  </div>
<?php
}
?>