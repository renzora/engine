FROM oven/bun:alpine
WORKDIR /app
COPY server/package.json server/bun.lockb ./server/
RUN cd server && bun install
COPY server/ ./server
EXPOSE 3000
CMD ["bun", "run", "server/server.ts"]
