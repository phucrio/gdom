import { Dialog } from "../ui/Dialog.tsx";
import {
  LIMITED_USE_BODY,
  LIMITED_USE_TITLE,
  PRIVACY_POLICY_SECTIONS,
  PRIVACY_POLICY_TITLE,
} from "./copy.ts";

type LegalDialogsProps = {
  open: "privacy" | "limited-use" | null;
  onClose: () => void;
};

export function LegalDialogs({ open, onClose }: LegalDialogsProps) {
  if (open === "privacy") {
    return (
      <Dialog title={PRIVACY_POLICY_TITLE} onClose={onClose} wide>
        <div className="legal-copy">
          {PRIVACY_POLICY_SECTIONS.map((section) => (
            <section key={section.heading}>
              <h3>{section.heading}</h3>
              <p>{section.body}</p>
            </section>
          ))}
        </div>
      </Dialog>
    );
  }

  if (open === "limited-use") {
    return (
      <Dialog title={LIMITED_USE_TITLE} onClose={onClose} wide>
        <div className="legal-copy">
          {LIMITED_USE_BODY.map((paragraph) => (
            <p key={paragraph}>{paragraph}</p>
          ))}
        </div>
      </Dialog>
    );
  }

  return null;
}
