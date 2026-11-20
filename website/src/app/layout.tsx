import type { Metadata, Viewport } from "next";
import { GeistMono } from "geist/font/mono";
import { GeistSans } from "geist/font/sans";
import { site } from "@/config/site";
import "./globals.css";

const title = `${site.name} — Your AI coding agents. One command center.`;

export const metadata: Metadata = {
  ...(site.origin ? { metadataBase: new URL(site.origin) } : {}),
  title,
  description: site.description,
  applicationName: site.name,
  openGraph: { type: "website", siteName: site.name, title, description: site.description },
  twitter: { card: "summary", title, description: site.description },
};

export const viewport: Viewport = { themeColor: "#090d18", colorScheme: "dark" };

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${GeistSans.variable} ${GeistMono.variable}`} suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: "document.documentElement.classList.add('js')" }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
