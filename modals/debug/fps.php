<div data-window='fps_monitor_window' class='window pixel-corners bg-gray-800 shadow-lg rounded-lg overflow-hidden fixed bottom-10 right-5' style='width: 300px; height: 200px;'>

    <div data-part='handle' class='window_title bg-gray-800 text-gray-100 font-semibold'>
        <div class='float-right'>
            <button class="icon close_dark mr-1 hint--left text-gray-100" aria-label="Close (ESC)" data-close>&times;</button>
        </div>
        <div data-part='title' class='title_bg'>FPS Monitor</div>
    </div>
    
    <div class='p-2 bg-gray-800'>
        <div id="fpsMonitorContainer" class="w-full h-48 bg-gray-800 rounded-md mb-2">
            <canvas id="fpsChart" class="w-full h-full"></canvas>
        </div>
    </div>

    <script>
var fps_monitor_window = {
    dpr: 1,  // Default value, will be updated in setupCanvas

    start: function() {
        this.setupCanvas();  // Set up the canvas before rendering
        window.addEventListener('resize', this.onResize.bind(this));  // Add resize listener
    },

    setupCanvas: function() {
        const canvas = document.getElementById('fpsChart');
        const ctx = canvas.getContext('2d');

        // Get device pixel ratio and adjust canvas size for high-DPI displays
        this.dpr = window.devicePixelRatio || 1;
        canvas.width = canvas.offsetWidth * this.dpr;
        canvas.height = canvas.offsetHeight * this.dpr;
        ctx.scale(this.dpr, this.dpr);

        this.renderChart(0);  // Initial render with 0 FPS to set up the chart layout
    },

    onResize: function() {
        this.setupCanvas();  // Re-setup canvas on resize to adjust size and re-render the chart
    },

    renderChart: function(fps) {
        const canvas = document.getElementById('fpsChart');
        const ctx = canvas.getContext('2d');
        const dpr = this.dpr;  // Use the stored dpr value

        const maxFps = 60;  // Standard FPS max value
        const fpsRange = maxFps;

        // Store the FPS value in history
        game.fpsHistory.push(fps);
        if (game.fpsHistory.length > game.maxFpsHistory) {
            game.fpsHistory.shift(); // Remove oldest entry if exceeding max history
        }

        const fpsHistory = game.fpsHistory || [];

        // Clear the previous frame
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        // Calculate dynamic padding based on canvas height
        const padding = Math.max(5, canvas.height / dpr * 0.1);

        // Draw vertical axis labels for FPS with padding
        ctx.fillStyle = 'rgba(255, 255, 255, 0.8)';
        ctx.font = '12px Arial';
        const numberOfLabels = 5;  // Number of FPS labels to display on the chart
        const labelStep = Math.floor(maxFps / numberOfLabels);  // Calculate the step between labels dynamically

        for (let i = 0; i <= numberOfLabels; i++) {
            const fpsValue = i * labelStep;
            const y = canvas.height / dpr - padding - (fpsValue / fpsRange) * (canvas.height / dpr - 2 * padding);
            ctx.fillText(fpsValue, 2, y + 4);
            ctx.beginPath();
            ctx.moveTo(35, y);
            ctx.lineTo(canvas.width / dpr, y);
            ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
            ctx.stroke();
        }

        // Draw the FPS chart with dynamic padding
        ctx.beginPath();
        ctx.moveTo(35, canvas.height / dpr - padding);

        fpsHistory.forEach((fps, index) => {
            const x = 35 + (index / game.maxFpsHistory) * (canvas.width / dpr - 35);
            const y = canvas.height / dpr - padding - (fps / fpsRange) * (canvas.height / dpr - 2 * padding);
            ctx.lineTo(x, y);
        });

        ctx.lineTo(canvas.width / dpr, canvas.height / dpr - padding);
        ctx.closePath();

        ctx.fillStyle = 'rgba(75, 192, 192, 0.3)';
        ctx.fill();
        ctx.strokeStyle = 'rgba(75, 192, 192, 1)';
        ctx.stroke();
    }
};

fps_monitor_window.start();  // Start the FPS monitor when the modal is loaded

    </script>

    <div class='resize-handle'></div>
</div>
