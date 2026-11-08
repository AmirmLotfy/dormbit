/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Recording builds only: "1" clicks through the journey for the demo video. */
  readonly VITE_AUTOPILOT?: string;
  /** Set to `mock` in development to run the UI on MockClient instead of Rust. */
  readonly VITE_DORMBIT_CLIENT?: string;
}
