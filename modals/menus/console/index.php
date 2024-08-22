<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window="console_window" id='console_window' class='window fixed top-0 left-0 h-screen w-screen bg-[#152032] transition-transform duration-300 ease-in-out -translate-x-full' style="border-radius: 0;" data-drag="false">

  <div id="tabs" class="console_window_tab_buttons fixed top-0 left-0 h-full bg-[#1c3660] flex flex-col w-[48px] space-y-2 py-4 transition-transform duration-300 ease-in-out border-r-2 border-r-[#151a23] ml-[400px]" style="margin-top: -1px;"></div>

  <div class='relative flex-1 window_body' style="max-height: 99%;">
    <div class="flex w-full bg-[#152032] h-full">
      <div class='flex-1'>
        <div id="console_window_content" class="text-white overflow-y-auto p-2 h-full"></div>
      </div>
    </div>
  </div>

  <script>
 var console_window = {
    isOpen: false,
    currentTabIndex: 0,
    eventListeners: [],

    start: function() {
        this.load_tab_buttons();
        this.toggleConsoleWindow(false);
        this.bindGamepadButtons();
        modal.front('console_window');
    },

    bindGamepadButtons: function() {
        this.bButton = gamepad.throttle(this.bButton.bind(this), 200);
        this.upButton = gamepad.throttle(this.upButton.bind(this), 200);
        this.downButton = gamepad.throttle(this.downButton.bind(this), 200);
    },

    bButton: function() {
        console.log("b button pressed from console_window");
        this.toggleConsoleWindow();
    },

    upButton: function() {
        console.log("up button pressed from console_window");
        this.navigateTabs('up');
    },

    downButton: function() {
        console.log("down button pressed from console_window");
        this.navigateTabs('down');
    },

    load_tab_buttons: function(id) {
        console.log(id);
        ui.ajax({
          method: 'GET',
          data: 'mode='+id,
          url: `modals/menus/console/mode.php`,
          success: function (data) {
            console.log(data);
            ui.html('.console_window_tab_buttons', data, 'replace');
            console_window.setupTabListeners();
          },
          error: function (err) {
            console.error("Failed to load content for tab:", target, err);
            contentDiv.innerHTML = `<div class="error">Failed to load content. Please try again later.</div>`;
          }
        });
      },

    toggleConsoleWindow: function(toggle = true, tabName = null) {
        const consoleElement = document.getElementById('console_window');
        const tabsElement = document.getElementById('tabs');

        if (toggle) this.isOpen = !this.isOpen;

        if (this.isOpen) {
            this.showConsoleWindow(consoleElement, tabsElement);
            this.activateCurrentTab();
        } else {
            this.hideConsoleWindow(consoleElement, tabsElement);
            modal.front('ui_inventory_window');
        }
    },

    showConsoleWindow: function(consoleElement, tabsElement) {
        consoleElement.classList.remove('-translate-x-full');
        consoleElement.classList.add('translate-x-0');
        consoleElement.style.marginLeft = '46px';
        tabsElement.style.marginLeft = '-48px';
    },

    hideConsoleWindow: function(consoleElement, tabsElement) {
        consoleElement.classList.remove('translate-x-0');
        consoleElement.classList.add('-translate-x-full');
        consoleElement.style.marginLeft = '0px';
        tabsElement.style.marginLeft = '407px';
        this.clearActiveTabs();
    },

    activateCurrentTab: function() {
        const tabs = document.querySelectorAll('.tab');
        tabs.forEach(t => t.classList.remove('bg-[#2b3b55]', 'text-white'));
        tabs[this.currentTabIndex].classList.add('bg-[#2b3b55]', 'text-white');
        this.loadTabContent(tabs[this.currentTabIndex].getAttribute('data-tab'));
    },

    setupTabListeners: function() {
        const tabs = document.querySelectorAll('.tab');
        tabs.forEach((tab, index) => {
            const listener = () => this.handleTabClick(tab, index);
            tab.addEventListener('click', listener);
            this.eventListeners.push({ element: tab, event: 'click', handler: listener });
        });
    },

    handleTabClick: function(tab, index) {
        console.log(`Tab clicked: ${tab.getAttribute('data-tab')}`);
        this.currentTabIndex = index;
        this.clearActiveTabs();
        tab.classList.add('bg-[#2b3b55]', 'text-white');

        if (!this.isOpen) {
            this.toggleConsoleWindow();
        } else {
            this.loadTabContent(tab.getAttribute('data-tab'));
        }
    },

    loadTabContent: function(target) {
        this.unmountCurrentTab();
        modal.front('console_window');
        const contentDiv = document.getElementById('console_window_content');
        if (contentDiv) {
            contentDiv.innerHTML = ''; // Clear previous content before loading new content
            ui.ajax({
                method: 'POST',
                url: `modals/menus/console/tabs/${target}/index.php`,
                success: function(data) {
                    ui.html(contentDiv, data, 'replace');
                    console.log(target, "loaded successfully");
                },
                error: function(err) {
                    console.error("Failed to load content for tab:", target, err);
                    contentDiv.innerHTML = `<div class="error">Failed to load content. Please try again later.</div>`;
                }
            });
        }
    },

    unmountCurrentTab: function() {
        ui.unmount('ui_console_tab_window');
        console.log("unmounting ui_console_tab_window");
        const contentDiv = document.getElementById('console_window_content');
        if (contentDiv) {
            contentDiv.innerHTML = ''; // Clear content on unmount
        }
    },

    clearActiveTabs: function() {
        const tabs = document.querySelectorAll('.tab');
        tabs.forEach(t => t.classList.remove('bg-[#2b3b55]', 'text-white'));
    },

    navigateTabs: function(direction) {
        const tabs = document.querySelectorAll('.tab');
        let newIndex = this.currentTabIndex;

        if (direction === 'up') {
            newIndex = (this.currentTabIndex - 1 + tabs.length) % tabs.length;
        } else if (direction === 'down') {
            newIndex = (this.currentTabIndex + 1) % tabs.length;
        }

        if (this.isOpen) {
            this.clearActiveTabs();
            tabs[newIndex].classList.add('bg-[#2b3b55]', 'text-white');
            this.unmountCurrentTab();
            this.loadTabContent(tabs[newIndex].getAttribute('data-tab'));
            this.currentTabIndex = newIndex;
        }
    },

    isMenuActive: function() {
        return this.isOpen;
    },

    unmount: function() {
        this.eventListeners.forEach(({ element, event, handler }) => {
            element.removeEventListener(event, handler);
        });
        this.eventListeners = []; // Clear the event listeners array
        this.unmountCurrentTab(); // Clear the current tab's content
        console.log("All event listeners have been removed.");
    }
};

console_window.start();
  </script>
</div>

<?php
}
?>
