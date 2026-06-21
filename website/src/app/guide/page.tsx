import type { Metadata } from "next";
import { Header } from "@/components/Header";
import { Guide } from "@/components/Guide";
import { Footer } from "@/components/Footer";

export const metadata: Metadata = {
  title: "User Guide — LoreGUI",
  description:
    "How to use LoreGUI: first launch, connecting to or hosting a server, the ⌘K command palette, branches and merging, history, storage backends, locks, dependencies, and theming — with real screenshots.",
  alternates: {
    canonical: "/guide",
  },
  openGraph: {
    title: "LoreGUI User Guide",
    description:
      "A walkthrough of every surface in LoreGUI — onboarding, the command palette, branches, history, storage, locks and theming — with real screenshots.",
    url: "/guide",
    type: "article",
  },
};

export default function GuidePage() {
  return (
    <>
      <Header />
      <main>
        <Guide />
      </main>
      <Footer />
    </>
  );
}
