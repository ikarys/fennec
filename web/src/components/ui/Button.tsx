/**
 * fennec's one button. Feature components pick a variant and a size and add
 * only layout classes (`shrink-0`, `ml-auto`, `w-full`), never colours or
 * paddings: the look lives here so every screen stays the same.
 */
import type { ButtonHTMLAttributes } from 'react';

export type ButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';
export type ButtonSize = 'sm' | 'md' | 'lg';

const BASE =
  'inline-flex items-center gap-1.5 rounded-lg font-medium transition-colors disabled:pointer-events-none disabled:opacity-50';
/** One line that never shrinks (the default), or a choice whose text wraps. */
const SINGLE_LINE = 'shrink-0 justify-center whitespace-nowrap';
const MULTILINE = 'min-w-0 justify-start py-1 text-left whitespace-normal';

const VARIANTS: Record<ButtonVariant, string> = {
  primary: 'bg-accent text-on-accent hover:bg-accent-strong',
  secondary: 'border border-border bg-raised text-text hover:border-border-strong',
  ghost: 'text-muted hover:bg-raised hover:text-text',
  danger: 'border border-border bg-raised text-text hover:border-err hover:text-err'
};

/** `wrap` is `text` whose height is a minimum: a wrapping choice grows with its text. */
const SIZES: Record<ButtonSize, { text: string; wrap: string; square: string }> = {
  sm: { text: 'h-7 px-2.5 text-xs', wrap: 'min-h-7 px-2.5 text-xs', square: 'size-7 text-xs' },
  md: { text: 'h-8 px-3 text-[0.8125rem]', wrap: 'min-h-8 px-3 text-[0.8125rem]', square: 'size-8 text-[0.8125rem]' },
  /** The composer's send button: the one action of the screen. */
  lg: { text: 'h-10 rounded-xl px-4 text-sm', wrap: 'min-h-10 rounded-xl px-4 text-sm', square: 'size-10 rounded-xl text-sm' }
};

export interface ButtonLook {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** A square button holding only an icon; give it an `aria-label`. */
  iconOnly?: boolean;
  /** A choice whose label and description wrap inside its container. */
  multiline?: boolean;
}

/** The classes of a button, for an element that must stay a link (`<a>`). */
export function buttonClass({ variant = 'secondary', size = 'sm', iconOnly = false, multiline = false }: ButtonLook = {}, layout = ''): string {
  const sizes = SIZES[size];
  const shape = iconOnly
    ? `${SINGLE_LINE} ${sizes.square}`
    : multiline
      ? `${MULTILINE} ${sizes.wrap}`
      : `${SINGLE_LINE} ${sizes.text}`;
  return `${BASE} ${VARIANTS[variant]} ${shape} ${layout}`.trim();
}

export function Button({ variant, size, iconOnly, multiline, className, type = 'button', ...props }: ButtonLook & ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button type={type} className={buttonClass({ variant, size, iconOnly, multiline }, className)} {...props} />;
}
