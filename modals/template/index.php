<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='template_window' class='window window_bg' style='width: 330px; background: #bba229;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Blank template</div>
    </div>
    
    <div class='clearfix'></div>
    
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        
        <div id="template_window_tabs">
          <div class="flex border-b border-gray-300">
            <button class="tab text-gray-800 p-2" data-tab="tab1" data-menu="template_window_menu">Tab 1</button>
            <button class="tab text-gray-800 p-2" data-tab="tab2" data-menu="template_window_menu2">Tab 2</button>
            <button class="tab text-gray-800 p-2" data-tab="tab3" data-menu="template_window_menu3">Tab 3</button>
          </div>

          <div class="tab-content p-2 hidden" data-tab-content="tab1">
            <p>Content for Tab 1</p>
            <div id="template_window_menu" class="bg-gray-800 p-2 rounded-md">
    <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item1">
        Menu Item 1
        <input type="checkbox" class="switch ml-2" data-control="switch1">
    </div>
    <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item2">
        Menu Item 2
        <input type="range" class="slider ml-2" min="0" max="100" data-control="slider1">
    </div>
    <div data-menu-item="item3" class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md">
        Menu Item 3
        <!-- Custom dropdown will be dynamically inserted here -->
    </div>
    <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md flex items-center" data-menu-item="item4">
    Toggle Switch
    <div id="toggleSwitchContainer" class="ml-auto"></div>
</div>
</div>

          </div>

          <div class="tab-content p-2 hidden" data-tab-content="tab2">
            <p>Content for Tab 2</p>
            <div id="template_window_menu2" class="bg-gray-800 p-2 rounded-md">
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item1">Menu Item 1</div>
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item2">Menu Item 2</div>
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item3">Menu Item 3</div>
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item4">Menu Item 4</div>
            </div>
          </div>

          <div class="tab-content p-2 hidden" data-tab-content="tab3">
            <p>Content for Tab 3</p>
            <div id="template_window_menu3" class="bg-gray-800 p-2 rounded-md">
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item1">Menu Item 1</div>
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item2">Menu Item 2</div>
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item3">Menu Item 3</div>
              <div class="menu-item p-2 text-gray-300 cursor-pointer hover:bg-gray-700 rounded-md" data-menu-item="item4">Menu Item 4</div>
            </div>
          </div>
        </div>

      </div>
    </div>

    <style>
.dropdown.hidden {
    display: none;
}

.menu-item.dropdown-active .dropdown {
    display: block;
}

option.highlighted {
    background-color: #2d3748; /* Darker background for highlighted option */
    color: #fff; /* White text */
}
      </style>

    <script>
      var template_window = {
        start: function() {
          ui.initTabs('template_window_tabs', 'tab1');

          ui.initMenu('template_window_menu', 'item1');
          ui.initMenu('template_window_menu2', 'item1');
          ui.initMenu('template_window_menu3', 'item1');

          ui.createCustomDropdown('template_window_menu', 'customDropdown1', ['Option 1', 'Option 2', 'Option 3'], (selectedOption) => {
    console.log(`CustomDropdown1 selected option: ${selectedOption}`);
});

          const menuId = this.getActiveMenuId('template_window_tabs');
          if (menuId) {
            ui.setActiveMenu(menuId);
          }

          this.toggleSwitch1 = ui.createToggleSwitch('toggleSwitchContainer', 'toggleSwitch1', false, (state) => {
    console.log(`ToggleSwitch1 new state: ${state}`);
});


          this.l1Button = gamepad.throttle(this.l1Button.bind(this), 150);
        this.r1Button = gamepad.throttle(this.r1Button.bind(this), 150);
        this.upButton = gamepad.throttle(this.upButton.bind(this), 150);
        this.downButton = gamepad.throttle(this.downButton.bind(this), 150);
        this.aButton = gamepad.throttle(this.aButton.bind(this), 300);
        this.bButton = gamepad.throttle(this.bButton.bind(this), 150);
        this.leftButton = gamepad.throttle(this.leftButton.bind(this), 100);
        this.rightButton = gamepad.throttle(this.rightButton.bind(this), 100);
        },

        unmount: function() {
          ui.destroyTabs('template_window_tabs');
        },

        aButton: function() {
        if (ui.activeDropdown) {
            ui.confirmDropdownSelection(); // Confirm selection if dropdown is active
        } else {
            const activeMenuId = ui.activeMenuId;
            if (activeMenuId) {
                ui.interactWithHighlightedItem(activeMenuId);
            }
        }
    },

    bButton: function() {
        if (ui.activeDropdown) {
            ui.cancelDropdownSelection(); // Cancel selection if dropdown is active
        } else {
            modal.close('template_window');
        }
    },

    leftButton: function(e) {
        const activeMenuId = ui.activeMenuId;
        if (activeMenuId) {
            ui.adjustSlider(activeMenuId, -1);
        }
    },

    rightButton: function(e) {
        const activeMenuId = ui.activeMenuId;
        if (activeMenuId) {
            ui.adjustSlider(activeMenuId, 1);
        }
    },

    upButton: function() {
    if (ui.dropdownOpen) {
        // If the dropdown is open, navigate within the dropdown options
        ui.highlightDropdownOption(-1); // Move up within dropdown options
    } else {
        // Otherwise, navigate to the previous menu item
        const activeMenuId = ui.activeMenuId;
        if (activeMenuId) {
            ui.highlightMenuItem(activeMenuId, -1); // Navigate up in menu
        }
    }
},

downButton: function() {
    if (ui.dropdownOpen) {
        // If the dropdown is open, navigate within the dropdown options
        ui.highlightDropdownOption(1); // Move down within dropdown options
    } else {
        // Otherwise, navigate to the next menu item
        const activeMenuId = ui.activeMenuId;
        if (activeMenuId) {
            ui.highlightMenuItem(activeMenuId, 1); // Navigate down in menu
        }
    }
},
        buttonStart: function(e) {},

        leftAxis: function(e) {},

        r2Button: function(e) {},

        l1Button: function() {
          console.log("left bumper");
          ui.switchTab('template_window_tabs', -1);

          const menuId = this.getActiveMenuId('template_window_tabs');
          if (menuId) {
            ui.setActiveMenu(menuId);
          }
        },

        r1Button: function() {
          console.log("right bumper");
          ui.switchTab('template_window_tabs', 1);

          const menuId = this.getActiveMenuId('template_window_tabs');
          if (menuId) {
            ui.setActiveMenu(menuId);
          }
        },

        getActiveMenuId: function(containerId) {
          const activeTabButton = document.querySelector(`#${containerId} .tab.active`);
          if (activeTabButton) {
            return activeTabButton.getAttribute('data-menu');
          }
          return null;
        }
      }

      template_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
