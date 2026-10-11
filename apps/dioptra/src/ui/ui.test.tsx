import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { nativeAccelerator } from '../app/nativeMenu';
import { Checkbox, NumberField, placeBox, Segmented, Select } from './index';

afterEach(cleanup);

// jsdom has no layout; the popover only needs these to exist.
globalThis.ResizeObserver ??= class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;
Element.prototype.scrollIntoView ??= () => {};

const rect = (left: number, top: number, w: number, h: number) => ({ left, top, right: left + w, bottom: top + h, width: w, height: h });

describe('placeBox', () => {
  it('opens below the anchor, aligned to its start', () => {
    expect(placeBox(rect(100, 100, 80, 24), 200, 150, 'bottom-start', 1000, 800)).toEqual({ x: 100, y: 128 });
  });
  it('flips above when there is no room below', () => {
    expect(placeBox(rect(100, 700, 80, 24), 200, 150, 'bottom-start', 1000, 800)).toEqual({ x: 100, y: 546 });
  });
  it('aligns to the end and stays inside the window', () => {
    expect(placeBox(rect(900, 10, 90, 24), 300, 100, 'bottom-end', 1000, 800).x).toBe(690);
    expect(placeBox(rect(980, 10, 10, 24), 300, 100, 'bottom-start', 1000, 800).x).toBe(692);
  });
  it('opens a submenu to the left at the right edge', () => {
    expect(placeBox(rect(700, 100, 240, 28), 240, 200, 'right-start', 1000, 800).x).toBe(462);
  });
});

describe('nativeAccelerator', () => {
  it('maps Mod chords to CmdOrCtrl', () => {
    expect(nativeAccelerator('Mod+Shift+S', undefined)).toBe('CmdOrCtrl+Shift+S');
    expect(nativeAccelerator('Mod+o', 'global')).toBe('CmdOrCtrl+O');
  });
  it('leaves text editing, grid keys and non-Mod keys to the web view', () => {
    expect(nativeAccelerator('Mod+Z', undefined)).toBeNull();
    expect(nativeAccelerator('Mod+D', 'grid')).toBeNull();
    expect(nativeAccelerator('F9', undefined)).toBeNull();
    expect(nativeAccelerator('Mod+\\', undefined)).toBeNull();
  });
});

function SelectHarness({ onChange }: { onChange: (v: string) => void }) {
  const [v, setV] = useState('apa');
  return (
    <Select
      label="P values"
      testId="sel"
      value={v}
      options={[
        { value: 'apa', label: 'APA' },
        { value: 'nejm', label: 'NEJM' },
        { value: 'gp', label: 'Asterisks' },
        { value: 'exact', label: 'Exact', disabled: true },
      ]}
      onChange={(x) => {
        setV(x);
        onChange(x);
      }}
    />
  );
}

describe('Select', () => {
  it('opens on click, moves with arrows, chooses with Enter', () => {
    const onChange = vi.fn();
    render(<SelectHarness onChange={onChange} />);
    fireEvent.click(screen.getByTestId('sel'));
    const list = screen.getByRole('listbox');
    fireEvent.keyDown(list, { key: 'ArrowDown' });
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(onChange).toHaveBeenCalledWith('nejm');
    expect(screen.getByTestId('sel').dataset.value).toBe('nejm');
    expect(screen.queryByRole('listbox')).toBeNull();
  });
  it('typeahead jumps to a matching option and End skips disabled ones', () => {
    const onChange = vi.fn();
    render(<SelectHarness onChange={onChange} />);
    fireEvent.click(screen.getByTestId('sel'));
    const list = screen.getByRole('listbox');
    fireEvent.keyDown(list, { key: 'a' });
    fireEvent.keyDown(list, { key: 's' });
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(onChange).toHaveBeenLastCalledWith('gp');
    fireEvent.click(screen.getByTestId('sel'));
    fireEvent.keyDown(screen.getByRole('listbox'), { key: 'End' });
    fireEvent.keyDown(screen.getByRole('listbox'), { key: 'Enter' });
    expect(onChange).toHaveBeenLastCalledWith('gp');
  });
  it('Escape closes without choosing', () => {
    const onChange = vi.fn();
    render(<SelectHarness onChange={onChange} />);
    fireEvent.click(screen.getByTestId('sel'));
    fireEvent.keyDown(screen.getByRole('listbox'), { key: 'Escape' });
    expect(screen.queryByRole('listbox')).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe('NumberField', () => {
  it('steps with the arrow keys, ten steps with Shift, clamped', () => {
    const onCommit = vi.fn();
    render(<NumberField label="Width" value={85} min={10} max={100} unit="mm" onCommit={onCommit} />);
    const input = screen.getByRole('textbox', { name: 'Width' });
    fireEvent.keyDown(input, { key: 'ArrowUp' });
    expect(onCommit).toHaveBeenLastCalledWith(86);
    fireEvent.keyDown(input, { key: 'ArrowUp', shiftKey: true });
    expect(onCommit).toHaveBeenLastCalledWith(96);
    fireEvent.keyDown(input, { key: 'ArrowUp', shiftKey: true });
    expect(onCommit).toHaveBeenLastCalledWith(100);
    expect(screen.getByText('mm')).toBeTruthy();
  });
  it('commits on blur, reverts an invalid entry and marks it', () => {
    const onCommit = vi.fn();
    render(<NumberField label="Decimals" value={2} min={0} max={15} integer onCommit={onCommit} />);
    const input = screen.getByRole('textbox', { name: 'Decimals' }) as HTMLInputElement;
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: '99' } });
    expect(input.getAttribute('aria-invalid')).toBe('true');
    fireEvent.blur(input);
    expect(onCommit).not.toHaveBeenCalled();
    expect(input.value).toBe('2');
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: '4' } });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenCalledWith(4);
  });
  it('accepts a decimal comma and empty for automatic', () => {
    const onCommit = vi.fn();
    render(<NumberField label="Line" value={1} onCommit={onCommit} />);
    const input = screen.getByRole('textbox', { name: 'Line' });
    fireEvent.change(input, { target: { value: '0,5' } });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenLastCalledWith(0.5);
    fireEvent.change(input, { target: { value: '' } });
    fireEvent.blur(input);
    expect(onCommit).toHaveBeenLastCalledWith(null);
  });
});

describe('Segmented and Checkbox', () => {
  it('moves the choice with arrow keys', () => {
    const onChange = vi.fn();
    render(
      <Segmented
        label="Ticks"
        value="out"
        onChange={onChange}
        choices={[
          { value: 'out', label: 'Out' },
          { value: 'in', label: 'In' },
          { value: 'cross', label: 'Cross' },
        ]}
      />,
    );
    const radios = screen.getAllByRole('radio');
    expect(radios[0]!.getAttribute('aria-checked')).toBe('true');
    expect(radios[1]!.tabIndex).toBe(-1);
    act(() => {
      fireEvent.keyDown(radios[0]!, { key: 'ArrowRight' });
    });
    expect(onChange).toHaveBeenCalledWith('in');
  });
  it('a checkbox is a named checkbox role that toggles', () => {
    const onChange = vi.fn();
    render(
      <Checkbox checked={false} onChange={onChange}>
        Points
      </Checkbox>,
    );
    const box = screen.getByRole('checkbox', { name: 'Points' });
    expect(box.getAttribute('aria-checked')).toBe('false');
    fireEvent.click(box);
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
