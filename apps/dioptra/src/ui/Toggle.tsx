// A switch for on/off settings that apply at once. The state is also written in words.
export function Toggle({
  checked,
  onChange,
  label,
  labelledBy,
  testId,
  disabled,
  showState = true,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  labelledBy?: string;
  testId?: string;
  disabled?: boolean;
  /** Write "On"/"Off" beside the track (nothing depends on colour alone). */
  showState?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      aria-labelledby={labelledBy}
      className="toggle"
      data-testid={testId}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-track" aria-hidden="true">
        <span className="toggle-knob" />
      </span>
      {showState && <span className="toggle-text">{checked ? 'On' : 'Off'}</span>}
    </button>
  );
}
