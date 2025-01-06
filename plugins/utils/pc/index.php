<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div class='window bg-gray-800' style='width: 800px; height: 600px;'>

    <div data-part='handle' class='window_title bg-gray-700 text-gray-100 p-2 rounded-t'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 text-white" aria-label="Close (ESC)" data-close>&times;</button>
      </div>
      <div data-part='title' class='title_bg window_border text-gray-100'>Fake PC</div>
    </div>

    <div class='clearfix'></div>

    <div class='relative window_body bg-gray-900'>
      <!-- Desktop area -->
      <div class='desktop grid grid-cols-4 gap-4 p-4'>
        <div class='icon text-center cursor-pointer' onclick="pc_window.openplugin('main_title_window', 'menus/main_title/index.php', 'Main Titles')">
          <div class='icon-image bg-gray-600 w-16 h-16 mx-auto rounded'></div>
          <span class='text-gray-300 text-sm'>Main Titles</span>
        </div>

        <div class='icon text-center cursor-pointer' onclick="pc_window.openplugin('settings_window', 'menus/settings/index.php', 'Settings')">
          <div class='icon-image bg-gray-600 w-16 h-16 mx-auto rounded'></div>
          <span class='text-gray-300 text-sm'>Settings</span>
        </div>
      </div>
    </div>
    </div>

    <script>
pc_window = {
        start: function() {
          console.log('PC Window Initialized');
        },

        openplugin: function(id, url, name) {
          plugin.load({
            id: id,
            url: url,
            name: name,
            drag: true,
            reload: true
          });
        },

        unmount: function() {
          console.log('PC Window Unmounted');
        }
      }
    </script>
<?php
}
?>
