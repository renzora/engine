<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div data-window='ui_window' data-close="false">

<div class='fixed bottom-0 right-2 z-10 text-sm mb-1 flex space-x-4 tracking-tight'>
  <span class="text-white rounded-md">Renzora v0.0.7</span>
  <span class="text-white rounded-md" id="gameFps"></span>
  <span id="game_time" class="text-white rounded-md">00:00</span>
</div>

<!-- Top Right: Health, Energy, Quick Items, Avatar -->
<div class='fixed top-0 left-1/2 mt-2 transform -translate-x-1/2 z-10 flex space-x-2 tracking-tight'>

  <div id="avatar" class="flex items-center justify-center w-20 h-18 bg-black bg-opacity-80 rounded-md shadow-2xl hover:shadow-2xl transition-shadow duration-300">
    <div class="items_icon items_sword scale-[3]"></div>
  </div>

  <div class="flex flex-col space-y-2 w-98">
    <div class="flex items-center space-x-2 w-full">
      <div class="relative w-1/2 bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px] flex items-center">
        <div class="mx-1">
          <div class="items_icon items_health scale-[1.2]"></div>
        </div>
        <div id="health" class="rounded bg-gradient-to-r from-lime-500 to-green-600 h-full transition-width duration-500 flex-grow"></div>
        <div class="absolute inset-0 flex items-center pl-8 text-white text-sm">0%</div>
      </div>

      <div class="relative w-1/2 bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px] flex items-center">
        <div class="mx-1">
          <div class="items_icon items_energy scale-[1.2]"></div>
        </div>
        <div id="energy" class="rounded bg-gradient-to-r from-cyan-400 to-blue-600 h-full transition-width duration-500 flex-grow"></div>
        <div class="absolute inset-0 flex items-center pl-8 text-white text-sm">0%</div>
      </div>
    </div>

    <!-- Quick Items -->
    <div class="flex space-x-2">
      <div id="quick_item_1" class="cursor-move w-12 h-12 bg-black bg-opacity-80 rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
        <div class="items_icon items_potion scale-[1.9]"></div>
      </div>
      <div id="quick_item_2" class="cursor-move w-12 h-12 bg-black bg-opacity-80 rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
        <div class="items_icon items_shield scale-[1.9]"></div>
      </div>
      <div id="quick_item_3" class="cursor-move w-12 h-12 bg-black bg-opacity-80 rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
        <div class="items_icon items_sword scale-[1.9]"></div>
      </div>
      <div id="quick_item_4" class="cursor-move w-12 h-12 bg-black bg-opacity-80 rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
        <div class="items_icon items_skull scale-[1.9]"></div>
      </div>
      <div id="quick_item_5" class="cursor-move w-12 h-12 bg-black bg-opacity-80 rounded-md shadow-inner hover:shadow-lg transition-shadow duration-300 flex items-center justify-center">
        <div class="items_icon items_key scale-[1.9]"></div>
      </div>
    </div>
  </div>
</div>

<div id="chat_box" class='fixed bottom-0 left-1/2 transform -translate-x-1/2 mb-5 z-10 w-98'>
  <div class='chat-container text-sm subpixel-antialiased tracking-tight'>
    <div id="drag_strip" class="drag-strip w-full h-10"></div>
    <div id="ui_window_chat" class="w-full max-w-full rounded-md bg-[#03031a] bg-opacity-95 border border-black">

      <div id="tabs" class="flex m-2 mb-0 w-full z-10">
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

      <div id="chat_messages" class="chat-messages w-full p-2 overflow-y-auto text-ellipsis bg-opacity-95 text-white">
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

<script>
var ui_window = {
  start: function() {
    ui.initTabs('ui_window_chat', 'servers');

    const chatBox = document.getElementById('chat_box');
    const dragStrip = document.getElementById('drag_strip');
    const chatMessages = document.getElementById('chat_messages');
    const tabs = document.querySelectorAll('.tab');
    const tabContents = document.querySelectorAll('.tab-content');

    tabs.forEach(tab => {
      tab.addEventListener('click', function() {
        const target = this.getAttribute('data-tab');

        tabs.forEach(t => t.classList.remove('active'));
        this.classList.add('active');

        tabContents.forEach(tc => tc.classList.add('hidden'));
        const contentDiv = document.querySelector(`.tab-content[data-tab-content="${target}"]`);

        if (contentDiv) {
          contentDiv.classList.remove('hidden');
          ui.ajax({
            method: 'POST',
            url: `modals/ui/tabs/${target}/index.php`,
            success: function(data) {
              contentDiv.innerHTML = data;
            }
          });
        }
      });
    });

    // Trigger click on the servers tab to load its content by default
    document.querySelector('.tab[data-tab="servers"]').click();

    let isDragging = false;
    let startY;
    let startHeight;

    dragStrip.addEventListener('mousedown', (e) => {
      isDragging = true;
      startY = e.clientY;
      startHeight = parseInt(document.defaultView.getComputedStyle(chatMessages).height, 10);
      document.addEventListener('mousemove', doDrag);
      document.addEventListener('mouseup', stopDrag);
    });

    function doDrag(e) {
      if (!isDragging) return;

      const newHeight = startHeight + (startY - e.clientY);
      const maxHeight = window.innerHeight / 2;
      const minHeight = 0;

      if (newHeight >= minHeight && newHeight <= maxHeight) {
        chatMessages.style.height = newHeight + 'px';
      }
    }

    function stopDrag() {
      isDragging = false;
      document.removeEventListener('mousemove', doDrag);
      document.removeEventListener('mouseup', stopDrag);
    }

    // Initialize default health and energy values based on sprite properties
    const mainSprite = game.sprites['main'];

    if (mainSprite) {
      mainSprite.updateHealth(0);
      mainSprite.updateHealth(mainSprite.health);
      mainSprite.updateEnergy(mainSprite.energy);
    }

    document.getElementById('chat_input').addEventListener('keypress', function(e) {
      if (e.key === 'Enter') {
        const message = e.target.value;
        const playerId = network.getPlayerId();
        if (message.trim() !== "") {
          network.send({
            command: 'chatMessage',
            data: {
              playerId: playerId,
              message: message
            }
          });
          e.target.value = '';
        }
      }
    });

    document.addEventListener('chatMessage', function(e) {
      const { playerId, message } = e.detail.data;
      game.updateChatMessages(game.sprites[playerId], message);
      const sprite = game.sprites[playerId];
      if (sprite) {
        sprite.chatMessage = message;
        sprite.chatMessageTime = Date.now();
      } else {
        console.error(`Sprite with ID ${playerId} not found.`);
      }
    });
  },

  unmount: function() {
    // Cleanup HUD elements if necessary
  }
};

ui_window.start();
</script>

</div>

<?php
}
?>
