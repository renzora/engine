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
<div class='fixed top-0 right-0 m-3 z-10 flex space-x-4 tracking-tight'>
  <div class="flex flex-col space-y-2 w-40">

  <div class="flex items-center space-x-2">
    <div class="relative w-full bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px]">
        <div id="health" class="rounded bg-gradient-to-r from-lime-500 to-green-600 h-full transition-width duration-500"></div>
        <div class="absolute inset-0 flex items-center pl-2 text-white text-sm">Health: 0%</div>
    </div>
</div>

<div class="flex items-center space-x-2">
    <div class="relative w-full bg-gray-900 rounded-md h-6 overflow-hidden shadow-inner bg-opacity-80 shadow-sm p-[1px]">
        <div id="energy" class="rounded bg-gradient-to-r from-cyan-400 to-blue-600 h-full transition-width duration-500"></div>
        <div class="absolute inset-0 flex items-center pl-2 text-white text-sm">Energy: 0%</div>
    </div>
</div>

    <!-- Quick Items -->
    <div class="flex space-x-2">
      <div id="quick_item_1" class="w-10 h-9 bg-gray-900 rounded-lg shadow-inner border border-gray-900 hover:shadow-lg transition-shadow duration-300 bg-opacity-80"></div>
      <div id="quick_item_2" class="w-10 h-9 bg-gray-900 rounded-lg shadow-inner border border-gray-900 hover:shadow-lg transition-shadow duration-300 bg-opacity-80"></div>
      <div id="quick_item_3" class="w-10 h-9 bg-gray-900 rounded-lg shadow-inner border border-gray-900 hover:shadow-lg transition-shadow duration-300 bg-opacity-80"></div>
      <div id="quick_item_4" class="w-10 h-9 bg-gray-900 rounded-lg shadow-inner border border-gray-900 hover:shadow-lg transition-shadow duration-300 bg-opacity-80"></div>
    </div>
  </div>
  <!-- Avatar Box -->
  <div id="avatar" class="w-20 h-20 bg-gray-900 rounded-lg shadow-2xl border border-gray-900 bg-opacity-80 hover:shadow-2xl transition-shadow duration-300"></div>
</div>

<div id="chat_box" class='fixed bottom-0 left-1/2 transform -translate-x-1/2 mb-3 z-10 w-50'>
  <div class='chat-container text-sm subpixel-antialiased tracking-tight'>
    <div id="drag_strip" class="drag-strip w-full h-10"></div>
    <div id="chat_messages" class="chat-messages w-full rounded-t-md p-2 h-[95px] overflow-y-auto text-ellipsis bg-[#03031a] bg-opacity-95 border-l border-t border-r border-black">
      <div class="text-white mb-1"><span class="text-blue-400 font-black">adam49</span> Yeah it's probably better if you use alt + c </div>
      <div class="text-white mb-1"><span class="text-red-400 font-black">Global Alert</span> <span class="text-yellow-400">There will be a Server maintenance in 10 minutes</span></div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">germanboi15</span> No because it's got auto aim and the radius is +8 on mg8</div>
      <div class="text-white mb-1"><span class="text-purple-400 font-black">chesslady123</span> oh nice I didn't know that....</div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">chesslady123</span> Yeah that's why perks are great lol</div>
      <div class="text-white mb-1"><span class="text-yellow-400 font-black">User2</span> Yeah it's probably better if you use alt + c </div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">User2</span> But won't that make the aim worse when I'm aiming for the head?</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> No because it's got auto aim and the radius is +8 on mg8</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> oh nice I didn't know that....</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> Yeah that's why perks are great lol</div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">adam49</span> Yeah it's probably better if you use alt + c </div>
      <div class="text-white mb-1"><span class="text-red-400 font-black">Global Alert</span> <span class="text-yellow-400">There will be a Server maintenance in 10 minutes</span></div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">germanboi15</span> No because it's got auto aim and the radius is +8 on mg8</div>
      <div class="text-white mb-1"><span class="text-purple-400 font-black">poopinglady</span> oh nice I didn't know that....</div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">poopinglady</span> Yeah that's why perks are great lol</div>
      <div class="text-white mb-1"><span class="text-yellow-400 font-black">User2</span> Yeah it's probably better if you use alt + c </div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">User2</span> But won't that make the aim worse when I'm aiming for the head?</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> No because it's got auto aim and the radius is +8 on mg8</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> oh nice I didn't know that....</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> Yeah that's why perks are great lol</div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">adam49</span> Yeah it's probably better if you use alt + c </div>
      <div class="text-white mb-1"><span class="text-red-400 font-black">Global Alert</span> <span class="text-yellow-400">There will be a Server maintenance in 10 minutes</span></div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">germanboi15</span> No because it's got auto aim and the radius is +8 on mg8</div>
      <div class="text-white mb-1"><span class="text-purple-400 font-black">poopinglady</span> oh nice I didn't know that....</div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">poopinglady</span> Yeah that's why perks are great lol</div>
      <div class="text-white mb-1"><span class="text-yellow-400 font-black">User2</span> Yeah it's probably better if you use alt + c </div>
      <div class="text-white mb-1"><span class="text-blue-400 font-black">User2</span> But won't that make the aim worse when I'm aiming for the head?</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> No because it's got auto aim and the radius is +8 on mg8</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> oh nice I didn't know that....</div>
      <div class="text-white mb-1"><span class="text-green-400 font-black">User2</span> Yeah that's why perks are great lol</div>
    </div>
    <div class="w-full">
    <input type="text" id="chat_input" class="w-full shadow p-2 text-white tracking-tight bg-[#03031a] border-l border-r border-b border-black shadow-lg rounded-b-md focus:outline-none" placeholder="Type a message...">

    </div>
  </div>
</div>


  <!-- Sidebar: Settings, Survival Mode, Servers -->
  <div class="fixed top-1/2 left-0 transform -translate-y-1/2 rounded-r-xl p-2 ml-1 mt-2 bg-opacity-10 bg-white z-10 scale-125">
    <div class="cursor-pointer">
      <div onclick="modal.load('servers')" aria-label="Servers" class="icon globe hint--right"></div>
    </div>
    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></span>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon friends hint--right"></div>
    </div>
    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></span>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon editor hint--right"></div>
    </div>
    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></span>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon chat hint--right"></div>
    </div>
    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></span>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon gift hint--right"></div>
    </div>
    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></span>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon avatar hint--right"></div>
    </div>
    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></span>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon music hint--right"></div>
    </div>

    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></pan>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon sword hint--right"></div>
    </div>
    <div class="relative cursor-pointer">
      <span id="market_notif" class="absolute top-0 left-0 transform -translate-x-1/2 -translate-y-1/2 badge rounded bg-red-600 border border-gray-900 shadow-md mt-3 ml-1 p-1 text-white text-xs hidden" style="z-index: 1;"></span>
      <div onclick="modal.load('ui/survival.php','survival_window'); modal.close('ui_window');" aria-label="Survival Mode" class="icon inventory hint--right"></div>
    </div>
    <div class="cursor-pointer">
      <div onclick="modal.load('settings')" aria-label="Game Settings" class="icon settings hint--right"></div>
    </div>
    
  </div>

</div>

<script>
var ui_window = {
  start: function() {
    const chatBox = document.getElementById('chat_box');
    const dragStrip = document.getElementById('drag_strip');
    const chatMessages = document.getElementById('chat_messages');

    let isDragging = false;
    let startY;
    let startHeight;

    // For resizing
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
      const minHeight = 30;

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
      // Initialize and reflect the current health and energy
      mainSprite.updateHealth(0);  // Ensure update occurs
      mainSprite.updateHealth(mainSprite.health);  
      mainSprite.updateEnergy(mainSprite.energy);  
    }


    document.getElementById('chat_input').addEventListener('keypress', function(e) {
    if (e.key === 'Enter') {
        const message = e.target.value;
        const playerId = network.getPlayerId(); // Ensure player ID is fetched
        console.log(playerId);
        if (message.trim() !== "") {
            network.send({
                command: 'chatMessage',
                data: {
                    playerId: playerId, // Include player ID in the data
                    message: message
                }
            });
            e.target.value = ''; // Clear the input field
        }
    }
});


document.addEventListener('chatMessage', function(e) {
    const { playerId, message } = e.detail.data;
    console.log(`Received chat message from player ${playerId}: ${message}`);
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

<?php
}
?>
