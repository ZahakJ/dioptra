// Buttons: primary (the one action a surface exists for), secondary, quiet (toolbar and
// inline actions) and danger. Icon-only buttons take a label, shown as a tooltip.
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';

export type ButtonVariant = 'primary' | 'secondary' | 'quiet' | 'danger';

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'title'> {
  variant?: ButtonVariant;
  size?: 'sm' | 'md';
  icon?: ReactNode;
  /** Icon at the end (a chevron on a menu button). */
  trailing?: ReactNode;
  /** Icon-only: this label is the accessible name and the tooltip. */
  iconOnly?: string;
  /** Tooltip for a labelled button. */
  tipText?: string;
  shortcut?: string;
  pressed?: boolean;
  testId?: string;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = 'secondary', size = 'md', icon, trailing, iconOnly, tipText, shortcut, pressed, testId, className, children, type = 'button', ...rest },
  ref,
) {
  const cls =
    'btn btn-' +
    variant +
    (size === 'sm' ? ' btn-sm' : '') +
    (iconOnly ? ' btn-icon' : '') +
    (pressed ? ' is-on' : '') +
    (className ? ` ${className}` : '');
  return (
    <button
      ref={ref}
      type={type}
      className={cls}
      aria-label={iconOnly}
      aria-pressed={pressed}
      data-testid={testId}
      data-tip={iconOnly ?? tipText}
      data-tip-kbd={shortcut}
      {...rest}
    >
      {icon}
      {!iconOnly && children !== undefined && children !== null && <span className="btn-text">{children}</span>}
      {trailing}
    </button>
  );
});
