<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='inventory_window' class='window window_bg' style='width: 400px; background: #222; border: 0;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#111 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #222; color: #ede8d6;'>Inventory</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative p-4'>

      <!-- Tabbed Menu -->
      <div class="tabs mb-4">
      <input type="text" class="p-2 mb-3 w-full" style="background: #333;" placeholder="search..." />
        <div class="flex">
          <button class="tab-button px-4 py-2 bg-blue-500 text-white" data-category="category1">Objects</button>
          <button class="tab-button px-4 py-2 bg-gray-800 text-gray-300" data-category="category2">Items</button>
        </div>
      </div>

      <!-- Inventory Grids -->
      <div id="category1" class="inventory-grid grid grid-cols-6 gap-4" style="height: 400px; overflow-y: auto;">
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 3</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 4</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 5</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 6</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 3</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 4</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 5</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 6</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 3</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 4</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 5</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 6</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 3</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 4</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 5</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 6</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 3</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 4</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 5</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 6</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 3</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 4</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 5</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 6</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 3</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 4</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 5</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 6</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 1</div>
        <div class="inventory-item rounded-lg bg-gray-700 p-2">Item 2</div>
      </div>
      <div id="category2" class="inventory-grid grid grid-cols-6 gap-4 hidden">

      </div>
    </div>

    <script>
      var inventory_window = {
        start: function() {
          const tabs = document.querySelectorAll('.tab-button');
          const categories = document.querySelectorAll('.inventory-grid');

          tabs.forEach(tab => {
            tab.addEventListener('click', function() {
              // Remove active class from all tabs
              tabs.forEach(t => t.classList.remove('bg-blue-500', 'text-white'));
              tabs.forEach(t => t.classList.add('bg-gray-800', 'text-gray-300'));

              // Hide all categories
              categories.forEach(category => category.classList.add('hidden'));

              // Add active class to clicked tab
              this.classList.remove('bg-gray-800', 'text-gray-300');
              this.classList.add('bg-blue-500', 'text-white');

              // Show the corresponding category
              const categoryId = this.getAttribute('data-category');
              document.getElementById(categoryId).classList.remove('hidden');
            });
          });
        },
        unmount: function() {
          // Cleanup if needed
        }
      }

      inventory_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
