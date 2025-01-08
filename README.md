# Requirements
- Docker Desktop: https://www.docker.com/products/docker-desktop
- WSL (windows) https://learn.microsoft.com/en-us/windows/wsl/install
- Github Desktop: https://desktop.github.com
- MongoDB Software: https://www.mongodb.com/products/tools/compass


# Installing on Windows WSL
- Demo: https://www.youtube.com/watch?v=3JHkjVUPoGU

Open your terminal of choice (make sure docker desktop is running)
```
wsl
```
```
curl -L renzora.net/dev | bash
```

# Start Server
- Make sure docker desktop is running and you're in WSL
```
renzora
```

# Notes
"WebSocket connection to 'wss://localhost:3000/' failed"
- because the local dev server is using a self-signed ssl certificate and not a domain specific CA certificate; browsers by default don't trust it. to get around this issue, visit ```https://localhost:3000``` You will be presented with a screen saying connection is not private. For the purposes of renzora development you can click on Advanced and then proceed anyway. clicking the proceed link wont actually do anything. You can close the window immediately. Then refresh the website at ```http://localhost``` and the websocket should now connect.


# Project location for Github Desktop
- Replace ```<distro>``` with your WSL distro and ```<username>``` with your WSL username
- Example: ```\\wsl$\ubuntu\home\johndoe\docker\web```

Renzora Docker setup:
```
Repo url: https://github.com/renzora/docker

Local path: \\wsl$\<distro>\home\<username>\docker\docker
```

Renzora Web frontend:
```
Repo url: https://github.com/renzora/web

Local path: \\wsl$\<distro>\home\<username>\docker\web
```

Renzora backend server:
```
Repo url: https://github.com/renzora/docker/server

Local path: \\wsl$\<distro>\home\<username>\docker\server
```

# Services
- Server: Nginx (gzip compression)
- Website: ```http://localhost```
- Websocket: ```wss://localhost:3000```

# Renzora Login
- Username: ```admin```
- Password: ```password```
- JWT key: ```key```

# MongoDB
- URI: mongodb://localhost:27017/
- Host: ```localhost```
- Port: ```27017```
- Database Name: ```renzora```
- Mongo Username: ```admin```
- Mongo Password: ```password```

# Tailwind css
to compile tailwindcss and watch for new changes, open up a terminal
```
wsl
```
```
cd ~
```
```
cd docker
```
```
chmod +x tailwindcss
```
```
./tailwindcss -i web/assets/css/style.css -o web/assets/css/output.css --watch
```

# Mac Installation
run the below line in your terminal
```
curl -sSL https://gist.githubusercontent.com/pianoplayerjames/d184f6b83669d1d4bc1caf3f1ade315f/raw/67c45c8fe641e24970111a389ef05978485e585e/setup.sh | zsh
```
