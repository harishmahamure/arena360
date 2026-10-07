import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ProductOptionGroup } from '../../services/product/recipe';
import { PosOptionPicker } from './PosOptionPicker';

const groups: ProductOptionGroup[] = [
  {
    id: 'g1',
    name: 'Patty',
    required: true,
    multiple: false,
    options: [
      { id: 'single', name: 'Single', priceDelta: 0, ingredients: [] },
      { id: 'double', name: 'Double', priceDelta: 40, ingredients: [] },
    ],
  },
  {
    id: 'g2',
    name: 'Extras',
    required: false,
    multiple: true,
    options: [{ id: 'cheese', name: 'Cheese', priceDelta: 15, ingredients: [] }],
  },
];

afterEach(cleanup);
describe('POS option picker', () => {
  it('holds the sale until required groups are chosen and keeps one pick per single group', () => {
    const onConfirm = vi.fn();
    render(
      <PosOptionPicker
        productName="Burger"
        basePrice={150}
        groups={groups}
        onCancel={vi.fn()}
        onConfirm={onConfirm}
      />,
    );
    const add = screen.getByRole('button', { name: 'Add to cart' });
    expect(add).toBeDisabled();
    fireEvent.click(screen.getByLabelText('Single'));
    fireEvent.click(screen.getByLabelText('Double (+₹40.00)'));
    fireEvent.click(screen.getByLabelText('Cheese (+₹15.00)'));
    expect(screen.getByText('₹205.00')).toBeInTheDocument();
    fireEvent.click(add);
    expect(onConfirm).toHaveBeenCalledWith({
      optionIds: ['double', 'cheese'],
      names: ['Double', 'Cheese'],
      priceDelta: 55,
    });
  });
});
