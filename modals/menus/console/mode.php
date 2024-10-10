<?php
$mode = isset($_GET['mode']) ? $_GET['mode'] : 'standard';

if ($mode === 'editor') {
    ?>
        <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor_inventory" aria-label="Inventory">
      <div class="ui_icon ui_backpack"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor_info" aria-label="Info">
      <div class="ui_icon ui_info"></div>
    </button>
   <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor_weather" aria-label="Weather">
      <div class="ui_icon ui_cloud"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor_lighting" aria-label="Lighting">
      <div class="ui_icon ui_lightbulb"></div>
    </button>
    <button class="console_tab flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor_music" aria-label="Music">
      <div class="ui_icon ui_music"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor_scripting" aria-label="Scripting">
      <div class="ui_icon ui_script"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="editor_permissions" aria-label="Permissions">
      <div class="ui_icon ui_team"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="add" aria-label="Eye">
      <div class="ui_icon ui_eye"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="add" aria-label="Security">
      <div class="ui_icon ui_key"></div>
    </button>
    <div class="flex-1"></div>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="add" aria-label="Undo">
      <div class="ui_icon ui_undo"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="add" aria-label="Redo">
      <div class="ui_icon ui_redo"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="add" aria-label="Save">
      <div class="ui_icon ui_save"></div>
    </button>
    <button class="console_tab relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" data-tab="add" aria-label="Delete">
      <div class="ui_icon ui_delete"></div>
    </button>
    <button class="relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" aria-label="Toggle Full Screen Mode" onclick="console_window.toggleFullScreen();">
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
    <button class="relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" aria-label="Edit Mode" onclick="console_window.load_tab_buttons('editor'); modal.load({ id: 'editor_window', url: 'editor/index.php', name: 'Editor', showInList: true }); modal.load({ id: 'editor_utils_window', url: 'editor/utils.php', name: 'Editor Utils', showInList: true });">
      <div class="icon editor"></div>
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
    <button class="relative flex items-center justify-center w-full h-12 text-gray-600 hover:text-white hint--right px-2" aria-label="Toggle Full Screen Mode" onclick="console_window.toggleFullScreen();">
      <div class="icon full_screen"></div>
    </button>
    <?php
}
?>