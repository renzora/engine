var assets = {
    loadedAssets: {},
    totalAssets: 0,
    loadedCount: 0,

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
        img.src = network.noCache('assets/' + asset.path);
    },

    loadJSON: function(asset, callback) {
        fetch(network.noCache('assets/' + asset.path))
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
    }
}