import { useMemo, useRef, useState } from 'react';
import type { ThreadItem } from '../../lib/fold';
import { seenImages, type SeenImage } from '../../lib/vision';
import { Button } from '../ui/Button';
import { SectionLabel } from '../ui/SectionLabel';

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
      <SectionLabel>Vision</SectionLabel>
      <p className="m-0 text-[0.6875rem] text-muted">Les images que le modèle a vues : captures, images lues ou jointes. Grisées : sorties de son contexte.</p>
      {partial && <div className="text-[0.6875rem] text-muted">(messages chargés seulement)</div>}
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
              <Button variant="secondary" size="sm" onClick={jumpToThread}>
                Voir dans le fil
              </Button>
              <Button variant="secondary" size="sm" onClick={closeDialog}>
                Fermer
              </Button>
            </div>
          </div>
        )}
      </dialog>
    </section>
  );
}
