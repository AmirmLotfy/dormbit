// Task 3 spike (b): load the wasm build in Node, print its report, and
// re-hash the tar bytes with WebCrypto (an implementation independent of the
// Rust sha2 crate) to confirm they match the digest Rust reported.
import { spike_report, spike_tar } from "./pkg/wasm_spike.js";

const report = spike_report();
console.log(`WASM ${report}`);

const tar = spike_tar();
const digest = await crypto.subtle.digest("SHA-256", tar);
const hex = Buffer.from(digest).toString("hex");
const reported = /tar_sha256=([0-9a-f]+)/.exec(report)?.[1];
console.log(`WEBCRYPTO tar_sha256=${hex}`);
console.log(`WEBCRYPTO_MATCHES_RUST=${hex === reported}`);
