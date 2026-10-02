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
  it('switches a rule to product sales and drops session-only device types', () => {
    const onChange = vi.fn();
    render(
      <PricingPolicyEditor value={JSON.stringify(policy)} onChange={onChange} disabled={false} />,
    );
    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Applies to' }));
    fireEvent.click(screen.getByRole('option', { name: 'Product sales' }));
    const rule = JSON.parse(onChange.mock.calls[0]?.[0]).rules[0];
    expect(rule.target).toBe('products');
    expect(rule.deviceTypes).toEqual([]);
  });
  it('rejects product rules with device types and session rules with products', () => {
    const rule = policy.rules[0];
    const withRule = (patch: object) =>
      JSON.stringify({ ...policy, rules: [{ ...rule, ...patch }] });
    expect(() => parsePricingPolicy(withRule({ target: 'products' }))).toThrow(/device types/);
    expect(() => parsePricingPolicy(withRule({ categories: ['meal'] }))).toThrow(/Session rule/);
    expect(() =>
      parsePricingPolicy(withRule({ target: 'products', deviceTypes: [], categories: ['meal'] })),
    ).not.toThrow();
  });
  it('prevents invalid rates from being saved', () => {
    expect(() => parsePricingPolicy(JSON.stringify({ ...policy, baseRate: '-5' }))).toThrow();
    expect(parsePricingPolicy(JSON.stringify(policy))).toEqual(policy);
  });
});
