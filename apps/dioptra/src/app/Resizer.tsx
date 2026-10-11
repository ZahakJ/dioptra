// A vertical splitter between panels: drag, or focus it and use the arrow keys.
export function Resizer({
  value,
  onChange,
  side,
  label,
  min,
  max,
}: {
  value: number;
  onChange: (v: number) => void;
  /** Which panel the splitter resizes: the one to its left or right. */
  side: 'left' | 'right';
  label: string;
  min: number;
  max: number;
}) {
  const sign = side === 'left' ? 1 : -1;
  return (
    <div
      className="resizer"
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onMouseDown={(e) => {
        e.preventDefault();
        const x0 = e.clientX;
        const v0 = value;
        const move = (m: MouseEvent) => onChange(v0 + sign * (m.clientX - x0));
        const up = () => {
          window.removeEventListener('mousemove', move);
          window.removeEventListener('mouseup', up);
          document.body.classList.remove('is-resizing');
        };
        document.body.classList.add('is-resizing');
        window.addEventListener('mousemove', move);
        window.addEventListener('mouseup', up);
      }}
      onKeyDown={(e) => {
        const step = e.shiftKey ? 48 : 12;
        if (e.key === 'ArrowLeft') onChange(value - sign * step);
        else if (e.key === 'ArrowRight') onChange(value + sign * step);
        else return;
        e.preventDefault();
      }}
    />
  );
}
