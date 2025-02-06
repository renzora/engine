assets = {
    loadedAssets: {},
    totalAssets: 0,
    loadedCount: 0,

    preload(assetsList, callback, force = false) {
        const uniqueAssets = assetsList.filter(asset => {
            if (!this.loadedAssets[asset.name] || force) {
                return true;
            }
            return false;
        });

        this.totalAssets += uniqueAssets.length;

        if (uniqueAssets.length === 0) {
            if (callback) callback();
            return;
        }

        document.getElementById('loadingBarContainer').classList.remove('hidden');

        const promises = uniqueAssets.map(asset => {
            this.updateLoadingBar(asset.name);
            const fileType = this.getFileType(asset.path);
            return this.loadAsset(asset, fileType);
        });

        Promise.all(promises).then(() => {
            document.getElementById('loadingBarContainer').classList.add('hidden');
            plugin.hook('onAssetsLoaded')
            if (callback) callback();
        });
    },

    loadAsset(asset, type) {
        switch(type) {
            case 'image':
                return new Promise((resolve, reject) => {
                    const img = new Image();
                    img.onload = () => {
                        this.assetLoaded(asset, img);
                        resolve(img);
                    };
                    img.onerror = reject;
                    img.src = asset.path;
                });
                
            case 'json':
            case 'audio':
                return fetch(asset.path)
                    .then(response => type === 'json' ? response.json() : response.arrayBuffer())
                    .then(data => {
                        if (type === 'audio') {
                            return audio.audioContext.decodeAudioData(data);
                        }
                        return data;
                    })
                    .then(data => {
                        this.assetLoaded(asset, data);
                        return data;
                    })
                    .catch(error => console.error(`Error loading ${type}:`, error));
        }
    },

    updateLoadingBar(assetName) {
        const percentage = Math.floor((this.loadedCount / this.totalAssets) * 100);
        const loadingBar = document.getElementById('loadingBar');
        const loadingPercentage = document.getElementById('loadingPercentage');
        loadingBar.style.width = percentage + '%';
        loadingPercentage.innerHTML = `Loading ${assetName}... ${percentage}%`;
    },

    getFileType(path) {
        const extension = path.split('?')[0].split('.').pop().toLowerCase();
        if (['png', 'jpg', 'jpeg', 'gif'].includes(extension)) return 'image';
        if (extension === 'json') return 'json';
        if (['mp3', 'wav', 'ogg'].includes(extension)) return 'audio';
        console.error('Unsupported file type:', extension);
        return null;
    },

    assetLoaded(asset, data) {
        this.loadedCount++;
        this.loadedAssets[asset.name] = data;
        this.updateLoadingBar(asset.name);
    },

    use(name) {
        return this.loadedAssets[name];
    },

    unload(assetName) {
        if (this.loadedAssets[assetName]) {
            delete this.loadedAssets[assetName];
            this.loadedCount--;
        }
    },

    reloadAssets(assetsList, callback) {
        this.preload(assetsList, callback, true);
        plugin.hook('onReloadAssets');
    }
};