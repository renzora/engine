<div
  class="window_body bg-gray-900 fixed bottom-0 left-0 flex justify-center items-center"
  style="width: 100%;"
>
  <div 
    class="keyboard-container w-full max-w-7xl mx-auto lg:mx-0 lg:px-0" 
  >
    <div id="keyboard" class="keyboard flex flex-col gap-3 text-white p-4"></div>
  </div>
  </div>

  <style>
    /* Key Press Effect */
    .key.pressed {
      background: linear-gradient(to bottom right, #1e90ff, #0074d9); /* Blue gradient */
      color: #ffffff; /* White text */
      box-shadow: 0 0 15px 5px rgba(30, 144, 255, 0.8); /* Blue glow */
      transition: background 0.2s, color 0.2s, box-shadow 0.2s;
    }

    /* Mic Listening Effect */
    #micButton.listening {
      background: #ff4d4d; /* Red for listening */
      color: #ffffff;
      box-shadow: 0 0 15px 5px rgba(255, 77, 77, 0.8); /* Red glow */
    }
  </style>

  <script>
window[id] = {
  id: id,
      capsLock: false,
      recognition: null,
      isListening: false,

      start: function () {
        const keyboardLayout = {
          rows: [
            ["@", "#", "$", "%", "^", "&", "*", "(", ")", "-", "_", "+", "Backspace"],
            ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"],
            ["Tab", "q", "w", "e", "r", "t", "y", "u", "i", "o", "p", "[", "]", "\\"],
            ["Caps", "a", "s", "d", "f", "g", "h", "j", "k", "l", ";", "'", "Enter"],
            ["Shift", "z", "x", "c", "v", "b", "n", "m", ",", ".", "/", "Shift"],
            ["Space", "Mic"],
          ],
        };

        const keyboardContainer = document.getElementById("keyboard");
        if (!keyboardContainer) return;

        keyboardLayout.rows.forEach((row) => {
          const rowDiv = document.createElement("div");
          rowDiv.classList.add("flex", "justify-center", "gap-2");

          row.forEach((key) => {
            const keyButton = document.createElement("button");
            keyButton.textContent = key;
            keyButton.classList.add(
              "key",
              "bg-gradient-to-br",
              "from-gray-700",
              "to-gray-800",
              "hover:from-gray-600",
              "hover:to-gray-700",
              "text-white",
              "rounded-md",
              "shadow",
              "transition",
              "duration-200",
              "ease-in-out",
              "text-lg",
              "py-3",
              "px-4",
              "flex",
              "justify-center",
              "items-center",
              "flex-grow",
              "md:text-xl",
              "md:py-4",
              "lg:text-2xl",
              "lg:py-5"
            );

            if (key === "Space") {
              keyButton.classList.add("w-full", "py-2", "rounded-full");
            } else if (key === "Mic") {
              keyButton.id = "micButton";
              keyButton.classList.add("flex-none", "px-12", "bg-blue-600", "hover:bg-blue-700");
              keyButton.textContent = "🎤";
            } else if (["Backspace", "Enter", "Shift", "Caps", "Tab"].includes(key)) {
              keyButton.classList.add("flex-none", "px-12");
            }

            if (key === "Caps") {
              keyButton.addEventListener("click", () => {
                this.capsLock = !this.capsLock;
                this.toggleCase();
              });
            }

            keyButton.addEventListener("click", () => {
              keyButton.classList.add("pressed");
              setTimeout(() => keyButton.classList.remove("pressed"), 200);
            });

            rowDiv.appendChild(keyButton);
          });

          keyboardContainer.appendChild(rowDiv);
        });

        this.initSpeechRecognition();

        const micButton = document.getElementById("micButton");
        micButton.addEventListener("click", () => {
          // Focus the first input field on the screen
          const firstInput = document.querySelector("input, textarea");
          if (firstInput) {
            firstInput.focus();
          }

          if (this.isListening) {
            this.stopListening();
          } else {
            this.startListening();
          }
        });
      },

      toggleCase: function () {
        document.querySelectorAll(".key").forEach((key) => {
          if (!["Backspace", "Enter", "Shift", "Caps", "Tab", "Space", "Mic"].includes(key.textContent)) {
            key.textContent = this.capsLock
              ? key.textContent.toUpperCase()
              : key.textContent.toLowerCase();
          }
        });
      },

      initSpeechRecognition: function () {
        if (!('webkitSpeechRecognition' in window)) {
          alert("Speech Recognition is not supported in this browser.");
          return;
        }

        const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition;
        this.recognition = new SpeechRecognition();
        this.recognition.lang = "en-US";
        this.recognition.interimResults = false;
        this.recognition.continuous = false;

        this.recognition.onresult = (event) => {
          const transcript = event.results[0][0].transcript;
          console.log("Detected speech:", transcript);

          // Store detected speech in the active input field
          const activeInput = document.activeElement;
          if (activeInput && (activeInput.tagName === "INPUT" || activeInput.tagName === "TEXTAREA")) {
            activeInput.value += transcript;
          } else {
            console.warn("No active input field found to insert text.");
          }
        };

        this.recognition.onerror = (event) => {
          console.error("Speech recognition error:", event.error);
        };

        this.recognition.onend = () => {
          this.isListening = false;
          this.updateMicButtonState();
        };
      },

      startListening: function () {
        if (this.recognition) {
          this.isListening = true;
          this.updateMicButtonState();
          this.recognition.start();
        }
      },

      stopListening: function () {
        if (this.recognition) {
          this.isListening = false;
          this.updateMicButtonState();
          this.recognition.stop();
        }
      },

      updateMicButtonState: function () {
        const micButton = document.getElementById("micButton");
        if (this.isListening) {
          micButton.classList.add("listening");
        } else {
          micButton.classList.remove("listening");
        }
      },

      unmount: function () {
        if (this.recognition) {
          this.recognition.abort();
        }
        const keyboardContainer = document.getElementById("keyboard");
        if (keyboardContainer) {
          keyboardContainer.innerHTML = "";
        }
      },
    };
  </script>
