import { useEffect, useState } from "react";
import { ProjectRecordSchema, type ProjectRecord } from "@dormbit/schema";
import type { DormbitClient } from "@dormbit/client";

/** Dev-only list of MockClient projects, each parsed with ProjectRecordSchema (Requirement 13.8). */
export function MockProjects({ client }: { client: DormbitClient }) {
  const [projects, setProjects] = useState<ProjectRecord[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let live = true;
    client
      .projectsList()
      .then((list) => {
        if (live) setProjects(list.map((p) => ProjectRecordSchema.parse(p)));
      })
      .catch(() => {
        if (live) setFailed(true);
      });
    return () => {
      live = false;
    };
  }, [client]);

  if (failed) return <p role="alert">Could not load the mock projects.</p>;
  if (!projects) return <p>Loading mock projects…</p>;
  return (
    <section aria-labelledby="mock-projects">
      <h2 id="mock-projects">Mock projects</h2>
      <ul>
        {projects.map((p) => (
          <li key={p.id}>
            <strong>{p.name.value}</strong> <code>{p.rel_root}</code>
          </li>
        ))}
      </ul>
    </section>
  );
}
