/** Temporary shell page. Replaced by the design system in native-experience Task 34. */
export function Placeholder({ title }: { title: string }) {
  return (
    <main>
      <h1>{title}</h1>
      <p>Pick up where your project stopped.</p>
    </main>
  );
}
