/**
 * Splits a model's text into the title a decision row shows folded (its first non-empty line, without
 * Markdown marks) and the rest the row shows opened, so the title is not read twice.
 */
export function splitTitle(text: string): { title: string; rest: string } {
  const lines = text.split('\n');
  const index = lines.findIndex((line) => line.trim() !== '');
  if (index === -1) return { title: '', rest: '' };
  return {
    title: lines[index].replace(/[#*_`]/g, '').trim(),
    rest: lines.slice(index + 1).join('\n').trim()
  };
}
