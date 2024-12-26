var audio = {
    audioContext: null,
    masterGain: null,
    oscillatorTypes: ['sine', 'square', 'sawtooth', 'triangle', 'pulse', 'noise'],
    channels: {},
    sources: {},
    queues: {},
    lastPlayedTimes: {},
    defaultVolume: 1,
    channelTempos: {},
    isLoopingAudioPlaying: {},

    start: function() {
        if (!this.audioContext) {
            this.audioContext = new (window.AudioContext || window.webkitAudioContext)();
            this.masterGain = this.audioContext.createGain();
            this.masterGain.connect(this.audioContext.destination);
            this.masterGain.gain.value = 1;
            
            this.channels = {};
            this.sources = {};
    
            this.createChannel('master', localStorage.getItem('master-volume') || this.defaultVolume);
            this.createChannel('music', localStorage.getItem('music-volume') || this.defaultVolume);
            this.setVolume('music', localStorage.getItem('music-volume') || 0.05);
            this.createChannel('sfx', localStorage.getItem('sfx-volume') || this.defaultVolume);
            this.createChannel('ambience', localStorage.getItem('ambience-volume') || 0.5);
            
            console.log("Audio context initialized. Master, music, sfx channels created.");
        } else {
            console.log("Audio context already started.");
        }
    },
    

    pauseAll: function() {
        if (this.audioContext && this.audioContext.state === 'running') {
            this.audioContext.suspend().then(() => {
                console.log('Audio context suspended.');
            });
        }
    },

    resumeAll: function() {
        if (this.audioContext && this.audioContext.state === 'suspended') {
            this.audioContext.resume().then(() => {
                console.log('Audio context resumed.');
            });
        }
    },

    stopAllSounds: function(channel) {
        if (this.sources[channel]) {
            this.sources[channel].forEach(source => source.stop());
            this.sources[channel] = [];
        }
    },

    setChannelTempo: function(channel, bpm) {
        this.channelTempos[channel] = bpm;
    },

    play: function(params, channel = 'synth') {
        const { bpm, ticks_per_beat, instruments, patterns, tempos } = params;
        let currentBpm = bpm;
        let startTime = this.audioContext.currentTime;

        console.log('Playing with BPM:', bpm, 'Ticks per beat:', ticks_per_beat, 'Tempos:', tempos, 'Channel:', channel);

        patterns.forEach(pattern => {
            pattern.pattern_commands.forEach(command => {
                const instrument = instruments[command.inst];
                const channelName = `instr-${command.inst}`;

                if (tempos && tempos[command.tick] !== undefined) {
                    currentBpm = tempos[command.tick];
                    console.log(`Tempo changed to ${currentBpm} BPM at tick ${command.tick}`);
                }

                const beatDuration = 60 / currentBpm;
                const tickDuration = beatDuration / ticks_per_beat;
                const commandStartTime = startTime + (command.tick * tickDuration);

                console.log(`Scheduling note ${command.note} for instrument ${command.inst} at time ${commandStartTime}`);

                this.playNote(
                    `note-${command.inst}-${command.tick}-${commandStartTime}`,
                    instrument,
                    command.note,
                    commandStartTime,
                    channel
                );
            });

            startTime += pattern.pattern_commands.length * (60 / currentBpm) / ticks_per_beat;
        });
    },

    playNote: function(id, instrument, combinedNote, startTime, channel = 'master') {
        console.log(`playNote called with id: ${id}, combinedNote: ${combinedNote}, startTime: ${startTime}`);
        const [pitch, octave] = this.parseNote(combinedNote);
        if (pitch === null || octave === null) {
            console.error(`Invalid note: ${combinedNote}`);
            return;
        }
        const noteNumber = this.noteToNumber(pitch, octave);
        console.log(`Playing note: ${combinedNote}, Pitch: ${pitch}, Octave: ${octave}, Note Number: ${noteNumber}`);
        this._playNote(
            id, 
            instrument, 
            noteNumber, 
            startTime, 
            channel
        );
    },

    _playNote: function(id, instrument, noteNumber, startTime, channel) {
        console.log(`Playing note number: ${noteNumber} on channel ${channel} at time ${startTime}`);
        let oscillator = null;
        let gainNode = this.audioContext.createGain();
    
        if (!this.sources[channel]) {
            this.sources[channel] = [];
        }
    
        oscillator = this.audioContext.createOscillator();
        oscillator.type = this.oscillatorTypes[instrument.oscillator - 1] || 'sine';
        oscillator.frequency.value = this.calculateFrequency(noteNumber);
        console.log(`Oscillator frequency: ${oscillator.frequency.value} Hz`);
    
        if (instrument.filter) {
            const filter = this.audioContext.createBiquadFilter();
            filter.type = instrument.filter.type;
            filter.frequency.value = instrument.filter.frequency;
            filter.Q.value = instrument.filter.Q;
            oscillator.connect(filter);
            filter.connect(gainNode);
        } else {
            oscillator.connect(gainNode);
        }
    
        const analyser = this.audioContext.createAnalyser();
        analyser.fftSize = 2048;
        gainNode.connect(analyser);
    
        const envelopeNode = this.applyEnvelope(gainNode, instrument.envelope, startTime);
        envelopeNode.connect(this.channels[channel] || this.masterGain);
    
        oscillator.start(startTime);
        const noteDuration = instrument.envelope.attack_time + instrument.envelope.decay_time + instrument.envelope.release_time;
        oscillator.stop(startTime + noteDuration);
        this.sources[channel].push(oscillator);
    
        this.detectPitch(analyser);
    },

    playAudio: function(id, audioBuffer, channel = 'sfx', loop = false) {
        
        if (this.audioContext.state !== 'running') {
            console.log('Audio context not running, attempting to resume.');
            this.audioContext.resume().then(() => {
                console.log('Audio context resumed.');
                this.playAudio(id, audioBuffer, channel, loop);
            });
            return;
        }
    
        const isPlaying = this.sources[channel]?.some(source => source.loopId === id && source.looping);
        if (isPlaying) {
            return;
        }
    
        const source = this.audioContext.createBufferSource();
        source.buffer = audioBuffer;
        const gainNode = this.audioContext.createGain();
        source.connect(gainNode);
        gainNode.connect(this.channels[channel] || this.masterGain);
    
        if (loop) {
            source.loop = true;
            source.looping = true;
            source.gainNode = gainNode;
            source.loopId = id;
        }
    
        source.onended = () => {
            this.sources[channel] = this.sources[channel].filter(s => s !== source);
        };
    
        source.start();
    
        if (!this.sources[channel]) {
            this.sources[channel] = [];
        }
        this.sources[channel].push(source);
    },    

    processQueue: function(channel) {
        if (!this.queues[channel] || this.queues[channel].length === 0) {
            return;
        }

        const nextAudio = this.queues[channel].shift();
        if (nextAudio) {

            const source = this.audioContext.createBufferSource();
            source.buffer = nextAudio.audioBuffer;
            const gainNode = this.audioContext.createGain();
            source.connect(gainNode);
            gainNode.connect(this.channels[channel] || this.masterGain);

            if (nextAudio.loop) {
                source.loop = true;
                source.looping = true;
                source.gainNode = gainNode;
                source.loopId = nextAudio.id;
            }

            source.onended = () => {
                this.sources[channel] = this.sources[channel].filter(s => s !== source);
                this.processQueue(channel);
            };

            source.start();

            if (!this.sources[channel]) {
                this.sources[channel] = [];
            }
            this.sources[channel].push(source);
        }
    },

    stopLoopingAudio: function(id, channel, fadeDuration = 0.5) {
        if (this.sources[channel]) {
            this.sources[channel].forEach(source => {
                if (source.looping && source.loopId === id) {
                    const gainNode = source.gainNode;
                    const currentTime = this.audioContext.currentTime;
                    gainNode.gain.setValueAtTime(gainNode.gain.value, currentTime);
                    gainNode.gain.linearRampToValueAtTime(0, currentTime + fadeDuration);
                    source.loop = false;
                    source.stop(currentTime + fadeDuration);
                }
            });
            this.sources[channel] = this.sources[channel].filter(source => !source.looping || source.loopId !== id);
        }

        if (this.isLoopingAudioPlaying[channel]) {
            delete this.isLoopingAudioPlaying[channel][id];
        }
    },

    setVolume: function(channel, volume) {
        console.log(`setVolume called for channel: ${channel} with volume: ${volume}`);

        if (isNaN(volume) || volume === null || volume === undefined) {
            volume = this.defaultVolume;
        }

        if (channel === 'master') {
            if (this.masterGain) {
                console.log("Previous master gain value:", this.masterGain.gain.value);
                this.masterGain.gain.value = volume;
                console.log("New master gain value:", this.masterGain.gain.value);
            } else {
                console.error("Master gain not initialized");
            }
        } else {
            if (this.channels[channel]) {
                console.log(`Previous gain value for ${channel}:`, this.channels[channel].gain.value);
                this.channels[channel].gain.value = volume;
                console.log(`New gain value for ${channel}:`, this.channels[channel].gain.value);
            } else {
                console.error(`Channel ${channel} not initialized`);
            }
        }
    },

    createChannel: function(name, volume = this.defaultVolume) {
        if (!this.channels[name]) {
            const gainNode = this.audioContext.createGain();
            gainNode.connect(this.masterGain);
            gainNode.gain.value = volume;
            this.channels[name] = gainNode;
            const event = new CustomEvent('channelCreated', { detail: { channel: name } });
            document.dispatchEvent(event);
        }
    },

    removeChannel: function(channel) {
        if (!this.channels[channel]) {
            console.error(`Channel ${channel} does not exist`);
            return;
        }
        this.channels[channel].disconnect();
        delete this.channels[channel];
        console.log(`Channel ${channel} removed`);
        document.dispatchEvent(new CustomEvent('channelRemoved', { detail: { channel } }));
    },

    routeChannel: function(sourceChannel, destinationChannel) {
        if (!this.channels[sourceChannel] || !this.channels[destinationChannel]) {
            console.error(`Either source channel ${sourceChannel} or destination channel ${destinationChannel} does not exist`);
            return;
        }

        this.channels[sourceChannel].disconnect();
        this.channels[sourceChannel].connect(this.channels[destinationChannel]);
        console.log(`Channel ${sourceChannel} routed to ${destinationChannel}`);
    },

    parseNote: function(combinedNote) {
        const match = combinedNote.match(/([A-G]#?)(\d)/);
        if (match) {
            console.log(`Parsed note: ${match[1]}${match[2]}`);
            return [match[1], parseInt(match[2])];
        }
        return [null, null];
    },
    
    noteToNumber: function(pitch, octave) {
        const notes = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
        const noteIndex = notes.indexOf(pitch);
        return noteIndex + ((octave + 1) * 12);
    },
    calculateFrequency: function(noteNumber) {
        return 440 * Math.pow(2, (noteNumber - 69) / 12);
    },

    createNoiseBuffer: function() {
        const bufferSize = this.audioContext.sampleRate;
        const buffer = this.audioContext.createBuffer(1, bufferSize, this.audioContext.sampleRate);
        const output = buffer.getChannelData(0);
        for (let i = 0; i < bufferSize; i++) {
            output[i] = Math.random() * 2 - 1;
        }
        return buffer;
    },

    applyEnvelope: function(gainNode, envelope, startTime) {
        console.log("applyEnvelope called with gainNode, envelope:", envelope, "startTime:", startTime);
        const { attack_time, attack_gain, decay_time, sustain_gain, release_time } = envelope;
        gainNode.gain.setValueAtTime(0, startTime);
        gainNode.gain.linearRampToValueAtTime(attack_gain, startTime + attack_time);
        gainNode.gain.linearRampToValueAtTime(sustain_gain, startTime + attack_time + decay_time);
        const releaseStartTime = startTime + attack_time + decay_time;
        const releaseEndTime = releaseStartTime + release_time;
        gainNode.gain.setValueAtTime(sustain_gain, releaseStartTime);
        gainNode.gain.linearRampToValueAtTime(0, releaseEndTime);
        return gainNode;
    },

    detectPitch: function(analyser) {
        const bufferLength = analyser.fftSize;
        const dataArray = new Float32Array(bufferLength);

        const autoCorrelate = (buffer, sampleRate) => {
            let SIZE = buffer.length;
            let rms = 0;

            for (let i = 0; i < SIZE; i++) {
                let val = buffer[i];
                rms += val * val;
            }

            rms = Math.sqrt(rms / SIZE);
            if (rms < 0.01) return -1;

            let r1 = 0, r2 = SIZE - 1, thres = 0.2;
            for (let i = 0; i < SIZE / 2; i++)
                if (Math.abs(buffer[i]) < thres) { r1 = i; break; }
            for (let i = 1; i < SIZE / 2; i++)
                if (Math.abs(buffer[SIZE - i]) < thres) { r2 = SIZE - i; break; }

            buffer = buffer.slice(r1, r2);
            SIZE = buffer.length;

            let c = new Array(SIZE).fill(0);
            for (let i = 0; i < SIZE; i++)
                for (let j = 0; j < SIZE - i; j++)
                    c[i] = c[i] + buffer[j] * buffer[j + i];

            let d = 0; while (c[d] > c[d + 1]) d++;
            let maxval = -1, maxpos = -1;
            for (let i = d; i < SIZE; i++) {
                if (c[i] > maxval) {
                    maxval = c[i];
                    maxpos = i;
                }
            }

            let T0 = maxpos;
            let x1 = c[T0 - 1], x2 = c[T0], x3 = c[T0 + 1];
            let a = (x1 + x3 - 2 * x2) / 2, b = (x3 - x1) / 2;
            if (a) T0 = T0 - b / (2 * a);

            return sampleRate / T0;
        };

        const frequencyToNoteName = (frequency) => {
            const noteStrings = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
            const A4 = 440;
            const semitone = 69 + 12 * Math.log2(frequency / A4);
            const noteIndex = Math.round(semitone) % 12;
            const octave = Math.floor(Math.round(semitone) / 12) - 1;
            return `${noteStrings[noteIndex]}${octave}`;
        };

        const detect = () => {
            analyser.getFloatTimeDomainData(dataArray);
            const frequency = autoCorrelate(dataArray, this.audioContext.sampleRate);

            if (frequency !== -1) {
                const note = frequencyToNoteName(frequency);
                document.getElementById('note').innerText = `Note: ${note}`;
                document.getElementById('frequency').innerText = `Frequency: ${frequency.toFixed(2)} Hz`;
            } else {
                document.getElementById('note').innerText = `Note: -`;
                document.getElementById('frequency').innerText = `Frequency: -`;
            }

            requestAnimationFrame(detect);
        };

        detect();
    },

    unmount: function() {
        if (this.audioContext) {
            this.audioContext.close();
        }
    }
};