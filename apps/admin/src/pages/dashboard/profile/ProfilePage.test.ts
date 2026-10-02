import { describe, expect, it } from 'vitest';
import { groupPermissions } from './ProfilePage';

describe('groupPermissions', () => {
  it('groups actions by module in alphabetical order', () => {
    expect(groupPermissions(['sessions:write', 'inventory:read', 'sessions:read'])).toEqual([
      ['inventory', ['read']],
      ['sessions', ['write', 'read']],
    ]);
  });
});
