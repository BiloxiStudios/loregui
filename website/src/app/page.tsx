import { Header } from "@/components/Header";
import { Hero } from "@/components/Hero";
import { Features } from "@/components/Features";
import { BuiltOnApi } from "@/components/BuiltOnApi";
import { ForAgents } from "@/components/ForAgents";
import { Ecosystem } from "@/components/Ecosystem";
import { Screenshots } from "@/components/Screenshots";
import { Install } from "@/components/Install";
import { Footer } from "@/components/Footer";

export default function Home() {
  return (
    <>
      <Header />
      <main>
        <Hero />
        <Features />
        <BuiltOnApi />
        <ForAgents />
        <Ecosystem />
        <Screenshots />
        <Install />
      </main>
      <Footer />
    </>
  );
}
