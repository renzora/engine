<?php
$mode = isset($_GET['mode']) ? $_GET['mode'] : 'standard';

if ($mode === 'editor') {
    ?>
       <button class="console_tab relative flex items-center justify-center w-full h-16 text-gray-600 hover:text-white hint--right px-2" data-tab="servers" aria-label="Online Servers">
      <div class="icon globe"></div>
    </button>
        <button class="console_tab relative flex items-center justify-center w-full h-16 text-gray-600 hover:text-white hint--right px-2" data-tab="inventory" aria-label="Inventory">
      <div class="ui_icon ui_backpack"></div>
    </button>

    <div class="flex-1"></div>

    <button class="console_tab relative flex items-center justify-center w-full h-16 text-gray-600 hover:text-white hint--right px-2" data-tab="settings" aria-label="Settings & Controls">
      <div class="icon settings"></div>
    </button>

    <button class="relative flex items-center justify-center w-full h-16 text-gray-600 hover:text-white hint--right px-2" aria-label="Toggle Full Screen Mode" onclick="utils.toggleFullScreen();">
      <div class="icon full_screen"></div>
    </button>
    <?php
} else {

    ?>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="servers" aria-label="Online Servers">
      <div class="icon globe"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="friends" aria-label="Friends">
      <div class="icon friends"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">1</span>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="inventory" aria-label="Inventory">
      <div class="icon inventory"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">48</span>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="chat" aria-label="Chat">
      <div class="icon chat"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">85</span>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="gift" aria-label="Market & Auction">
      <div class="icon gift"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">34</span>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="avatar" aria-label="Change Avatar">
      <div class="icon avatar"></div>
    </button>
    <div class="flex-1"></div>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="warroom" aria-label="Renzora Admin Panel">
      <div class="icon mod"></div>
      <span class="absolute top-0 right-0.5 bg-red-700 text-white text-xs rounded-sm px-0.5 flex items-center justify-center">16</span>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="help" aria-label="Help & FAQ">
      <div class="icon admin"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="settings" aria-label="Settings & Controls">
      <div class="icon settings"></div>
    </button>
    <button class="relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" aria-label="Toggle Full Screen Mode" onclick="utils.toggleFullScreen();">
      <div class="icon full_screen"></div>
    </button>
    <?php
}
?>