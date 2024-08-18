<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
    $sceneId = $_GET['id'] ?? null;
    $sceneName = $_GET['name'] ?? '';
    $serverId = $_GET['serverId'] ?? null;
?>
  <div data-window='scene_edit_window' class='window window_bg' style='width: 356px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
    <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Edit Scene</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        <div>
          <label for="scene_name" class="text-white">Scene Name:</label>
          <input type="text" id="scene_name" class="w-full w-full light_input p-2 rounded my-2" value="<?php echo htmlspecialchars($sceneName); ?>">
          <button id="save_scene_btn" class="green_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Save</button>
          <button id="delete_scene_btn" class="red_button text-white font-bold py-2 px-3 rounded w-48 mt-2 shadow-md">Delete</button>
        </div>
      </div>
    </div>

    <script>
var scene_edit_window = {
    start: function(sceneId) {
        this.sceneId = sceneId;

        document.getElementById('save_scene_btn').addEventListener('click', this.saveScene.bind(this));
        document.getElementById('delete_scene_btn').addEventListener('click', this.deleteScene.bind(this));
    },
    saveScene: function() {
        var sceneName = document.getElementById('scene_name').value.trim();
        if (!sceneName) {
            alert('Scene name cannot be empty.');
            return;
        }

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/menus/console/tabs/servers/ajax/editScene.php',
            data: JSON.stringify({ id: this.sceneId, name: sceneName }),
            headers: {
                'Content-Type': 'application/json'
            },
            success: function(data) {
                if (data.message === 'success' || data.message === 'No documents were modified') {
                    modal.close('scene_edit_window');
                    ui_servers_tab_window.loadScenes(data.server_id); // Refresh scene list
                } else {
                    alert('Error updating scene: ' + data.message);
                }
            }.bind(this),
            error: function(data) {
                alert('Error updating scene.');
                console.error('Error updating scene:', data);
            }
        });
    },
    deleteScene: function() {
        if (confirm('Are you sure you want to delete this scene?')) {
            ui.ajax({
                outputType: 'json',
                method: 'POST',
                url: 'modals/menus/console/tabs/servers/ajax/deleteScene.php',
                data: JSON.stringify({ id: this.sceneId }),
                headers: {
                    'Content-Type': 'application/json'
                },
                success: function(data) {
                    if (data.message === 'success') {
                        modal.close('scene_edit_window');
                        ui_servers_tab_window.loadScenes(data.server_id); // Refresh scene list
                    } else if (data.message === 'Unauthorized') {
                        alert('You are not authorized to delete this scene.');
                    } else {
                        alert('Error deleting scene: ' + data.message);
                    }
                }.bind(this),
                error: function(data) {
                    alert('Error deleting scene.');
                    console.error('Error deleting scene:', data);
                }
            });
        }
    },
    unmount: function() {
        var saveBtn = document.getElementById('save_scene_btn');
        var deleteBtn = document.getElementById('delete_scene_btn');

        if (saveBtn) saveBtn.removeEventListener('click', this.saveScene.bind(this));
        if (deleteBtn) deleteBtn.removeEventListener('click', this.deleteScene.bind(this));
        console.log('Unmounting scene edit window.');
    }
};
scene_edit_window.start('<?php echo $sceneId; ?>');
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
