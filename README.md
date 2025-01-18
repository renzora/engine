<h1 align="center">A game engine that renders to javascript canvas for web</h1>

> ⚠️ **Warning**  
> This is a very early development version and is extremely unstable and will most likely break or cause problems. Only use if you intend to help with development.

![](https://i.imgur.com/V2j1yIL.png)

# Requirements
- Docker Desktop: https://www.docker.com/products/docker-desktop
- MongoDB Software: https://www.mongodb.com/products/tools/compass

# Installing Renzora Engine

Open your terminal of choice (make sure docker desktop is running)
```
git clone https://github.com/renzora/engine.git
cd engine
```
Before starting the server make sure you edit ```.env``` to change the environment variables to suit your needs.

```
docker-compose up --build
```

# Services
- Client: Nginx ```(port 80/443)```
- Website: ```http://localhost```
- Nodejs Express endpoint: ```http://localhost:3000```

# Default Login
- Username: ```admin```
- Password: ```password```

# Default MongoDB
- URI: ```mongodb://admin:this_is_a_test_password@localhost:27017/```
- Host: ```localhost```
- Port: ```27017```
- Database Name: ```renzora```
- Mongo Username: ```admin```
- Mongo Password: ```this_is_a_test_password```

# Build Tools (minify & bundle css/js in client directory with tailwindcss/esbuild)
- If you edit any of the core javascript in client/assets/js/engine you will need to rebuild the renzora.min.js file to see changes in the browser. Also make sure you have caching disabled in dev tools. you can click on 'disable cache' in the network tab. Or you can clear the browser cache. If you're using a cdn, you will need to clear the cache from the network.

```
cd build
npm install
```

build js
`npm run js`

build css
`npm run css`

build css + watch for changes
`npm run watch:css`

build both
`npm run build`

# Notes
"WebSocket connection to 'wss://localhost:3000/' failed"
because the local dev server is using a self-signed ssl certificate and not a domain specific CA certificate; browsers by default don't trust it. to get around this issue, visit ```https://localhost:3000``` You will be presented with a screen saying connection is not private. For the purposes of development you can click on Advanced and then proceed anyway. clicking the proceed link wont actually do anything. You can close the window immediately. Then refresh the website at ```http://localhost``` and the websocket should now connect. You shouldn't have this issue on a production server using full ssl.

# Installing node modules
In the dev environment, the node modules are installed on the docker container with a mount to the local computer. Because of this, you don't install new modules locally. Instead, you would add package.json dependencies with the flag `npm install --package-lock-only <package>` then you would `docker-compose build` to rebuild the container. This doesn't apply to the build directory as build tools don't use docker.