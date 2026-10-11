// A labelled form field for dialogs: the label above, the control, then a hint or an error.
// The control receives the label's id so it is named for assistive technology.
import { useId, type ReactNode } from 'react';

export function Field({
  label,
  hint,
  error,
  className,
  children,
}: {
  label: ReactNode;
  hint?: ReactNode;
  error?: ReactNode;
  className?: string;
  children: (labelId: string) => ReactNode;
}) {
  const id = useId();
  return (
    <div className={'field' + (className ? ` ${className}` : '')}>
      <span className="field-label" id={id}>
        {label}
      </span>
      {children(id)}
      {error ? <span className="field-error">{error}</span> : hint && <span className="field-hint">{hint}</span>}
    </div>
  );
}
