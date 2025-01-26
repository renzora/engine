audio = {
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

    start() {
        if (!this.audioContext) {
            this.audioContext = new (window.AudioContext || window.webkitAudioContext)();
            this.masterGain = this.audioContext.createGain();
            this.masterGain.connect(this.audioContext.destination);
            this.masterGain.gain.value = 1;
            this.channels = {};
            this.sources = {};
            this.createChannel('master', localStorage.getItem('master-volume') || this.defaultVolume);
        }
    },

    unmount() {
        if (this.audioContext) {
            this.audioContext.close();
        }
    },

    onGameCreate() {

    },

    pauseAll() {
        if (this.audioContext && this.audioContext.state === 'running') {
            this.audioContext.suspend();
        }
    },

    resumeAll() {
        if (this.audioContext && this.audioContext.state === 'suspended') {
            this.audioContext.resume();
        }
    },

    stopAllSounds(channel) {
        if (this.sources[channel]) {
            this.sources[channel].forEach(source => source.stop());
            this.sources[channel] = [];
        }
    },

    setChannelTempo(channel, bpm) {
        this.channelTempos[channel] = bpm;
    },

    play(params, channel = 'synth') {
        const { bpm, ticks_per_beat, instruments, patterns, tempos } = params;
        let currentBpm = bpm;
        let startTime = this.audioContext.currentTime;

        patterns.forEach(pattern => {
            pattern.pattern_commands.forEach(command => {
                const instrument = instruments[command.inst];
                const channelName = `instr-${command.inst}`;

                if (tempos && tempos[command.tick] !== undefined) {
                    currentBpm = tempos[command.tick];
                }

                const beatDuration = 60 / currentBpm;
                const tickDuration = beatDuration / ticks_per_beat;
                const commandStartTime = startTime + (command.tick * tickDuration);

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

    playNote(id, instrument, combinedNote, startTime, channel = 'master') {
        const [pitch, octave] = this.parseNote(combinedNote);
        if (pitch === null || octave === null) {
            return;
        }
        const noteNumber = this.noteToNumber(pitch, octave);
        this._playNote(
            id, 
            instrument, 
            noteNumber, 
            startTime, 
            channel
        );
    },

    _playNote(id, instrument, noteNumber, startTime, channel) {
        let oscillator = null;
        let gainNode = this.audioContext.createGain();
    
        if (!this.sources[channel]) {
            this.sources[channel] = [];
        }
    
        oscillator = this.audioContext.createOscillator();
        oscillator.type = this.oscillatorTypes[instrument.oscillator - 1] || 'sine';
        oscillator.frequency.value = this.calculateFrequency(noteNumber);
    
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

    playAudio(id, audioBuffer, channel = 'sfx', loop = false) {
        if (this.audioContext.state !== 'running') {
            this.audioContext.resume().then(() => {
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

    processQueue(channel) {
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

    stopLoopingAudio(id, channel, fadeDuration = 0.5) {
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

    setVolume(channel, volume) {

        if (isNaN(volume) || volume === null || volume === undefined) {
            volume = this.defaultVolume;
        }

        if (channel === 'master') {
            if (this.masterGain) {
                this.masterGain.gain.value = volume;
            }
        } else {
            if (this.channels[channel]) {
                this.channels[channel].gain.value = volume;
            }
        }
    },

    createChannel(name, volume = this.defaultVolume) {
        if (!this.channels[name]) {
            const gainNode = this.audioContext.createGain();
            gainNode.connect(this.masterGain);
            gainNode.gain.value = volume;
            this.channels[name] = gainNode;
            const event = new CustomEvent('channelCreated', { detail: { channel: name } });
            document.dispatchEvent(event);
        }
    },

    removeChannel(channel) {
        if (!this.channels[channel]) {
            return;
        }
        this.channels[channel].disconnect();
        delete this.channels[channel];
        document.dispatchEvent(new CustomEvent('channelRemoved', { detail: { channel } }));
    },

    routeChannel(sourceChannel, destinationChannel) {
        if (!this.channels[sourceChannel] || !this.channels[destinationChannel]) {
            return;
        }

        this.channels[sourceChannel].disconnect();
        this.channels[sourceChannel].connect(this.channels[destinationChannel]);
    },

    parseNote(combinedNote) {
        const match = combinedNote.match(/([A-G]#?)(\d)/);
        if (match) {
            return [match[1], parseInt(match[2])];
        }
        return [null, null];
    },
    
    noteToNumber(pitch, octave) {
        const notes = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];
        const noteIndex = notes.indexOf(pitch);
        return noteIndex + ((octave + 1) * 12);
    },
    calculateFrequency(noteNumber) {
        return 440 * Math.pow(2, (noteNumber - 69) / 12);
    },

    createNoiseBuffer() {
        const bufferSize = this.audioContext.sampleRate;
        const buffer = this.audioContext.createBuffer(1, bufferSize, this.audioContext.sampleRate);
        const output = buffer.getChannelData(0);
        for (let i = 0; i < bufferSize; i++) {
            output[i] = Math.random() * 2 - 1;
        }
        return buffer;
    },

    applyEnvelope(gainNode, envelope, startTime) {
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

    detectPitch(analyser) {
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
    }
}