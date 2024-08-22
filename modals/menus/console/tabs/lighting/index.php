<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
<div class='container text-light window_body p-2'>
 

</div>

<script>
var ui_console_tab_window = {
    start: function() {
         
    },

    unmount: function() {
        console.log('Lighting window unmounted');
    },
   
};
ui_console_tab_window.start();
</script>
<?php
}
?>