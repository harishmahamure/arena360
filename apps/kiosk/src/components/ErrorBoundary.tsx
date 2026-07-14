import React, { type ErrorInfo, type ReactNode } from 'react';
import { appendKioskLog } from '../lib/bootDiagnostics';

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: Error | null;
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
  zIndex: 10000,
};

const cardStyle: React.CSSProperties = {
  maxWidth: '32rem',
  width: '100%',
  padding: '1.5rem',
  borderRadius: '12px',
  border: '1px solid #2a3550',
  background: '#141a2a',
};

export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  override state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    const stack = info.componentStack?.trim();
    const detail = stack ? `${error.message} ${stack}` : error.message;
    void appendKioskLog('error', `react boundary: ${detail}`);
  }

  override render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    return (
      <div style={overlayStyle} role="alert">
        <div style={cardStyle}>
          <h1 style={{ margin: '0 0 0.75rem', color: '#ff6900', fontSize: '1.25rem' }}>
            Station error
          </h1>
          <p style={{ margin: '0 0 0.5rem', lineHeight: 1.5 }}>{error.message}</p>
          <p style={{ margin: 0, color: '#94a3b8', fontSize: '0.875rem' }}>
            Restart the station. Details are in the Arena360 log file.
          </p>
        </div>
      </div>
    );
  }
}
