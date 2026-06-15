import { describe, expect, it } from 'vitest';
import { usernameSchema } from './yup-schemas';

describe('usernameSchema', () => {
  it('transforms spaced input to underscores', async () => {
    const result = await usernameSchema.validate('Pranshu  Jha');
    expect(result).toBe('Pranshu_Jha');
  });

  it('rejects usernames that are too short after normalize', async () => {
    await expect(usernameSchema.validate('  ab ')).rejects.toThrow();
  });

  it('accepts legacy dotted usernames', async () => {
    const result = await usernameSchema.validate('Sumit.45');
    expect(result).toBe('Sumit.45');
  });
});
