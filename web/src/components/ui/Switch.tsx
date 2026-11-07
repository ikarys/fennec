/**
 * An on/off switch: a setting that takes effect at once, where a checkbox reads
 * as a form field. It stops the click so it can sit in a `<summary>` without
 * opening it, and is a plain button so it never submits the form around it.
 */
export function Switch({
  checked,
  onChange,
  label,
  disabled = false
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={label}
      disabled={disabled}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        onChange(!checked);
      }}
      className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
        checked ? 'bg-accent' : 'bg-border-strong'
      }`}
    >
      <span
        className={`inline-block h-4 w-4 rounded-full bg-on-accent shadow transition-transform ${
          checked ? 'translate-x-4' : 'translate-x-0.5'
        }`}
      />
    </button>
  );
}
