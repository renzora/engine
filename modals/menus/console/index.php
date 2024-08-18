<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div data-window="console_window" id='console_window' class='window fixed top-0 left-0 h-screen w-screen bg-[#152032] transition-transform duration-300 ease-in-out -translate-x-full' style="border-radius: 0;" data-drag="false">

  <div id="tabs" class="fixed top-0 left-0 h-full bg-[#1f2e46] flex flex-col w-[48px] space-y-2 py-4 transition-transform duration-300 ease-in-out border-r-2 border-r-[#151a23] ml-[400px]" style="margin-top: -1px;">
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="servers" aria-label="Online Servers">
      <div class="icon globe"></div>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="story" aria-label="Story Mode">
      <div class="icon sword"></div>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="friends" aria-label="Friends">
      <div class="icon friends"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">1</span>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="inventory" aria-label="Inventory">
      <div class="icon inventory"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">48</span>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor" aria-label="Edit Mode">
      <div class="icon editor"></div>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="chat" aria-label="Chat">
      <div class="icon chat"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">85</span>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="gift" aria-label="Market & Auction">
      <div class="icon gift"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">34</span>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="avatar" aria-label="Change Avatar">
      <div class="icon avatar"></div>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="add" aria-label="Add Tab">
      <div class="icon plus"></div>
    </button>
    <div class="flex-1"></div>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="warroom" aria-label="War Room">
      <div class="icon mod"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">16</span>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="help" aria-label="Help & FAQ">
      <div class="icon admin"></div>
    </button>
    <button class="tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="settings" aria-label="Settings & Controls">
      <div class="icon settings"></div>
    </button>
  </div>

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
      start: function () {
        console_window.toggleConsoleWindow(false);
        console_window.setupTabListeners();
        this.bButton = gamepad.throttle(this.bButton.bind(this), 200);
        this.upButton = gamepad.throttle(this.upButton.bind(this), 200);
        this.downButton = gamepad.throttle(this.downButton.bind(this), 200);
        modal.front('console_window');
      },
      LeftTrigger: function () {
        console.log("left trigger called from console_window");
      },
      bButton: function () {
        console.log("b button pressed from console_window");
        console_window.toggleConsoleWindow();
      },
      upButton: function() {
        console.log("up button pressed from console_window");
        console_window.navigateTabs('up');
      },
      downButton: function() {
        console.log("down button pressed from console_window");
        console_window.navigateTabs('down');
      },
      aButton: function() {
        
      },
      unmount: function () {
        ui.unmount('ui_console_tab_window');
      },
      toggleConsoleWindow: function (toggle = true, tabName = null) {
        const consoleElement = document.getElementById('console_window');
        const tabsElement = document.getElementById('tabs');

        if (toggle) {
          console_window.isOpen = !console_window.isOpen;
        }

        if (console_window.isOpen) {
          consoleElement.classList.remove('-translate-x-full');
          consoleElement.classList.add('translate-x-0');
          consoleElement.style.marginLeft = '46px';
          tabsElement.style.marginLeft = '-48px';
          const tabs = document.querySelectorAll('.tab');
          tabs.forEach(t => t.classList.remove('bg-[#2b3b55]', 'text-white'));
          tabs[console_window.currentTabIndex].classList.add('bg-[#2b3b55]', 'text-white');
          const currentTab = tabs[console_window.currentTabIndex].getAttribute('data-tab');
          console_window.loadTabContent(currentTab);

        } else {
          consoleElement.classList.remove('translate-x-0');
          consoleElement.classList.add('-translate-x-full');
          consoleElement.style.marginLeft = '0px';
          tabsElement.style.marginLeft = '407px';
          const tabs = document.querySelectorAll('.tab');
          tabs.forEach(t => t.classList.remove('bg-[#2b3b55]', 'text-white'));
          modal.front('ui_inventory_window');
        }
      },
      setupTabListeners: function () {
        const tabs = document.querySelectorAll('.tab');

        tabs.forEach((tab, index) => {
          tab.addEventListener('click', function () {
            console.log(`Tab clicked: ${tab.getAttribute('data-tab')}`);
            console_window.currentTabIndex = index;
            const target = tab.getAttribute('data-tab');

            if (console_window.isOpen) {
              console_window.loadTabContent(target);
              tabs.forEach(t => t.classList.remove('bg-[#2b3b55]', 'text-white'));
              tab.classList.add('bg-[#2b3b55]', 'text-white');
            }

            if (!console_window.isOpen) {
              console_window.toggleConsoleWindow();
            }
          });
        });

        if (!console_window.isOpen && tabs.length > 0) {
          tabs.forEach(t => t.classList.remove('bg-[#2b3b55]', 'text-white'));
        }
      },
      loadTabContent: function (target) {
        ui.unmount('ui_console_tab_window');
        modal.front('console_window');
        const contentDiv = document.getElementById('console_window_content');
        ui.ajax({
          method: 'POST',
          url: `modals/menus/console/tabs/${target}/index.php`,
          success: function (data) {
            ui.html(contentDiv, data, 'replace');
            console.log(target, "loaded ehe");
          },
          error: function (err) {
            console.error("Failed to load content for tab:", target, err);
            contentDiv.innerHTML = `<div class="error">Failed to load content. Please try again later.</div>`;
          }
        });
      },
      navigateTabs: function (direction) {
        const tabs = document.querySelectorAll('.tab');
        let newIndex = console_window.currentTabIndex;

        if (direction === 'up') {
          newIndex = (console_window.currentTabIndex - 1 + tabs.length) % tabs.length;
        } else if (direction === 'down') {
          newIndex = (console_window.currentTabIndex + 1) % tabs.length;
        }

        if (console_window.isOpen) {
          tabs[console_window.currentTabIndex].classList.remove('bg-[#2b3b55]', 'text-white');
          tabs[newIndex].classList.add('bg-[#2b3b55]', 'text-white');

          // Load the new tab's content
          const target = tabs[newIndex].getAttribute('data-tab');
          console_window.loadTabContent(target);

          // Update the current tab index
          console_window.currentTabIndex = newIndex;
        }
      },
      isMenuActive: function () {
        return console_window.isOpen;
      }
    };

    console_window.start();
  </script>
</div>

<?php
}
?>
