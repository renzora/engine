<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
    $serverId = $_GET['id'] ?? null;
    $serverName = $_GET['name'] ?? '';
?>
  <div data-window='server_edit_window' class='window window_bg' style='width: 356px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
    <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Edit Server</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <div>
          <label for="server_name" class="text-white">Server Name:</label>
          <input type="text" id="server_name" class="w-full light_input p-2 rounded my-2" value="<?php echo htmlspecialchars($serverName); ?>">
          <button id="save_server_btn" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Save</button>
          <button id="delete_server_btn" class="red_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Delete</button>
        </div>
      </div>
    </div>

    <script>
var server_edit_window = {
    start: function(serverId) {
        this.serverId = serverId;

        document.getElementById('save_server_btn').addEventListener('click', this.saveServer.bind(this));
        document.getElementById('delete_server_btn').addEventListener('click', this.deleteServer.bind(this));
    },
    saveServer: function() {
        var serverName = document.getElementById('server_name').value.trim();
        if (!serverName) {
            alert('Server name cannot be empty.');
            return;
        }

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/quick_menu/tabs/servers/ajax/editServer.php',
            data: JSON.stringify({ id: this.serverId, name: serverName }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                if (data.message === 'success' || data.error === 'No documents were modified.') {
                    modal.close('server_edit_window');
                    ui_servers_tab_window.loadServers(); // Refresh server list
                } else {
                    alert('Error updating server: ' + data.message + ' (' + data.error + ')');
                }
            },
            error: function(data) {
                alert('Error updating server.');
                console.error('Error updating server:', data);
            }
        });
    },
    deleteServer: function() {
        if (confirm('Are you sure you want to delete this server? All associated scenes will also be deleted.')) {
            ui.ajax({
                outputType: 'json',
                method: 'POST',
                url: 'modals/quick_menu/tabs/servers/ajax/deleteServer.php',
                data: JSON.stringify({ id: this.serverId }),
                headers: {
                    'Content-Type': 'application/json'
                },
                success: function(data) {
                    if (data.message === 'success') {
                        modal.close('server_edit_window');
                        ui_servers_tab_window.loadServers(); // Refresh server list
                    } else if (data.message === 'Unauthorized') {
                        alert('You are not authorized to delete this server.');
                    } else {
                        alert('Error deleting server: ' + data.message + ' (' + data.error + ')');
                    }
                },
                error: function(data) {
                    alert('Error deleting server.');
                    console.error('Error deleting server:', data);
                }
            });
        }
    },
    unmount: function() {
        var saveBtn = document.getElementById('save_server_btn');
        var deleteBtn = document.getElementById('delete_server_btn');

        if (saveBtn) saveBtn.removeEventListener('click', this.saveServer.bind(this));
        if (deleteBtn) deleteBtn.removeEventListener('click', this.deleteServer.bind(this));
    }
};
server_edit_window.start('<?php echo $serverId; ?>');
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
