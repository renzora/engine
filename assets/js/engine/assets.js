var assets = {
    loadedAssets: {},
    totalAssets: 0,
    loadedCount: 0,

    reloadAssets: function(assetNames, callback) {
        const promises = assetNames.map(assetName => {
            const assetPath = this.getAssetPathByName(assetName);
            if (assetPath) {
                const fileType = this.getFileType(assetPath);
                if (fileType === 'json') {
                    return fetch(this.noCache('assets/' + assetPath))
                        .then(response => response.json())
                        .then(data => {
                            this.loadedAssets[assetName] = data;
                        })
                        .catch(error => console.error(`Error loading JSON:`, error));
                }
            }
            return Promise.resolve(); // Resolve empty promise for non-JSON assets
        });

        Promise.all(promises).then(() => {
            if (callback) {
                callback();
            }
        });
    },

    getAssetPathByName: function(assetName) {
        // Helper method to get asset path by its name
        // You can define this mapping based on your asset loading logic
        const assetMapping = {
            'objectData': 'json/objectData.json',
            'objectScript': 'json/objectScript.json',
            'roomData': 'json/roomData.json',
            // Add more mappings as needed
        };
        return assetMapping[assetName];
    },

    preload: function(assetsList, callback) {
        this.totalAssets = assetsList.length;
        this.loadedCount = 0;

        assetsList.forEach(asset => {
            const fileType = this.getFileType(asset.path);
            
            if (fileType === 'image') {
                this.loadImage(asset, callback);
            } else if (fileType === 'json') {
                this.loadJSON(asset, callback);
            } else if (fileType === 'audio') {
                this.loadAudio(asset, callback);
            }
        });
    },

    getFileType: function(path) {
        // Split the path by '?' to get the file extension
        const pathParts = path.split('?');
        const extension = pathParts[0].split('.').pop();
      
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

    loadImage: function(asset, callback) {
        const img = new Image();
        img.onload = () => {
            this.assetLoaded(asset, img, callback);
        };
        img.src = this.noCache('assets/' + asset.path);
    },

    loadJSON: function(asset, callback) {
        fetch(this.noCache('assets/' + asset.path))
            .then(response => response.json())
            .then(data => {
                this.assetLoaded(asset, data, callback);
            })
            .catch(error => console.error(`Error loading JSON:`, error));
    },

    loadAudio: function(asset, callback) {
        const audio = new Audio('assets/' + asset.path);
        audio.onloadeddata = () => {
            this.assetLoaded(asset, audio, callback);
        };
        audio.onerror = (error) => {
            console.error(`Error loading AUDIO:`, error);
        };
    },

    assetLoaded: function(asset, data, callback) {
        this.loadedCount++;
        this.loadedAssets[asset.name] = data;

        if (this.loadedCount === this.totalAssets) {
            callback();
        }
    },

    load: function(name) {
        return this.loadedAssets[name];
    },

    noCache: function(url) {
        const timestamp = new Date().getTime();
        if (url.includes('?')) {
          return `${url}t=${timestamp}`;
        } else {
          return `${url}?t=${timestamp}`;
        }
      },
}