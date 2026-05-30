import { useEffect } from 'react';

export interface ToastMessage {
  id: string;
  text: string;
  tone?: 'info' | 'success' | 'warning';
}

interface ToastHostProps {
  toasts: ToastMessage[];
  onDismiss: (id: string) => void;
  /** Auto-dismiss delay in ms (default 5000). */
  ttlMs?: number;
}

export function ToastHost({ toasts, onDismiss, ttlMs = 5000 }: ToastHostProps) {
  useEffect(() => {
    if (toasts.length === 0) return;
    const timers = toasts.map((t) => setTimeout(() => onDismiss(t.id), ttlMs));
    return () => {
      for (const timer of timers) clearTimeout(timer);
    };
  }, [toasts, onDismiss, ttlMs]);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-host" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.tone ?? 'info'}`} role="status">
          {t.text}
        </div>
      ))}
    </div>
  );
}
