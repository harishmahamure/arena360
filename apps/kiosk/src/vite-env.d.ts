/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_URL: string;
  readonly VITE_API_URL_WS?: string;
  readonly VITE_GALLERY_URL?: string;
  readonly VITE_LOGIN_BACKGROUND_VIDEO_URL?: string;
  readonly VITE_KIOSK_LOGO_URL?: string;
  readonly VITE_OFFLINE_GRACE_MINUTES?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
