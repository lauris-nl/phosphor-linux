import { useEffect, useState, useCallback } from 'react';
import { TerminalPanel } from '../shared/TerminalPanel';
import { getSavedCards, deleteSavedCard, type SavedCard } from '../../lib/api';
import { useWizard } from '../../hooks/useWizard';

function formatLocalTime(isoStr: string): string {
  const d = new Date(isoStr);
  if (isNaN(d.getTime())) return isoStr.replace('T', ' ').slice(0, 19);
  const pad2 = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}

function formatFrequency(freq: string): string {
  const lower = freq.toLowerCase();
  if (lower.includes('lf') || lower.includes('125') || lower.includes('low')) return '125 kHz';
  if (lower.includes('hf') || lower.includes('13.56') || lower.includes('high')) return '13.56 MHz';
  return freq;
}

function parseDecoded(json: string): Record<string, string> {
  try {
    const parsed = JSON.parse(json);
    if (typeof parsed === 'object' && parsed !== null) {
      const result: Record<string, string> = {};
      for (const [k, v] of Object.entries(parsed)) {
        result[k] = String(v);
      }
      return result;
    }
  } catch { /* malformed JSON */ }
  return {};
}

const buttonStyle: React.CSSProperties = {
  background: 'none',
  border: '1px solid var(--green-dim)',
  color: 'var(--green-dim)',
  fontFamily: 'var(--font-mono)',
  fontSize: '11px',
  padding: '2px 8px',
  cursor: 'pointer',
};

export function SavedView() {
  const [cards, setCards] = useState<SavedCard[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loadedCardKey, setLoadedCardKey] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);
  const wizard = useWizard();

  const refresh = useCallback(() => setRefreshKey(k => k + 1), []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    const start = Date.now();
    getSavedCards()
      .then((data) => {
        if (!cancelled) setCards(data);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          const msg = err instanceof Error ? err.message : 'Failed to load saved cards';
          setError(msg);
        }
      })
      .finally(() => {
        if (cancelled) return;
        const elapsed = Date.now() - start;
        const delay = Math.max(0, 300 - elapsed);
        setTimeout(() => { if (!cancelled) setLoading(false); }, delay);
      });
    return () => { cancelled = true; };
  }, [refreshKey]);

  const handleDelete = async (id: number) => {
    try {
      await deleteSavedCard(id);
      setExpandedId(null);
      setLoadedCardKey(null);
      refresh();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message
        : typeof err === 'object' && err !== null ? (Object.values(err)[0] as string) ?? String(err)
        : String(err);
      setError(msg);
    }
  };

  const handleUseSavedTag = async (card: SavedCard) => {
    if (!wizard.context.port) {
      setLoadError('Connect the Proxmark3 in SCAN before loading a saved tag.');
      return;
    }
    const decoded = parseDecoded(card.decoded);
    setLoadError(null);
    try {
      await wizard.loadSavedCard({
        frequency: card.frequency,
        cardType: card.cardType,
        uid: card.uid,
        raw: card.raw,
        decoded,
        cloneable: card.cloneable,
        recommendedBlank: card.recommendedBlank,
      });
      setLoadedCardKey(`${card.id ?? 'saved'}:${card.uid}`);
    } catch (err: unknown) {
      const message = err instanceof Error ? err.message : String(err);
      setLoadError(`Could not load this saved tag: ${message}`);
    }
  };

  // Loading state
  if (loading) {
    return (
      <TerminalPanel title="SAVED CARDS">
        <div style={{
          fontFamily: 'var(--font-mono)',
          fontSize: '12px',
          color: 'var(--green-dim)',
          padding: '12px 0',
        }}>
          [..] Loading saved cards...
        </div>
      </TerminalPanel>
    );
  }

  // Error state
  if (error) {
    return (
      <TerminalPanel title="SAVED CARDS">
        <div style={{
          fontFamily: 'var(--font-mono)',
          fontSize: '12px',
          color: 'var(--red-bright)',
          padding: '12px 0',
        }}>
          [XX] Error: {error}
        </div>
        <button
          onClick={refresh}
          style={buttonStyle}
          onMouseEnter={(e) => { e.currentTarget.style.color = 'var(--green-bright)'; e.currentTarget.style.borderColor = 'var(--green-bright)'; }}
          onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--green-dim)'; e.currentTarget.style.borderColor = 'var(--green-dim)'; }}
        >
          RETRY
        </button>
      </TerminalPanel>
    );
  }

  // Empty state
  if (cards.length === 0) {
    return (
      <TerminalPanel title="SAVED CARDS">
        <div style={{
          fontFamily: 'var(--font-mono)',
          fontSize: '12px',
          color: 'var(--green-dim)',
          padding: '12px 0',
        }}>
          No saved cards.
        </div>
        <button
          onClick={refresh}
          style={buttonStyle}
          onMouseEnter={(e) => { e.currentTarget.style.color = 'var(--green-bright)'; e.currentTarget.style.borderColor = 'var(--green-bright)'; }}
          onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--green-dim)'; e.currentTarget.style.borderColor = 'var(--green-dim)'; }}
        >
          REFRESH
        </button>
      </TerminalPanel>
    );
  }

  const expanded = expandedId !== null ? cards.find(c => c.id === expandedId) : null;
  const expandedIsLoaded = expanded
    ? loadedCardKey === `${expanded.id ?? 'saved'}:${expanded.uid}`
    : false;

  return (
    <TerminalPanel title="SAVED CARDS">
      {/* Table */}
      <table
        style={{
          width: '100%',
          borderCollapse: 'collapse',
          fontFamily: 'var(--font-mono)',
          fontSize: '12px',
        }}
      >
        <thead>
          <tr>
            {['#', 'NAME', 'TYPE', 'DATE'].map(h => (
              <th
                key={h}
                style={{
                  padding: '4px 8px',
                  textAlign: 'left',
                  color: 'var(--green-mid)',
                  borderBottom: '1px solid var(--green-dim)',
                  fontWeight: 600,
                  fontSize: '11px',
                  letterSpacing: '1px',
                }}
              >
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {cards.map((card, idx) => {
            const isExpanded = card.id === expandedId;
            return (
              <tr
                key={card.id ?? idx}
                onClick={() => setExpandedId(isExpanded ? null : (card.id ?? null))}
                style={{
                  cursor: 'pointer',
                  background: isExpanded ? 'rgba(0, 255, 65, 0.05)' : 'transparent',
                }}
                onMouseEnter={(e) => { if (!isExpanded) e.currentTarget.style.background = 'rgba(0, 255, 65, 0.03)'; }}
                onMouseLeave={(e) => { if (!isExpanded) e.currentTarget.style.background = 'transparent'; }}
              >
                <td style={{ padding: '3px 8px', color: isExpanded ? 'var(--green-bright)' : 'var(--green-mid)', borderBottom: '1px solid rgba(0,255,65,0.1)' }}>{card.id ?? idx + 1}</td>
                <td style={{ padding: '3px 8px', color: isExpanded ? 'var(--green-bright)' : 'var(--green-mid)', borderBottom: '1px solid rgba(0,255,65,0.1)' }}>{card.name}</td>
                <td style={{ padding: '3px 8px', color: isExpanded ? 'var(--green-bright)' : 'var(--green-mid)', borderBottom: '1px solid rgba(0,255,65,0.1)' }}>{card.cardType}</td>
                <td style={{ padding: '3px 8px', color: isExpanded ? 'var(--green-bright)' : 'var(--green-mid)', borderBottom: '1px solid rgba(0,255,65,0.1)' }}>{formatLocalTime(card.createdAt)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>

      {/* Expanded detail panel */}
      {expanded && (
        <div style={{
          marginTop: '12px',
          padding: '8px 12px',
          border: '1px solid var(--green-dim)',
          fontFamily: 'var(--font-mono)',
          fontSize: '12px',
          lineHeight: '1.6',
        }}>
          <div style={{ color: 'var(--green-bright)', marginBottom: '6px' }}>
            {`[::] ${expanded.name} -- detail`}
          </div>
          <div style={{ color: 'var(--green-mid)' }}>
            {`  UID ......... ${expanded.uid}`}
          </div>
          <div style={{ color: 'var(--green-mid)' }}>
            {`  FREQ ........ ${formatFrequency(expanded.frequency)}`}
          </div>
          <div style={{ color: 'var(--green-mid)' }}>
            {`  BLANK ....... ${expanded.recommendedBlank}`}
          </div>
          <div style={{ color: 'var(--green-mid)' }}>
            {`  CLONEABLE ... ${expanded.cloneable ? 'YES' : 'NO'}`}
          </div>
          {expanded.raw && (
            <div style={{ color: 'var(--green-mid)' }}>
              {`  RAW ......... ${expanded.raw}`}
            </div>
          )}
          {/* Decoded fields */}
          {(() => {
            const decoded = parseDecoded(expanded.decoded);
            const keys = Object.keys(decoded);
            if (keys.length === 0) return null;
            return (
              <>
                <div style={{ color: 'var(--green-dim)', marginTop: '4px' }}>
                  {`  ${'─'.repeat(30)}`}
                </div>
                {keys.map(k => (
                  <div key={k} style={{ color: 'var(--green-mid)' }}>
                    {`  ${k.toUpperCase().padEnd(12)} ${decoded[k]}`}
                  </div>
                ))}
              </>
            );
          })()}

          <div style={{ color: 'var(--green-dim)', marginTop: '8px', fontSize: '11px' }}>
            USE SAVED TAG loads this saved data into the workflow. WRITE changes the physical target tag.
          </div>

          {expandedIsLoaded && (
            <div style={{ color: 'var(--green-bright)', marginTop: '8px' }}>
              [+] {expanded.name} is loaded. Open WRITE to detect the target tag.
            </div>
          )}

          {loadError && (
            <div style={{ color: 'var(--amber)', marginTop: '8px' }}>
              [!] {loadError}
            </div>
          )}

          {/* Action buttons */}
          <div style={{ marginTop: '10px', display: 'flex', gap: '8px' }}>
            <button
              onClick={(e) => { e.stopPropagation(); void handleUseSavedTag(expanded); }}
              disabled={!wizard.context.port || expandedIsLoaded}
              style={{
                ...buttonStyle,
                color: wizard.context.port ? 'var(--green-bright)' : 'var(--green-dim)',
                borderColor: wizard.context.port ? 'var(--green-bright)' : 'var(--green-dim)',
                cursor: wizard.context.port && !expandedIsLoaded ? 'pointer' : 'not-allowed',
                opacity: wizard.context.port && !expandedIsLoaded ? 1 : 0.6,
              }}
              onMouseEnter={(e) => { if (wizard.context.port && !expandedIsLoaded) e.currentTarget.style.background = 'rgba(0, 255, 65, 0.1)'; }}
              onMouseLeave={(e) => { e.currentTarget.style.background = 'none'; }}
            >
              {!wizard.context.port
                ? 'CONNECT READER FIRST'
                : expandedIsLoaded
                  ? 'LOADED ✓'
                  : 'USE SAVED TAG'}
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); if (expanded.id !== null) handleDelete(expanded.id); }}
              style={{
                ...buttonStyle,
                color: 'var(--red-bright)',
                borderColor: 'var(--red-bright)',
              }}
              onMouseEnter={(e) => { e.currentTarget.style.background = 'rgba(255, 0, 0, 0.1)'; }}
              onMouseLeave={(e) => { e.currentTarget.style.background = 'none'; }}
            >
              DELETE
            </button>
          </div>
        </div>
      )}

      {/* Footer with count and refresh */}
      <div style={{
        marginTop: '12px',
        fontSize: '11px',
        color: 'var(--green-dim)',
        display: 'flex',
        alignItems: 'center',
        gap: '12px',
      }}>
        <span>{cards.length} record{cards.length !== 1 ? 's' : ''}</span>
        <button
          onClick={refresh}
          style={buttonStyle}
          onMouseEnter={(e) => { e.currentTarget.style.color = 'var(--green-bright)'; e.currentTarget.style.borderColor = 'var(--green-bright)'; }}
          onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--green-dim)'; e.currentTarget.style.borderColor = 'var(--green-dim)'; }}
        >
          REFRESH
        </button>
      </div>
    </TerminalPanel>
  );
}
