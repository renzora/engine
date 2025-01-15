FROM node:latest

WORKDIR /usr/src/app/server

COPY ./server/package*.json ./

RUN npm install

CMD ["npm", "start"]