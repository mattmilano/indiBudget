/**
 * A plain calendar date from the backend ("2026-10-20"), as a `Date` at local
 * midnight on that day.
 *
 * `new Date('2026-10-20')` means midnight UTC, which anywhere west of
 * Greenwich is still the evening of the 19th — so a bill due on the 20th was
 * shown as due on the 19th. Anything that is not a plain date is read as is.
 */
export function localDate(value: string): Date {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return new Date(value);
  return new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
}
