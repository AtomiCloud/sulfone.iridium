FROM oven/bun:1.1.30
WORKDIR /app
LABEL cyanprint.dev=true
COPY package.json .
COPY bun.lock .
RUN bun install
COPY . .
USER bun
CMD [ "bun", "run", "index.ts" ]
