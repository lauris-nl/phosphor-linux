import { useEffect, useState } from 'react';
import { useSettings } from '../../hooks/useSettings';
import { useSfx } from '../../hooks/useSfx';
import { TerminalPanel } from '../shared/TerminalPanel';
import { getPm3ClientInfo, invokeErrorMessage, locatePm3Client, type Pm3ClientInfo } from '../../lib/api';

export function SettingsView() {
  const { settings, updateSettings } = useSettings();
  const sfx = useSfx();
  const [pm3Client, setPm3Client] = useState<Pm3ClientInfo | null>(null);
  const [pm3Error, setPm3Error] = useState<string | null>(null);

  const refreshClient = () => {
    getPm3ClientInfo()
      .then((info) => { setPm3Client(info); setPm3Error(null); })
      .catch((error) => { setPm3Client(null); setPm3Error(invokeErrorMessage(error)); });
  };

  useEffect(refreshClient, []);

  const toggleExpert = () => {
    sfx.click();
    updateSettings({ expertMode: !settings.expertMode });
  };

  const statusText = settings.expertMode ? '[ON]' : '[OFF]';
  const statusColor = settings.expertMode ? 'var(--green-bright)' : 'var(--green-dim)';

  return (
    <TerminalPanel title="SETTINGS">
      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
        {/* Expert Mode */}
        <div>
          <div style={{ color: 'var(--green-mid)', fontSize: '13px', fontWeight: 600 }}>
            EXPERT MODE
          </div>
          <div style={{ color: 'var(--green-dim)', fontSize: '12px', marginTop: '4px' }}>
            Allow raw PM3 command input in terminal
          </div>
          <div style={{ marginTop: '8px', fontSize: '13px' }}>
            <span style={{ color: 'var(--green-mid)' }}>STATUS: </span>
            <span
              onClick={toggleExpert}
              onMouseEnter={(e) => {
                sfx.hover();
                e.currentTarget.style.textShadow = '0 0 6px var(--green-bright)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.textShadow = 'none';
              }}
              style={{
                color: statusColor,
                cursor: 'pointer',
                userSelect: 'none',
                fontWeight: 600,
                transition: 'color 0.15s, text-shadow 0.15s',
              }}
            >
              {statusText}
            </span>
          </div>
        </div>
        <div style={{ borderTop: '1px solid var(--green-dim)', paddingTop: '12px' }}>
          <div style={{ color: 'var(--green-mid)', fontSize: '13px', fontWeight: 600 }}>
            PROXMARK3 CLIENT
          </div>
          <div style={{ color: 'var(--green-dim)', fontSize: '12px', marginTop: '4px', overflowWrap: 'anywhere' }}>
            {pm3Client ? pm3Client.path : 'No compatible RRG/Iceman client configured or discovered.'}
          </div>
          {pm3Client && (
            <div style={{ color: 'var(--green-dim)', fontSize: '11px', marginTop: '4px' }}>
              Source: {pm3Client.source} · {pm3Client.version}
            </div>
          )}
          {pm3Error && <div style={{ color: 'var(--amber)', fontSize: '11px', marginTop: '4px' }}>{pm3Error}</div>}
          <button
            onClick={async () => {
              sfx.action();
              try {
                const selected = await locatePm3Client();
                if (selected) { setPm3Client(selected); setPm3Error(null); }
              } catch (error) {
                setPm3Error(invokeErrorMessage(error));
              }
            }}
            style={{
              marginTop: '8px', background: 'var(--bg-void)', color: 'var(--green-bright)',
              border: '1px solid var(--green-bright)', fontFamily: 'var(--font-mono)',
              fontSize: '12px', padding: '5px 12px', cursor: 'pointer',
            }}
          >
            LOCATE PROXMARK3
          </button>
        </div>
      </div>
    </TerminalPanel>
  );
}
