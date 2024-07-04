<div data-window='ui_modals_list_window' data-close="false">
  <div class='fixed bottom-3 left-2 text-sm mb-1'>
    <button id="ui_show_modals_button" class="green_button text-white font-bold py-1 px-2 rounded shadow-md relative" onclick="ui_modals_list_window.showModalsList()">
      Modals
    </button>
    <div id="ui_modals_list" class="hidden absolute bottom-10 left-0 bg-gray-800 p-2 rounded shadow-md z-20 w-64"></div>
  </div>

  <script>
  var ui_modals_list_window = {
    showModalsList: function() {
      const modalsList = document.getElementById('ui_modals_list');
      if (modalsList) {
        modalsList.classList.toggle('hidden');
      }
    }
  };
  </script>
</div>
