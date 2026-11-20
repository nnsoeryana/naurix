import type { ReactNode } from "react";

interface Props {
  eyebrow?: string;
  title: ReactNode;
  description?: ReactNode;
  id?: string;
  align?: "left" | "center";
}

export function SectionHeading({ eyebrow, title, description, id, align = "center" }: Props) {
  const center = align === "center";
  return (
    <div className={`reveal max-w-3xl ${center ? "mx-auto text-center" : ""}`}>
      {eyebrow && (
        <p className="mb-4 font-mono text-xs uppercase tracking-[0.2em] text-cyan">{eyebrow}</p>
      )}
      <h2 id={id} className="text-balance text-3xl font-semibold tracking-tight sm:text-4xl lg:text-5xl">
        {title}
      </h2>
      {description && <p className="mt-5 text-pretty text-base leading-relaxed text-fg-2 sm:text-lg">{description}</p>}
    </div>
  );
}
