<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='joypad_window' class='window bg-yellow-700' style='width: 330px;'>

    <div data-part='handle' class='window_title bg-yellow-600 text-yellow-100 p-2 rounded-t'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 text-white" aria-label="Close (ESC)" data-close>&times;</button>
      </div>
      <div data-part='title' class='title_bg window_border text-yellow-100'>Template</div>
    </div>
    
    <div class='clearfix'></div>
    
    <div class='relative'>
      <div class='container text-white p-2'>
        <p>Basic content goes here</p>
      </div>
    </div>

    <script>
      var joypad_window = {
        start: function() {
          // Basic initialization code
        },

        unmount: function() {
          // Clean up code
        }
      }

      joypad_window.start();
    </script>

  </div>
<?php
}
?>
