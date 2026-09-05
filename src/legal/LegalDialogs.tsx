import { Dialog } from "../ui/Dialog.tsx";
import {
  LIMITED_USE_BODY,
  LIMITED_USE_TITLE,
  PRIVACY_POLICY_SECTIONS,
  PRIVACY_POLICY_TITLE,
} from "./copy.ts";

export type LegalDocumentId = "privacy" | "limited-use";

type LegalDialogsProps = {
  open: LegalDocumentId | null;
  onClose: () => void;
};

export function legalDocumentTitle(document: LegalDocumentId): string {
  return document === "privacy" ? PRIVACY_POLICY_TITLE : LIMITED_USE_TITLE;
}

export function LegalDocument({ document }: { document: LegalDocumentId }) {
  if (document === "privacy") {
    return (
      <div className="legal-copy">
        {PRIVACY_POLICY_SECTIONS.map((section) => (
          <section key={section.heading}>
            <h3>{section.heading}</h3>
            <p>{section.body}</p>
          </section>
        ))}
      </div>
    );
  }

  return (
    <div className="legal-copy">
      {LIMITED_USE_BODY.map((paragraph) => (
        <p key={paragraph}>{paragraph}</p>
      ))}
    </div>
  );
}

export function LegalDialogs({ open, onClose }: LegalDialogsProps) {
  if (open === null) {
    return null;
  }

  return (
    <Dialog title={legalDocumentTitle(open)} onClose={onClose} wide>
      <LegalDocument document={open} />
    </Dialog>
  );
}
