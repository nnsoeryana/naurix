import { Footer } from "@/components/layout/Footer";
import { Navbar } from "@/components/layout/Navbar";
import { AgentEcosystem } from "@/components/sections/AgentEcosystem";
import { BrandPhilosophy } from "@/components/sections/BrandPhilosophy";
import { DeveloperBenefits } from "@/components/sections/DeveloperBenefits";
import { Features } from "@/components/sections/Features";
import { FinalCTA } from "@/components/sections/FinalCTA";
import { Hero } from "@/components/sections/Hero";
import { Naurix } from "@/components/sections/Naurix";
import { OpenSource } from "@/components/sections/OpenSource";
import { WhyNaurix } from "@/components/sections/WhyNaurix";
import { RevealObserver } from "@/components/ui/RevealObserver";

export default function Home() {
  return (
    <div id="top">
      <Navbar />
      <main id="main">
        <Hero />
        <WhyNaurix />
        <Features />
        <Naurix />
        <AgentEcosystem />
        <DeveloperBenefits />
        <OpenSource />
        <BrandPhilosophy />
        <FinalCTA />
      </main>
      <Footer />
      <RevealObserver />
    </div>
  );
}
