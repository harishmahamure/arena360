import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { afterEach, describe, expect, it, vi } from 'vitest';
import PricingPolicyEditor from './PricingPolicyEditor';
import { parsePricingPolicy } from './pricingPolicy';

const policy = {
  baseRate: '60',
  roundingScale: 2,
  rules: [
    {
      id: 'night',
      name: 'Night rate',
      priority: 100,
      deviceTypes: ['PC'],
      weekdays: [],
      startTime: '23:00:00',
      endTime: '08:00:00',
      action: { type: 'multiplier', value: '1.25' },
    },
  ],
};
afterEach(cleanup);
describe('visual pricing policy editor', () => {
  it('retains advanced rule properties when changing a base rate', () => {
    const onChange = vi.fn();
    render(
      <PricingPolicyEditor value={JSON.stringify(policy)} onChange={onChange} disabled={false} />,
    );
    fireEvent.change(screen.getByLabelText('Base rate'), { target: { value: '80' } });
    expect(JSON.parse(onChange.mock.calls[0]?.[0])).toEqual({ ...policy, baseRate: '80' });
  });
  it('uses the backend weekday convention, Monday=1', () => {
    const onChange = vi.fn();
    render(
      <PricingPolicyEditor value={JSON.stringify(policy)} onChange={onChange} disabled={false} />,
    );
    fireEvent.click(screen.getByRole('checkbox', { name: 'Mon' }));
    expect(JSON.parse(onChange.mock.calls[0]?.[0]).rules[0].weekdays).toEqual([1]);
  });
  it('keeps malformed advanced JSON editable without crashing the page', () => {
    render(
      <PricingPolicyEditor
        value='{"baseRate":"60","rules":[null]}'
        onChange={vi.fn()}
        disabled={false}
      />,
    );
    expect(screen.getByLabelText('Pricing policy JSON')).toBeInTheDocument();
    expect(() =>
      parsePricingPolicy('{"baseRate":"60","roundingScale":2,"rules":[null]}'),
    ).toThrow();
  });
  it('prevents invalid rates from being saved', () => {
    expect(() => parsePricingPolicy(JSON.stringify({ ...policy, baseRate: '-5' }))).toThrow();
    expect(parsePricingPolicy(JSON.stringify(policy))).toEqual(policy);
  });
});
