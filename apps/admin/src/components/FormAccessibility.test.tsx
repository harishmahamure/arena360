import '@testing-library/jest-dom/vitest';
import { FormBuilder } from '@gaming-cafe/ui';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

afterEach(cleanup);
it('gives shared text, credentials, numeric, and choice fields accessible names', () => {
  const values = {
    username: '',
    phone: '',
    password: '',
    name: '',
    balance: 0,
    role: 'player',
    location: 'main',
  };
  render(
    <FormBuilder<typeof values>
      defaultValues={values}
      onSubmit={vi.fn()}
      fields={[
        {
          name: 'location',
          label: 'Location',
          type: 'select',
          options: [{ value: 'main', label: 'Main venue' }],
        },
        { name: 'username', label: 'Username', type: 'username' },
        { name: 'phone', label: 'Phone number', type: 'phone' },
        { name: 'password', label: 'Password', type: 'password' },
        { name: 'name', label: 'Name', type: 'text' },
        { name: 'balance', label: 'Opening balance', type: 'currency' },
        {
          name: 'role',
          label: 'Role',
          type: 'select',
          options: [{ value: 'player', label: 'Player' }],
        },
      ]}
    />,
  );
  for (const label of ['Username', 'Phone number', 'Password', 'Name', 'Opening balance'])
    expect(screen.getByLabelText(label)).toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: 'Role' })).toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: 'Location' })).toBeInTheDocument();
});
