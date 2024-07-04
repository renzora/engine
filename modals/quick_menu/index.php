<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='quick_menu_window' class='window window_bg fixed bottom-2 left-1' style='width: 383px; height: 540px; background: #3d6a91; overflow: hidden;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#363657 1px, transparent 0) !important;'>
      <div class='float-right'>
      <button class="icon minimize_dark hint--left" aria-label="Minimise" data-minimize></button>
        <button class="icon close_dark mr-1 mt-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #3d6a91; color: #ede8d6;'>Console</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
    <div id="ui_window_chat" class="w-full bg-[#151a23] bg-opacity-90 border border-black rounded-lg mt-1">
              <div id="tabs" class="flex flex-wrap m-2 w-full">
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top active" data-tab="servers" aria-label="Servers">
                  <div class="icon globe"></div>
                </button>
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top" data-tab="friends" aria-label="Friends">
                  <div class="icon friends"></div>
                </button>
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top" data-tab="editor" aria-label="Editor">
                  <div class="icon editor"></div>
                </button>
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top" data-tab="chat" aria-label="Chat">
                  <div class="icon chat"></div>
                </button>
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top" data-tab="gift" aria-label="Gift">
                  <div class="icon gift"></div>
                </button>
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top" data-tab="avatar" aria-label="Avatar">
                  <div class="icon avatar"></div>
                </button>
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top" data-tab="survival" aria-label="Survival">
                  <div class="icon sword"></div>
                </button>
                <button class="tab px-2 py-1 text-gray-600 rounded hint--top" data-tab="settings" aria-label="Settings">
                  <div class="icon settings"></div>
                </button>
              </div>
      <div class='container text-light window_body' style='height: 450px;'>
        
        <!-- Tab Menu -->
        <div id="chat_box" class=''>
          <div class='chat-container text-sm subpixel-antialiased tracking-tight'>
        
              <div id="tab_content_container" class="w-full p-2 overflow-x-hidden overflow-y-auto bg-opacity-95 text-white" style="height: calc(100% - 50px);">
                <div class="tab-content" data-tab-content="servers"></div>
                <div class="tab-content hidden" data-tab-content="friends"></div>
                <div class="tab-content hidden" data-tab-content="editor"></div>
                <div class="tab-content hidden" data-tab-content="chat"></div>
                <div class="tab-content hidden" data-tab-content="gift"></div>
                <div class="tab-content hidden" data-tab-content="avatar"></div>
                <div class="tab-content hidden" data-tab-content="survival"></div>
                <div class="tab-content hidden" data-tab-content="settings"></div>
              </div>
            </div>
          </div>
        </div>

      </div>
    </div>

    <script>
      var quick_menu_window = {
        start: function() {
          ui.initTabs('ui_window_chat', 'servers');
          const chatBox = document.getElementById('chat_box');
          const tabs = document.querySelectorAll('.tab');
          const tabContents = document.querySelectorAll('.tab-content');

          let activeTabContentId = 'servers'; // Track the currently active tab content

          tabs.forEach(tab => {
            tab.addEventListener('click', function() {
              const target = this.getAttribute('data-tab');

              tabs.forEach(t => t.classList.remove('active'));
              this.classList.add('active');

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

                contentDiv.classList.remove('hidden');
                ui.ajax({
                  method: 'POST',
                  url: `modals/quick_menu/tabs/${target}/index.php`,
                  success: function(data) {
                    ui.html(contentDiv, data, 'append');
                    if(target === 'editor') {
                      ui_editor_tab_window.start();
                    }
                  }
                });
              }
            });
          });

          // Trigger click on the servers tab to load its content by default
          document.querySelector('.tab[data-tab="servers"]').click();

          // Other initialization code...
        },

        unmount: function() {
          // Cleanup HUD elements if necessary
        }
      };

      quick_menu_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
