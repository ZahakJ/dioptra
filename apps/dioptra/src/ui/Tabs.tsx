// Tabs: a row of labels with a 2 px accent underline under the chosen one.
import { useRef, type ReactNode } from 'react';

export interface TabItem<T extends string> {
  value: T;
  label: ReactNode;
  testId?: string;
}

export function Tabs<T extends string>({
  tabs,
  value,
  onChange,
  label,
  className,
}: {
  tabs: TabItem<T>[];
  value: T;
  onChange: (v: T) => void;
  label: string;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const idx = tabs.findIndex((t) => t.value === value);
  return (
    <div
      ref={ref}
      className={'ui-tabs' + (className ? ` ${className}` : '')}
      role="tablist"
      aria-label={label}
      onKeyDown={(e) => {
        const d = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
        if (!d) return;
        e.preventDefault();
        const n = (Math.max(0, idx) + d + tabs.length) % tabs.length;
        onChange(tabs[n]!.value);
        requestAnimationFrame(() => ref.current?.querySelectorAll<HTMLElement>('[role="tab"]')[n]?.focus());
      }}
    >
      {tabs.map((t) => (
        <button
          key={t.value}
          type="button"
          role="tab"
          aria-selected={t.value === value}
          tabIndex={t.value === value ? 0 : -1}
          className="ui-tab"
          data-testid={t.testId}
          onClick={() => onChange(t.value)}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}
