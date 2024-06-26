<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='audio_window' class='window window_bg' style='width: 330px; background: #242d41;'>

    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#5c6271 1px, transparent 0) !important;'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 mt-1 hint--left" aria-label="Close (ESC)" data-close></button>
      </div>
      <div data-part='title' class='title_bg window_border' style='background: #242d41; color: #ede8d6;'>Audio Channels</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
      <div class='container text-light window_body p-2'>
        
        <div id="test_tab">
          <div id="tabs" class="flex border-b border-gray-300">
            <button class="tab text-gray-800" data-tab="tab1">Tab 1</button>
            <button class="tab text-gray-800" data-tab="tab2">Tab 2</button>
            <button class="tab text-gray-800" data-tab="tab3">Tab 3</button>
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab1">
            <p>Content for Tab 1</p>
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab2">
            <!-- This will be dynamically populated -->
          </div>

          <div class="tab-content p-4 hidden" data-tab-content="tab3">
            <p>Content for Tab 3</p>
          </div>
        </div>

      </div>
    </div>

    <script>
      var audio_window = {
        start: function() {
          ui.initTabs('test_tab', 'tab2');

          // Initialize the audio context if it is not already initialized
          if (!audio.audioContext) {
            audio.start();
          }

          // Add UI for existing channels
          Object.keys(audio.channels).forEach(channel => {
            this.addChannelUI(channel);
          });

          // Listen for channel creation events
          document.addEventListener('channelCreated', (event) => {
            this.addChannelUI(event.detail.channel);
          });

          document.addEventListener('channelRemoved', (event) => {
            this.removeChannelUI(event.detail.channel);
          });

          // Initialize volume meters
          Object.keys(audio.channels).forEach(channel => {
            this.startVolumeMeterUpdates(channel);
          });
        },

        updateVolumeMeter: function(channel, volume) {
          var meter = document.getElementById(channel + '-volume-meter');
          var fill = meter.querySelector('.volume-meter-fill');
          fill.style.width = (volume * 100) + '%';
        },

        startVolumeMeterUpdates: function(channel) {
          if (!audio.channels[channel] && channel !== 'master') {
            return;
          }

          var analyser = audio.audioContext.createAnalyser();
          analyser.fftSize = 2048;

          if (channel === 'master') {
            audio.masterGain.connect(analyser);
          } else {
            audio.channels[channel].connect(analyser);
          }

          var dataArray = new Uint8Array(analyser.fftSize);

          function update() {
            analyser.getByteTimeDomainData(dataArray);
            var sum = 0;
            for (var i = 0; i < dataArray.length; i++) {
              var value = dataArray[i] / 128 - 1;
              sum += value * value;
            }
            var rms = Math.sqrt(sum / dataArray.length);
            var volume = Math.max(-100, 20 * Math.log10(rms));
            volume = (volume + 100) / 100; // Normalize to [0, 1]
            audio_window.updateVolumeMeter(channel, volume);
            requestAnimationFrame(update);
          }

          update();
        },

        playSounds: function() {
          // Define an instrument for synth sounds
          const instrument = {
            oscillator: 1,
            envelope: {
              attack_time: 0.01,
              attack_gain: 1,
              decay_time: 0.2,
              sustain_gain: 0.5,
              release_time: 0.2
            }
          };

          // Play a synth note on the 'music' channel
          audio.playNote("music_note_1", instrument, 'C4', audio.audioContext.currentTime, 'music');

          // Play a synth note on the 'sfx' channel
          audio.playNote("sfx_note_1", instrument, 'E4', audio.audioContext.currentTime, 'sfx');

          // Play an audio file on the 'music' channel
          audio.playAudio("music_1", 'assets/audio/music/season1_ending_credits_sequence.mp3', 'music');

          // Play an audio file on the 'sfx' channel
          audio.playAudio("music_sfx_1", 'assets/audio/sfx/music/music.mp3', 'sfx');
        },

        addChannelUI: function(channel) {
          // Check if the channel UI already exists
          if (document.getElementById(`${channel}-container`)) {
            return; // UI already exists, do nothing
          }

          // Create new UI elements for the new channel
          const container = document.createElement('div');
          container.id = `${channel}-container`;

          const label = document.createElement('label');
          label.htmlFor = `${channel}-volume`;
          label.className = 'block text-sm font-medium text-gray-100';
          label.textContent = `${channel.charAt(0).toUpperCase() + channel.slice(1)} Volume`;

          const input = document.createElement('input');
          input.type = 'range';
          input.id = `${channel}-volume`;
          input.min = '0';
          input.max = '1';
          input.step = '0.01';
          input.value = '0.7';
          input.className = 'w-full mt-1';

          const meterContainer = document.createElement('div');
          meterContainer.id = `${channel}-volume-meter`;
          meterContainer.className = 'w-full h-2 mt-2 bg-gray-900 rounded relative';

          const meterFill = document.createElement('div');
          meterFill.className = 'volume-meter-fill absolute left-0 top-0 h-full rounded bg-gradient-to-r from-green-400 via-yellow-400 via-orange-400 to-red-500';

          meterContainer.appendChild(meterFill);
          container.appendChild(label);
          container.appendChild(input);
          container.appendChild(meterContainer);

          document.querySelector('.tab-content[data-tab-content="tab2"]').appendChild(container);

          // Add event listener for the new volume control
          input.addEventListener('input', function() {
            audio.setVolume(channel, this.value);
          });

          // Start volume meter updates for the new channel
          this.startVolumeMeterUpdates(channel);
        },

        removeChannelUI: function(channel) {
          // Remove the UI elements for the removed channel
          const container = document.getElementById(`${channel}-container`);
          if (container) {
            container.remove();
          }
        },

        unmount: function() {
          ui.destroyTabs('test_tab');
        }
      }
      audio_window.start();
    </script>

    <div class='resize-handle'></div>
  </div>
<?php
}
?>
