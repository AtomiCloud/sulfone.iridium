FROM oven/bun:1.1.30
WORKDIR /app
LABEL cyanprint.dev=true
COPY package.json .
COPY bun.lockb .
RUN bun install
COPY . .
USER bun
CMD [ "bun", "run", "index.ts" ]
