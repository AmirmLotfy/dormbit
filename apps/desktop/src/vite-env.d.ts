/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Set to `mock` in development to run the UI on MockClient instead of Rust. */
  readonly VITE_DORMBIT_CLIENT?: string;
}
