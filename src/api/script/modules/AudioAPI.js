// === AUDIO API MODULE ===

import {
  Sound,
  Analyser,
  SoundTrack,
  Vector3
} from '@babylonjs/core';

export class AudioAPI {
  constructor(scene) {
    this.scene = scene;
  }

  // === BASIC AUDIO CREATION ===

  createSound(name, urlOrArrayBuffer, options = {}) {
    const sound = new Sound(name, urlOrArrayBuffer, this.scene, null, {
      loop: options.loop || false,
      autoplay: options.autoplay || false,
      volume: options.volume || 1.0,
      playbackRate: options.playbackRate || 1.0,
      ...options
    });
    return sound;
  }

  create3DSound(name, urlOrArrayBuffer, position = [0, 0, 0], options = {}) {
    const sound = new Sound(name, urlOrArrayBuffer, this.scene, null, {
      loop: options.loop || false,
      autoplay: options.autoplay || false,
      volume: options.volume || 1.0,
      spatialSound: true,
      maxDistance: options.maxDistance || 100,
      distanceModel: options.distanceModel || 'exponential',
      ...options
    });
    
    this.setSoundPosition(sound, ...position);
    return sound;
  }

  createSpatialSound(name, urlOrArrayBuffer, mesh, options = {}) {
    const sound = new Sound(name, urlOrArrayBuffer, this.scene, null, {
      loop: options.loop || false,
      autoplay: options.autoplay || false,
      volume: options.volume || 1.0,
      spatialSound: true,
      maxDistance: options.maxDistance || 100,
      ...options
    });
    
    sound.attachToMesh(mesh);
    return sound;
  }

  // === AUDIO PLAYBACK ===

  playSound(sound, time = 0) {
    if (!sound) return false;
    sound.play(time);
    return true;
  }

  stopSound(sound) {
    if (!sound) return false;
    sound.stop();
    return true;
  }

  pauseSound(sound) {
    if (!sound) return false;
    sound.pause();
    return true;
  }

  setSoundVolume(sound, volume) {
    if (!sound) return false;
    sound.setVolume(Math.max(0, Math.min(1, volume)));
    return true;
  }

  setSoundPlaybackRate(sound, rate) {
    if (!sound) return false;
    sound.setPlaybackRate(Math.max(0.1, Math.min(4.0, rate)));
    return true;
  }

  setSoundLoop(sound, loop) {
    if (!sound) return false;
    sound.loop = loop;
    return true;
  }

  // === 3D AUDIO POSITIONING ===

  setSoundPosition(sound, x, y, z) {
    if (!sound) return false;
    sound.setPosition(new Vector3(x, y, z));
    return true;
  }

  attachSoundToMesh(sound, mesh) {
    if (!sound || !mesh) return false;
    sound.attachToMesh(mesh);
    return true;
  }

  detachSoundFromMesh(sound) {
    if (!sound) return false;
    sound.detachFromMesh();
    return true;
  }

  setSoundMaxDistance(sound, distance) {
    if (!sound) return false;
    sound.maxDistance = Math.max(0, distance);
    return true;
  }

  setSoundDistanceModel(sound, model) {
    if (!sound) return false;
    // 'linear', 'inverse', 'exponential'
    sound.distanceModel = model;
    return true;
  }

  setSoundRolloffFactor(sound, rolloff) {
    if (!sound) return false;
    sound.rolloffFactor = Math.max(0, rolloff);
    return true;
  }

  setSoundCone(sound, innerAngle = 360, outerAngle = 360, outerGain = 0) {
    if (!sound) return false;
    sound.setDirectionalCone(innerAngle, outerAngle, outerGain);
    return true;
  }

  setSoundDirection(sound, x, y, z) {
    if (!sound) return false;
    sound.setLocalDirectionToMesh(new Vector3(x, y, z));
    return true;
  }

  // === AUDIO EFFECTS ===

  setSoundLowpass(sound, frequency = 22050, Q = 1) {
    if (!sound || !sound.soundTrackId) return false;
    
    const audioContext = this.scene.getEngine().audioContext;
    if (!audioContext) return false;
    
    const filter = audioContext.createBiquadFilter();
    filter.type = 'lowpass';
    filter.frequency.value = frequency;
    filter.Q.value = Q;
    
    // Connect filter
    sound.connectToSoundTrackAudioNode(filter);
    return true;
  }

  setSoundHighpass(sound, frequency = 350, Q = 1) {
    if (!sound || !sound.soundTrackId) return false;
    
    const audioContext = this.scene.getEngine().audioContext;
    if (!audioContext) return false;
    
    const filter = audioContext.createBiquadFilter();
    filter.type = 'highpass';
    filter.frequency.value = frequency;
    filter.Q.value = Q;
    
    sound.connectToSoundTrackAudioNode(filter);
    return true;
  }

  setSoundReverb(sound, roomSize = 0.5, decay = 1.5, wetness = 0.3) {
    if (!sound) return false;
    
    const audioContext = this.scene.getEngine().audioContext;
    if (!audioContext) return false;
    
    const convolver = audioContext.createConvolver();
    const impulse = this.generateReverbImpulse(audioContext, roomSize, decay);
    convolver.buffer = impulse;
    
    const wetGain = audioContext.createGain();
    wetGain.gain.value = wetness;
    
    const dryGain = audioContext.createGain();
    dryGain.gain.value = 1 - wetness;
    
    sound.connectToSoundTrackAudioNode(convolver);
    sound.connectToSoundTrackAudioNode(wetGain);
    
    return true;
  }

  generateReverbImpulse(audioContext, roomSize = 0.5, decay = 1.5) {
    const length = audioContext.sampleRate * decay;
    const impulse = audioContext.createBuffer(2, length, audioContext.sampleRate);
    
    for (let channel = 0; channel < 2; channel++) {
      const channelData = impulse.getChannelData(channel);
      for (let i = 0; i < length; i++) {
        const n = length - i;
        channelData[i] = (Math.random() * 2 - 1) * Math.pow(n / length, roomSize);
      }
    }
    
    return impulse;
  }

  // === AUDIO ANALYSIS ===

  createAudioAnalyser(fftSize = 256) {
    const analyser = new Analyser(this.scene);
    analyser.FFT_SIZE = fftSize;
    return analyser;
  }

  getAudioFrequencyData(analyser) {
    if (!analyser) return [];
    return analyser.getByteFrequencyData();
  }

  getAudioWaveformData(analyser) {
    if (!analyser) return [];
    return analyser.getByteTimeDomainData();
  }

  getAudioLevel(analyser) {
    if (!analyser) return 0;
    
    const frequencyData = analyser.getByteFrequencyData();
    let sum = 0;
    for (let i = 0; i < frequencyData.length; i++) {
      sum += frequencyData[i];
    }
    return sum / frequencyData.length / 255; // Normalize to 0-1
  }

  // === SOUND TRACKS ===

  createSoundTrack(name) {
    const track = new SoundTrack(this.scene, { mainTrack: false });
    track.id = name;
    return track;
  }

  addSoundToTrack(track, sound) {
    if (!track || !sound) return false;
    track.addSound(sound);
    return true;
  }

  removeSoundFromTrack(track, sound) {
    if (!track || !sound) return false;
    track.removeSound(sound);
    return true;
  }

  setSoundTrackVolume(track, volume) {
    if (!track) return false;
    track.setVolume(Math.max(0, Math.min(1, volume)));
    return true;
  }

  muteMainTrack(muted = true) {
    if (this.scene.mainSoundTrack) {
      this.scene.mainSoundTrack.setVolume(muted ? 0 : 1);
      return true;
    }
    return false;
  }

  // === AUDIO UTILITIES ===

  isSoundPlaying(sound) {
    if (!sound) return false;
    return sound.isPlaying;
  }

  isSoundReady(sound) {
    if (!sound) return false;
    return sound.isReady();
  }

  getSoundDuration(sound) {
    if (!sound || !sound._htmlAudioElement) return 0;
    return sound._htmlAudioElement.duration || 0;
  }

  getSoundCurrentTime(sound) {
    if (!sound || !sound._htmlAudioElement) return 0;
    return sound._htmlAudioElement.currentTime || 0;
  }

  setSoundCurrentTime(sound, time) {
    if (!sound || !sound._htmlAudioElement) return false;
    sound._htmlAudioElement.currentTime = Math.max(0, time);
    return true;
  }

  cloneSound(sound, _name) {
    if (!sound || !sound.clone) return null;
    return sound.clone();
  }

  disposeSound(sound) {
    if (!sound) return false;
    sound.dispose();
    return true;
  }

  // === MASTER VOLUME CONTROLS ===

  setMasterVolume(volume) {
    if (this.scene.audioEngine) {
      this.scene.audioEngine.setGlobalVolume(Math.max(0, Math.min(1, volume)));
      return true;
    }
    return false;
  }

  getMasterVolume() {
    if (this.scene.audioEngine) {
      return this.scene.audioEngine.getGlobalVolume();
    }
    return 1.0;
  }

  muteAllSounds(muted = true) {
    const volume = muted ? 0 : 1;
    this.scene.sounds.forEach(sound => {
      if (sound.setVolume) {
        sound.setVolume(volume);
      }
    });
    return true;
  }

  // === AUDIO PRESETS ===

  createFootstepSound(name, url, options = {}) {
    return this.createSound(name, url, {
      volume: options.volume || 0.5,
      playbackRate: options.playbackRate || 1.0,
      loop: false,
      autoplay: false,
      ...options
    });
  }

  createAmbientSound(name, url, options = {}) {
    return this.createSound(name, url, {
      volume: options.volume || 0.3,
      loop: true,
      autoplay: options.autoplay || true,
      ...options
    });
  }

  createUISound(name, url, options = {}) {
    return this.createSound(name, url, {
      volume: options.volume || 0.7,
      loop: false,
      autoplay: false,
      playbackRate: options.playbackRate || 1.0,
      ...options
    });
  }

  createExplosionSound(name, url, position = null, options = {}) {
    const soundOptions = {
      volume: options.volume || 0.8,
      loop: false,
      autoplay: false,
      maxDistance: options.maxDistance || 50,
      ...options
    };
    
    if (position) {
      const sound = this.create3DSound(name, url, position, soundOptions);
      return sound;
    } else {
      return this.createSound(name, url, soundOptions);
    }
  }

  // === PROCEDURAL AUDIO ===

  createToneSound(name, frequency = 440, duration = 1.0, options = {}) {
    const audioContext = this.scene.getEngine().audioContext;
    if (!audioContext) return null;
    
    const sampleRate = audioContext.sampleRate;
    const numSamples = sampleRate * duration;
    const buffer = audioContext.createBuffer(1, numSamples, sampleRate);
    const data = buffer.getChannelData(0);
    
    for (let i = 0; i < numSamples; i++) {
      const t = i / sampleRate;
      data[i] = Math.sin(2 * Math.PI * frequency * t) * (options.amplitude || 0.5);
    }
    
    const sound = new Sound(name, buffer, this.scene, null, {
      loop: options.loop || false,
      autoplay: options.autoplay || false,
      volume: options.volume || 1.0
    });
    
    return sound;
  }

  createNoiseSound(name, duration = 1.0, options = {}) {
    const audioContext = this.scene.getEngine().audioContext;
    if (!audioContext) return null;
    
    const sampleRate = audioContext.sampleRate;
    const numSamples = sampleRate * duration;
    const buffer = audioContext.createBuffer(1, numSamples, sampleRate);
    const data = buffer.getChannelData(0);
    
    for (let i = 0; i < numSamples; i++) {
      data[i] = (Math.random() * 2 - 1) * (options.amplitude || 0.3);
    }
    
    const sound = new Sound(name, buffer, this.scene, null, {
      loop: options.loop || false,
      autoplay: options.autoplay || false,
      volume: options.volume || 1.0
    });
    
    return sound;
  }

  // === AUDIO LOADING ===

  loadSoundAsync(name, url) {
    return new Promise((resolve, reject) => {
      const sound = new Sound(name, url, this.scene, () => {
        resolve(sound);
      }, {
        autoplay: false,
        loop: false
      });
      
      sound.onError = (error) => {
        reject(new Error(`Failed to load audio: ${error}`));
      };
    });
  }

  loadMultipleSoundsAsync(soundData) {
    const promises = soundData.map(data => 
      this.loadSoundAsync(data.name, data.url)
    );
    return Promise.all(promises);
  }

  // === AUDIO STREAMING ===

  createStreamingSound(name, url, options = {}) {
    return new Sound(name, url, this.scene, null, {
      streaming: true,
      loop: options.loop || false,
      autoplay: options.autoplay || false,
      volume: options.volume || 1.0,
      ...options
    });
  }

  // === AUDIO SYNTHESIS ===

  createBeepSound(name, pitch = 1000, duration = 0.1, options = {}) {
    return this.createToneSound(name, pitch, duration, {
      amplitude: options.amplitude || 0.5,
      volume: options.volume || 0.7,
      ...options
    });
  }

  createChordSound(name, frequencies = [261.63, 329.63, 392.00], duration = 1.0, options = {}) {
    const audioContext = this.scene.getEngine().audioContext;
    if (!audioContext) return null;
    
    const sampleRate = audioContext.sampleRate;
    const numSamples = sampleRate * duration;
    const buffer = audioContext.createBuffer(1, numSamples, sampleRate);
    const data = buffer.getChannelData(0);
    
    const amplitude = (options.amplitude || 0.3) / frequencies.length;
    
    for (let i = 0; i < numSamples; i++) {
      const t = i / sampleRate;
      let sample = 0;
      
      frequencies.forEach(freq => {
        sample += Math.sin(2 * Math.PI * freq * t) * amplitude;
      });
      
      data[i] = sample;
    }
    
    return new Sound(name, buffer, this.scene, null, {
      loop: options.loop || false,
      autoplay: options.autoplay || false,
      volume: options.volume || 1.0
    });
  }

  // === AUDIO INFO ===

  getSoundInfo(sound) {
    if (!sound) return null;
    
    return {
      name: sound.name,
      isPlaying: sound.isPlaying,
      isPaused: sound.isPaused,
      isReady: sound.isReady(),
      volume: sound.getVolume(),
      playbackRate: sound._playbackRate || 1.0,
      loop: sound.loop,
      duration: this.getSoundDuration(sound),
      currentTime: this.getSoundCurrentTime(sound),
      spatialSound: sound.spatialSound,
      maxDistance: sound.maxDistance || null,
      position: sound._position ? [sound._position.x, sound._position.y, sound._position.z] : null
    };
  }

  getAllSounds() {
    return this.scene.sounds.map(sound => ({
      name: sound.name,
      isPlaying: sound.isPlaying,
      isReady: sound.isReady(),
      spatialSound: sound.spatialSound
    }));
  }

  findSoundByName(name) {
    return this.scene.sounds.find(sound => sound.name === name) || null;
  }

  // === ADVANCED AUDIO ===

  createAudioSpectrum(sound, fftSize = 256) {
    if (!sound) return null;
    
    const analyser = new Analyser(this.scene);
    analyser.FFT_SIZE = fftSize;
    sound.connectToSoundTrackAudioNode(analyser);
    
    return {
      analyser,
      getSpectrum: () => analyser.getByteFrequencyData(),
      getWaveform: () => analyser.getByteTimeDomainData()
    };
  }

  setAudioListenerPosition(x, y, z) {
    const audioEngine = this.scene.audioEngine;
    if (audioEngine && audioEngine.audioContext && audioEngine.audioContext.listener) {
      audioEngine.audioContext.listener.setPosition(x, y, z);
      return true;
    }
    return false;
  }

  setAudioListenerOrientation(forwardX, forwardY, forwardZ, upX = 0, upY = 1, upZ = 0) {
    const audioEngine = this.scene.audioEngine;
    if (audioEngine && audioEngine.audioContext && audioEngine.audioContext.listener) {
      audioEngine.audioContext.listener.setOrientation(forwardX, forwardY, forwardZ, upX, upY, upZ);
      return true;
    }
    return false;
  }

  // === AUDIO OCCLUSION ===

  setAudioOcclusion(sound, occlusionFactor = 0.5) {
    if (!sound) return false;
    
    // Simple occlusion by reducing volume and adding lowpass
    const occludedVolume = sound.getVolume() * (1 - occlusionFactor);
    sound.setVolume(occludedVolume);
    
    // Add lowpass filter for occlusion effect
    this.setSoundLowpass(sound, 1000 * (1 - occlusionFactor), 1);
    
    return true;
  }

  // === MUSIC SEQUENCING ===

  createMusicPlaylist(sounds, fadeTime = 1.0) {
    if (!sounds || sounds.length === 0) return null;
    
    let currentIndex = 0;
    let isPlaying = false;
    
    const playlist = {
      sounds,
      currentIndex,
      fadeTime,
      
      play() {
        if (currentIndex < sounds.length) {
          isPlaying = true;
          const sound = sounds[currentIndex];
          sound.play();
          
          // Set up next track when current ends
          sound.onended = () => {
            this.next();
          };
        }
      },
      
      next() {
        if (isPlaying && currentIndex < sounds.length - 1) {
          const currentSound = sounds[currentIndex];
          currentIndex++;
          const nextSound = sounds[currentIndex];
          
          // Cross-fade
          this.crossFade(currentSound, nextSound, fadeTime);
        }
      },
      
      previous() {
        if (isPlaying && currentIndex > 0) {
          const currentSound = sounds[currentIndex];
          currentIndex--;
          const prevSound = sounds[currentIndex];
          
          this.crossFade(currentSound, prevSound, fadeTime);
        }
      },
      
      stop() {
        isPlaying = false;
        sounds.forEach(sound => sound.stop());
      },
      
      crossFade: (fromSound, toSound, duration) => {
        this.crossFadeSounds(fromSound, toSound, duration);
      }
    };
    
    return playlist;
  }

  crossFadeSounds(fromSound, toSound, duration = 1.0) {
    if (!fromSound || !toSound) return false;
    
    const fadeSteps = 60; // 60 steps for smooth fade
    const stepTime = (duration * 1000) / fadeSteps;
    // const volumeStep = 1.0 / fadeSteps; // Not used, progress calculation is used instead
    
    let step = 0;
    
    // Start the new sound at volume 0
    toSound.setVolume(0);
    toSound.play();
    
    const fadeInterval = setInterval(() => {
      const progress = step / fadeSteps;
      
      fromSound.setVolume(1 - progress);
      toSound.setVolume(progress);
      
      step++;
      
      if (step >= fadeSteps) {
        clearInterval(fadeInterval);
        fromSound.stop();
        toSound.setVolume(1);
      }
    }, stepTime);
    
    return true;
  }

  // === ENVIRONMENTAL AUDIO ===

  createEnvironmentalAudio(environmentType = 'forest') {
    const environments = {
      forest: {
        ambient: { url: 'forest_ambient.wav', volume: 0.4, loop: true },
        birds: { url: 'birds.wav', volume: 0.3, loop: true },
        wind: { url: 'wind.wav', volume: 0.2, loop: true }
      },
      city: {
        traffic: { url: 'traffic.wav', volume: 0.5, loop: true },
        crowds: { url: 'crowd.wav', volume: 0.3, loop: true },
        ambient: { url: 'city_ambient.wav', volume: 0.4, loop: true }
      },
      underwater: {
        bubbles: { url: 'bubbles.wav', volume: 0.4, loop: true },
        ambient: { url: 'underwater.wav', volume: 0.6, loop: true }
      }
    };
    
    const env = environments[environmentType];
    if (!env) return null;
    
    const sounds = {};
    Object.keys(env).forEach(key => {
      const config = env[key];
      sounds[key] = this.createSound(`${environmentType}_${key}`, config.url, config);
    });
    
    return {
      sounds,
      play() {
        Object.values(sounds).forEach(sound => sound.play());
      },
      stop() {
        Object.values(sounds).forEach(sound => sound.stop());
      },
      setVolume(volume) {
        Object.values(sounds).forEach(sound => sound.setVolume(volume));
      }
    };
  }

  // === AUDIO EVENTS ===

  onSoundEnd(sound, callback) {
    if (!sound || !callback) return false;
    sound.onended = callback;
    return true;
  }

  onSoundReady(sound, callback) {
    if (!sound || !callback) return false;
    
    if (sound.isReady()) {
      callback();
    } else {
      sound.onReady = callback;
    }
    return true;
  }

  // === AUDIO PERFORMANCE ===

  getAudioMemoryUsage() {
    let totalSize = 0;
    this.scene.sounds.forEach(sound => {
      if (sound._audioBuffer && sound._audioBuffer.byteLength) {
        totalSize += sound._audioBuffer.byteLength;
      }
    });
    return totalSize;
  }

  optimizeAudioMemory() {
    // Dispose sounds that are not playing and not set to autoplay
    const disposable = this.scene.sounds.filter(sound => 
      !sound.isPlaying && !sound.autoplay && !sound.loop
    );
    
    disposable.forEach(sound => {
      if (sound._lastTimeWhenPlayed && Date.now() - sound._lastTimeWhenPlayed > 30000) {
        sound.dispose();
      }
    });
    
    return disposable.length;
  }
}