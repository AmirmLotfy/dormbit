# ADR-002: Capsule container is an uncompressed tar of per-file gzipped objects

- **Status:** Accepted, Nov 4, 2026
- **Task:** P00-T03
- **Evidence:** [docs/evidence/P00-T03-spikes.md](../evidence/P00-T03-spikes.md)

## Context
A `.dormbit` capsule must be:
1. readable with standard tools, so users aren't locked in;
2. byte-deterministic, so golden tests and native/wasm parity work;
3. able to pinpoint which file is damaged, rather than failing as a whole;
4. safe to parse as untrusted input.

Options considered: a zip, a single `.tar.gz`, a custom binary format, and an uncompressed tar of individually compressed objects.

## Decision
An **uncompressed ustar archive** with fixed headers (mtime from the manifest, uid/gid 0, mode 0644/0755) containing, in order:
1. `DORMBIT` marker (`dormbit-capsule/1`)
2. `manifest.json`
3. `objects/<sha256>`: one gzip stream per unique file content, gzip mtime 0
4. `END` trailer with the manifest digest and object count

## Why
- **Standard tooling:** `tar -tvf` lists it; any object can be extracted and gunzipped by hand.
- **Determinism:** the spike produced identical bytes natively and in wasm32 with fixed headers and gzip mtime 0.
- **Damage isolation:** each object has its own stored hash and content hash, so corruption in one object names one file. A single `.tar.gz` would make one flipped byte unrecoverable for everything after it.
- **Parsing safety:** tar entry names are never used as output paths; the verifier checks the manifest and object set match exactly, and restore writes only manifest paths that pass validation.
- **Zip rejected:** central-directory/local-header mismatches are a known source of parser confusion, and zip timestamps are lower resolution.
- **Custom format rejected:** loses the standard-tooling escape hatch.

## Consequences
- Compression ratio is slightly worse than solid compression on many small similar files. Acceptable for project-sized capsules.
- The verifier must enforce zero padding and header checksums itself; it can't rely on the `tar` crate's lenient reading.
- Identical files are stored once (content addressing).
