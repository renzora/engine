<div data-window='fps_monitor_window' class='window pixel-corners bg-gray-800 shadow-lg rounded-lg overflow-hidden fixed bottom-10 right-5' style='width: 400px;height: 300px;'>

    <div data-part='handle' class='window_title bg-gray-800 text-gray-100 font-semibold'>
        <div class='float-right'>
            <button class="icon close_dark mr-1 hint--left text-gray-100" aria-label="Close (ESC)" data-close>&times;</button>
        </div>
        <div data-part='title' class='title_bg'>Activity Monitor</div>
    </div>
    
    <div class='p-2 bg-gray-800'>
        <div id="fpsMonitorContainer" class="w-full h-48 bg-gray-800 rounded-md">
            <canvas id="fpsChart" class="w-full" style="height: 280px;"></canvas>
        </div>
    </div>

<script>
var fps_monitor_window = {
    dpr: 1,  // Default value, will be updated in setupCanvas
    hiddenFunctions: {}, // Track hidden functions

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

        // Add click event listener for toggling function visibility
        canvas.addEventListener('click', this.handleCanvasClick.bind(this));

        this.renderChart();  // Initial render to set up the chart layout
    },

    onResize: function() {
        this.setupCanvas();  // Re-setup canvas on resize to adjust size and re-render the chart
    },

    handleCanvasClick: function(event) {
        const canvas = event.target;
        const rect = canvas.getBoundingClientRect();
        const x = (event.clientX - rect.left) * this.dpr;
        const y = (event.clientY - rect.top) * this.dpr;

        const functionCalls = utils.getTrackedCalls();
        const trackedFunctions = Object.keys(functionCalls);

        // Detect if a label is clicked
        const labelArea = 15; // Height of each label area
        trackedFunctions.forEach((functionName, index) => {
            const labelY = 20 + index * labelArea;
            if (y > labelY - 10 && y < labelY + 5) {
                // Toggle visibility
                this.hiddenFunctions[functionName] = !this.hiddenFunctions[functionName];
                this.renderChart(); // Re-render the chart
            }
        });
    },

renderChart: function () {
    const canvas = document.getElementById('fpsChart');
    const ctx = canvas.getContext('2d');
    const dpr = this.dpr;

    // Retrieve all tracked calls dynamically
    const functionCalls = utils.getTrackedCalls();

    // Filter out functions with both `count` and `value` at 0
    const trackedFunctions = Object.keys(functionCalls).filter((functionName) => {
        const { countHistory, valueHistory } = functionCalls[functionName];
        const latestCount = countHistory?.[countHistory.length - 1] || 0;
        const latestValue = valueHistory?.[valueHistory.length - 1] || 0;
        return !(latestCount === 0 && latestValue === 0); // Remove if both are 0
    });

    // Determine maximum value for scaling
    let globalMaxValue = 0;
    trackedFunctions.forEach((functionName) => {
        const { countHistory, valueHistory } = functionCalls[functionName];
        const localMax = Math.max(
            ...(countHistory || []),
            ...(valueHistory || [])
        );
        if (localMax > globalMaxValue) globalMaxValue = localMax;
    });

    // Add buffer to the maximum value for visual spacing
    const maxValue = Math.ceil(globalMaxValue * 1.1); // Round up for whole number max

    // Clear the previous frame
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Padding and dimensions
    const padding = 30;
    const chartHeight = canvas.height / dpr - 2 * padding;
    const chartWidth = canvas.width / dpr - 2 * padding;

    // Helper to format function metrics (whole numbers without decimals, others toFixed(2))
    const formatMetric = (num) => (Number.isInteger(num) ? num : num.toFixed(2));

    // Draw vertical axis labels on both sides (whole numbers only)
    const numberOfLabels = 5; // Number of vertical labels
    ctx.fillStyle = 'rgba(255, 255, 255, 0.8)';
    ctx.font = '12px Arial';

    for (let i = 0; i <= numberOfLabels; i++) {
        const value = Math.round((i / numberOfLabels) * maxValue); // Ensure whole number
        const y = canvas.height / dpr - padding - (value / maxValue) * chartHeight;

        // Draw left-side labels
        ctx.textAlign = 'left';
        ctx.fillText(value, 5, y + 4);

        // Draw right-side labels
        ctx.textAlign = 'right';
        ctx.fillText(value, canvas.width / dpr - 5, y + 4);

        // Draw gridlines
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.2)';
        ctx.beginPath();
        ctx.moveTo(padding, y);
        ctx.lineTo(canvas.width / dpr - padding, y);
        ctx.stroke();
    }

    // Draw line charts for each tracked function
    trackedFunctions.forEach((functionName, index) => {
        const { countHistory, valueHistory } = functionCalls[functionName];
        const latestCount = countHistory?.[countHistory.length - 1] || 0;
        const latestValue = valueHistory?.[valueHistory.length - 1];

        // Use count if value is missing
        const displayMetric = typeof latestValue === 'number' ? latestValue : latestCount;

        const baseColor = `hsl(${(index / trackedFunctions.length) * 360}, 70%, 60%)`; // Assign unique color

        // Skip rendering lines for hidden functions
        if (!this.hiddenFunctions[functionName]) {
            ctx.strokeStyle = baseColor;
            ctx.lineWidth = 2;

            // Draw counts
            ctx.beginPath();
            countHistory.forEach((count, i) => {
                const x = padding + (i / countHistory.length) * chartWidth;
                const y = canvas.height / dpr - padding - (count / maxValue) * chartHeight;
                if (i === 0) {
                    ctx.moveTo(x, y);
                } else {
                    ctx.lineTo(x, y);
                }
            });
            ctx.stroke();

            // Draw values (optional)
            ctx.strokeStyle = `rgba(${parseInt((index / trackedFunctions.length) * 255)}, 255, 255, 0.7)`;
            ctx.beginPath();
            valueHistory.forEach((value, i) => {
                const x = padding + (i / valueHistory.length) * chartWidth;
                const y = canvas.height / dpr - padding - (value / maxValue) * chartHeight;
                if (i === 0) {
                    ctx.moveTo(x, y);
                } else {
                    ctx.lineTo(x, y);
                }
            });
            ctx.stroke();
        }
    });

    // Draw labels as the last step to ensure they are on top of all chart elements
    trackedFunctions.forEach((functionName, index) => {
        const { countHistory, valueHistory } = functionCalls[functionName];
        const latestCount = countHistory?.[countHistory.length - 1] || 0;
        const latestValue = valueHistory?.[valueHistory.length - 1];

        // Use count if value is missing
        const displayMetric = typeof latestValue === 'number' ? latestValue : latestCount;

        const baseColor = `hsl(${(index / trackedFunctions.length) * 360}, 70%, 60%)`; // Assign unique color

        // Label coordinates
        const labelX = padding + 5;
        const labelY = 20 + index * 15;

        // Draw background rectangle for label
        ctx.fillStyle = 'rgba(0, 0, 0, 0.6)'; // Opaque black background with 60% opacity
        const labelText = `${functionName}: ${formatMetric(displayMetric)}`;
        const textWidth = ctx.measureText(labelText).width;
        const textHeight = 12; // Approximate height of text
        ctx.fillRect(labelX - 2, labelY - textHeight + 2, textWidth + 4, textHeight + 4);

        // Draw label text
        ctx.fillStyle = baseColor;
        ctx.textAlign = 'left';
        ctx.fillText(labelText, labelX, labelY);

        // Draw strikethrough for hidden functions
        if (this.hiddenFunctions[functionName]) {
            ctx.strokeStyle = baseColor; // Strikethrough matches chart color
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.moveTo(labelX, labelY - 5); // Start the line slightly above the text
            ctx.lineTo(labelX + textWidth, labelY - 5);
            ctx.stroke();
        }
    });
}


};

fps_monitor_window.start();  // Start the FPS monitor when the plugin is loaded
</script>


    <div class='resize-handle'></div>
</div>
