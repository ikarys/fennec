import { useMemo, useRef, useState } from 'react';
import type { ThreadItem } from '../../lib/fold';
import { seenImages, type SeenImage } from '../../lib/vision';

export function VisionSection({ items, partial }: { items: ThreadItem[]; partial: boolean }) {
  const images = useMemo(() => seenImages(items), [items]);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const [selected, setSelected] = useState<SeenImage | null>(null);

  const closeDialog = () => {
    dialogRef.current?.close();
  };

  const jumpToThread = () => {
    const anchorId = selected?.anchorId;
    closeDialog();
    if (anchorId) {
      document.getElementById(anchorId)?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  return (
    <section className="flex flex-col gap-1.5">
      <h2 className="text-[11px] font-semibold tracking-[0.06em] text-muted uppercase">
        {partial ? 'Vision des messages chargés' : 'Vision'}
      </h2>
      {images.length === 0 ? (
        <div className="text-xs text-muted">Aucune image vue.</div>
      ) : (
        <div className="grid grid-cols-3 gap-1.5">
          {images.map((image) => (
            <button
              key={image.id}
              type="button"
              aria-label={`Agrandir ${image.label}`}
              title={image.inContext ? image.label : 'Plus dans le contexte du modèle'}
              onClick={() => {
                setSelected(image);
                dialogRef.current?.showModal();
              }}
              className={`block w-full overflow-hidden rounded-md ${image.inContext ? '' : 'opacity-40'}`}
            >
              <img src={image.src} alt={image.label} className="aspect-square w-full rounded-md border border-border object-cover" />
            </button>
          ))}
        </div>
      )}
      <dialog
        ref={dialogRef}
        aria-label={selected?.label ?? 'Vision'}
        onClick={(event) => {
          if (event.target === dialogRef.current) {
            closeDialog();
          }
        }}
        className="m-auto w-auto max-w-[min(90vw,1100px)] rounded-xl border border-border bg-panel p-4 text-text backdrop:bg-black/60"
      >
        {selected && (
          <div className="flex flex-col gap-3">
            <img src={selected.src} alt={selected.label} className="max-h-[75vh] max-w-full object-contain" />
            <div className="flex flex-col gap-1">
              <div className="font-mono text-xs text-muted">{selected.label}</div>
              {!selected.inContext && <div className="text-xs text-muted">Plus dans le contexte du modèle</div>}
            </div>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={jumpToThread}
                className="rounded-lg border border-border bg-raised px-2.5 py-1 text-xs text-text"
              >
                Voir dans le fil
              </button>
              <button
                type="button"
                onClick={closeDialog}
                className="rounded-lg border border-border bg-raised px-2.5 py-1 text-xs text-text"
              >
                Fermer
              </button>
            </div>
          </div>
        )}
      </dialog>
    </section>
  );
}
