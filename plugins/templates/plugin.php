<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>

<div class="window bg-yellow-700" style="width: 330px;">
    <div class="window_title bg-yellow-600 text-yellow-100 p-2 rounded-t">
        <button data-close class="icon close_dark text-white" aria-label="Close">&times;</button>
        <span>Template Window</span>
    </div>
    <div class="container text-white p-2">
        <p>Basic content goes here</p>
    </div>
</div>
  
<style>

</style>
  
<script>
plugin_window = {
    start: function() {
        console.log(`Plugin started: ${this.id}`);
    },

    test_function: function() {
        console.log(`Test function executed for plugin: ${this.id}`);
    },

    unmount: function() {
        console.log(`Plugin unmounted: ${this.id}`);
    }
};
</script>

<?php
}
?>