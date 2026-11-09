/** The answer without its trailing `## Sources` section, and that section's body; sources is null when the text ends with none. */
export function splitSources(text: string): { body: string; sources: string | null } {
  const lines = text.split('\n');

  // Use the last matching heading so an earlier `## Sources` does not win.
  let headingIndex = -1;
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    const line = lines[i];
    if (line !== undefined && /^##\s+Sources\s*$/.test(line)) {
      headingIndex = i;
      break;
    }
  }

  if (headingIndex === -1) {
    return { body: text, sources: null };
  }

  // The section is only trailing if no level-2 heading follows it; a later `## `
  // means the answer continues and the Sources text is not a final section.
  for (let i = headingIndex + 1; i < lines.length; i += 1) {
    const line = lines[i];
    if (line !== undefined && /^##\s/.test(line)) {
      return { body: text, sources: null };
    }
  }

  const body = lines.slice(0, headingIndex).join('\n').trimEnd();
  // An empty body after the heading is valid: streaming can show the heading first.
  const sources = lines.slice(headingIndex + 1).join('\n').trim();

  return { body, sources };
}
