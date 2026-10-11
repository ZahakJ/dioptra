// Human-readable times for provenance: "today 23:18", "yesterday 09:12", "11 Oct 2026, 09:12".
// The engine records ISO 8601 instants; the interface shows them in the viewer's local time.

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const pad = (n: number) => String(n).padStart(2, '0');
const sameDay = (a: Date, b: Date) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();

/** One instant, relative to `now` for today and yesterday, else a short date. */
export function formatWhen(when: string | Date, now: Date = new Date()): string {
  const d = typeof when === 'string' ? new Date(when) : when;
  if (Number.isNaN(d.getTime())) return typeof when === 'string' ? when : '';
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (sameDay(d, now)) return `today ${time}`;
  const y = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  if (sameDay(d, y)) return `yesterday ${time}`;
  return `${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()}, ${time}`;
}

/** ISO 8601 date-times inside a sentence: `2026-10-11T23:18:39.305Z`, with or without an offset. */
const ISO = /\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?/g;

/** Rewrite every ISO instant in `text` with `formatWhen`; "at today 23:18" reads "today at 23:18". */
export function humanizeTimestamps(text: string, now: Date = new Date()): string {
  return text
    .replace(ISO, (m) => formatWhen(m, now))
    .replace(/\bat (today|yesterday) (\d{2}:\d{2})/g, '$1 at $2')
    .replace(/\bat (\d{1,2} [A-Z][a-z]{2} \d{4}), (\d{2}:\d{2})/g, 'on $1 at $2');
}
