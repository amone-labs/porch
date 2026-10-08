/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** PostHog project key; builds without it send no usage events (ADR 0006). */
  readonly VITE_POSTHOG_KEY?: string;
}
