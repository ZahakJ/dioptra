// Text fields: the one designed input (and text area) every free-text value uses.
import { forwardRef, type InputHTMLAttributes, type TextareaHTMLAttributes } from 'react';

export interface TextInputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, 'size' | 'title'> {
  size?: 'sm' | 'md';
  mono?: boolean;
  invalid?: boolean;
  testId?: string;
}

export const TextInput = forwardRef<HTMLInputElement, TextInputProps>(function TextInput(
  { size = 'md', mono, invalid, testId, className, ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      type="text"
      spellCheck={false}
      autoComplete="off"
      className={'input' + (size === 'sm' ? ' input-sm' : '') + (mono ? ' mono' : '') + (className ? ` ${className}` : '')}
      aria-invalid={invalid || undefined}
      data-testid={testId}
      {...rest}
    />
  );
});

export const TextArea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement> & { mono?: boolean; testId?: string }>(
  function TextArea({ mono, testId, className, ...rest }, ref) {
    return (
      <textarea
        ref={ref}
        spellCheck={false}
        className={'input textarea' + (mono ? ' mono' : '') + (className ? ` ${className}` : '')}
        data-testid={testId}
        {...rest}
      />
    );
  },
);
