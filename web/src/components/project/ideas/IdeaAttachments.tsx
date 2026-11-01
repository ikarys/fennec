/**
 * An idea's attached images and files, as thumbnails and chips.
 */
import { FileIcon } from "../../Icons";
import type { IdeaView } from "../../../lib/ideas";

export function IdeaAttachments({
  attachments,
}: {
  attachments: IdeaView["attachments"];
}) {
  if (attachments.length === 0) return null;
  return (
    <div className="mt-1.5 flex flex-wrap gap-1">
      {attachments.map((attachment, i) =>
        attachment.mimeType.startsWith("image/") ? (
          <img
            key={i}
            src={attachment.data}
            alt={attachment.filename}
            title={attachment.filename}
            className="h-9 w-9 rounded-md border border-border object-cover"
          />
        ) : (
          <span
            key={i}
            title={attachment.filename}
            className="flex items-center gap-1 rounded-md border border-border bg-bg px-1.5 py-0.5 text-xs text-muted"
          >
            <FileIcon className="h-3 w-3" />
            <span className="max-w-[120px] truncate">
              {attachment.filename}
            </span>
          </span>
        ),
      )}
    </div>
  );
}
