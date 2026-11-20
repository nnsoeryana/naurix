"use client";

import { Menu, X } from "lucide-react";
import { useEffect, useState } from "react";
import { nav, primaryCta, urls } from "@/config/site";
import { GithubIcon } from "@/components/ui/GithubIcon";
import { Logo } from "@/components/ui/Logo";

export function Navbar() {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState<string>("");
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 8);
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });

    const ids = nav.filter((n) => !("external" in n && n.external)).map((n) => n.href.slice(1));
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) if (e.isIntersecting) setActive(e.target.id);
      },
      { rootMargin: "-35% 0px -55% 0px" },
    );
    ids.forEach((id) => {
      const el = document.getElementById(id);
      if (el) io.observe(el);
    });
    return () => {
      window.removeEventListener("scroll", onScroll);
      io.disconnect();
    };
  }, []);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  const linkCls = (isActive: boolean) =>
    `rounded-lg px-3 py-2 text-sm transition-colors ${isActive ? "text-fg" : "text-fg-2 hover:text-fg"}`;

  return (
    <header
      className={`sticky top-0 z-50 border-b transition-colors duration-300 ${
        scrolled || open ? "border-line bg-bg/85 backdrop-blur-md" : "border-transparent bg-transparent"
      }`}
    >
      <a href="#main" className="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-4 focus:rounded-lg focus:bg-fg focus:px-3 focus:py-2 focus:text-bg">
        Skip to content
      </a>
      <div className="mx-auto flex h-16 max-w-7xl items-center justify-between px-4 sm:px-6 lg:px-8">
        <a href="#top" aria-label="Naurix home">
          <Logo />
        </a>

        <nav aria-label="Primary" className="hidden items-center gap-1 md:flex">
          {nav.map((n) => {
            const ext = "external" in n && n.external;
            const isActive = !ext && active === n.href.slice(1);
            return (
              <a key={n.label} href={n.href} {...(ext ? { target: "_blank", rel: "noopener noreferrer" } : {})} aria-current={isActive ? "true" : undefined} className={linkCls(isActive)}>
                {n.label}
              </a>
            );
          })}
        </nav>

        <div className="hidden items-center gap-2 md:flex">
          <a href={urls.github} target="_blank" rel="noopener noreferrer" aria-label="Naurix on GitHub" className="grid size-10 place-items-center rounded-lg text-fg-2 transition-colors hover:text-fg">
            <GithubIcon />
          </a>
          <a href={primaryCta.href} {...(primaryCta.download ? {} : { target: "_blank", rel: "noopener noreferrer" })} className="rounded-lg bg-fg px-4 py-2 text-sm font-medium text-bg transition-colors hover:bg-white">
            {primaryCta.label}
          </a>
        </div>

        <button
          type="button"
          className="grid size-10 place-items-center rounded-lg text-fg md:hidden"
          aria-expanded={open}
          aria-controls="mobile-menu"
          aria-label={open ? "Close menu" : "Open menu"}
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <X className="size-5" /> : <Menu className="size-5" />}
        </button>
      </div>

      <div
        id="mobile-menu"
        className={`grid overflow-hidden transition-[grid-template-rows,opacity] duration-300 md:hidden ${open ? "grid-rows-[1fr] opacity-100" : "grid-rows-[0fr] opacity-0"}`}
        inert={!open}
      >
        <div className="min-h-0">
          <nav aria-label="Mobile" className="flex flex-col gap-1 px-4 pb-5 pt-2">
            {nav.map((n) => {
              const ext = "external" in n && n.external;
              return (
                <a key={n.label} href={n.href} onClick={() => setOpen(false)} {...(ext ? { target: "_blank", rel: "noopener noreferrer" } : {})} className="rounded-lg px-3 py-3 text-base text-fg-2 hover:bg-surface hover:text-fg">
                  {n.label}
                </a>
              );
            })}
            <div className="mt-3 flex gap-3">
              <a href={primaryCta.href} className="flex-1 rounded-lg bg-fg px-4 py-3 text-center text-sm font-medium text-bg">
                {primaryCta.label}
              </a>
              <a href={urls.github} target="_blank" rel="noopener noreferrer" aria-label="Naurix on GitHub" className="grid size-12 place-items-center rounded-lg border border-line-2 text-fg">
                <GithubIcon />
              </a>
            </div>
          </nav>
        </div>
      </div>
    </header>
  );
}
