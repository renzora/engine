<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='edit_mode_window' class='window window_bg position-fixed top-2 justify-center flex' style='width: 600px; height: 47px; background: #3a445b; border-radius: 0;'>

    <!-- Handle that spans the whole left side -->
    <div data-part='handle' class='window_title rounded-none' style='width: 20px; background-image: radial-gradient(#e5e5e58a 1px, transparent 0) !important; border-radius: 0;'>
    </div>

    <!-- Rest of the content -->
    <div class='relative flex-grow'>
      <div class='container text-light window_body p-2 mx-2'>
        <button type="button" id="items_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Inventory</button>
        <button type="button" id="select_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Select</button>
        <button type="button" id="move_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Move</button>
        <button type="button" id="pickup_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Pick Up</button>
        <button type="button" id="drop_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Drop</button>
        <button type="button" id="navigate_button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;">Pan</button>
        <button type="button" class="mode-button shadow appearance-none border rounded py-2 px-3 text-white leading-tight focus:outline-none focus:shadow-outline" style="background: #276b49; border: 1px rgba(0,0,0,0.5) solid;" onclick="modal.load('editMode/manager.php','editMode_manager_window')">Manager</button>
      </div>
    </div>

    <!-- Close button on the right -->
    <button class="icon close_dark hint--right ml-auto" aria-label="Close (ESC)" data-close></button>
  </div>

<style>
body.move-cursor {
  cursor: move !important;
}
  </style>

  <script>
    var Modes = {
      ITEMS: 'items',
      SELECT: 'select',
      MOVE: 'move',
      PICKUP: 'pickup',
      DROP: 'drop',
      NAVIGATE: 'navigate'
    };

    var modeButtons = {
      items: document.getElementById('items_button'),
      select: document.getElementById('select_button'),
      move: document.getElementById('move_button'),
      pickup: document.getElementById('pickup_button'),
      drop: document.getElementById('drop_button'),
      navigate: document.getElementById('navigate_button')
    };

    var modeChangeHandlers = {};

    var edit_mode_window = {
    start: function() {
        modal.hide('ui_window');
        editor.isEditMode = true;
        editor.currentMode = null;
        this.changeMode(Modes.SELECT);

        Object.keys(modeButtons).forEach(mode => {
            var handler = () => this.changeMode(mode);
            modeChangeHandlers[mode] = handler;
            modeButtons[mode].addEventListener('click', handler);
        });
    },
    unmount: function() {
    modal.show('ui_window');
    game.zoomLevel = 4;
    editor.isEditMode = false;
    editor.currentMode = null;

    // Reset editor data
    editor.selectedTiles = [];
    editor.selectionStart = null;
    editor.selectionEnd = null;
    editor.history = [];
    editor.redoStack = [];
    editor.tempSelectedTiles = [];

    // Clear any other instances of data used by the editor
    // ...

    Object.keys(modeButtons).forEach(mode => {
        var handler = modeChangeHandlers[mode];
        if (handler) {
            modeButtons[mode].removeEventListener('click', handler);
        }
    });
},
    changeMode: function(newMode) {
        // Reset styles for all buttons
        Object.values(modeButtons).forEach(button => {
            button.style.background = '#276b49';
            button.style.color = 'white'; // Reset text color if changed
        });

        // Highlight the active mode button
        if (modeButtons[newMode]) {
            modeButtons[newMode].style.background = 'white';
            modeButtons[newMode].style.color = '#276b49'; // Change text color if needed
        }

        // Handle mode-specific actions
        if (newMode === Modes.ITEMS) {
            modal.load('inventory');
        } else {
            modal.hide('inventory_window');
        }

        // Set the editor's current mode
        editor.currentMode = newMode;
        console.log(`Current mode: ${newMode}`);
    }
};

edit_mode_window.start();
  </script>
<?php
}
?>
