/**
 * Form fields with one look: a text input, a select and a text area. Callers
 * add only width or layout classes.
 */
import type { InputHTMLAttributes, SelectHTMLAttributes, TextareaHTMLAttributes } from 'react';

const FIELD =
  'rounded-md border border-border bg-bg/40 px-2.5 py-1.5 text-sm text-text outline-none transition-colors placeholder:text-faint hover:border-border-strong focus:border-accent focus:ring-1 focus:ring-accent-soft disabled:opacity-60';

export function TextInput({ className = '', type = 'text', ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return <input type={type} className={`${FIELD} ${className}`.trim()} {...props} />;
}

export function Select({ className = '', ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={`${FIELD} pr-7 ${className}`.trim()} {...props} />;
}

export function TextArea({ className = '', ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={`${FIELD} resize-y leading-relaxed ${className}`.trim()} {...props} />;
}

/** A label above its field, with an optional hint below. */
export function FieldRow({ label, hint, htmlFor, children }: { label: string; hint?: string; htmlFor?: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={htmlFor} className="text-xs font-medium text-muted">
        {label}
      </label>
      {children}
      {hint && <p className="m-0 text-xs text-faint">{hint}</p>}
    </div>
  );
}
