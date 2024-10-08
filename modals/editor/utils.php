<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='editor_utils_window' class='window window_bg fixed top-0 left-1/2 transform -translate-x-1/2 flex items-center' style='background: #3a3a3a; padding: 5px 10px; border: 1px solid #222; border-top-left-radius: 0 !important; border-top-right-radius: 0 !important; border-bottom-left-radius: 5px !important; border-bottom-right-radius: 5px !important;'>

    <!-- Grid Checkbox styled as button -->
    <div class="flex items-center mr-4">
        <label for="grid_checkbox" class="mode-button shadow border rounded-sm px-4 py-1 text-white leading-tight focus:outline-none flex items-center justify-between bg-gray-700 border-gray-600 hover:bg-gray-800 cursor-pointer">
            <span class="text-xs text-white mr-2 cursor-pointer">Grid</span>
            <input type="checkbox" id="grid_checkbox" class="custom-checkbox cursor-pointer">
        </label>
    </div>

    <!-- Snap Checkbox styled as button -->
    <div class="flex items-center mr-4">
        <label for="snap_checkbox" class="mode-button shadow border rounded-sm px-4 py-1 text-white leading-tight focus:outline-none flex items-center justify-between bg-gray-700 border-gray-600 hover:bg-gray-800 cursor-pointer">
            <span class="text-xs text-white mr-2 cursor-pointer">Snap</span>
            <input type="checkbox" id="snap_checkbox" class="custom-checkbox cursor-pointer">
        </label>
    </div>

    <!-- Brush Amount Input -->
    <div class="flex items-center mr-4">
        <label for="brush_amount" class="text-xs text-white mr-2">Size</label>
        <input type="number" id="brush_amount" value="1" class="shadow rounded-sm px-2 py-1 text-black leading-tight focus:outline-none border border-gray-600" min="1" style="width: 60px;">
    </div>

    <!-- Move to Front Icon -->
    <div class="flex items-center mr-4 cursor-pointer" id="move_front_button">
        <div class="ui_icon ui_bring_front"></div>
    </div>

    <!-- Move to Back Icon -->
    <div class="flex items-center mr-4 cursor-pointer" id="move_back_button">
        <div class="ui_icon ui_bring_back"></div>
    </div>

    <style>
        /* Hide the default checkbox */
        .custom-checkbox {
            appearance: none;
            background-color: #fff;
            margin: 0;
            font: inherit;
            color: currentColor;
            width: 16px;
            height: 16px;
            border: 1px solid #707070;
            display: inline-block;
            vertical-align: middle;
            border-radius: 2px;
            position: relative;
        }

        /* Checked style */
        .custom-checkbox:checked {
            background-color: #4f618b;
            border-color: #276b49;
        }

        /* Checkbox checked indicator */
        .custom-checkbox:checked::after {
            content: '✓'; /* Corrected Checkmark */
            font-size: 12px;
            position: absolute;
            top: -1px;
            left: 3px;
            color: white;
        }
    </style>

    <script>
    var editor_utils_window = {
        modeButtons: {},
        isGridEnabled: true,  // Set to true or false based on your desired default state
        isSnapEnabled: false,  // Set to true or false based on your desired default state

        start: function () {
    this.modeButtons = {
        brushAmount: document.getElementById('brush_amount'),
        gridCheckbox: document.getElementById('grid_checkbox'),
        snapCheckbox: document.getElementById('snap_checkbox'),
        bringFrontButton: document.getElementById('move_front_button'),
        bringBackButton: document.getElementById('move_back_button')
    };

    // Programmatically set checkbox states based on the isGridEnabled and isSnapEnabled flags
    this.modeButtons.gridCheckbox.checked = this.isGridEnabled;
    this.modeButtons.snapCheckbox.checked = this.isSnapEnabled;

    // Add event listeners for checkbox changes
    this.modeButtons.gridCheckbox.addEventListener('change', () => this.updateGridCheckboxState());
    this.modeButtons.snapCheckbox.addEventListener('change', () => this.updateSnapCheckboxState());

    // Add event listener for brush amount input
    document.getElementById('brush_amount').addEventListener('change', () => this.updateBrushAmount());

    // Hide the brush size input on start
    this.toggleBrushSizeInput(false);

    // Hide the bring front and bring back buttons on start
    this.toggleBringButtons(false);

    // Add event listeners for the Move Front and Move Back buttons
    this.modeButtons.bringFrontButton.addEventListener('click', () => {
        edit_mode_window.pushSelectedObjectsToTop(); // Move objects to the front
    });

    this.modeButtons.bringBackButton.addEventListener('click', () => {
        edit_mode_window.pushSelectedObjectsToBottom(); // Move objects to the back
    });
},

// Function to toggle the visibility of the brush size input
toggleBrushSizeInput: function (show) {
    const brushSizeInput = document.getElementById('brush_amount').parentElement;
    if (brushSizeInput) {
        brushSizeInput.style.display = show ? 'flex' : 'none';
    }
},

// Function to toggle the visibility of the bring front and bring back buttons
toggleBringButtons: function (show) {
    const bringFrontButton = this.modeButtons.bringFrontButton;
    const bringBackButton = this.modeButtons.bringBackButton;
    
    if (bringFrontButton && bringBackButton) {
        bringFrontButton.style.display = show ? 'flex' : 'none';
        bringBackButton.style.display = show ? 'flex' : 'none';
    }
},
        unmount: function () {
            console.log("Utility window unmounted and features reset.");

            // Remove event listeners
            this.modeButtons.gridCheckbox.removeEventListener('change', this.updateGridCheckboxState);
            this.modeButtons.snapCheckbox.removeEventListener('change', this.updateSnapCheckboxState);
            document.getElementById('brush_amount').removeEventListener('change', this.updateBrushAmount);

            // Reset flags and disable grid and snap
            this.isGridEnabled = false;
            this.isSnapEnabled = false;
            this.modeButtons.gridCheckbox.checked = false;
            this.modeButtons.snapCheckbox.checked = false;
            console.log("Grid and snap disabled.");

            // Clear brush and other settings
            this.modeButtons.brushAmount.value = 1;  // Reset brush size to default
        },

        // Function to update grid state when checkbox is manually clicked
        updateGridCheckboxState: function () {
            this.isGridEnabled = this.modeButtons.gridCheckbox.checked;
            console.log(this.isGridEnabled ? "Grid enabled" : "Grid disabled");
            game.render(); // Trigger re-render of the game to show/hide the grid
        },

        // Function to update snap state when checkbox is manually clicked
        updateSnapCheckboxState: function () {
            this.isSnapEnabled = this.modeButtons.snapCheckbox.checked;
            console.log(this.isSnapEnabled ? "Snap enabled" : "Snap disabled");
        },

        // Render the grid when it's enabled
        renderGrid: function () {
            if (!this.isGridEnabled) return;

            // Set grid line style to be dark and subtle
            game.ctx.strokeStyle = 'rgba(0, 0, 0, 0.1)'; // Dark color (black) with opacity 0.1 (subtle but visible)
            game.ctx.lineWidth = 1;

            // Draw vertical lines
            for (let x = 0; x < game.worldWidth; x += 16) {
                game.ctx.beginPath();
                game.ctx.moveTo(x, 0);
                game.ctx.lineTo(x, game.worldHeight);
                game.ctx.stroke();
            }

            // Draw horizontal lines
            for (let y = 0; y < game.worldHeight; y += 16) {
                game.ctx.beginPath();
                game.ctx.moveTo(0, y);
                game.ctx.lineTo(game.worldWidth, y);
                game.ctx.stroke();
            }
        },

        // Update brush amount value
        updateBrushAmount: function () {
            let brushAmountValue = this.modeButtons.brushAmount.value;
            console.log("Brush size updated to:", brushAmountValue);
        }
    };

    // Start the utility window
    editor_utils_window.start();
    </script>

</div>

<?php
}
?>
