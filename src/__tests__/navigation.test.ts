/**
 * Trap #7 from the indiAccounting handoff, pinned.
 *
 * They shipped multi-user invisible: the screens, the routes and the whole
 * backend landed, and nobody added the entry to the navigation. The guarantee
 * the navigation module exists to provide held perfectly — every surface
 * agreed, and what they agreed was that the feature did not exist.
 *
 * So this walks every route and fails on any the navigation cannot reach.
 */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

import { navItems } from '../navigation';

/**
 * Read the router's paths from source rather than importing it.
 *
 * Importing would pull in every view and need a DOM. The paths are a plain
 * list of string literals, and reading them keeps this test dependency-free.
 */
function routerPaths(): string[] {
  const source = readFileSync(resolve(__dirname, '../router/index.ts'), 'utf8');
  return [...source.matchAll(/path:\s*'([^']+)'/g)].map((m) => m[1]);
}

describe('navigation and routes agree', () => {
  it('every route can be reached from the navigation', () => {
    const navPaths = new Set(navItems.map((i) => i.path));
    const unreachable = routerPaths().filter(
      (path) => !navPaths.has(path) && !path.includes(':') && path !== '/:pathMatch(.*)*'
    );

    expect(
      unreachable,
      `these screens exist but nothing in the navigation points at them, so nobody can find them: ${unreachable.join(', ')}`
    ).toEqual([]);
  });

  it('every navigation entry points at a real route', () => {
    const routes = new Set(routerPaths());
    const broken = navItems.filter((i) => !routes.has(i.path)).map((i) => `${i.name} -> ${i.path}`);

    expect(broken, `navigation entries with no route: ${broken.join(', ')}`).toEqual([]);
  });

  it('navigation paths and names are unique', () => {
    expect(new Set(navItems.map((i) => i.path)).size).toBe(navItems.length);
    expect(new Set(navItems.map((i) => i.name)).size).toBe(navItems.length);
  });

  it('every navigation entry has an icon', () => {
    for (const item of navItems) {
      expect(item.icon.length, `${item.name} has no icon`).toBeGreaterThan(0);
    }
  });

  it('the sharing screen is reachable', () => {
    // The specific thing that went wrong last time, named so a regression
    // says what it broke rather than only that a count changed.
    expect(navItems.map((i) => i.path)).toContain('/sharing');
    expect(routerPaths()).toContain('/sharing');
  });
});
