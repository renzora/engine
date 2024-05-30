<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='craft_window' class='window window_bg bg-yellow-700 p-4 rounded-md' style='width: 900px;'>

    <div data-part='handle' class='window_title bg-yellow-600 p-2 rounded-t-md flex justify-between items-center'>
      <div class='title_bg text-white'>Craft Items</div>
      <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close>&times;</button>
    </div>
    <div class='clearfix'></div>
    <div class='relative p-4 bg-yellow-500 rounded-b-md'>
      <div class='flex justify-between'>
        <div class="items-container w-2/5 bg-yellow-300 p-2 rounded-md">
          <h3 class="text-center text-black mb-2">Items</h3>
          <div class="grid grid-cols-5 gap-1">
            <?php for ($i = 0; $i < 25; $i++): ?>
              <div class="item bg-green-500 text-white flex justify-center items-center h-16 cursor-move rounded-md" draggable="true" ondragstart="craft_window.drag(event)" id="item-<?php echo $i; ?>">Item <?php echo $i; ?></div>
            <?php endfor; ?>
          </div>
        </div>
        <div class="grid-container w-2/5 bg-yellow-300 p-2 rounded-md">
          <h3 class="text-center text-black mb-2">Crafting Grid</h3>
          <div class="grid grid-cols-5 gap-1">
            <?php for ($i = 0; $i < 25; $i++): ?>
              <div class="grid-slot bg-gray-200 border-2 border-dashed border-gray-400 h-16 flex justify-center items-center rounded-md" id="slot-<?php echo $i; ?>" ondrop="craft_window.drop(event)" ondragover="craft_window.allowDrop(event)">
              </div>
            <?php endfor; ?>
          </div>
        </div>
      </div>
    </div>

    <script>
      var craft_window = {
        allowDrop: function(ev) {
          ev.preventDefault();
        },
        drag: function(ev) {
          ev.dataTransfer.setData("text", ev.target.id);
          ev.dataTransfer.setDragImage(ev.target, ev.target.clientWidth / 2, ev.target.clientHeight / 2);
        },
        drop: function(ev) {
          ev.preventDefault();
          var data = ev.dataTransfer.getData("text");
          var item = document.getElementById(data);
          if (ev.target.classList.contains("grid-slot")) {
            ev.target.appendChild(item);
          }
        },
        start: function() {
          // Additional initialization if needed
        },
        unmount: function() {
          // Cleanup if needed
        }
      }
    </script>
  </div>
<?php
}
?>
