// A drawn checkbox with its label; the whole row is the hit target.
import { Check, Minus } from 'lucide-react';
import type { ReactNode } from 'react';

export function Checkbox({
  checked,
  onChange,
  children,
  label,
  testId,
  disabled,
  indeterminate,
  className,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  children?: ReactNode;
  /** Accessible name when there is no visible label. */
  label?: string;
  testId?: string;
  disabled?: boolean;
  indeterminate?: boolean;
  className?: string;
}) {
  return (
    <button
      type="button"
      role="checkbox"
      aria-checked={indeterminate ? 'mixed' : checked}
      aria-label={label}
      className={'checkbox' + (className ? ` ${className}` : '')}
      data-testid={testId}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    >
      <span className="checkbox-box" aria-hidden="true">
        {indeterminate ? <Minus size={12} strokeWidth={2.25} /> : checked ? <Check size={12} strokeWidth={2.5} /> : null}
      </span>
      {children !== undefined && <span className="checkbox-label">{children}</span>}
    </button>
  );
}
