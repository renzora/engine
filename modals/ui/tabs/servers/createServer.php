<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='server_create_window' class='window window_bg' style='width: 330px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
    <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Create New Server</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <div>
          <label for="server_name" class="text-white">Server Name:</label>
          <input type="text" id="server_name" class="w-full p-2 mt-1 mb-4 border border-gray-300 rounded" placeholder="Enter server name">
          <button id="create_server_btn" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Create server</button>
        </div>
      </div>
    </div>

    <script>
var server_create_window = {
    start: function() {
        document.getElementById('create_server_btn').addEventListener('click', function() {
            var serverName = document.getElementById('server_name').value.trim();
            if (!serverName) {
                serverName = 'default server';
            }

            ui.ajax({
                outputType: 'json',
                method: 'POST',
                url: 'modals/ui/tabs/servers/ajax/createServer.php',
                data: JSON.stringify({ name: serverName }),
                headers: {
                    'Content-Type': 'application/json'
                },
                success: function(data) {
                    if (data.message === 'success') {
                        modal.close('server_create_window');
                        ui_servers_tab_window.loadScenes(data.server_id); // Load scenes list for the new server
                        ui_servers_tab_window.loadCreateSceneModal(data.server_id); // Open create scene modal
                    } else {
                        alert('Error creating server: ' + data.message + ' (' + data.error + ')');
                    }
                },
                error: function(data) {
                    alert('Error creating server.');
                    console.error('Error creating server:', data);
                }
            });
        });
    },
    unmount: function() {
        var createBtn = document.getElementById('create_server_btn');
        if (createBtn) createBtn.removeEventListener('click', this.createServer);
    }
};
server_create_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
