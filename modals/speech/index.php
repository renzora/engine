<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='speech_window' class='window bg-gray-900 fixed inset-x-0 bottom-10 mx-auto p-6 border-4 border-yellow-600 text-center' style='width: 700px;'>

    <div class='relative grid grid-cols-3 gap-4'>
      <!-- Left column for image -->
      <div class='col-span-1'>
        <img src="https://via.placeholder.com/200x200.png" alt="Character" class="w-full h-auto">
      </div>

      <!-- Right column for text -->
      <div class='col-span-2 flex items-center'>
        <div class='container text-yellow-100 text-2xl font-mono tracking-wider'>
          <p id="speech_content">This is where the speech text will appear.</p>
        </div>
      </div>
    </div>

    <script>
var speech_window = {
    speechText: [],
    currentSpeechIndex: 0,
    typingInProgress: false,
    speechFinished: false,
    throttle: false,
    endSpeechCallback: null,

    start: function() {
        modal.minimize('speech_window'); // Close the modal after the last speech
        modal.front('ui_inventory_window');
    },

    startSpeech: function(speechArray, callback) {
        // Check if the speechArray is a string; if so, convert it to an array
        if (typeof speechArray === 'string') {
            speechArray = [speechArray]; // Wrap the string into an array
        }

        // Reset the speech index and add the new text to speechText
        this.speechText = speechArray;
        this.currentSpeechIndex = 0;

        // Store the callback to reset the flag when speech ends
        this.endSpeechCallback = callback;

        // Bring the speech window to the front
        modal.show('speech_window');
        modal.front('speech_window');

        // Start typing the first speech text
        this.typeSpeech(this.speechText[this.currentSpeechIndex]);
    },

    typeSpeech: function(text) {
        let i = 0;
        let speed = 50; // Speed of the typewriter effect
        let speechElement = document.getElementById('speech_content');
        speechElement.textContent = ''; // Clear previous text
        this.typingInProgress = true;
        this.speechFinished = false; // Speech is not yet finished

        const typeWriter = () => {
            if (i < text.length && this.typingInProgress) {
                speechElement.textContent += text.charAt(i);
                i++;
                setTimeout(typeWriter, speed);
            } else {
                // Typing is done
                this.typingInProgress = false;
                this.speechFinished = true;
            }
        };

        typeWriter();
    },

    aButton: function() {
        if (this.throttle) return;

        this.throttle = true;
        setTimeout(() => { this.throttle = false; }, 300);

        if (this.typingInProgress) {
            let fullText = this.speechText[this.currentSpeechIndex];
            document.getElementById('speech_content').textContent = fullText;
            this.typingInProgress = false;
            this.speechFinished = true;
        } else if (this.speechFinished) {
            this.nextSpeech();
        }
    },

    nextSpeech: function() {
        if (this.currentSpeechIndex < this.speechText.length - 1) {
            this.currentSpeechIndex++;
            this.typeSpeech(this.speechText[this.currentSpeechIndex]);
        } else {
            modal.minimize('speech_window');
            modal.show('ui_inventory_window');
            modal.front('ui_inventory_window');
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

      // Example of how you can trigger the speech box
      speech_window.start();
    </script>

  </div>
<?php
}
?>
