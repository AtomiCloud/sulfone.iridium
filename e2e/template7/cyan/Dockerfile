FROM oven/bun:1.1.31
WORKDIR /app
LABEL cyanprint.dev=true
COPY package.json .
COPY bun.lock .
RUN bun install
COPY . .
CMD ["bun", "run", "index.ts"]
