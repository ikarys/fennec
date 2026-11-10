/**
 * The foot of the node and edge side panels: the validation issues of the selected
 * element, then its delete button.
 */
import type { WorkflowIssue } from '../../lib/workflow';
import { Button } from '../ui/Button';
import { TrashIcon } from '../ui/Icons';
import { useT } from '../../i18n';

export function PanelFooter({
  issues,
  deleteLabel,
  onDelete,
  titleClass
}: {
  issues: WorkflowIssue[];
  deleteLabel: string;
  onDelete: () => void;
  titleClass: string;
}) {
  const t = useT();
  return (
    <>
      {issues.length > 0 && (
        <div className="flex flex-col gap-1">
          <div className={`${titleClass} font-medium text-err`}>{t('edgeIssues')}</div>
          {issues.map((issue, index) => (
            <div key={index} className="rounded-md bg-del-bg px-2 py-1 text-xs text-err">
              {issue.message}
            </div>
          ))}
        </div>
      )}

      <Button
        variant="danger"
        size="sm"
        type="button"
        iconOnly
        onClick={onDelete}
        aria-label={deleteLabel}
        title={deleteLabel}
        className="mt-2 self-start"
      >
        <TrashIcon className="h-4 w-4" />
      </Button>
    </>
  );
}
