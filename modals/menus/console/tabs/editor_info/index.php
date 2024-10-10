<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div>
    <form>
        <div class="mb-4">
            <label for="scene_name" class="block text-lg font-medium text-gray-300">Scene Name</label>
            <input type="text" id="scene_name" name="scene_name" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md text-gray-900 bg-gray-100 focus:outline-none focus:ring-blue-500 focus:border-blue-500 text-lg" placeholder="Enter scene name">
        </div>
        
        <div class="mb-4">
            <label for="description" class="block text-lg font-medium text-gray-300">Description</label>
            <textarea id="description" name="description" rows="3" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md text-gray-900 bg-gray-100 focus:outline-none focus:ring-blue-500 focus:border-blue-500 text-lg" placeholder="Enter description"></textarea>
        </div>
        
        <div class="mb-4">
            <label for="category" class="block text-lg font-medium text-gray-300">Category</label>
            <select id="category" name="category" class="mt-1 block w-full px-3 py-2 border border-gray-300 rounded-md text-gray-900 bg-gray-100 focus:outline-none focus:ring-blue-500 focus:border-blue-500 text-lg">
                <option value="">Select category</option>
                <option value="action">Action</option>
                <option value="adventure">Adventure</option>
                <option value="puzzle">Puzzle</option>
            </select>
        </div>
        
        <div>
            <button type="submit" class="inline-flex justify-center py-2 px-4 border border-transparent text-lg font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500">
                Save
            </button>
        </div>
    </form>
</div>

<script>
var ui_console_editor_info = {

    start: function() {

    },

    unmount: function() {
        console.log('ui console editor info unmounted eeeee');
    }
};

ui_console_editor_info.start();
</script>

<?php
}
?>
