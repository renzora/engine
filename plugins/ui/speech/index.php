<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config/db.php';
if ($auth) {
?>
  <div class='window bg-gray-800/90 shadow-xl rounded-lg fixed inset-x-0 bottom-10 mx-auto text-center pixel-corners transition-all transform hover:scale-105' style='width: 500px; position: relative;'>
    
    <div class='relative grid grid-cols-3 gap-2'>
      <!-- Left column for image -->
      <div class='col-span-1 flex justify-center items-center rounded-lg'>
        <canvas id="speech_icon_canvas" class="w-full h-auto" height="250"></canvas>
      </div>

      <!-- Right column for text -->
      <div class='col-span-2 flex items-center mr-2'>
        <div class='container text-yellow-100 text-xl md:text-2xl lg:text-3xl font-mono tracking-widest leading-snug'>
          <p id="speech_content" class="pixel-corners">This is where the speech text will appear.</p>
        </div>
      </div>
    </div>

    <!-- Buttons for [A] Next and [A] Close -->
    <div class="flex justify-center items-center mb-2">
      <button class="text-yellow-100 text-xl md:text-xl lg:text-2xl font-mono tracking-widest px-4 py-2 bg-gray-700 border-2 rounded-lg hover:bg-gray-600" onclick="speech_window.aButton()">
        Press A
      </button>
    </div>
    </div>

    <script>
window[id] = {
    id: id,
    speechText: [],
    currentSpeechIndex: 0,
    typingInProgress: false,
    speechFinished: false,
    throttle: false,
    endSpeechCallback: null,

    start: function() {
        plugin.minimize('speech_window'); // Close the plugin after the last speech
        plugin.front('ui_inventory_window');
    },

    startSpeech: function(speechArray, callback, icon) {
        if (typeof speechArray === 'string') {
            speechArray = [speechArray];
        }

        this.speechText = speechArray;
        this.currentSpeechIndex = 0;
        this.endSpeechCallback = callback;

        plugin.show('speech_window');
        plugin.front('speech_window');

        // Check if the icon is "self" or a specific object and render it
        this.renderIcon(icon);

        this.typeSpeech(this.speechText[this.currentSpeechIndex]);
    },

    renderIcon: function(icon) {
    const canvas = document.getElementById('speech_icon_canvas');
    if (!canvas) {
        console.log('Canvas not found');
        return;
    }
    const ctx = canvas.getContext('2d');

    // Disable image smoothing for pixel-perfect rendering
    ctx.imageSmoothingEnabled = false;

    // Clear any previous icon drawings
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    console.log('Canvas cleared for rendering icon');

    // Use the icon (context.id) as a key to find the object in objectData
    const objectToRender = game.objectData[icon] ? game.objectData[icon][0] : null;
    if (!objectToRender) {
        console.log(`Object with key ${icon} not found in game.objectData`);
        return;
    }

    console.log(`Object found: ${objectToRender.n} (Type: ${objectToRender.t})`);

    // Get the object image and render it on the canvas
    const img = assets.use(objectToRender.t); // Load the texture based on 't' (texture reference)
    if (!img) {
        console.log(`Image for texture ${objectToRender.t} not found`);
        return;
    }

    const objectWidth = (objectToRender.a + 1) * 16; // Width in pixels
    const objectHeight = (objectToRender.b + 1) * 16; // Height in pixels

    // Calculate the scaling factor to fit the object within the canvas
    const scaleX = canvas.width / objectWidth;
    const scaleY = canvas.height / objectHeight;
    const scale = Math.min(scaleX, scaleY); // Use the smallest scaling factor to maintain aspect ratio

    // Adjust the position to keep the object centered
    let posX = Math.round((canvas.width - Math.round(objectWidth * scale)) / 2);
    let posY = Math.round((canvas.height - Math.round(objectHeight * scale)) / 2);

    // Parse the range if it's a range string (e.g., "306-353")
    const parseRange = (rangeString) => {
        const [start, end] = rangeString.split('-').map(Number);
        const rangeArray = [];
        for (let i = start; i <= end; i++) {
            rangeArray.push(i);
        }
        return rangeArray;
    };

    let frameIndices = [];
    if (typeof objectToRender.i[0] === 'string' && objectToRender.i[0].includes('-')) {
        frameIndices = parseRange(objectToRender.i[0]);
    } else {
        frameIndices = objectToRender.i;
    }

    let frameIndex = 0;
    for (let row = 0; row < objectToRender.b + 1; row++) {
        for (let col = 0; col < objectToRender.a + 1; col++) {
            if (frameIndex >= frameIndices.length) break;

            const tileFrameIndex = frameIndices[frameIndex];
            const srcX = (tileFrameIndex % 150) * 16;
            const srcY = Math.floor(tileFrameIndex / 150) * 16;

            const tilePosX = Math.round(posX + col * Math.round(16 * scale));
            const tilePosY = Math.round(posY + row * Math.round(16 * scale));

            // Draw the image with scaling and smoothing disabled
            ctx.drawImage(img, srcX, srcY, 16, 16, tilePosX, tilePosY, Math.round(16 * scale), Math.round(16 * scale));
            frameIndex++;
        }
    }

    console.log(`Icon rendered on canvas with scaling and smoothing disabled, object key: ${icon}`);
},


    typeSpeech: function(text) {
        let i = 0;
        let speed = 50; // Speed of the typewriter effect
        let speechElement = document.getElementById('speech_content');
        speechElement.textContent = ''; // Clear previous text
        this.typingInProgress = true;
        this.speechFinished = false; // Speech is not yet finished

        // Start playing the audio when typing begins
        audio.playAudio('electronic_readout_01', assets.use('electronic_readout_01'), 'sfx', true);

        const typeWriter = () => {
            if (i < text.length && this.typingInProgress) {
                speechElement.textContent += text.charAt(i);
                i++;
                setTimeout(typeWriter, speed);
            } else {
                // Typing is done
                this.typingInProgress = false;
                this.speechFinished = true;

                // Stop the looping audio once typing is finished
                audio.stopLoopingAudio('electronic_readout_01', 'sfx');
            }
        };

        typeWriter();
    },

    aButton: function() {
        if (this.throttle) return;

        this.throttle = true;
        setTimeout(() => { this.throttle = false; }, 300);

        // Play speech_menu_01 for regular speech interactions
        if (this.currentSpeechIndex < this.speechText.length - 1 || this.typingInProgress) {
            audio.playAudio('speech_menu_01', assets.use('speech_menu_01'), 'sfx', false);
        }

        if (this.typingInProgress) {
            let fullText = this.speechText[this.currentSpeechIndex];
            document.getElementById('speech_content').textContent = fullText;
            this.typingInProgress = false;
            this.speechFinished = true;

            // Stop the audio when skipping the typing
            audio.stopLoopingAudio('electronic_readout_01', 'sfx');
        } else if (this.speechFinished) {
            this.nextSpeech();
        }
    },

    nextSpeech: function() {
        if (this.currentSpeechIndex < this.speechText.length - 1) {
            this.currentSpeechIndex++;
            this.typeSpeech(this.speechText[this.currentSpeechIndex]);
        } else {
            // Play closeplugin sound when the last speech ends
            audio.playAudio('closeplugin', assets.use('closeplugin'), 'sfx', false);

            plugin.minimize('speech_window');
            plugin.show('ui_inventory_window');
            plugin.front('ui_inventory_window');
            this.resetSpeech();
        }
    },

    resetSpeech: function() {
        this.currentSpeechIndex = 0;

        if (this.endSpeechCallback) {
            this.endSpeechCallback();
        }

        this.endSpeechCallback = null;
    }
};
    </script>
<?php
}
?>
