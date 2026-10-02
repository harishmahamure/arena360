#!/bin/bash
set -e

# ============================================
# Runtime Environment Variable Injection
# ============================================
# This script runs at container startup and replaces
# __VITE_*__ placeholders in JS files with actual env values.
#
# How it works:
# 1. Dockerfile builds with placeholder values (e.g., __VITE_API_URL__)
# 2. Vite bakes these placeholders into the JS bundle
# 3. This script replaces placeholders with real values from Helm/K8s env vars

echo "🚀 Injecting runtime environment variables..."

HTML_DIR="/usr/share/nginx/html"

# Replace placeholder values in all JS files
find "${HTML_DIR}" -type f -name "*.js" | while read -r file; do
    if grep -q "__VITE_" "$file" 2>/dev/null; then
        echo "📝 Processing: $file"
        
        # Replace API URL placeholder
        if [ -n "${VITE_API_URL}" ]; then
            sed -i "s|__VITE_API_URL__|${VITE_API_URL}|g" "$file"
        fi
        
        # Replace Gateway URL placeholder
        if [ -n "${VITE_GATEWAY_URL}" ]; then
            sed -i "s|__VITE_GATEWAY_URL__|${VITE_GATEWAY_URL}|g" "$file"
        fi
    fi
done

echo "✅ Environment injection complete!"
echo "   VITE_API_URL: ${VITE_API_URL:-not set}"
echo "   VITE_GATEWAY_URL: ${VITE_GATEWAY_URL:-not set}"
