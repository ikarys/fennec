// A thin vertical rule between the groups of a bar (the header, the status bar).
export function Divider({ className = 'h-5' }: { className?: string }) {
  return <span aria-hidden className={`mx-1 w-px shrink-0 self-center bg-border ${className}`} />;
}
