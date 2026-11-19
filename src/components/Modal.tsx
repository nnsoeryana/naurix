import type { ReactNode } from "react";

/** Centered dialog with a dimmed backdrop; Escape and a click outside close it. */
export function Modal({ title, onClose, children, wide }: { title: string; onClose: () => void; children: ReactNode; wide?: boolean }) {
  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center overflow-auto bg-overlay py-12" onMouseDown={onClose}>
      <div
        className={`${wide ? "w-[44rem]" : "w-[32rem]"} max-w-[95vw] rounded-lg border border-line bg-dialog p-4 shadow-xl`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <h2 className="mb-3 text-sm font-semibold">{title}</h2>
        {children}
      </div>
    </div>
  );
}
