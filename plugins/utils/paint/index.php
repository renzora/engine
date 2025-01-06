<div class="window bg-yellow-700" style="width: 500px;">
    <div class="window_title bg-yellow-600 text-yellow-100 p-2 rounded-t">
        <button data-close class="icon close_dark text-white" aria-label="Close">&times;</button>
        <span>Paint App</span>
    </div>
    <div class="container window_body text-white p-2">
        <canvas id="paintCanvas" width="480" height="360" class="bg-white"></canvas>
        <div class="tools mt-2">
            <label class="mr-2">
                Color:
                <input type="color" id="colorPicker" value="#000000">
            </label>
            <label class="mr-2">
                Brush Size:
                <input type="range" id="brushSize" min="1" max="20" value="5">
            </label>
            <button id="clearCanvas" class="bg-red-600 text-white px-2 py-1 rounded">Clear</button>
        </div>
    </div>
</div>

<style>
    #paintCanvas {
        border: 1px solid #ccc;
        cursor: crosshair;
    }
</style>

<script>
paint_window = {
    canvas: null,
    ctx: null,
    painting: false,

    startPainting: function(e) {
        this.painting = true;
        this.draw(e);
    },

    stopPainting: function() {
        this.painting = false;
        this.ctx.beginPath();
    },

    draw: function(e) {
        if (!this.painting) return;

        const brushSize = document.getElementById("brushSize").value;
        const color = document.getElementById("colorPicker").value;

        this.ctx.lineWidth = brushSize;
        this.ctx.lineCap = "round";
        this.ctx.strokeStyle = color;

        const rect = this.canvas.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;

        this.ctx.lineTo(x, y);
        this.ctx.stroke();
        this.ctx.beginPath();
        this.ctx.moveTo(x, y);
    },

    clearCanvas: function() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    },

    start: function() {
        this.canvas = document.getElementById("paintCanvas");
        this.ctx = this.canvas.getContext("2d");

        const clearButton = document.getElementById("clearCanvas");

        this.canvas.addEventListener("mousedown", this.startPainting.bind(this));
        this.canvas.addEventListener("mouseup", this.stopPainting.bind(this));
        this.canvas.addEventListener("mousemove", this.draw.bind(this));
        clearButton.addEventListener("click", this.clearCanvas.bind(this));
    },

    unmount: function() {
        this.canvas.removeEventListener("mousedown", this.startPainting.bind(this));
        this.canvas.removeEventListener("mouseup", this.stopPainting.bind(this));
        this.canvas.removeEventListener("mousemove", this.draw.bind(this));
    }
};
</script>
