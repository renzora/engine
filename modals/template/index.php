<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='template_window' class='window bg-yellow-700' style='width: 330px;'>

    <div data-part='handle' class='window_title bg-yellow-600 text-yellow-100 p-2 rounded-t'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 text-white" aria-label="Close (ESC)" data-close>&times;</button>
      </div>
      <div data-part='title' class='title_bg window_border text-yellow-100'>Blank template</div>
    </div>
    
    <div class='clearfix'></div>
    
    <div class='relative'>
      <div class='container text-white p-2'>

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

        <div id="template_window_accordion" class="accordion bg-gray-800 p-2 rounded-md mt-4">
          <div class="accordion-item">
            <div class="accordion-header p-2 text-white cursor-pointer bg-blue-600 rounded-md">
              Accordion Header 1
            </div>
            <div class="accordion-content block overflow-hidden transition-max-height duration-300" style="max-height: 1000px;">
              <div class="p-2 text-gray-300">
                Accordion Content 1
              </div>
            </div>
          </div>

          <div class="accordion-item mt-2">
            <div class="accordion-header p-2 text-white cursor-pointer bg-blue-600 rounded-md">
              Accordion Header 2
            </div>
            <div class="accordion-content hidden overflow-hidden transition-max-height duration-300">
              <div class="p-2 text-gray-300">
                Accordion Content 2
              </div>
            </div>
          </div>

          <div class="accordion-item mt-2">
            <div class="accordion-header p-2 text-white cursor-pointer bg-blue-600 rounded-md">
              Accordion Header 3
            </div>
            <div class="accordion-content hidden overflow-hidden transition-max-height duration-300">
              <div class="p-2 text-gray-300">
                Accordion Content 3
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <style>
.accordion-content {
    max-height: 0;
    transition: max-height 0.3s ease;
}

.accordion-content.block {
    max-height: 1000px; /* Arbitrary large value for smooth transition */
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

          ui.initAccordion('template_window_accordion');

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
          ui.destroyAccordion('template_window_accordion');
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
