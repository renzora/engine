<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div id="ui_window_chat">
  <!-- Tabs: Vertical Alignment on the Left with Background -->
  <div id="tabs" class="fixed top-0 left-0 h-full bg-[#1f2e46] flex flex-col w-[50px] space-y-2 py-4 z-50 transition-transform duration-300 ease-in-out">

    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="servers" aria-label="Servers">
      <div class="icon globe"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="friends" aria-label="Friends">
      <div class="icon friends"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="inventory" aria-label="Inventory">
      <div class="icon sword"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor" aria-label="Editor">
      <div class="icon editor"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="chat" aria-label="Chat">
      <div class="icon chat"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="gift" aria-label="Gift">
      <div class="icon gift"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="avatar" aria-label="Change Avatar">
      <div class="icon avatar"></div>
    </button>

    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="avatar" aria-label="Add Menu">
      <div class="icon plus"></div>
    </button>
    
    <div class="flex-1"></div>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="avatar" aria-label="Mod">
      <div class="icon mod"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="avatar" aria-label="Admin">
      <div class="icon admin"></div>
    </button>
    <button class="tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="settings" aria-label="Settings">
      <div class="icon settings"></div>
    </button>
  </div>

  <!-- Console Window -->
  <div id='console_window' class='window fixed top-0 left-0 h-full w-[calc(100%-60px)] bg-[#152032] transform -translate-x-full transition-transform duration-300 ease-in-out z-40' style="border-radius: 0;">
    <div class='relative flex h-full ml-[50px]'>
      <div class="flex h-full w-full bg-[#151a23]">

        <!-- Scrollable Content -->
        <div class='flex-1 overflow-y-auto'>
          <div id="chat_box" class='h-full p-2'>
            <div class='chat-container text-sm subpixel-antialiased tracking-tight'>
              <div id="tab_content_container" class="w-full bg-opacity-95 text-white">
                <div class="tab-content" data-tab-content="servers"></div>
                <div class="tab-content hidden" data-tab-content="friends"></div>
                <div class="tab-content hidden" data-tab-content="editor"></div>
                <div class="tab-content hidden" data-tab-content="chat"></div>
                <div class="tab-content hidden" data-tab-content="gift"></div>
                <div class="tab-content hidden" data-tab-content="avatar"></div>
                <div class="tab-content hidden" data-tab-content="inventory"></div>
                <div class="tab-content hidden" data-tab-content="settings"></div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <script>
  var console_window = {
    start: function() {
      ui.initTabs('ui_window_chat', 'servers');
      const tabs = document.querySelectorAll('.tab');
      const tabContents = document.querySelectorAll('.tab-content');
      const consoleWindow = document.getElementById('console_window'); // Select the console window

      let activeTabContentId = 'servers'; // Track the currently active tab content

      tabs.forEach(tab => {
        tab.addEventListener('click', function() {
          const target = this.getAttribute('data-tab');

          // Check if the menu is closed and open it if necessary
          if (consoleWindow.classList.contains('-translate-x-full')) {
            consoleWindow.classList.remove('-translate-x-full');
            consoleWindow.classList.add('translate-x-0');
          }

          // Reset all tabs and show the current tab as active
          tabs.forEach(t => t.classList.remove('active', 'text-white'));
          this.classList.add('active', 'text-white');

          // Hide all tab contents and show the targeted content
          tabContents.forEach(tc => tc.classList.add('hidden'));
          const contentDiv = document.querySelector(`.tab-content[data-tab-content="${target}"]`);

          if (contentDiv) {
            // Unmount the previously active tab content
            ui.unmount('ui_' + activeTabContentId + '_tab_window');

            // Clear the content of the previously active tab
            const previousContentDiv = document.querySelector(`.tab-content[data-tab-content="${activeTabContentId}"]`);
            if (previousContentDiv) {
              previousContentDiv.innerHTML = '';
            }

            // Set the new active tab content id
            activeTabContentId = target;

            console.log(`modals/console/tabs/${target}/index.php`);

            // Load content into the current tab
            contentDiv.classList.remove('hidden');
            ui.ajax({
              method: 'POST',
              url: `modals/console/tabs/${target}/index.php`,
              success: function(data) {
                ui.html(contentDiv, data, 'replace'); // Use 'replace' to ensure content loads correctly
                if (target === 'editor') {
                  ui_editor_tab_window.start(); // Initialize any special content if needed
                }
              },
              error: function(err) {
                console.error("Failed to load content for tab:", target, err);
                contentDiv.innerHTML = `<div class="error">Failed to load content. Please try again later.</div>`;
              }
            });
          }
        });
      });

      // Trigger click on the servers tab to load its content by default
      document.querySelector('.tab[data-tab="servers"]').click();
    },

    LeftTrigger: function() {
      console.log("left trigger called from console_window");
    },

    B: function() {
      modal.close('console_window');
    },

    unmount: function() {
      // Cleanup HUD elements if necessary
    },
    toggleConsoleWindow: function() {
        const consoleWindow = document.getElementById('console_window');
        const tabs = document.querySelectorAll('.tab'); // Select all tab elements

        // Check if the console window is open
        if (!consoleWindow.classList.contains('-translate-x-full')) {
            // If it's open, close it by adding the class to hide it
            consoleWindow.classList.add('-translate-x-full');
            consoleWindow.classList.remove('translate-x-0');

            // Remove the active class from all tabs
            tabs.forEach(tab => tab.classList.remove('active', 'text-white'));
        }
    }
  };
  console_window.start();
</script>


    <div class='resize-handle'></div>
  </div>
    </div>
<?php
}
?>
