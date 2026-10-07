import { expect, it } from 'vitest';
import { type AccessRole, effectiveGrants, normalizeGrants } from './access';

it('unions assigned roles while excluding templates and disabled modules', () => {
  const role = (id: string, permissions: string[], isTemplate = false): AccessRole => ({
    id,
    name: id,
    description: '',
    permissions,
    isTemplate,
    revision: 1,
    memberCount: 0,
  });
  expect(
    effectiveGrants(
      ['a', 'b', 'template'],
      [
        role('a', ['kitchen:read']),
        role('b', ['kitchen:read', 'finance:read']),
        role('template', ['access:manage'], true),
      ],
      [{ key: 'finance', enabled: false, revision: 1 }],
    ),
  ).toEqual(['kitchen:read']);
});
it('write grants include matching read permission but do not add other modules', () => {
  expect(normalizeGrants(['kitchen:write'])).toEqual(['kitchen:write', 'kitchen:read']);
  expect(normalizeGrants(['access:manage'])).toEqual(['access:manage', 'access:read']);
});
