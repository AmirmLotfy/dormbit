/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** The commit ci built from; the footer shows its short form. */
  readonly VITE_BUILD_SHA?: string;
}
