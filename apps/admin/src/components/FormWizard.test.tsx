import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import '@testing-library/jest-dom/vitest';
import { FormBuilder } from '@gaming-cafe/ui';
import { Button, TextField } from '@mui/material';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import * as yup from 'yup';
import { GuidedForm, GuidedStep } from './GuidedForm';

afterEach(cleanup);
describe('shared form wizard', () => {
  it('validates each step, preserves values when going back, masks passwords, and submits only after review', async () => {
    const submit = vi.fn();
    render(
      <FormBuilder
        wizard
        defaultValues={{ name: '', password: '' }}
        schema={yup.object({
          name: yup.string().required('Name is required'),
          password: yup.string().min(8).required(),
        })}
        sections={[
          { title: 'Profile', fields: [{ name: 'name', label: 'Name', type: 'text' }] },
          {
            title: 'Security',
            fields: [{ name: 'password', label: 'Password', type: 'password' }],
          },
        ]}
        onSubmit={submit}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(await screen.findByText('Name is required')).toBeInTheDocument();
    expect(submit).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Arena player' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    await waitFor(() => expect(screen.getByLabelText('Password')).toBeVisible());
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'SuperSecret123!' } });
    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(screen.getByLabelText('Name')).toHaveValue('Arena player');
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    await waitFor(() => expect(screen.getByLabelText('Password')).toBeVisible());
    expect(screen.getByLabelText('Password')).toHaveValue('SuperSecret123!');
    fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
    expect(await screen.findByText('••••••••')).toBeInTheDocument();
    expect(screen.getByDisplayValue('SuperSecret123!')).not.toBeVisible();
    expect(submit).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Create' }));
    await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
    expect(submit.mock.calls[0]?.[0]).toEqual({
      name: 'Arena player',
      password: 'SuperSecret123!',
    });
  });
  it('returns cross-step validation errors to their field after review', async () => {
    const submit = vi.fn();
    render(
      <FormBuilder
        wizard
        defaultValues={{ minimum: 1, maximum: 3 }}
        schema={yup.object({
          minimum: yup.number().max(yup.ref('maximum'), 'Minimum exceeds maximum'),
          maximum: yup.number().required(),
        })}
        sections={[
          { title: 'Minimum', fields: [{ name: 'minimum', label: 'Minimum', type: 'number' }] },
          { title: 'Maximum', fields: [{ name: 'maximum', label: 'Maximum', type: 'number' }] },
        ]}
        onSubmit={submit}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    await waitFor(() => expect(screen.getByLabelText('Maximum')).toBeVisible());
    fireEvent.change(screen.getByLabelText('Maximum'), { target: { value: '0' } });
    fireEvent.blur(screen.getByLabelText('Maximum'));
    fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Create' }));
    expect(await screen.findByText('Minimum exceeds maximum')).toBeInTheDocument();
    expect(screen.getByLabelText('Minimum')).toBeVisible();
    expect(submit).not.toHaveBeenCalled();
  });
});

describe('custom workflow wizard', () => {
  function Demo({ submit }: { submit: () => void }) {
    const [name, setName] = useState('');
    return (
      <GuidedForm actions={<Button onClick={submit}>Save record</Button>}>
        <GuidedStep title="Identity">
          <TextField
            required
            label="Supplier name"
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </GuidedStep>
        <GuidedStep title="Notes">
          <TextField label="Notes" />
        </GuidedStep>
      </GuidedForm>
    );
  }
  it('blocks missing required fields and revalidates corrections at final submission', async () => {
    const submit = vi.fn();
    render(<Demo submit={submit} />);
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    expect(screen.getByRole('alert')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText(/Supplier name/), { target: { value: 'Vendor A' } });
    fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
    fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
    expect(screen.getByLabelText(/Supplier name/)).toHaveValue('Vendor A');
    fireEvent.change(screen.getByLabelText(/Supplier name/), { target: { value: '' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save record' }));
    expect(submit).not.toHaveBeenCalled();
    expect(screen.getByLabelText(/Supplier name/)).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Save record' })).not.toBeInTheDocument();
  });
});

it('keeps search selections across steps, shows their labels in review, and clears them on reset', async () => {
  render(
    <QueryClientProvider
      client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
    >
      <FormBuilder
        wizard
        showReset
        defaultValues={{ playerId: '', note: '' }}
        sections={[
          {
            title: 'Choose player',
            fields: [
              {
                name: 'playerId',
                label: 'Player search',
                type: 'search',
                required: true,
                onSearch: async () => [{ id: 'player-123', label: 'Alice' }],
              },
            ],
          },
          { title: 'Session note', fields: [{ name: 'note', label: 'Note', type: 'text' }] },
        ]}
        onSubmit={vi.fn()}
      />
    </QueryClientProvider>,
  );
  fireEvent.change(screen.getByRole('combobox', { name: 'Player search' }), {
    target: { value: 'Ali' },
  });
  fireEvent.click(await screen.findByRole('option', { name: 'Alice' }));
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  await waitFor(() => expect(screen.getByLabelText('Note')).toBeVisible());
  fireEvent.click(screen.getByRole('button', { name: 'Back' }));
  expect(screen.getByText('Alice')).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: 'Continue' }));
  await waitFor(() => expect(screen.getByLabelText('Note')).toBeVisible());
  fireEvent.click(screen.getByRole('button', { name: 'Review details' }));
  await screen.findByRole('button', { name: 'Create' });
  expect(screen.getAllByText('Alice').some((element) => element.tagName === 'DD')).toBe(true);
  fireEvent.click(screen.getByRole('button', { name: 'Reset' }));
  await waitFor(() => expect(screen.queryByText('Alice')).not.toBeInTheDocument());
});
