import { site, urls } from "@/config/site";
import { GithubIcon } from "@/components/ui/GithubIcon";
import { Logo } from "@/components/ui/Logo";

const cols = [
  { heading: "Product", links: [["Overview", "#product"], ["Features", "#features"], ["Naurix", "#naurix"]] },
  {
    heading: "Project",
    links: [["Documentation", urls.docs], ["Roadmap", urls.roadmap], ["Contribution guide", urls.contributing], ["Security", urls.security], ["License", urls.license]],
  },
] as const;

export function Footer() {
  return (
    <footer className="border-t border-line bg-bg-2/60">
      <div className="mx-auto grid max-w-7xl gap-12 px-4 py-14 sm:px-6 md:grid-cols-[1.4fr_1fr_1fr] lg:px-8">
        <div>
          <Logo />
          <p className="mt-4 max-w-xs text-sm leading-relaxed text-fg-2">
            An open-source, lightweight terminal and workspace for AI coding agents.
          </p>
          <a href={urls.github} target="_blank" rel="noopener noreferrer" className="mt-5 inline-flex items-center gap-2 text-sm text-fg-2 hover:text-fg">
            <GithubIcon className="size-4" /> GitHub
          </a>
        </div>
        {cols.map((c) => (
          <nav key={c.heading} aria-label={c.heading}>
            <h2 className="text-sm font-semibold">{c.heading}</h2>
            <ul className="mt-4 space-y-2.5">
              {c.links.map(([label, href]) => {
                const ext = href.startsWith("http");
                return (
                  <li key={label}>
                    <a href={href} {...(ext ? { target: "_blank", rel: "noopener noreferrer" } : {})} className="text-sm text-fg-2 transition-colors hover:text-fg">
                      {label}
                    </a>
                  </li>
                );
              })}
            </ul>
          </nav>
        ))}
      </div>
      <div className="border-t border-line">
        <p className="mx-auto max-w-7xl px-4 py-6 text-xs text-fg-3 sm:px-6 lg:px-8">
          © {site.year} Naurix contributors. Licensed under {site.license}.
        </p>
      </div>
    </footer>
  );
}
