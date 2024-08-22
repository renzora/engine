<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='editor_window' class='window window_bg position-fixed top-2 right-2 rounded-sm' style='width: 57px;background: #3a445b;'>

<!-- Handle that spans the whole left side -->
<div data-part='handle' class='window_title rounded-none w-full mb-1' style='height: 15px; background-image: radial-gradient(#e5e5e58a 1px, transparent 0) !important; border-radius: 0;'>
</div>

<!-- Rest of the content -->
<div class='relative flex-grow'>
  <div class='container text-light window_body px-1 py-1'>
    <button type="button" id="select_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_select"></div>
    </button>

    <button type="button" id="drop_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_pencil"></div>
    </button>

    <button type="button" id="brush_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #276b4f618b49; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_brush"></div>
    </button>

    <button type="button" id="move_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_scissors"></div>
    </button>

    <button type="button" id="pickup_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_magnify"></div>
    </button>

    <button type="button" id="navigate_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_delete"></div>
    </button>

    <button type="button" id="undo_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_undo"></div>
    </button>

    <button type="button" id="redo_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline mb-1" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_redo"></div>
    </button>
    <button type="button" id="save_button" class="mode-button shadow appearance-none border rounded py-1 px-2 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #4f618b; border: 1px rgba(0,0,0,0.5) solid;">
        <div class="ui_icon ui_save"></div>
    </button>
  </div>
</div>

  <style>
    body.move-cursor {
      cursor: move !important;
    }
  </style>

  <script>
var edit_mode_window = {
    modeButtons: {},

    start: function () {
        this.modeButtons = {
            brush: document.getElementById('brush_button'),
            select: document.getElementById('select_button'),
            move: document.getElementById('move_button'),
            pickup: document.getElementById('pickup_button'),
            drop: document.getElementById('drop_button'),
            navigate: document.getElementById('navigate_button')
        };

        //game.isEditorActive = true;
        this.changeMode('select');

        Object.keys(this.modeButtons).forEach(mode => {
            var handler = () => this.changeMode(mode);
            this.modeButtons[mode].addEventListener('click', handler.bind(this));
        });

        game.displaySprite = false;
    },

    unmount: function() {
        modal.showAll();
        game.isEditorActive = false;
        game.pathfinding = true;
    },

    changeMode: function (newMode) {
        game.editorMode = newMode;
        console.log(`Editor mode changed to: ${newMode}`);
        Object.values(this.modeButtons).forEach(button => {
            button.style.background = '#4f618b';
            button.style.color = 'white';
        });

        if (this.modeButtons[newMode]) {
            this.modeButtons[newMode].style.background = 'white';
            this.modeButtons[newMode].style.color = '#276b49';
        }
    },
    
    saveRoomData: function () {
        const data = {
            sceneid: game.sceneid,
            roomData: game.roomData
        };
        const dataToSend = JSON.stringify(data);
        console.log('Data being sent to server:', dataToSend);

        ui.ajax({
            outputType: 'json',
            method: 'POST',
            url: 'modals/inventory/ajax/save_map.php',
            data: dataToSend,
            headers: {
                'Content-Type': 'application/json'
            },
            success: function (data) {
                console.log('Room data saved successfully:', data);
            },
            error: function (data) {
                console.error('Error saving room data:', data);
            }
        });
    }
};

edit_mode_window.start();
  </script>

</div>
<?php
}
?>