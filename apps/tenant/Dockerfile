FROM node:20-alpine AS builder

WORKDIR /app
RUN corepack enable && corepack prepare pnpm@9.15.9 --activate

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml turbo.json tsconfig.base.json .npmrc ./
COPY apps/tenant/package.json ./apps/tenant/
COPY packages/ui/package.json ./packages/ui/
COPY packages/theme/package.json ./packages/theme/
COPY packages/providers/package.json ./packages/providers/
COPY packages/utils/package.json ./packages/utils/
COPY packages/contracts/package.json ./packages/contracts/
COPY packages/proto/package.json ./packages/proto/
COPY packages/api-types/package.json ./packages/api-types/
COPY packages/tsconfig/package.json ./packages/tsconfig/
RUN pnpm install --frozen-lockfile

COPY apps/tenant ./apps/tenant
COPY packages ./packages
RUN pnpm --filter @gaming-cafe/tenant build

FROM nginx:1.25-alpine AS production
RUN rm -rf /usr/share/nginx/html/*
COPY apps/tenant/docker/portal.conf.template /etc/nginx/templates/default.conf.template
COPY --from=builder /app/apps/tenant/dist /usr/share/nginx/html
ENV PLATFORM_API_UPSTREAM=http://arena360-backend:3000
ENV NGINX_ENVSUBST_FILTER=PLATFORM_API_UPSTREAM
EXPOSE 80
