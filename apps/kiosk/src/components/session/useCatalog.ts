import { useEffect, useState } from 'react';
import type { KioskGame } from '../../lib/games';

/** Load a catalog slice (games or tools) once, tracking loading state. */
export function useCatalog(loader: () => Promise<KioskGame[]>): {
  items: KioskGame[];
  loading: boolean;
} {
  const [items, setItems] = useState<KioskGame[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let active = true;
    loader()
      .then((list) => {
        if (active) setItems(list);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [loader]);

  return { items, loading };
}
