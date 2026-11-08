// The apps run under a CSP without 'unsafe-eval'. Zod's JIT probe (`new Function`) would be
// blocked there and reported as a `securitypolicyviolation`, so it is off before any parse.
import { z } from "zod";

z.config({ jitless: true });
