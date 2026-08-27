import { useSfx } from '../../hooks/useSfx';

interface TopBarProps {
  connectionState: 'connected' | 'disconnected' | 'client-required';
  onDisconnect?: () => void;
}

export function TopBar({ connectionState, onDisconnect }: TopBarProps) {
  const sfx = useSfx();
  const connected = connectionState === 'connected';
  const connectionLabel = connected
    ? '[PM3:CONNECTED]'
    : connectionState === 'client-required'
      ? '[PM3:CLIENT REQUIRED]'
      : '[PM3:DISCONNECTED]';

  return (
    <div
      style={{
        height: '32px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '0 12px',
        background: 'var(--bg-panel)',
        borderBottom: '1px solid var(--green-dim)',
        fontFamily: 'var(--font-mono)',
        fontSize: '13px',
        zIndex: 10,
        position: 'relative',
      }}
    >
      <div style={{ color: 'var(--green-mid)', fontWeight: 600 }}>
        PHOSPHOR v0.2.1
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
        <div
          style={{
            color: connected ? 'var(--green-bright)'
              : connectionState === 'client-required' ? 'var(--amber)'
                : 'var(--red-bright)',
            fontWeight: 500,
          }}
        >
          {connectionLabel}
        </div>
        {connected && onDisconnect && (
          <div
            onClick={() => { sfx.click(); onDisconnect(); }}
            style={{
              color: 'var(--green-dim)',
              cursor: 'pointer',
              userSelect: 'none',
            }}
            onMouseEnter={(e) => {
              sfx.hover();
              e.currentTarget.style.color = 'var(--green-bright)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.color = 'var(--green-dim)';
            }}
          >
            [DISCONNECT]
          </div>
        )}
      </div>
    </div>
  );
}
