//! File contents of the Dockline project tree (foundation Requirement 18). Everything is
//! fictional. Where a file changes over the history, each version is its own constant.

pub const GITIGNORE: &str = "node_modules/\ndist/\ndata/*.sqlite\n.env\n";
pub const NVMRC: &str = "22.13.1\n";

pub const PACKAGE_JSON_V1: &str = r#"{
  "name": "dockline-invoices",
  "version": "0.0.1",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "api": "tsx server/index.ts",
    "test": "vitest run",
    "db:push": "drizzle-kit push"
  },
  "dependencies": {
    "@fastify/cors": "^10.0.2",
    "better-sqlite3": "^11.8.1",
    "drizzle-orm": "^0.39.3",
    "fastify": "^5.2.1",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@vitejs/plugin-react": "^4.3.4",
    "drizzle-kit": "^0.30.4",
    "tsx": "^4.19.2",
    "typescript": "^5.7.3",
    "vite": "^6.1.0",
    "vitest": "^3.0.5"
  }
}
"#;

pub const PACKAGE_JSON_V2: &str = r#"{
  "name": "dockline-invoices",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "api": "tsx server/index.ts",
    "test": "vitest run",
    "db:push": "drizzle-kit push"
  },
  "dependencies": {
    "@fastify/cors": "^10.0.2",
    "better-sqlite3": "^11.8.1",
    "drizzle-orm": "^0.39.3",
    "fastify": "^5.2.1",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@vitejs/plugin-react": "^4.3.4",
    "drizzle-kit": "^0.30.4",
    "tsx": "^4.19.2",
    "typescript": "^5.7.3",
    "vite": "^6.1.0",
    "vitest": "^3.0.5"
  }
}
"#;

pub const TSCONFIG_JSON: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noEmit": true
  },
  "include": ["src", "server"]
}
"#;

pub const VITE_CONFIG: &str = r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: { proxy: { "/api": "http://localhost:3001" } },
});
"#;

pub const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head><meta charset="UTF-8" /><title>Dockline Invoices</title></head>
  <body><div id="root"></div><script type="module" src="/src/main.tsx"></script></body>
</html>
"#;

pub const MAIN_TSX: &str = r#"import { createRoot } from "react-dom/client";
import { App } from "./App";

createRoot(document.getElementById("root")!).render(<App />);
"#;

pub const APP_TSX: &str = r#"import { Invoices } from "./pages/Invoices";

export function App() {
  return (
    <main>
      <h1>Dockline Invoices</h1>
      <Invoices />
    </main>
  );
}
"#;

pub const README_MD: &str = "# Dockline Invoices\n\nInvoicing for a small marine supply shop: customers, invoices with line items and tax, and\n(soon) PDF export.\n\n## Run it\n\n```sh\nnpm install\nnpm run db:push\nnpm run api   # http://localhost:3001\nnpm run dev   # http://localhost:5173\n```\n\nCopy `.env.example` to `.env` first.\n";

pub const SERVER_INDEX: &str = r#"import Fastify from "fastify";
import cors from "@fastify/cors";
import { health } from "./routes/health";
import { invoices } from "./routes/invoices";

const app = Fastify({ logger: true });
await app.register(cors);
await app.register(health);
await app.register(invoices, { prefix: "/api" });
await app.listen({ port: 3001 });
"#;

pub const HEALTH_TS: &str = r#"import type { FastifyInstance } from "fastify";

export async function health(app: FastifyInstance) {
  app.get("/health", async () => ({ ok: true }));
}
"#;

pub const DRIZZLE_CONFIG: &str = r#"import { defineConfig } from "drizzle-kit";

export default defineConfig({
  dialect: "sqlite",
  schema: "./server/db/schema.ts",
  dbCredentials: { url: "data/dev.sqlite" },
});
"#;

pub const DB_SCHEMA: &str = r#"import { integer, real, sqliteTable, text } from "drizzle-orm/sqlite-core";

export const customers = sqliteTable("customers", {
  id: integer("id").primaryKey(),
  name: text("name").notNull(),
  email: text("email"),
});

export const invoices = sqliteTable("invoices", {
  id: integer("id").primaryKey(),
  customerId: integer("customer_id").references(() => customers.id),
  issuedOn: text("issued_on").notNull(),
  taxRate: real("tax_rate").notNull().default(0.0825),
});

export const lineItems = sqliteTable("line_items", {
  id: integer("id").primaryKey(),
  invoiceId: integer("invoice_id").references(() => invoices.id),
  description: text("description").notNull(),
  quantity: integer("quantity").notNull(),
  unitCents: integer("unit_cents").notNull(),
});
"#;

pub const INVOICES_ROUTE_V1: &str = r#"import type { FastifyInstance } from "fastify";
import { db } from "../db/client";
import { invoices } from "../db/schema";

export async function invoices(app: FastifyInstance) {
  app.get("/invoices", async () => db.select().from(invoices));
}
"#;

pub const INVOICES_ROUTE_V2: &str = r#"import type { FastifyInstance } from "fastify";
import { db } from "../db/client";
import { invoices } from "../db/schema";

const PAGE = 25;

export async function invoices(app: FastifyInstance) {
  app.get<{ Querystring: { page?: string } }>("/invoices", async (req) => {
    const page = Math.max(0, Number(req.query.page ?? 0));
    return db.select().from(invoices).limit(PAGE).offset(page * PAGE);
  });
}
"#;

/// The change that is stashed ("rate limits"), never committed.
pub const INVOICES_ROUTE_STASHED: &str = r#"import type { FastifyInstance } from "fastify";
import { db } from "../db/client";
import { invoices } from "../db/schema";

const PAGE = 25;
const PER_MINUTE = 60;

export async function invoices(app: FastifyInstance) {
  app.get<{ Querystring: { page?: string } }>(
    "/invoices",
    { config: { rateLimit: { max: PER_MINUTE, timeWindow: "1 minute" } } },
    async (req) => {
      const page = Math.max(0, Number(req.query.page ?? 0));
      return db.select().from(invoices).limit(PAGE).offset(page * PAGE);
    },
  );
}
"#;

pub const INVOICES_PAGE: &str = r#"import { useEffect, useState } from "react";

type Invoice = { id: number; issuedOn: string };

export function Invoices() {
  const [rows, setRows] = useState<Invoice[]>([]);
  useEffect(() => {
    fetch("/api/invoices").then((r) => r.json()).then(setRows);
  }, []);
  return <ul>{rows.map((i) => <li key={i.id}>#{i.id} · {i.issuedOn}</li>)}</ul>;
}
"#;

pub const CUSTOMERS_ROUTE: &str = r#"import type { FastifyInstance } from "fastify";
import { db } from "../db/client";
import { customers } from "../db/schema";

export async function customersRoutes(app: FastifyInstance) {
  app.get("/customers", async () => db.select().from(customers));
  app.post<{ Body: { name: string; email?: string } }>("/customers", async (req, reply) => {
    if (!req.body.name.trim()) return reply.code(400).send({ error: "name is required" });
    return db.insert(customers).values(req.body).returning();
  });
}
"#;

pub const MONEY_V1: &str = r#"export type Line = { quantity: number; unitCents: number };

export function subtotalCents(lines: Line[]): number {
  return lines.reduce((sum, l) => sum + l.quantity * l.unitCents, 0);
}

export function taxCents(subtotal: number, rate: number): number {
  return subtotal * rate;
}
"#;

pub const MONEY_V2: &str = r#"export type Line = { quantity: number; unitCents: number };

export function subtotalCents(lines: Line[]): number {
  return lines.reduce((sum, l) => sum + l.quantity * l.unitCents, 0);
}

/** Rounds half up to whole cents, once, on the subtotal. */
export function taxCents(subtotal: number, rate: number): number {
  return Math.round(subtotal * rate);
}
"#;

pub const SEED_TS: &str = r#"import { db } from "./client";
import { customers, invoices } from "./schema";

await db.insert(customers).values([
  { id: 1, name: "Harbor Light Charters", email: "billing@harborlight.invalid" },
  { id: 2, name: "Pelican Point Marina", email: "office@pelicanpoint.invalid" },
]);
await db.insert(invoices).values([{ id: 1, customerId: 1, issuedOn: "2025-03-20" }]);
"#;

pub const ENV_EXAMPLE: &str = "DATABASE_URL=\nSESSION_SECRET=\nSMTP_PASSWORD=\n";
/// Ignored by `.gitignore`. Every value says `fake`, and none looks like a real secret.
pub const ENV: &str =
    "DATABASE_URL=fake://localhost/dockline\nSESSION_SECRET=fake\nSMTP_PASSWORD=fake\n";

pub const MONEY_TEST: &str = r#"import { describe, expect, it } from "vitest";
import { subtotalCents, taxCents } from "./money";

describe("money", () => {
  it("adds line items", () => {
    expect(subtotalCents([{ quantity: 2, unitCents: 1250 }, { quantity: 1, unitCents: 499 }])).toBe(2999);
  });
  it("rounds tax to whole cents", () => {
    expect(taxCents(2999, 0.0825)).toBe(247);
  });
});
"#;

pub const INVOICE_DETAIL_V1: &str = r#"export function InvoiceDetail({ id }: { id: number }) {
  return (
    <section>
      <h2>Invoice #{id}</h2>
    </section>
  );
}
"#;

pub const INVOICE_DETAIL_V2: &str = r#"import { renderInvoice } from "../pdf/renderInvoice";

export function InvoiceDetail({ id }: { id: number }) {
  return (
    <section>
      <h2>Invoice #{id}</h2>
      <button onClick={() => void renderInvoice(id)}>Download PDF</button>
    </section>
  );
}
"#;

pub const ROADMAP_V1: &str = "# Roadmap\n\n- [x] Customers and invoices\n- [x] Tax rates per invoice\n- [ ] PDF export (spec in `.kiro/specs/pdf-export`)\n- [ ] Email invoices to customers\n- [ ] Rate limits on the API\n";
/// Working-tree change: modified, not committed.
pub const ROADMAP_V2: &str = "# Roadmap\n\n- [x] Customers and invoices\n- [x] Tax rates per invoice\n- [ ] PDF export (spec in `.kiro/specs/pdf-export`), totals still wrong\n- [ ] Email invoices to customers\n- [ ] Rate limits on the API (started, see stash)\n- [ ] Multi-currency\n";

pub const CHANGELOG_V1: &str = "# Changelog\n\n## Unreleased\n\n- Customers, invoices, and line items\n- Tax rates per invoice\n- Paginated invoice list\n";
pub const CHANGELOG_V2: &str = "# Changelog\n\n## 0.1.0 (2025-05-02)\n\n- Customers, invoices, and line items\n- Tax rates per invoice\n- Paginated invoice list\n- Tax rounds once, on the subtotal\n";

pub const PDF_REQUIREMENTS: &str = "# PDF export\n\n## Requirement 1\n\nWHEN the user clicks Download PDF on an invoice, THE app SHALL produce a PDF with the header,\nline items, subtotal, tax, and total.\n";

const TASK_TITLES: [&str; 7] = [
    "Add a renderInvoice entry point",
    "Lay out the header and line items",
    "Wire the Download PDF button",
    "Compute totals with the shared money helpers",
    "Embed the shop logo",
    "Snapshot-test the rendered PDF",
    "Document PDF export in the README",
];

/// `.kiro/specs/pdf-export/tasks.md` with the first `checked` tasks done.
pub fn pdf_tasks(checked: usize) -> String {
    let mut s = String::from("# Implementation plan\n\n");
    for (i, t) in TASK_TITLES.iter().enumerate() {
        let mark = if i < checked { "x" } else { " " };
        s.push_str(&format!("- [{mark}] {}. {t}\n", i + 1));
    }
    s
}

pub const RENDER_V1: &str = r#"export async function renderInvoice(id: number): Promise<Blob> {
  throw new Error(`renderInvoice(${id}) is not implemented yet`);
}
"#;

pub const RENDER_V2: &str = r#"import { PDFDocument, StandardFonts } from "pdf-lib";

export async function renderInvoice(id: number): Promise<Blob> {
  const invoice = await fetch(`/api/invoices/${id}`).then((r) => r.json());
  const doc = await PDFDocument.create();
  const page = doc.addPage();
  const font = await doc.embedFont(StandardFonts.Helvetica);
  page.drawText(`Invoice #${invoice.id}`, { x: 50, y: 760, size: 18, font });
  invoice.lines.forEach((l: { description: string }, i: number) => {
    page.drawText(l.description, { x: 50, y: 720 - i * 18, size: 11, font });
  });
  return new Blob([await doc.save()], { type: "application/pdf" });
}
"#;

pub const RENDER_V3: &str = r#"import { PDFDocument, StandardFonts } from "pdf-lib";
import { subtotalCents, taxCents } from "../lib/money";

export async function renderInvoice(id: number): Promise<Blob> {
  const invoice = await fetch(`/api/invoices/${id}`).then((r) => r.json());
  const doc = await PDFDocument.create();
  const page = doc.addPage();
  const font = await doc.embedFont(StandardFonts.Helvetica);
  page.drawText(`Invoice #${invoice.id}`, { x: 50, y: 760, size: 18, font });
  invoice.lines.forEach((l: { description: string }, i: number) => {
    page.drawText(l.description, { x: 50, y: 720 - i * 18, size: 11, font });
  });
  const subtotal = subtotalCents(invoice.lines);
  // TODO: totals block (subtotal, tax, total) aligned right
  // FIXME: tax is computed per line here, which double-rounds; use taxCents(subtotal) once
  const tax = invoice.lines.reduce((t: number, l: { unitCents: number }) => t + Math.round(l.unitCents * invoice.taxRate), 0);
  page.drawText(`Total ${((subtotal + tax) / 100).toFixed(2)}`, { x: 50, y: 120, size: 12, font });
  return new Blob([await doc.save()], { type: "application/pdf" });
}
"#;

/// Working-tree change: modified, not committed. Keeps the TODO and FIXME.
pub const RENDER_V4: &str = r#"import { PDFDocument, StandardFonts } from "pdf-lib";
import { subtotalCents, taxCents } from "../lib/money";

export async function renderInvoice(id: number): Promise<Blob> {
  const invoice = await fetch(`/api/invoices/${id}`).then((r) => r.json());
  const doc = await PDFDocument.create();
  const page = doc.addPage();
  const font = await doc.embedFont(StandardFonts.Helvetica);
  page.drawText(`Invoice #${invoice.id}`, { x: 50, y: 760, size: 18, font });
  invoice.lines.forEach((l: { description: string }, i: number) => {
    page.drawText(l.description, { x: 50, y: 720 - i * 18, size: 11, font });
  });
  const subtotal = subtotalCents(invoice.lines);
  // TODO: totals block (subtotal, tax, total) aligned right
  // FIXME: tax is computed per line here, which double-rounds; use taxCents(subtotal) once
  const tax = taxCents(subtotal, invoice.taxRate); // trying the shared helper
  page.drawText(`Subtotal ${(subtotal / 100).toFixed(2)}`, { x: 360, y: 140, size: 12, font });
  page.drawText(`Total ${((subtotal + tax) / 100).toFixed(2)}`, { x: 360, y: 120, size: 12, font });
  return new Blob([await doc.save()], { type: "application/pdf" });
}
"#;

pub const RENDER_TEST: &str = r#"import { describe, expect, it } from "vitest";
import { renderInvoice } from "./renderInvoice";

describe("renderInvoice", () => {
  it("returns a PDF blob", async () => {
    const blob = await renderInvoice(1);
    expect(blob.type).toBe("application/pdf");
  });
  it.skip("prints totals that match the invoice", async () => {
    // Waiting on the totals block (see the FIXME in renderInvoice.ts).
  });
});
"#;

/// Untracked note, never added to Git.
pub const NOTES_PDF_IDEAS: &str = "# PDF ideas\n\n- Use the shared taxCents helper so the PDF matches the web total.\n- Logo top left, invoice number top right.\n- Ask Theo whether pdf-lib can embed the marina's font.\n";

pub const DIST_INDEX: &str =
    "<!doctype html><title>Dockline Invoices (build output placeholder)</title>\n";
pub const NODE_MODULES_PLACEHOLDER: &str =
    "placeholder: dependencies are not installed in the fixture\n";

/// `data/dev.sqlite`: the 16-byte SQLite header followed by zeros. Not a usable database.
pub fn dev_sqlite() -> Vec<u8> {
    let mut b = b"SQLite format 3\0".to_vec();
    b.resize(4096, 0);
    b
}

pub const README_V1: &str = "# Dockline Invoices\n\nInvoicing for a small marine supply shop.\n";

pub const PDF_REQUIREMENTS_V1: &str = "# PDF export\n\n## Requirement 1\n\nWHEN the user clicks Download PDF on an invoice, THE app SHALL produce a PDF of it.\n";

pub const RENDER_TEST_V1: &str = r#"import { describe, expect, it } from "vitest";
import { renderInvoice } from "./renderInvoice";

describe("renderInvoice", () => {
  it("returns a PDF blob", async () => {
    const blob = await renderInvoice(1);
    expect(blob.type).toBe("application/pdf");
  });
  it("prints totals that match the invoice", async () => {
    expect(await renderInvoice(1)).toBeTruthy();
  });
});
"#;
