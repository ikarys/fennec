// About page brand block.
import { useT } from '../../i18n';

export function AboutBrand() {
  const t = useT();

  return (
    <div className="flex flex-col items-center gap-2 pb-2 pt-4 text-center">
      <img src="/emblem.png" alt="" className="h-24 w-24 rounded-2xl" />
      <div className="text-2xl font-semibold tracking-[-0.01em] text-text">fennec</div>
      <p className="m-0 max-w-md text-sm text-muted">{t('aboutTagline')}</p>
    </div>
  );
}
