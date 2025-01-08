<div class="window window_bg text-white" style="width: 330px;">
    <div class="window_title p-2">
        <span>Plugin Window</span>
    </div>
    <div class="container window_body text-center p-2">
        <p>Plugin content goes here</p>
        <button data-close class="white_button p-2 rounded mt-2" aria-label="Close">Okay</button>
    </div>
</div>
  
<style>

</style>
  
<script>
window[id] = {
    id: id,
    start: function() {
        console.log(`Plugin started: ${this.id}`);
    },

    unmount: function() {
        console.log(`Plugin unmounted: ${this.id}`);
    }
};
</script>