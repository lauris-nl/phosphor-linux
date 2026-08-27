import { TerminalPanel } from '../shared/TerminalPanel';
import { useSfx } from '../../hooks/useSfx';
import { useState } from 'react';
import type { RecoveryAction } from '../../machines/types';
import { invokeErrorMessage } from '../../lib/api';

interface ErrorStepProps {
  message?: string | null;
  recoverable?: boolean;
  recoveryAction?: RecoveryAction | null;
  errorSource?: 'scan' | 'write' | 'detect' | 'verify' | 'blank' | null;
  onRetry: () => void;
  onReset: () => void;
  onLocateClient: () => Promise<void>;
}

function getRetryLabel(action: RecoveryAction | null | undefined, source?: string | null): string {
  if (action === 'Retry' && (source === 'write' || source === 'blank')) {
    return 'BACK TO READER';
  }
  switch (action) {
    case 'Reconnect':
      return 'RECONNECT';
    case 'Retry':
      return 'RETRY';
    case 'GoBack':
      return 'GO BACK';
    default:
      return 'RETRY';
  }
}

const DETECT_HINTS = [
  'Try a different USB cable (some cables are charge-only)',
  'Check Device Manager for a COM port (Ports section)',
  'PM3 Easy may need CH340 driver — download from wch-ic.com',
  'Antivirus may block proxmark3.exe — add it to exceptions',
];

export function ErrorStep({ message, recoverable, recoveryAction, errorSource, onRetry, onReset, onLocateClient }: ErrorStepProps) {
  const sfx = useSfx();
  const [selectionError, setSelectionError] = useState<string | null>(null);

  const displayMessage = message || 'An unexpected error occurred.';
  const clientRequired = errorSource === 'detect'
    && (displayMessage.includes('Proxmark3 client required')
      || displayMessage.includes('not a compatible Proxmark3 client'));
  const retryLabel = clientRequired ? 'RETRY' : getRetryLabel(recoveryAction, errorSource);
  const showDetectHints = errorSource === 'detect' && !clientRequired && !message?.includes('firmware');

  return (
    <TerminalPanel title="ERROR">
      <div style={{ fontSize: '13px', lineHeight: '1.8' }}>
        <div style={{ color: 'var(--red-bright)', fontWeight: 700, marginBottom: '8px' }}>
          {clientRequired ? '[!!] PROXMARK3 CLIENT REQUIRED' : '[!!] ERROR'}
        </div>

        {clientRequired && (
          <div style={{ marginBottom: '16px', color: 'var(--green-dim)', fontSize: '12px' }}>
            Phosphor needs a separately installed current RRG/Iceman Proxmark3 client.
            Select its executable, or install it in PATH and retry.
            {selectionError && (
              <div style={{ color: 'var(--red-bright)', marginTop: '8px' }}>{selectionError}</div>
            )}
          </div>
        )}

        <div style={{ color: 'var(--red-bright)', marginBottom: showDetectHints ? '12px' : '16px' }}>
          {displayMessage}
        </div>

        {showDetectHints && (
          <div style={{ marginBottom: '16px', fontSize: '12px', lineHeight: '1.8' }}>
            <div style={{ color: 'var(--green-mid)', marginBottom: '4px' }}>
              [?] Troubleshooting:
            </div>
            {DETECT_HINTS.map((hint, i) => (
              <div key={i} style={{ color: 'var(--green-dim)', paddingLeft: '12px' }}>
                {`${i + 1}. ${hint}`}
              </div>
            ))}
          </div>
        )}

        <div style={{ display: 'flex', gap: '12px' }}>
          {clientRequired && (
            <button
              onClick={async () => {
                sfx.action();
                setSelectionError(null);
                try {
                  await onLocateClient();
                } catch (error) {
                  setSelectionError(invokeErrorMessage(error));
                }
              }}
              style={{
                background: 'var(--bg-void)',
                color: 'var(--green-bright)',
                border: '2px solid var(--green-bright)',
                fontFamily: 'var(--font-mono)',
                fontSize: '13px',
                fontWeight: 600,
                padding: '6px 20px',
                cursor: 'pointer',
              }}
            >
              LOCATE PROXMARK3
            </button>
          )}
          {recoverable && (
            <button
              onClick={() => { sfx.action(); onRetry(); }}
              style={{
                background: 'var(--bg-void)',
                color: 'var(--amber)',
                border: '2px solid var(--amber)',
                fontFamily: 'var(--font-mono)',
                fontSize: '13px',
                fontWeight: 600,
                padding: '6px 20px',
                cursor: 'pointer',
              }}
              onMouseEnter={(e) => {
                sfx.hover();
                e.currentTarget.style.background = 'rgba(255, 184, 0, 0.08)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.background = 'var(--bg-void)';
              }}
            >
              {retryLabel}
            </button>
          )}

          <button
            onClick={() => { sfx.action(); onReset(); }}
            style={{
              background: 'var(--bg-void)',
              color: 'var(--green-bright)',
              border: '2px solid var(--green-bright)',
              fontFamily: 'var(--font-mono)',
              fontSize: '13px',
              fontWeight: 600,
              padding: '6px 20px',
              cursor: 'pointer',
            }}
            onMouseEnter={(e) => {
              sfx.hover();
              e.currentTarget.style.background = 'var(--green-ghost)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'var(--bg-void)';
            }}
          >
            RESET
          </button>
        </div>
      </div>
    </TerminalPanel>
  );
}
