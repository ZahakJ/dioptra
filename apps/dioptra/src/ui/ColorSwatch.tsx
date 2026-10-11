// A colour control: a swatch button that opens the current palette's colours and a hex field.
// "Palette" clears the override so the series takes its colour from the palette again.
import { Check } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Popover } from './Popover';

const HEX = /^#[0-9a-fA-F]{6}$/;

export function ColorSwatch({
  value,
  fallback,
  palette,
  onChange,
  label,
  testId,
}: {
  /** The chosen colour, or undefined when the palette decides. */
  value: string | undefined;
  /** The colour the palette gives when nothing is chosen. */
  fallback: string;
  palette: string[];
  onChange: (hex: string | null) => void;
  label: string;
  testId?: string;
}) {
  const [open, setOpen] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  const shown = value ?? fallback;
  return (
    <>
      <button
        ref={btn}
        type="button"
        className={'colour-btn' + (open ? ' is-open' : '')}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-label={`${label}: ${shown}${value ? '' : ' (palette)'}`}
        data-testid={testId}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="sw sw-lg" style={{ background: shown }} aria-hidden="true" />
        <span className="colour-btn-hex">{shown.toUpperCase()}</span>
        <span className="colour-btn-src">{value ? 'Custom' : 'Palette'}</span>
      </button>
      {open && (
        <Popover anchor={btn.current} open onClose={() => setOpen(false)} className="colour-pop" role="dialog" label={label}>
          <ColourPanel
            value={value}
            fallback={fallback}
            palette={palette}
            label={label}
            onChange={(c) => {
              onChange(c);
            }}
            onDone={() => {
              setOpen(false);
              requestAnimationFrame(() => btn.current?.focus());
            }}
          />
        </Popover>
      )}
    </>
  );
}

function ColourPanel({
  value,
  fallback,
  palette,
  label,
  onChange,
  onDone,
}: {
  value: string | undefined;
  fallback: string;
  palette: string[];
  label: string;
  onChange: (hex: string | null) => void;
  onDone: () => void;
}) {
  const [hex, setHex] = useState(value ?? '');
  const first = useRef<HTMLButtonElement>(null);
  useEffect(() => first.current?.focus(), []);
  const cur = (value ?? '').toLowerCase();
  return (
    <div
      className="colour-panel"
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onDone();
        }
      }}
    >
      <div className="colour-grid" role="radiogroup" aria-label={label}>
        {palette.map((c, i) => (
          <button
            key={c + i}
            ref={i === 0 ? first : undefined}
            type="button"
            role="radio"
            aria-checked={cur === c.toLowerCase()}
            aria-label={c}
            data-tip={c.toUpperCase()}
            className="colour-cell"
            style={{ background: c }}
            onClick={() => {
              onChange(c.toUpperCase());
              onDone();
            }}
          >
            {cur === c.toLowerCase() && <Check size={12} strokeWidth={2.5} />}
          </button>
        ))}
      </div>
      <div className="colour-hex">
        <span className="sw" style={{ background: HEX.test(hex) ? hex : fallback }} aria-hidden="true" />
        <input
          className="input input-sm mono"
          value={hex}
          placeholder={fallback.toUpperCase()}
          aria-label={`${label} hex`}
          aria-invalid={hex !== '' && !HEX.test(hex)}
          spellCheck={false}
          onChange={(e) => setHex(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              if (HEX.test(hex)) {
                onChange(hex.toUpperCase());
                onDone();
              }
            }
          }}
          onBlur={() => HEX.test(hex) && hex.toLowerCase() !== cur && onChange(hex.toUpperCase())}
        />
        <button
          type="button"
          className="btn btn-quiet btn-sm"
          disabled={!value}
          onClick={() => {
            onChange(null);
            onDone();
          }}
        >
          Use palette
        </button>
      </div>
    </div>
  );
}
