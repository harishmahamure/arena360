# Admin Panel — pnpm monorepo multi-stage build
FROM node:20-alpine AS builder

WORKDIR /app

RUN corepack enable && corepack prepare pnpm@9.15.9 --activate

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml turbo.json tsconfig.base.json .npmrc ./
COPY apps/admin/package.json ./apps/admin/
COPY packages/ui/package.json ./packages/ui/
COPY packages/theme/package.json ./packages/theme/
COPY packages/providers/package.json ./packages/providers/
COPY packages/utils/package.json ./packages/utils/
COPY packages/contracts/package.json ./packages/contracts/
COPY packages/tsconfig/package.json ./packages/tsconfig/

RUN pnpm install --frozen-lockfile

COPY apps/admin ./apps/admin
COPY packages ./packages

ARG VITE_API_URL=__VITE_API_URL__
ARG VITE_GATEWAY_URL=__VITE_GATEWAY_URL__
ENV VITE_API_URL=$VITE_API_URL
ENV VITE_GATEWAY_URL=$VITE_GATEWAY_URL

RUN pnpm --filter @gaming-cafe/admin build

FROM nginx:1.25-alpine AS production

RUN apk add --no-cache bash

RUN rm /etc/nginx/conf.d/default.conf

COPY apps/admin/docker/nginx.conf /etc/nginx/nginx.conf
COPY --from=builder /app/apps/admin/dist /usr/share/nginx/html

COPY apps/admin/docker/entrypoint.sh /docker-entrypoint.d/40-inject-env.sh
RUN chmod +x /docker-entrypoint.d/40-inject-env.sh

EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
