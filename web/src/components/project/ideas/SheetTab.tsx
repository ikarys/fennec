/**
 * The task sheet of the drawer, edited by hand like a ticket: one section per
 * field, saved on blur, with a model suggestion to accept or reject per field.
 * Completeness and provenance come from the board.
 */
import { useRef, useState, type ReactNode } from 'react';
import { TextArea } from '../../ui/Field';
import { CriteriaEditor } from './CriteriaEditor';
import { FIELD_HELP, SheetSection } from './SheetSection';
import {
  saveSheet,
  suggestSheetField,
  type IdeaBoard,
  type IdeaView,
  type SheetField,
  type SheetSuggestion,
  type TaskSheet,
} from '../../../lib/ideas';
import { useT } from '../../../i18n';

type TextField = Exclude<SheetField, 'doneWhen'>;

const TEXT_FIELDS: TextField[] = ['objective', 'context', 'constraints', 'outOfScope', 'deliverable', 'approach'];

export function SheetTab({
  idea,
  sheet,
  projectId,
  run,
  editable,
}: {
  idea: IdeaView;
  sheet: TaskSheet;
  projectId: string;
  run: (action: () => Promise<IdeaBoard>) => Promise<boolean>;
  editable: boolean;
}) {
  const t = useT();
  const [drafts, setDrafts] = useState<Partial<Record<SheetField, SheetSuggestion>>>({});
  const [suggesting, setSuggesting] = useState<SheetField | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Edits merge into the latest sheet, not the one of the last render: two fields
  // blurred before the board comes back must not overwrite each other.
  const latest = useRef(sheet);
  const seen = useRef(sheet);
  if (seen.current !== sheet) {
    seen.current = sheet;
    latest.current = sheet;
  }
  const save = (change: Partial<TaskSheet>, by?: 'agent') => {
    latest.current = { ...latest.current, ...change };
    const next = latest.current;
    void run(() => saveSheet(projectId, idea.id, next, by));
  };

  const suggest = async (field: SheetField) => {
    setSuggesting(field);
    setError(null);
    try {
      const suggestion = await suggestSheetField(projectId, idea.id, field);
      setDrafts((d) => ({ ...d, [field]: suggestion }));
    } catch (err) {
      setError(err instanceof Error ? err.message : t('unknownError'));
    } finally {
      setSuggesting(null);
    }
  };

  const clear = (field: SheetField) =>
    setDrafts((d) => {
      const { [field]: _dropped, ...rest } = d;
      return rest;
    });

  const accept = (field: SheetField) => {
    const s = drafts[field];
    if (!s) return;
    const current = latest.current;
    clear(field);
    save(
      'text' in s
        ? { [field]: s.text }
        : { doneWhen: [...current.doneWhen.filter((c) => c.text.trim()), ...s.criteria.map((text) => ({ text }))] },
      'agent',
    );
  };

  const draftView = (field: SheetField) => {
    const s = drafts[field];
    if (!s) return null;
    return 'text' in s ? (
      <div className="whitespace-pre-wrap">{s.text}</div>
    ) : (
      <ul className="m-0 list-disc pl-5">
        {s.criteria.map((c, i) => (
          <li key={i}>{c}</li>
        ))}
      </ul>
    );
  };

  const section = (field: SheetField, required: boolean, body: ReactNode) => (
    <SheetSection
      key={field}
      field={field}
      required={required}
      model={idea.modelFields.includes(field)}
      editable={editable}
      suggesting={suggesting === field}
      onSuggest={() => void suggest(field)}
      draft={draftView(field)}
      onAccept={() => accept(field)}
      onReject={() => clear(field)}
    >
      {body}
    </SheetSection>
  );

  return (
    <div className="flex flex-col gap-3">
      {error ? (
        <p className="m-0 text-xs text-err" role="alert">
          {error}
        </p>
      ) : null}
      {TEXT_FIELDS.filter((f) => f !== 'approach' || sheet.approach.trim()).map((field) =>
        section(
          field,
          field === 'objective',
          <TextArea
            key={sheet[field]}
            defaultValue={sheet[field]}
            placeholder={t(FIELD_HELP[field])}
            rows={field === 'objective' ? 2 : 3}
            disabled={!editable}
            className="w-full"
            onBlur={(e) => {
              if (e.target.value !== latest.current[field]) save({ [field]: e.target.value });
            }}
          />,
        ),
      )}
      {section(
        'doneWhen',
        true,
        <CriteriaEditor
          criteria={sheet.doneWhen}
          editable={editable}
          onSave={(doneWhen) => save({ doneWhen })}
        />,
      )}
    </div>
  );
}
