/** Format pieces for display, including whole boxes when unitsPerBox > 1. */
export function formatTransferQuantity(quantityPieces: number, unitsPerBox: number): string {
  if (unitsPerBox <= 1) {
    return `${quantityPieces} pc${quantityPieces === 1 ? '' : 's'}`;
  }

  const wholeBoxes = Math.floor(quantityPieces / unitsPerBox);
  const remainder = quantityPieces % unitsPerBox;

  if (wholeBoxes > 0 && remainder === 0) {
    return `${wholeBoxes} box${wholeBoxes === 1 ? '' : 'es'} (${quantityPieces} pcs)`;
  }

  if (wholeBoxes > 0) {
    return `${wholeBoxes} box${wholeBoxes === 1 ? '' : 'es'} + ${remainder} pc (${quantityPieces} pcs)`;
  }

  return `${quantityPieces} pc${quantityPieces === 1 ? '' : 's'}`;
}

export function piecesFromQuantityInput(
  quantity: number,
  mode: 'pieces' | 'boxes',
  unitsPerBox: number,
): number {
  const safeQty = Math.max(1, Math.floor(quantity));
  if (mode === 'boxes') {
    return safeQty * Math.max(1, unitsPerBox);
  }
  return safeQty;
}
