import { describe, expect, it } from 'vitest';
import { localDate } from '../utils/dates';

describe('localDate', () => {
  it('keeps a plain date on its own day, whatever the time zone', () => {
    const d = localDate('2026-10-20');
    expect([d.getFullYear(), d.getMonth() + 1, d.getDate()]).toEqual([2026, 10, 20]);
    expect(d.getHours()).toBe(0);
  });

  it('reads a full timestamp as it always was', () => {
    expect(localDate('2026-10-20T12:00:00Z').toISOString()).toBe('2026-10-20T12:00:00.000Z');
  });
});
