import { useEffect, useState } from 'react';
import { getBootDiagnosticsSnapshot, subscribeBootErrors } from '../lib/bootDiagnostics';

interface BootErrorOverlayProps {
  message?: string | null;
  onRetry?: () => void;
}

const overlayStyle: React.CSSProperties = {
  position: 'fixed',
  inset: 0,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  background: '#0b0f1a',
  color: '#e8ecf4',
  fontFamily: 'system-ui, sans-serif',
  padding: '2rem',
  zIndex: 9999,
};

const cardStyle: React.CSSProperties = {
  maxWidth: '36rem',
  width: '100%',
  padding: '1.5rem',
  borderRadius: '12px',
  border: '1px solid #2a3550',
  background: '#141a2a',
};

const buttonStyle: React.CSSProperties = {
  marginTop: '1rem',
  padding: '0.65rem 1.25rem',
  borderRadius: '8px',
  border: 'none',
  background: '#ff6900',
  color: '#0b0f1a',
  fontWeight: 600,
  cursor: 'pointer',
};

export function BootErrorOverlay({ message, onRetry }: BootErrorOverlayProps) {
  const [errors, setErrors] = useState<string[]>([]);
  const [logPath, setLogPath] = useState<string | null>(null);

  useEffect(() => {
    void getBootDiagnosticsSnapshot().then((snap) => {
      setErrors(snap.errors);
      setLogPath(snap.logPath);
    });
    return subscribeBootErrors((snap) => {
      setErrors(snap.errors);
      setLogPath(snap.logPath);
    });
  }, []);

  const lines = [...(message ? [message] : []), ...errors.filter((e) => e !== message)];
  if (lines.length === 0 && !message) return null;

  return (
    <div style={overlayStyle} role="alert">
      <div style={cardStyle}>
        <h1 style={{ margin: '0 0 0.75rem', color: '#ff6900', fontSize: '1.25rem' }}>
          Station failed to start
        </h1>
        {lines.map((line) => (
          <p key={line} style={{ margin: '0 0 0.5rem', lineHeight: 1.5, wordBreak: 'break-word' }}>
            {line}
          </p>
        ))}
        {logPath ? (
          <p style={{ margin: '0.75rem 0 0', color: '#94a3b8', fontSize: '0.8rem' }}>
            Log: {logPath}
          </p>
        ) : null}
        {onRetry ? (
          <button type="button" style={buttonStyle} onClick={() => void onRetry()}>
            Retry
          </button>
        ) : null}
      </div>
    </div>
  );
}
