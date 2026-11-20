import { ArrowRight, ArrowUpRight, Download } from "lucide-react";
import type { ReactNode } from "react";

interface ButtonProps {
  href: string;
  children: ReactNode;
  variant?: "primary" | "secondary";
  external?: boolean;
  download?: boolean;
  className?: string;
}

const base =
  "group inline-flex items-center justify-center gap-2 rounded-xl px-5 py-3 text-sm font-medium transition-[background-color,border-color,transform,box-shadow] duration-200 active:translate-y-px";
const variants = {
  primary:
    "bg-fg text-bg hover:bg-white hover:shadow-[0_0_0_1px_rgba(66,217,232,0.5),0_8px_30px_-8px_rgba(86,139,255,0.6)]",
  secondary: "border border-line-2 bg-surface/60 text-fg hover:border-cyan/60 hover:bg-elevated",
};

export function Button({ href, children, variant = "primary", external, download, className = "" }: ButtonProps) {
  const Icon = download ? Download : external ? ArrowUpRight : ArrowRight;
  return (
    <a
      href={href}
      {...(external ? { target: "_blank", rel: "noopener noreferrer" } : {})}
      className={`${base} ${variants[variant]} ${className}`}
    >
      {children}
      <Icon
        aria-hidden="true"
        className={`size-4 transition-transform duration-200 ${download ? "" : "group-hover:translate-x-0.5"}`}
      />
    </a>
  );
}
