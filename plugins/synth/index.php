<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='synth_window' class='window bg-yellow-500 p-4 rounded-lg' style='width: 700px;'>

    <div data-part='handle' class='window_title bg-yellow-600 rounded-t-lg py-2 px-4 flex justify-between items-center'>
      <div data-part='title' class='text-lg font-semibold text-gray-800'>Synth Keyboard</div>
      <button class="icon close_dark hint--left text-gray-800" aria-label="Close (ESC)" data-close>×</button>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>

      <div class="flex justify-between mt-4 space-x-4">
    <button id="new-instrument" class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded-full">
        New Instrument
    </button>
    <button id="add-beats" class="bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded-full">
        Add Beats
    </button>
</div>
<div id="pitch-detection-display" class="mt-4 text-center">
    <p id="note">Note: -</p>
    <p id="frequency">Frequency: -</p>
</div>

        <div class="flex justify-center mt-4">
    <div class="timeline-container relative w-full">
        <div class="relative flex flex-col w-full">
            <div id="instruments" class="instruments bg-gray-800 border border-gray-700 p-2 rounded-t flex flex-col space-y-2 absolute left-0 z-10 pb-4">
                <!-- Tempo and instrument names will be dynamically generated -->
            </div>
            <div class="overflow-x-auto p-2 bg-gray-800 border border-gray-700 rounded-r" style="margin-left: 120px;">
                <div id="timeline" class="timeline flex flex-col space-y-2" style="width: max-content;">
                    <!-- Timeline content will be dynamically generated -->
                </div>
            </div>
        </div>
    </div>
</div>


<!-- Controls Section -->
<div class="flex justify-between items-center mt-4 mb-2">
    <div class="flex space-x-4">
        <select id="instrument-select" class="bg-gray-200 p-2">
            <option value="" disabled selected>Select Instrument</option>
        </select>
    </div>
    <div class="flex space-x-4 items-center">
        <button id="play" class="control-button bg-gray-700 text-white px-4 py-2 m-1 rounded">Play</button>
        <button id="pause" class="control-button bg-gray-700 text-white px-4 py-2 m-1 rounded">Pause</button>
        <button id="stop" class="control-button bg-gray-700 text-white px-4 py-2 m-1 rounded">Stop</button>
        <button id="loop" class="control-button bg-gray-700 text-white px-4 py-2 m-1 rounded">Loop</button>
        <button id="save" class="control-button bg-gray-700 text-white px-4 py-2 m-1 rounded">Save</button>
    </div>
    <div class="flex space-x-2 items-center">
    <button id="octave-down" class="bg-green-600 border border-black text-white px-2 py-2 rounded">-</button>
    <div id="octave-range" class="text-gray-800 font-bold text-md whitespace-nowrap">C4 - B5</div>
    <button id="octave-up" class="bg-green-600 border border-black text-white px-2 py-2 rounded">+</button>
</div>

</div>

        <div class="flex justify-center">
          <div class="keyboard-container relative flex cursor-pointer">

            <!-- Keys structure -->
            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="C4"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="C#4"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="D4"></div>
              <div class="black-key bg-black border border-black h-24 z-10 w-8 absolute top-0 left-10" data-note="D#4"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="E4"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="F4"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="F#4"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="G4"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="G#4"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="A4"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="A#4"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="B4"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="C5"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="C#5"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="D5"></div>
              <div class="black-key bg-black border border-black h-24 z-10 w-8 absolute top-0 left-10" data-note="D#5"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="E5"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="F5"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="F#5"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="G5"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="G#5"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="A5"></div>
              <div class="black-key bg-black border border-black h-24 w-8 z-10 absolute top-0 left-10" data-note="A#5"></div>
            </div>

            <div class="relative">
              <div class="white-key bg-white border border-black h-40 w-14" data-note="B5"></div>
            </div>
            
          </div>
        </div>

        <div class="flex justify-center mt-4 space-x-6">
          <div class="flex flex-col items-center">
            <label for="volume" class="text-gray-800 mb-2">Volume</label>
            <div id="volume-knob" class="knob relative bg-gray-800 text-white flex items-center justify-center rounded-full cursor-pointer" data-value="0.5" data-min="0" data-max="1" data-step="0.01" style="width: 50px; height: 50px;">
              <div class="indicator absolute w-1 bg-white top-1 left-1/2 transform -translate-x-1/2" style="height: 10px;"></div>
            </div>
            <div class="value text-center text-sm mt-2"></div>
          </div>

          <div class="flex flex-col items-center">
            <label for="attack" class="text-gray-800 mb-2">Attack</label>
            <div id="attack-knob" class="knob relative bg-gray-800 text-white flex items-center justify-center rounded-full cursor-pointer" data-value="0.01" data-min="0" data-max="1" data-step="0.01" style="width: 50px; height: 50px;">
              <div class="indicator absolute w-1 bg-white top-1 left-1/2 transform -translate-x-1/2" style="height: 10px;"></div>
            </div>
            <div class="value text-center text-sm mt-2"></div>
          </div>

          <div class="flex flex-col items-center">
            <label for="decay" class="text-gray-800 mb-2">Decay</label>
            <div id="decay-knob" class="knob relative bg-gray-800 text-white flex items-center justify-center rounded-full cursor-pointer" data-value="0.2" data-min="0" data-max="1" data-step="0.01" style="width: 50px; height: 50px;">
              <div class="indicator absolute w-1 bg-white top-1 left-1/2 transform -translate-x-1/2" style="height: 10px;"></div>
            </div>
            <div class="value text-center text-sm mt-2"></div>
          </div>

          <div class="flex flex-col items-center">
            <label for="sustain" class="text-gray-800 mb-2">Sustain</label>
            <div id="sustain-knob" class="knob relative bg-gray-800 text-white flex items-center justify-center rounded-full cursor-pointer" data-value="0.5" data-min="0" data-max="1" data-step="0.01" style="width: 50px; height: 50px;">
              <div class="indicator absolute w-1 bg-white top-1 left-1/2 transform -translate-x-1/2" style="height: 10px;"></div>
            </div>
            <div class="value text-center text-sm mt-2"></div>
          </div>

          <div class="flex flex-col items-center">
            <label for="release" class="text-gray-800 mb-2">Release</label>
            <div id="release-knob" class="knob relative bg-gray-800 text-white flex items-center justify-center rounded-full cursor-pointer" data-value="0.2" data-min="0" data-max="1" data-step="0.01" style="width: 50px; height: 50px;">
              <div class="indicator absolute w-1 bg-white top-1 left-1/2 transform -translate-x-1/2" style="height: 10px;"></div>
            </div>
            <div class="value text-center text-sm mt-2"></div>
          </div>

          <div class="flex flex-col items-center">
            <label for="oscillator" class="text-gray-800 mb-2">Oscillator</label>
            <div id="oscillator-knob" class="knob relative bg-gray-800 text-white flex items-center justify-center rounded-full cursor-pointer" data-value="1" data-min="1" data-max="6" data-step="1" style="width: 50px; height: 50px;">
              <div class="indicator absolute w-1 bg-white top-1 left-1/2 transform -translate-x-1/2" style="height: 10px;"></div>
            </div>
            <div class="value text-center text-sm mt-2"></div>
          </div>
        </div>

      </div>
    </div>

    <script>
var synth_window = {
    octave: 4,
    instruments: [],
    selectedInstrumentIndex: null,
    timeline: [],
    isRecording: false,
    oscillatorTypes: ['sine', 'square', 'sawtooth', 'triangle', 'pulse', 'noise'],
    currentBeat: 0, // Track the current beat being played
    beatInterval: null, // Store the interval for clearing later
    isLooping: false, // Track if looping is enabled
    totalBeats: 16,
    chunkSize: 16,
    tempos: [], // Store tempo changes

    start: function() {
        audio.createChannel('synth');
    audio.setVolume('synth', 0.5);

    this.initKeyboard();
    this.initKnobs();
    this.initOctaveButtons();
    this.initNewInstrumentButton();
    this.initAddBeatsButton();
    this.initInstrumentSelect();
    this.initTimelineControls();
    this.createDefaultInstrument();
    this.createTempoTrack();
    this.updateOctaveRangeDisplay();

    },

    createDefaultInstrument: function() {
    const defaultInstrument = {
        volume: 0.5,
        attack: 0.01,
        decay: 0.2,
        sustain: 0.5,
        sustain_time: 0.5, // Add sustain time
        release: 0.2,
        oscillator: 1,
        envelope: {
            attack_time: 0.01,
            attack_gain: 1,
            decay_time: 0.2,
            sustain_gain: 0.5,
            sustain_time: 0.5, // Add sustain time
            release_time: 0.2
        }
    };
    this.instruments.push(defaultInstrument);
    this.selectedInstrumentIndex = 0;
    this.updateInstrumentSelect();
    this.loadSelectedInstrument();
    this.createTimelineBoxes(); // Ensure default instrument bar is created
    this.createInstrumentChannel(this.selectedInstrumentIndex); // Create and route channel
},

    createTempoTrack: function() {
        // Create the tempo track only once
        const tempoInstrumentName = document.createElement('div');
        tempoInstrumentName.className = 'text-white bg-gray-700 p-2 rounded whitespace-nowrap'; // Prevent wrapping
        tempoInstrumentName.textContent = 'Tempo';
        document.getElementById('instruments').prepend(tempoInstrumentName);

        const tempoRow = document.createElement('div');
        tempoRow.className = 'flex items-center space-x-2 tempo-row';

        for (let i = 0; i < this.totalBeats; i++) {
            const tempoBox = document.createElement('div');
            tempoBox.className = 'tempo-box bg-gray-700 border border-gray-600 h-10 w-16 flex items-center justify-center text-white cursor-pointer rounded-lg';
            tempoBox.dataset.index = i;
            tempoBox.addEventListener('click', (e) => this.handleTempoBoxClick(e, tempoBox));
            tempoRow.appendChild(tempoBox);
        }
        document.getElementById('timeline').prepend(tempoRow);
    },

    unmount: function() {
        audio.removeChannel('synth');
    },

    initKeyboard: function() {
        this.updateKeyboardNotes();
        document.querySelectorAll('.white-key, .black-key').forEach(key => {
            key.addEventListener('click', () => {
                const note = key.getAttribute('data-note');
                console.log('Key pressed:', note); // Output log
                if (note) {
                    try {
                        if (!audio.audioContext) {
                            audio.start();
                        }
                        this.playNoteAndRecord(note); // Play only the selected instrument
                        this.inputNotesToSelectedBeats(note); // Input note to selected beats
                    } catch (error) {
                        console.error('Error playing note:', note, error); // Error log
                    }
                } else {
                    console.error('No note found for this key.'); // Error log
                }
            });
        });
    },

    updateKeyboardNotes: function() {
    const whiteKeys = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
    const blackKeys = ['C#', 'D#', 'F#', 'G#', 'A#'];
    let currentOctave = this.octave;
    document.querySelectorAll('.white-key').forEach((key, index) => {
        const note = whiteKeys[index % 7] + currentOctave;
        if (currentOctave === 0 && index === 0) {
            key.setAttribute('data-note', 'C0');
            key.innerText = 'C0';
        } else if (currentOctave === 0 && index === 1) {
            key.setAttribute('data-note', 'B0');
            key.innerText = 'B0';
        } else {
            key.setAttribute('data-note', note);
            key.innerText = note; // Display note text on the key
        }
        key.style.textAlign = 'center'; // Center text horizontally
        key.style.paddingTop = '70%'; // Add padding to position text at the bottom
        key.style.fontSize = '12px'; // Adjust font size if needed
        if (index % 7 === 6) {
            currentOctave++;
        }
    });
    currentOctave = this.octave;
    document.querySelectorAll('.black-key').forEach((key, index) => {
        const note = blackKeys[index % 5] + currentOctave;
        key.setAttribute('data-note', note);
        key.innerText = note; // Display note text on the key
        key.style.textAlign = 'center'; // Center text horizontally
        key.style.paddingTop = '60%'; // Add padding to position text at the bottom
        key.style.color = 'white'; // Change text color to white
        key.style.fontSize = '12px'; // Adjust font size if needed
        if ((index + 1) % 5 === 0) {
            currentOctave++;
        }
    });
},

initOctaveButtons: function() {
    document.getElementById('octave-up').addEventListener('click', () => {
        if (this.octave < 6) { // Maximum octave range check (C6-B7, next step would be C8)
            this.octave += 2;
            this.updateKeyboardNotes();
            this.updateOctaveRangeDisplay();
        }
    });
    document.getElementById('octave-down').addEventListener('click', () => {
        if (this.octave > 0) { // Minimum octave range check (A0-B1, next step would be C2)
            this.octave -= 2;
            this.updateKeyboardNotes();
            this.updateOctaveRangeDisplay();
        }
    });
},

updateOctaveRangeDisplay: function() {
    const startNote = this.octave === 0 ? 'C0' : `C${this.octave}`;
    const endNote = this.octave === 7 ? 'B7' : `B${this.octave + 1}`;
    document.getElementById('octave-range').textContent = `${startNote} - ${endNote}`;
},

    initKnobs: function() {
        const updateInstrumentConfig = (knobId, value) => {
            if (this.selectedInstrumentIndex !== null) {
                this.instruments[this.selectedInstrumentIndex][knobId] = value;
                if (knobId === 'oscillator') {
                    document.getElementById('oscillator-knob').nextElementSibling.textContent = this.oscillatorTypes[Math.round(value) - 1]; // Update oscillator display
                }
            }
        };

        document.querySelectorAll('.knob').forEach(knob => {
            let value = parseFloat(knob.dataset.value);
            let min = parseFloat(knob.dataset.min);
            let max = parseFloat(knob.dataset.max);
            let step = parseFloat(knob.dataset.step);

            let indicator = knob.querySelector('.indicator');
            if (!indicator) {
                indicator = document.createElement('div');
                indicator.className = 'indicator';
                knob.appendChild(indicator);
            }

            let valueDisplay = knob.nextElementSibling;
            if (!valueDisplay) {
                valueDisplay = document.createElement('div');
                valueDisplay.className = 'value text-center text-sm mt-2';
                knob.parentNode.appendChild(valueDisplay);
            }
            valueDisplay.textContent = knob.id === 'oscillator-knob' ? this.oscillatorTypes[Math.round(value) - 1] : value.toFixed(2);

            knob.addEventListener('mousedown', (event) => {
                let startValue = parseFloat(knob.dataset.value);
                let startX = event.clientX;
                let startY = event.clientY;
                let startAngle = (startValue - min) / (max - min) * 270 - 135;

                const onMouseMove = (event) => {
                    let deltaX = event.clientX - startX;
                    let deltaY = event.clientY - startY;
                    let deltaDistance = Math.sqrt(deltaX * deltaX + deltaY * deltaY);
                    let deltaAngle = deltaDistance * 1.5;

                    if (deltaY > 0) deltaAngle = -deltaAngle;

                    let newAngle = startAngle + deltaAngle;
                    if (newAngle < -135) newAngle = -135;
                    if (newAngle > 135) newAngle = 135;
                    let newValue = min + ((newAngle + 135) / 270) * (max - min);
                    newValue = Math.round(newValue / step) * step;
                    synth_window.updateKnobRotation(knob, newValue);
                    knob.dataset.value = newValue.toFixed(2);
                    valueDisplay.textContent = knob.id === 'oscillator-knob' ? synth_window.oscillatorTypes[Math.round(newValue) - 1] : newValue.toFixed(2); // Update display with correct context
                    updateInstrumentConfig(knob.id.replace('-knob', ''), newValue);
                };

                const onMouseUp = () => {
                    document.removeEventListener('mousemove', onMouseMove);
                    document.removeEventListener('mouseup', onMouseUp);
                    document.body.style.userSelect = '';
                    knob.style.cursor = 'grab';
                };

                document.addEventListener('mousemove', onMouseMove);
                document.addEventListener('mouseup', onMouseUp);
                document.body.style.userSelect = 'none';
                knob.style.cursor = 'grabbing';
            });

            synth_window.updateKnobRotation(knob, value);
        });
    },

    initNewInstrumentButton: function() {
        document.getElementById('new-instrument').addEventListener('click', () => {
            if (this.instruments.length >= 5) {
                alert('You have reached the maximum number of instruments.');
                return;
            }
            const newInstrument = {
                volume: parseFloat(document.getElementById('volume-knob').dataset.value),
                attack: parseFloat(document.getElementById('attack-knob').dataset.value),
                decay: parseFloat(document.getElementById('decay-knob').dataset.value),
                sustain: parseFloat(document.getElementById('sustain-knob').dataset.value),
                release: parseFloat(document.getElementById('release-knob').dataset.value),
                oscillator: parseInt(document.getElementById('oscillator-knob').dataset.value),
                envelope: {
                    attack_time: parseFloat(document.getElementById('attack-knob').dataset.value),
                    attack_gain: 1,
                    decay_time: parseFloat(document.getElementById('decay-knob').dataset.value),
                    sustain_gain: parseFloat(document.getElementById('sustain-knob').dataset.value),
                    release_time: parseFloat(document.getElementById('release-knob').dataset.value)
                }
            };
            this.instruments.push(newInstrument);
            console.log('New instrument created:', newInstrument);
            this.selectedInstrumentIndex = this.instruments.length - 1;
            this.updateInstrumentSelect();
            this.loadSelectedInstrument();
            this.createTimelineBoxes(); // Create new timeline row for the new instrument
            this.createInstrumentChannel(this.selectedInstrumentIndex); // Create and route channel
        });
    },

    createInstrumentChannel: function(index) {
        const channelName = `instr-${index}`;
        audio.createChannel(channelName); // Create a channel for the instrument
        audio.routeChannel(channelName, 'synth'); // Route instrument channel to master synth channel
    },

    initAddBeatsButton: function() {
    document.getElementById('add-beats').addEventListener('click', () => {
        this.totalBeats += 4; // Add 4 more beats
        // Update tempo track
        const tempoRow = document.querySelector('.tempo-row');
        for (let i = this.totalBeats - 4; i < this.totalBeats; i++) {
            const tempoBox = document.createElement('div');
            tempoBox.className = 'tempo-box bg-gray-700 border border-gray-600 h-10 w-16 flex items-center justify-center text-white cursor-pointer rounded-lg';
            tempoBox.dataset.index = i;
            tempoBox.addEventListener('click', (e) => this.handleTempoBoxClick(e, tempoBox));
            tempoRow.appendChild(tempoBox);
        }
        this.createTimelineBoxes(); // Update the timeline UI
    });
},

    initInstrumentSelect: function() {
        document.getElementById('instrument-select').addEventListener('change', (event) => {
            this.selectedInstrumentIndex = parseInt(event.target.value);
            this.loadSelectedInstrument();
        });
    },

    updateInstrumentSelect: function() {
        const instrumentSelect = document.getElementById('instrument-select');
        instrumentSelect.innerHTML = '<option value="" disabled>Select Instrument</option>';
        this.instruments.slice(0, 5).forEach((instrument, index) => { // Limit to 5 instruments
            const option = document.createElement('option');
            option.value = index;
            option.textContent = `Instr ${index + 1}`;
            instrumentSelect.appendChild(option);
        });
        if (this.selectedInstrumentIndex !== null && this.selectedInstrumentIndex < 5) {
            instrumentSelect.value = this.selectedInstrumentIndex;
        } else {
            instrumentSelect.selectedIndex = 0;
        }
    },

    initTimelineControls: function() {
        document.getElementById('play').addEventListener('click', () => {
            this.playTimeline();
        });
        document.getElementById('pause').addEventListener('click', () => {
            this.pauseTimeline();
        });
        document.getElementById('stop').addEventListener('click', () => {
            this.stopTimeline();
        });
        document.getElementById('loop').addEventListener('click', () => {
            this.toggleLoop();
        });
        document.getElementById('save').addEventListener('click', () => {
            this.saveTimeline();
        });
        this.createTimelineBoxes();
        this.addKeyboardInput();
    },

    handleTempoBoxClick: function(e, box) {
    const index = parseInt(box.dataset.index);
    const newTempo = prompt('Enter new tempo:');
    if (newTempo) {
        this.tempos[index] = parseInt(newTempo);
        box.textContent = `${newTempo}`;
        box.classList.add('active');
        this.instruments.forEach((_, instIndex) => {
            audio.setChannelTempo(`instr-${instIndex}`, newTempo);
        });
    }
},

    createTimelineBoxes: function() {
    const timeline = document.getElementById('timeline');
    const instruments = document.getElementById('instruments');
    // Remove only instrument rows, not the tempo track
    while (instruments.children.length > 1) {
        instruments.removeChild(instruments.lastChild);
    }
    while (timeline.children.length > 1) {
        timeline.removeChild(timeline.lastChild);
    }

    const beatWidth = 40; // Width of each beat box
    const totalWidth = this.totalBeats * beatWidth; // Calculate total width based on number of beats

    timeline.style.width = `${totalWidth}px`; // Set the width of the timeline container

    this.instruments.slice(0, 5).forEach((instrument, instIndex) => { // Limit to 5 instruments
        const instName = document.createElement('div');
        instName.className = 'text-white bg-gray-700 p-2 rounded whitespace-nowrap'; // Prevent wrapping
        instName.textContent = `Instr ${instIndex + 1}`;
        instruments.appendChild(instName);

        const row = document.createElement('div');
        row.className = 'flex items-center space-x-2';

        for (let i = 0; i < this.totalBeats; i++) {
            const box = document.createElement('div');
            box.className = 'timeline-box bg-gray-700 border border-gray-600 h-10 w-16 flex items-center justify-center text-white cursor-pointer rounded-lg';
            box.dataset.index = i;
            box.dataset.instIndex = instIndex;
            box.addEventListener('click', (e) => this.handleTimelineBoxClick(e, box));
            box.addEventListener('contextmenu', (e) => this.handleTimelineBoxRightClick(e, box));
            row.appendChild(box);
        }
        timeline.appendChild(row);
    });

    this.updateTimelineUI();
},

    handleTimelineBoxClick: function(e, box) {
    box.classList.toggle('active');
    box.classList.toggle('border-blue-500');
},

    handleTimelineBoxRightClick: function(e, box) {
        e.preventDefault();
        const index = parseInt(box.dataset.index);
        const instIndex = parseInt(box.dataset.instIndex);
        this.removeNoteFromTimeline(index, instIndex);
        box.textContent = '';
        box.classList.remove('active'); // Deactivate the box after removing the note
        box.classList.remove('border-blue-500'); // Remove the blue border
    },

    removeNoteFromTimeline: function(index, instIndex) {
        const currentPattern = this.getCurrentPattern();
        currentPattern.pattern_commands = currentPattern.pattern_commands.filter(
            command => !(command.tick === index && command.inst === instIndex)
        );
        console.log(`Note removed from timeline at index ${index} for instrument ${instIndex}`); // Output log
        this.updateTimelineUI();
    },

    addKeyboardInput: function() {
        document.querySelectorAll('.white-key, .black-key').forEach(key => {
            key.addEventListener('click', () => {
                document.querySelectorAll('.white-key, .black-key').forEach(k => k.classList.remove('active'));
                key.classList.add('active');
            });
        });
    },

    updateScrollbarPosition: function() {
        const timelineContainer = document.querySelector('.overflow-x-auto');
        const boxWidth = 40; // Width of each timeline box

        // Calculate the measure number (0-based index)
        const measureNumber = Math.floor(this.currentBeat / this.chunkSize);
        // Calculate the scroll position based on the measure number and the incremental offset
        const scrollPosition = (measureNumber * this.chunkSize + 15 - measureNumber) * boxWidth;
        timelineContainer.scrollLeft = scrollPosition;
    },

    resetScrollbarPosition: function() {
        const timelineContainer = document.querySelector('.overflow-x-auto');
        timelineContainer.scrollLeft = 0;
    },

    updateTimelineUI: function() {
        // Clear existing notes in the timeline UI
        document.querySelectorAll('.timeline-box').forEach(box => {
            box.textContent = '';
        });

        // Populate the timeline UI with notes from the current state
        this.timeline.forEach((pattern) => {
            pattern.pattern_commands.forEach(command => {
                const noteDiv = document.querySelector(`.timeline-box[data-index='${command.tick}'][data-inst-index='${command.inst}']`);
                if (noteDiv) {
                    noteDiv.textContent = command.note;
                }
            });
        });

        // Populate the tempo track UI with tempos from the current state
        document.querySelectorAll('.tempo-box').forEach((box, index) => {
            if (this.tempos[index] !== undefined) {
                box.textContent = `${this.tempos[index]}`;
                box.classList.add('active');
            } else {
                box.textContent = '';
                box.classList.remove('active');
            }
        });
    },
    

    playTimeline: function() {
    document.getElementById('play').disabled = true;

    this.currentBeat = 0; // Reset to the first beat
    this.initPlayTimelineHighlight(); // Initialize by clearing any existing highlights

    let currentBpm = 120; // Set your initial BPM here
    const ticksPerBeat = 4; // Set ticks per beat
    let currentTickDuration = (60 / currentBpm) * 1000 / ticksPerBeat; // Calculate duration of each tick in milliseconds
    const startTime = audio.audioContext.currentTime; // Reference start time for synchronization

    const updateInstrumentSettings = () => {
        this.instruments.forEach((instrument, index) => {
            // Update instrument settings from knob values
            instrument.volume = parseFloat(document.getElementById('volume-knob').dataset.value);
            instrument.attack = parseFloat(document.getElementById('attack-knob').dataset.value);
            instrument.decay = parseFloat(document.getElementById('decay-knob').dataset.value);
            instrument.sustain = parseFloat(document.getElementById('sustain-knob').dataset.value);
            instrument.release = parseFloat(document.getElementById('release-knob').dataset.value);
            instrument.oscillator = parseInt(document.getElementById('oscillator-knob').dataset.value);

            // Ensure the envelope is updated with the current values
            instrument.envelope.attack_time = instrument.attack;
            instrument.envelope.decay_time = instrument.decay;
            instrument.envelope.sustain_gain = instrument.sustain;
            instrument.envelope.release_time = instrument.release;
        });
    };

    const playAudio = () => {
        const jsonData = {
            bpm: currentBpm,
            ticks_per_beat: ticksPerBeat,
            instruments: this.instruments.map(instr => ({
                oscillator: instr.oscillator,
                envelope: instr.envelope
            })),
            patterns: this.timeline.map(pattern => ({
                ticks_per_beat: ticksPerBeat,
                bpm: currentBpm,
                ticks_per_pattern: this.totalBeats, // Use dynamic totalBeats
                pattern_commands: pattern.pattern_commands
            })),
            tempos: this.tempos
        };

        console.log('Sending JSON data to audio object:', JSON.stringify(jsonData));
        audio.play(jsonData, 'synth');
    };

    const scheduleNextBeat = () => {
        if (this.tempos[this.currentBeat] !== undefined) {
            currentBpm = this.tempos[this.currentBeat];
            currentTickDuration = (60 / currentBpm) * 1000 / ticksPerBeat;
            console.log(`Tempo changed to ${currentBpm} BPM at beat ${this.currentBeat}`);
            this.instruments.forEach((_, index) => {
                audio.setChannelTempo(`instr-${index}`, currentBpm);
            });
        }

        const elapsedTime = audio.audioContext.currentTime - startTime;
        const expectedBeatTime = (this.currentBeat * currentTickDuration) / 1000;

        setTimeout(() => {
            this.highlightCurrentBeat(this.currentBeat, currentBpm);
            console.log(`Highlighting beat ${this.currentBeat} at time ${audio.audioContext.currentTime}`);
        }, Math.max(0, (expectedBeatTime - elapsedTime) * 1000));

        this.currentBeat++;
        if (this.currentBeat < this.totalBeats) {
            this.beatInterval = setTimeout(scheduleNextBeat, currentTickDuration);
        } else if (this.isLooping) {
            updateInstrumentSettings(); // Update settings before restarting the loop
            this.currentBeat = 0;
            this.resetScrollbarPosition();
            audio.stopAllSounds('synth');
            playAudio(); // Restart playback with updated settings
            this.beatInterval = setTimeout(scheduleNextBeat, currentTickDuration);
        } else {
            this.stopTimeline();
        }
    };

    updateInstrumentSettings(); // Initial update before playback starts
    playAudio(); // Initial playback
    this.beatInterval = setTimeout(scheduleNextBeat, 0); // Start immediately to synchronize with audio
},

highlightCurrentBeat: function(beat, currentBpm) {
    const timelineRows = document.querySelectorAll('#timeline > div');
    timelineRows.forEach(row => {
        const box = row.querySelector(`.timeline-box[data-index='${beat}'], .tempo-box[data-index='${beat}']`);
        if (box) {
            box.classList.add('highlight');
        }
    });
    setTimeout(() => {
        this.unhighlightCurrentBeat(beat);
    }, (60 / currentBpm) * 1000 / 4); // Unhighlight after the duration of one beat
},

    stopTimeline: function() {
        clearTimeout(this.beatInterval); // Stop the beat highlight interval
        this.initPlayTimelineHighlight(); // Clear any highlights when stopped
        this.stopAllInstruments(); // Stop all instrument channels
        document.getElementById('play').disabled = false; // Re-enable play button
    },

    stopAllInstruments: function() {
        this.instruments.forEach((_, index) => {
            const channelName = `instr-${index}`;
            audio.stopAllSounds(channelName); // Use the stopAllSounds method to stop each instrument channel
        });
    },

    toggleLoop: function() {
        this.isLooping = !this.isLooping;
        const loopButton = document.getElementById('loop');
        loopButton.classList.toggle('active', this.isLooping); // Update button appearance
        if (this.isLooping) {
            loopButton.classList.add('flashing'); // Add flashing class if looping
        } else {
            loopButton.classList.remove('flashing'); // Remove flashing class if not looping
        }
    },

    saveTimeline: function() {
    const jsonData = {
        num_channels: this.instruments.length,
        instruments: this.instruments.map(instr => ({
            oscillator: instr.oscillator,
            envelope: instr.envelope
        })),
        patterns: this.timeline.map(pattern => ({
            ticks_per_beat: audio.ticksPerBeat,
            bpm: audio.bpm, // Set the BPM here
            ticks_per_pattern: this.totalBeats,
            pattern_commands: pattern.pattern_commands
        })),
        tempos: this.tempos // Include the tempos in the saved data
    };

    console.log(JSON.stringify(jsonData));
},

playNoteAndRecord: function(note) {
    try {
        if (!audio.audioContext) {
            audio.start();
        }
        const instrument = this.instruments[this.selectedInstrumentIndex];

        // Retrieve current knob values and update the instrument configuration
        instrument.volume = parseFloat(document.getElementById('volume-knob').dataset.value);
        instrument.attack = parseFloat(document.getElementById('attack-knob').dataset.value);
        instrument.decay = parseFloat(document.getElementById('decay-knob').dataset.value);
        instrument.sustain = parseFloat(document.getElementById('sustain-knob').dataset.value);
        instrument.sustain_time = 0.5; // Define a sustain time if not available from UI
        instrument.release = parseFloat(document.getElementById('release-knob').dataset.value);
        instrument.oscillator = parseInt(document.getElementById('oscillator-knob').dataset.value);

        // Ensure the envelope is updated with the current values
        instrument.envelope.attack_time = instrument.attack;
        instrument.envelope.decay_time = instrument.decay;
        instrument.envelope.sustain_gain = instrument.sustain;
        instrument.envelope.sustain_time = 0.5; // Define a sustain time if not available from UI
        instrument.envelope.release_time = instrument.release;

        console.log('Playing note:', note, 'at time:', audio.audioContext.currentTime); // Log the note being played
        const channelName = `instr-${this.selectedInstrumentIndex}`;

        audio.playNote('noteId', instrument, note, audio.audioContext.currentTime, channelName); // Route note to its instrument channel
    } catch (error) {
        console.error('Error playing note:', note, error);
    }
},

    inputNotesToSelectedBeats: function(note) {
    const selectedBoxes = document.querySelectorAll('.timeline-box.active');
    selectedBoxes.forEach(box => {
        const index = parseInt(box.dataset.index);
        const instIndex = parseInt(box.dataset.instIndex);
        this.addNoteToTimeline(note, index, instIndex);
        box.textContent = note;
        box.classList.remove('active'); // Deselect after adding the note
        box.classList.remove('border-blue-500'); // Remove the blue border
    });
},

    addNoteToTimeline: function(note, index = null, instIndex = null) {
        const currentPattern = this.getCurrentPattern();
        if (index === null) {
            index = currentPattern.pattern_commands.length % this.totalBeats;
        }
        if (instIndex === null) {
            instIndex = this.selectedInstrumentIndex;
        }
        currentPattern.pattern_commands.push({
            inst: instIndex,
            note: note,
            tick: index
        });
        console.log('Note added to timeline:', { note, index, instIndex }); // Output log
        this.updateTimelineUI();
    },

    getCurrentPattern: function() {
        if (this.timeline.length === 0 || this.timeline[this.timeline.length - 1].pattern_commands.length >= this.totalBeats) {
            const newPattern = {
                bpm: audio.bpm,
                ticks_per_beat: audio.ticksPerBeat,
                ticks_per_pattern: this.totalBeats,
                pattern_commands: []
            };
            this.timeline.push(newPattern);
        }
        return this.timeline[this.timeline.length - 1];
    },

    loadSelectedInstrument: function() {
        if (this.selectedInstrumentIndex !== null) {
            const instrument = this.instruments[this.selectedInstrumentIndex];
            document.getElementById('volume-knob').dataset.value = instrument.volume;
            document.getElementById('attack-knob').dataset.value = instrument.attack;
            document.getElementById('decay-knob').dataset.value = instrument.decay;
            document.getElementById('sustain-knob').dataset.value = instrument.sustain;
            document.getElementById('release-knob').dataset.value = instrument.release;
            document.getElementById('oscillator-knob').dataset.value = instrument.oscillator;
            document.getElementById('oscillator-knob').nextElementSibling.textContent = this.oscillatorTypes[instrument.oscillator - 1]; // Update oscillator display
            this.updateKnobRotation(document.getElementById('volume-knob'), instrument.volume);
            this.updateKnobRotation(document.getElementById('attack-knob'), instrument.attack);
            this.updateKnobRotation(document.getElementById('decay-knob'), instrument.decay);
            this.updateKnobRotation(document.getElementById('sustain-knob'), instrument.sustain);
            this.updateKnobRotation(document.getElementById('release-knob'), instrument.release);
            this.updateKnobRotation(document.getElementById('oscillator-knob'), instrument.oscillator);
        }
    },

    updateKnobRotation: function(knob, value) {
        let min = parseFloat(knob.dataset.min);
        let max = parseFloat(knob.dataset.max);
        let angle = (value - min) / (max - min) * 270 - 135;
        knob.style.transform = `rotate(${angle}deg)`;
    },

    unhighlightCurrentBeat: function(beat) {
    const timelineRows = document.querySelectorAll('#timeline > div');
    timelineRows.forEach(row => {
        const box = row.querySelector(`.timeline-box[data-index='${beat}'], .tempo-box[data-index='${beat}']`);
        if (box) {
            box.classList.remove('highlight');
        }
    });
},

initPlayTimelineHighlight: function() {
    const timelineBoxes = document.querySelectorAll('.timeline-box, .tempo-box');
    timelineBoxes.forEach(box => {
        box.classList.remove('highlight');
    });
}
};

synth_window.start();
</script>

<style>
.timeline-container {
    display: flex;
    align-items: flex-start;
    width: 100%;
    position: relative;
}

.instruments {
    width: 120px; /* Width for instrument names */
    padding-bottom: 24px; /* Add padding to match the height of the scrollbar */
}

.timeline {
    display: flex;
    flex: 1;
    max-height: 300px;
}

.highlight {
    background-color: green !important;
}
</style>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
