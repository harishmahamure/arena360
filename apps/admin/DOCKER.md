# Admin Panel - Docker & Kubernetes Deployment

This document describes how to build, deploy, and manage the Admin Panel application using Docker and Kubernetes with Helm.

## 📁 Project Structure

```
apps/admin/
├── docker/
│   ├── nginx.conf           # Nginx configuration for SPA
│   ├── entrypoint.sh        # Runtime env injection script
│   ├── env-config.js        # Local development config
│   └── env-config.js.template  # Runtime config template
├── helm/
│   └── admin-panel/
│       ├── Chart.yaml       # Helm chart metadata
│       ├── values.yaml      # Default values
│       ├── values-uat.yaml  # UAT environment values
│       ├── values-prod.yaml # Production environment values
│       └── templates/       # Kubernetes manifests
├── Dockerfile               # Multi-stage Docker build
├── .dockerignore            # Docker build exclusions
├── .env.local               # UAT environment config
├── .env.prod                # Production environment config
└── .env.example             # Template for local development
```

## 🐳 Docker

### Building the Docker Image

```bash
# Navigate to project root
cd /path/to/admin-panel

# Build for UAT
docker build \
  --build-arg VITE_API_URL=https://api-uat.example.com \
  --build-arg VITE_GATEWAY_URL=wss://gateway-uat.example.com \
  -t admin-panel:uat \
  -f apps/admin/Dockerfile .

# Build for Production
docker build \
  --build-arg VITE_API_URL=https://api.example.com \
  --build-arg VITE_GATEWAY_URL=wss://gateway.example.com \
  -t admin-panel:latest \
  -f apps/admin/Dockerfile .

# Build with placeholder values (for runtime injection)
docker build \
  -t admin-panel:latest \
  -f apps/admin/Dockerfile .
```

### Running Locally

```bash
# Run with environment variables
docker run -d \
  -p 8080:80 \
  -e VITE_API_URL=http://localhost:3001 \
  -e VITE_GATEWAY_URL=ws://localhost:3002 \
  --name admin-panel \
  admin-panel:latest

# Check health
curl http://localhost:8080/health

# View logs
docker logs -f admin-panel
```

### Push to Registry

```bash
# Tag for registry
docker tag admin-panel:latest your-registry.com/admin-panel:latest

# Push
docker push your-registry.com/admin-panel:latest
```

## ☸️ Kubernetes Deployment with Helm

### Prerequisites

- Kubernetes cluster (1.23+)
- Helm 3.x installed
- kubectl configured to access your cluster
- Ingress controller (nginx-ingress recommended)
- cert-manager (optional, for TLS)

### Installing the Chart

```bash
# Navigate to helm directory
cd apps/admin/helm

# Install for UAT
helm install admin-panel ./admin-panel \
  -f ./admin-panel/values-uat.yaml \
  -n uat \
  --create-namespace

# Install for Production
helm install admin-panel ./admin-panel \
  -f ./admin-panel/values-prod.yaml \
  -n production \
  --create-namespace
```

### Upgrading

```bash
# Upgrade UAT with new image
helm upgrade admin-panel ./admin-panel \
  -f ./admin-panel/values-uat.yaml \
  --set image.tag=v1.2.0 \
  -n uat

# Upgrade Production
helm upgrade admin-panel ./admin-panel \
  -f ./admin-panel/values-prod.yaml \
  --set image.tag=v1.2.0 \
  -n production
```

### Overriding Environment Variables

You can override any environment variable using `--set`:

```bash
helm install admin-panel ./admin-panel \
  --set env.VITE_API_URL=https://custom-api.example.com \
  --set env.VITE_GATEWAY_URL=wss://custom-gateway.example.com \
  -n custom-env
```

### Uninstalling

```bash
helm uninstall admin-panel -n uat
helm uninstall admin-panel -n production
```

## 🔧 Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `VITE_API_URL` | Backend API URL | `http://localhost:3001` |
| `VITE_GATEWAY_URL` | WebSocket Gateway URL | `ws://localhost:3002` |

### How Environment Injection Works

1. **Build Time**: The Dockerfile builds the app with placeholder values (`__VITE_API_URL__`, etc.)
2. **Runtime**: The `entrypoint.sh` script runs at container startup and:
   - Replaces placeholders in bundled JS files with actual environment variable values
   - Generates `env-config.js` from the template with actual values
3. **Application**: The app loads `env-config.js` which sets `window.__ENV__`

This approach allows the same Docker image to be used across different environments by simply changing environment variables.

## 📊 Values Reference

### Key Configuration Options

| Value | Description | Default |
|-------|-------------|---------|
| `replicaCount` | Number of pod replicas | `1` |
| `image.repository` | Docker image repository | `your-registry.com/admin-panel` |
| `image.tag` | Image tag | `""` (uses appVersion) |
| `ingress.enabled` | Enable ingress | `false` |
| `autoscaling.enabled` | Enable HPA | `false` |
| `resources.limits.cpu` | CPU limit | `200m` |
| `resources.limits.memory` | Memory limit | `256Mi` |

See `values.yaml` for complete configuration options.

## 🚀 CI/CD Integration

### GitHub Actions Example

```yaml
name: Build and Deploy

on:
  push:
    branches: [main, uat]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3
      
      - name: Login to Registry
        uses: docker/login-action@v3
        with:
          registry: your-registry.com
          username: ${{ secrets.REGISTRY_USER }}
          password: ${{ secrets.REGISTRY_PASSWORD }}
      
      - name: Build and push
        uses: docker/build-push-action@v5
        with:
          context: .
          file: apps/admin/Dockerfile
          push: true
          tags: your-registry.com/admin-panel:${{ github.sha }}
      
      - name: Deploy to Kubernetes
        run: |
          if [ "${{ github.ref }}" == "refs/heads/uat" ]; then
            helm upgrade --install admin-panel apps/admin/helm/admin-panel \
              -f apps/admin/helm/admin-panel/values-uat.yaml \
              --set image.tag=${{ github.sha }} \
              -n uat
          elif [ "${{ github.ref }}" == "refs/heads/main" ]; then
            helm upgrade --install admin-panel apps/admin/helm/admin-panel \
              -f apps/admin/helm/admin-panel/values-prod.yaml \
              --set image.tag=${{ github.sha }} \
              -n production
          fi
```

## 🔒 Security Notes

- The container runs as non-root user (nginx user, UID 101)
- Security headers are configured in nginx.conf
- Pod security context drops all capabilities
- Network policies can be enabled via `networkPolicy.enabled`

## 📈 Monitoring

Health endpoints:
- `/health` - Liveness probe endpoint
- `/ready` - Readiness probe endpoint

## 🐛 Troubleshooting

### Common Issues

1. **Env variables not injected**: Check if the entrypoint script ran correctly:
   ```bash
   kubectl exec -it <pod-name> -- cat /usr/share/nginx/html/env-config.js
   ```

2. **Pod failing health checks**: Check nginx logs:
   ```bash
   kubectl logs <pod-name>
   ```

3. **Ingress not working**: Verify ingress controller and annotations:
   ```bash
   kubectl describe ingress admin-panel
   ```






