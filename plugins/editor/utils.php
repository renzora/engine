<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div class='window window_bg fixed top-2 left-1/2 transform -translate-x-1/2 flex flex-col items-start' style='background: #3a3a3a; border: 1px solid #222; border-radius: 5px !important;'>



    <!-- Dropdown Menu -->
    <div class="flex bg-gray-800 text-white text-sm w-full rounded-t-md">
        <!-- Lighting -->
        <div class="relative">
            <button class="px-4 py-2 hover:bg-gray-700 w-full text-left dropdown-trigger">Lighting</button>
            
            <div class="absolute left-0 mt-1 hidden dropdown-content bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-10" style="min-width: 200px;">
                <label class="flex items-center px-4 py-2 hover:bg-gray-700 cursor-pointer">
                    <input type="checkbox" id="snap_checkbox" class="custom-checkbox mr-2">
                    Snap
                </label>
                <label class="flex items-center px-4 py-2 hover:bg-gray-700 cursor-pointer">
                    <input type="checkbox" id="group_objects_checkbox" class="custom-checkbox mr-2">
                    Group
                </label>
            </div>
        </div>

                <!-- Effects -->
        <div class="relative">
            <button class="px-4 py-2 hover:bg-gray-700 w-full text-left dropdown-trigger">Select</button>
            <div class="absolute left-0 mt-1 hidden dropdown-content bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-10" style="min-width: 200px;">
                        <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left" id="move_front_button">
            Move object in Front
        </button>
        <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left" id="move_back_button">
            Move object behind
        </button>
                <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left" id="select_type_button">
                    Select all by Type
                </button>
            </div>
        </div>

        <!-- Effects -->
        <div class="relative">
            <button class="px-4 py-2 hover:bg-gray-700 w-full text-left dropdown-trigger">Effects</button>
            <div class="absolute left-0 mt-1 hidden dropdown-content bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-10" style="min-width: 200px;">
                <label class="flex items-center px-4 py-2 hover:bg-gray-700 cursor-pointer">
                    <input type="checkbox" id="console_toggle_checkbox" class="custom-checkbox mr-2">
                    Console Toggle
                </label>
                <label class="flex items-center px-4 py-2 hover:bg-gray-700 cursor-pointer">
                    <input type="number" id="brush_amount" value="1" class="shadow rounded-sm px-2 py-1 text-black border border-gray-600 mr-2 w-16">
                    Brush Size
                </label>
            </div>
        </div>


        <!-- Tools -->
        <div class="relative">
            <button class="px-4 py-2 hover:bg-gray-700 w-full text-left dropdown-trigger">Tools</button>
            <div class="absolute left-0 mt-1 hidden dropdown-content bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-10" style="min-width: 200px;">
                <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left" id="select_terrain_button">
                    Terrain Editor
                </button>
                <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left" id="select_tileset_button">
                    Tileset Manager
                </button>
            </div>
        </div>

        <!-- Views -->
<div class="relative">
    <button class="px-4 py-2 hover:bg-gray-700 w-full text-left dropdown-trigger">Settings</button>
    <div class="absolute left-0 mt-1 hidden dropdown-content bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-10" style="min-width: 200px;">
        <!-- Main dropdown menu -->
        <div class="relative group">
            <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left">
                Grid
                <svg class="w-4 h-4 ml-auto" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"></path>
                </svg>
            </button>
            <!-- Submenu -->
            <div class="absolute left-full top-0 hidden group-hover:block bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-20" style="min-width: 200px;">
                <label class="flex items-center px-4 py-2 hover:bg-gray-700 cursor-pointer">
                    <input type="checkbox" id="grid_checkbox" class="custom-checkbox mr-2">
                    Toggle Grid
                </label>
            </div>
        </div>

        <!-- Main dropdown menu -->
        <div class="relative group">
            <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left">
                Console Menu
                <svg class="w-4 h-4 ml-auto" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7"></path>
                </svg>
            </button>
            <!-- Submenu -->
            <div class="absolute left-full top-0 hidden group-hover:block bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-20" style="min-width: 200px;">
                <label class="flex items-center px-4 py-2 hover:bg-gray-700 cursor-pointer">
                    <input type="checkbox" id="console_toggle_checkbox" class="custom-checkbox mr-2">
                    Allow menu toggle
                </label>
            </div>
        </div>
    </div>
</div>

                <div class="relative">
            <button class="px-4 py-2 hover:bg-gray-700 w-full text-left dropdown-trigger">Help</button>
            <div class="absolute left-0 mt-1 hidden dropdown-content bg-gray-900 text-white border border-gray-600 rounded shadow-lg z-10" style="min-width: 200px;">
                <button class="flex items-center px-4 py-2 hover:bg-gray-700 w-full text-left" id="editor_help_tutorials">
                Tutorials
                </button>
            </div>
        </div>

    </div>
    </div>

    <style>
        /* Style for custom checkboxes */
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

        .custom-checkbox:checked {
            background-color: #4f618b;
            border-color: #276b49;
        }

        .custom-checkbox:checked::after {
            content: '✓';
            font-size: 12px;
            position: absolute;
            top: -1px;
            left: 3px;
            color: white;
        }
    </style>

<script>
editor_utils_window = {
    modeButtons: {},
    isGridEnabled: true,  // Set to true or false based on your desired default state
    isSnapEnabled: true,  // Set to true or false based on your desired default state
    isGroupObjectsEnabled: false,

    start: function () {
        this.modeButtons = {
            groupObjectsCheckbox: document.getElementById('group_objects_checkbox'),
            gridCheckbox: document.getElementById('grid_checkbox'),
            snapCheckbox: document.getElementById('snap_checkbox'),
            brushAmount: document.getElementById('brush_amount'),
            bringFrontButton: document.getElementById('move_front_button'),
            bringBackButton: document.getElementById('move_back_button'),
            selectTypeButton: document.getElementById('select_type_button'),
            consoleToggleCheckbox: document.getElementById('console_toggle_checkbox') // New Toggle
        };

        this.initializeDropdowns();

        // Set initial states for checkboxes and other options
        this.modeButtons.groupObjectsCheckbox.checked = this.isGroupObjectsEnabled;
        this.modeButtons.gridCheckbox.checked = this.isGridEnabled;
        this.modeButtons.snapCheckbox.checked = this.isSnapEnabled;
        this.modeButtons.consoleToggleCheckbox.checked = console_window.allowToggle; // Set default state

        // Add event listeners for checkboxes
        this.modeButtons.groupObjectsCheckbox.addEventListener('change', () => this.updateGroupObjectsCheckboxState());
        this.modeButtons.gridCheckbox.addEventListener('change', () => this.updateGridCheckboxState());
        this.modeButtons.snapCheckbox.addEventListener('change', () => this.updateSnapCheckboxState());
        this.modeButtons.consoleToggleCheckbox.addEventListener('change', () => this.updateConsoleToggleState()); // New Event Listener

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

        // Add event listener for Select Type button
        this.modeButtons.selectTypeButton.addEventListener('click', () => this.selectAllObjectsOfType());

        console.log("Editor utilities started with the following settings:");
        console.log("Grid Enabled:", this.isGridEnabled);
        console.log("Snap Enabled:", this.isSnapEnabled);
        console.log("Group Objects Enabled:", this.isGroupObjectsEnabled);
        console.log("Console Toggle Allowed:", console_window.allowToggle); // Log the console toggle state
    },

    initializeDropdowns: function () {
        const dropdownTriggers = document.querySelectorAll('.dropdown-trigger');
        const dropdownContents = document.querySelectorAll('.dropdown-content');

        // Attach click event to all dropdown triggers
        dropdownTriggers.forEach(trigger => {
            trigger.addEventListener('click', (event) => {
                event.stopPropagation(); // Prevent bubbling
                const dropdownContent = trigger.nextElementSibling;

                // Check the current visibility state
                const isHidden = dropdownContent.classList.contains('hidden');

                // Hide all other dropdowns
                dropdownContents.forEach(content => content.classList.add('hidden'));

                // Toggle the clicked dropdown's visibility
                if (isHidden) {
                    dropdownContent.classList.remove('hidden');
                } else {
                    dropdownContent.classList.add('hidden');
                }
            });
        });

        // Prevent clicks inside the dropdown menu from closing it
        dropdownContents.forEach(content => {
            content.addEventListener('click', (event) => {
                event.stopPropagation(); // Prevent closing the dropdown
            });
        });

        // Close all dropdowns when clicking outside
        document.addEventListener('click', () => {
            dropdownContents.forEach(content => {
                content.classList.add('hidden');
            });
        });
    },

    updateConsoleToggleState: function () {
        console_window.allowToggle = this.modeButtons.consoleToggleCheckbox.checked;
        console.log(console_window.allowToggle ? "Console toggle enabled" : "Console toggle disabled");
    },

    updateGroupObjectsCheckboxState: function () {
        this.isGroupObjectsEnabled = this.modeButtons.groupObjectsCheckbox.checked;
        console.log(this.isGroupObjectsEnabled ? "Group Objects enabled" : "Group Objects disabled");
        edit_mode_window.renderSelectedTiles(); // Re-render selected tiles
    },

    toggleBrushSizeInput: function (show) {
        const brushSizeInput = document.getElementById('brush_amount').parentElement;
        if (brushSizeInput) {
            brushSizeInput.style.display = show ? 'flex' : 'none';
        }
    },

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

    // Safely remove event listeners for all checkboxes
    if (this.modeButtons.groupObjectsCheckbox) {
        this.modeButtons.groupObjectsCheckbox.removeEventListener('change', this.updateGroupObjectsCheckboxState);
    }
    if (this.modeButtons.gridCheckbox) {
        this.modeButtons.gridCheckbox.removeEventListener('change', this.updateGridCheckboxState);
    }
    if (this.modeButtons.snapCheckbox) {
        this.modeButtons.snapCheckbox.removeEventListener('change', this.updateSnapCheckboxState);
    }
    if (this.modeButtons.consoleToggleCheckbox) {
        this.modeButtons.consoleToggleCheckbox.removeEventListener('change', this.updateConsoleToggleState);
    }

    // Safely remove event listeners for other inputs
    const brushAmountInput = document.getElementById('brush_amount');
    if (brushAmountInput) {
        brushAmountInput.removeEventListener('change', this.updateBrushAmount);
    }

    // Reset all states
    this.isGridEnabled = false;
    this.isSnapEnabled = false;
    this.isGroupObjectsEnabled = false;

    // Reset UI elements
    if (this.modeButtons.groupObjectsCheckbox) {
        this.modeButtons.groupObjectsCheckbox.checked = false;
    }
    if (this.modeButtons.gridCheckbox) {
        this.modeButtons.gridCheckbox.checked = false;
    }
    if (this.modeButtons.snapCheckbox) {
        this.modeButtons.snapCheckbox.checked = false;
    }
    if (this.modeButtons.consoleToggleCheckbox) {
        this.modeButtons.consoleToggleCheckbox.checked = false;
    }
    if (brushAmountInput) {
        brushAmountInput.value = 1;
    }

    console.log("All utilities reset.");
},


    updateGridCheckboxState: function () {
        this.isGridEnabled = this.modeButtons.gridCheckbox.checked;
        console.log(this.isGridEnabled ? "Grid enabled" : "Grid disabled");
        game.render(); // Re-render the game to show/hide the grid
    },

    updateSnapCheckboxState: function () {
        this.isSnapEnabled = this.modeButtons.snapCheckbox.checked;
        console.log(this.isSnapEnabled ? "Snap enabled" : "Snap disabled");
    },

    updateBrushAmount: function () {
        const brushAmountValue = this.modeButtons.brushAmount.value;
        console.log("Brush size updated to:", brushAmountValue);
    },

    selectAllObjectsOfType: function () {
        if (edit_mode_window.selectedObjects.length === 0) {
            console.log("No objects selected.");
            return;
        }

        // Get unique object type IDs
        const selectedTypeIds = new Set(edit_mode_window.selectedObjects.map(obj => obj.id));

        // Get all matching objects
        const roomDataArray = game.roomData.items;
        if (!Array.isArray(roomDataArray)) {
            console.error("roomData.items is not an array.");
            return;
        }

        const objectsOfType = roomDataArray.filter(obj => selectedTypeIds.has(obj.id));
        edit_mode_window.selectedObjects = objectsOfType;

        console.log(`Selected all objects with ids: ${[...selectedTypeIds].join(', ')}`);
        console.log(edit_mode_window.selectedObjects);

        // Optionally switch to move mode
        edit_mode_window.changeMode('move');
    },

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
        }
};
</script>

<?php
}
?>
