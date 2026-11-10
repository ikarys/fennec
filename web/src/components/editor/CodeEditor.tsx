/**
 * CodeMirror 6 wrapper for the editor page: syntax highlighting, line numbers,
 * Ctrl/Cmd+S save, and external reloads.
 */
import { useEffect, useRef } from 'react';
import { Compartment, EditorState, Prec } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import { basicSetup } from 'codemirror';
import { HighlightStyle, LanguageDescription, syntaxHighlighting } from '@codemirror/language';
import { languages } from '@codemirror/language-data';
import { tags as t } from '@lezer/highlight';

const highlightStyle = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.modifier], color: 'var(--accent)' },
  { tag: [t.string, t.special(t.string)], color: 'var(--ok)' },
  { tag: [t.number, t.bool, t.null, t.atom], color: 'var(--attention)' },
  { tag: [t.comment, t.lineComment, t.blockComment], color: 'var(--faint)', fontStyle: 'italic' },
  { tag: [t.typeName, t.className, t.namespace], color: 'var(--info)' },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: 'var(--accent-strong)' },
  { tag: [t.propertyName, t.attributeName], color: 'var(--info)' },
  { tag: t.heading, color: 'var(--accent-strong)', fontWeight: 'bold' },
  { tag: [t.link, t.url], color: 'var(--info)', textDecoration: 'underline' },
  { tag: t.emphasis, fontStyle: 'italic' },
  { tag: t.strong, fontWeight: 'bold' },
  { tag: [t.meta, t.processingInstruction], color: 'var(--muted)' },
  { tag: t.invalid, color: 'var(--err)' },
]);

const editorTheme = EditorView.theme({
  '&': {
    height: '100%',
    fontSize: '12.5px',
    backgroundColor: 'var(--code)',
    color: 'var(--text)',
  },
  '.cm-scroller': {
    fontFamily: 'var(--font-mono, ui-monospace, monospace)',
    lineHeight: '1.6',
  },
  '.cm-gutters': {
    backgroundColor: 'var(--code)',
    color: 'var(--faint)',
    border: 'none',
    borderRight: '1px solid var(--border)',
  },
  '.cm-activeLine': {
    backgroundColor: 'color-mix(in srgb, var(--accent) 6%, transparent)',
  },
  '.cm-activeLineGutter': {
    backgroundColor: 'transparent',
    color: 'var(--accent)',
  },
  '.cm-cursor': {
    borderLeftColor: 'var(--accent)',
  },
  '&.cm-focused .cm-selectionBackground, .cm-selectionBackground': {
    backgroundColor: 'color-mix(in srgb, var(--accent) 22%, transparent)',
  },
  '&.cm-focused': {
    outline: 'none',
  },
});

export function CodeEditor({
  value,
  path,
  readOnly,
  onChange,
  onSave,
}: {
  value: string;
  path: string;
  readOnly: boolean;
  onChange: (value: string) => void;
  onSave: () => void;
}) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const initialValueRef = useRef(value);
  const initialReadOnlyRef = useRef(readOnly);
  const onChangeRef = useRef(onChange);
  const onSaveRef = useRef(onSave);
  const readOnlyCompartmentRef = useRef(new Compartment());
  const languageCompartmentRef = useRef(new Compartment());

  onChangeRef.current = onChange;
  onSaveRef.current = onSave;

  useEffect(() => {
    if (!rootRef.current) return;

    const view = new EditorView({
      parent: rootRef.current,
      state: EditorState.create({
        doc: initialValueRef.current,
        extensions: [
          Prec.highest(
            keymap.of([
              {
                key: 'Mod-s',
                preventDefault: true,
                run: () => {
                  onSaveRef.current();
                  return true;
                },
              },
            ]),
          ),
          basicSetup,
          readOnlyCompartmentRef.current.of([
            EditorState.readOnly.of(initialReadOnlyRef.current),
            EditorView.editable.of(!initialReadOnlyRef.current),
          ]),
          languageCompartmentRef.current.of([]),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) {
              onChangeRef.current(update.state.doc.toString());
            }
          }),
          editorTheme,
          syntaxHighlighting(highlightStyle),
        ],
      }),
    });

    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
  }, []);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;

    const current = view.state.doc.toString();
    if (current !== value) {
      view.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      });
    }
  }, [value]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;

    view.dispatch({
      effects: readOnlyCompartmentRef.current.reconfigure([
        EditorState.readOnly.of(readOnly),
        EditorView.editable.of(!readOnly),
      ]),
    });
  }, [readOnly]);

  useEffect(() => {
    const view = viewRef.current;
    if (!view) return;

    let cancelled = false;
    const description = LanguageDescription.matchFilename(languages, path);

    if (!description) {
      view.dispatch({ effects: languageCompartmentRef.current.reconfigure([]) });
      return;
    }

    void description.load().then((language) => {
      if (cancelled || !viewRef.current) return;
      viewRef.current.dispatch({
        effects: languageCompartmentRef.current.reconfigure(language ? [language] : []),
      });
    });

    return () => {
      cancelled = true;
    };
  }, [path]);

  return <div ref={rootRef} className="h-full min-h-0 overflow-hidden rounded-md border border-border" />;
}
