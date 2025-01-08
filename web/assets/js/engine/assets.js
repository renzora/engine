assets = {
    loadedAssets: {},
    totalAssets: 0,
    loadedCount: 0,

    showLoadingBar: function() {
        document.getElementById('loadingBarContainer').classList.remove('hidden');
    },

    updateLoadingBar: function(assetName) {
        const percentage = Math.floor((this.loadedCount / this.totalAssets) * 100);
        const loadingBar = document.getElementById('loadingBar');
        const loadingPercentage = document.getElementById('loadingPercentage');
        loadingBar.style.width = percentage + '%';
        loadingPercentage.innerHTML = `Loading ${assetName}... ${percentage}%`;
    },

    hideLoadingBar: function() {
        document.getElementById('loadingBarContainer').classList.add('hidden');
    },

    preload: function(assetsList, callback, force = false) {
        const uniqueAssets = {};
        // If `force` is true, we skip the "already loaded" check.
        const assetsToLoad = assetsList.filter(asset => {
            if (!uniqueAssets[asset.name]) {
                uniqueAssets[asset.name] = true;
                return force ? true : !this.isAssetLoaded(asset.name);
            }
            return false;
        });

        this.totalAssets += assetsToLoad.length;

        if (assetsToLoad.length === 0) {
            if (callback) callback();
            return;
        }

        this.showLoadingBar();

        const promises = assetsToLoad.map(asset => {
            const fileType = this.getFileType(asset.path);
            this.updateLoadingBar(asset.name);

            if (fileType === 'image') {
                return this.loadImage(asset);
            } else if (fileType === 'json') {
                return this.loadJSON(asset);
            } else if (fileType === 'audio') {
                return this.loadAudio(asset);
            } else {
                return Promise.resolve(null);
            }
        });

        Promise.all(promises).then(() => {
            this.hideLoadingBar();
            if (callback) callback();
        });
    },

    reloadAssets: function(assetsList, callback) {
        this.preload(assetsList, callback, true);
    },

    getFileType: function(path) {
        const pathParts = path.split('?');
        const extension = pathParts[0].split('.').pop().toLowerCase();

        if (['png', 'jpg', 'jpeg', 'gif', 'php'].includes(extension)) {
            return 'image';
        } else if (extension === 'json') {
            return 'json';
        } else if (['mp3', 'wav', 'ogg'].includes(extension)) {
            return 'audio';
        }

        console.error('Unsupported file type:', extension);
        return null;
    },

    loadImage: function(asset) {
        return new Promise((resolve, reject) => {
            const img = new Image();
            img.onload = () => {
                this.assetLoaded(asset, img);
                resolve(img);
            };
            img.onerror = reject;
            img.src = 'assets/' + asset.path;
        });
    },

    loadJSON: function(asset) {
        return fetch('assets/' + asset.path)
            .then(response => response.json())
            .then(data => {
                this.assetLoaded(asset, data);
                return data;
            })
            .catch(error => console.error(`Error loading JSON:`, error));
    },

    loadAudio: function(asset) {
        return fetch('assets/' + asset.path)
            .then(response => response.arrayBuffer())
            .then(arrayBuffer => {
                return audio.audioContext.decodeAudioData(arrayBuffer);
            })
            .then(audioBuffer => {
                this.assetLoaded(asset, audioBuffer);
                return audioBuffer;
            })
            .catch(error => console.error(`Error loading audio:`, error));
    },

    assetLoaded: function(asset, data) {
        this.loadedCount++;
        this.loadedAssets[asset.name] = data;
        this.updateLoadingBar(asset.name);
    },

    use: function(name) {
        return this.loadedAssets[name];
    },

    isAssetLoaded: function(name) {
        return !!this.loadedAssets[name];
    },

    unload: function(assetName) {
        if (this.loadedAssets[assetName]) {
            delete this.loadedAssets[assetName];
            this.loadedCount--;
            console.log(`Unloaded asset: ${assetName}`);
        }
    }
};
