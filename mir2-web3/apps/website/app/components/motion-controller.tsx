"use client";

import { useEffect } from "react";

export function MotionController() {
  useEffect(() => {
    const root = document.documentElement;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");

    if (reducedMotion.matches) {
      root.classList.add("motion-ready", "reduced-motion");
      return;
    }

    let pointerFrame = 0;
    let scrollFrame = 0;

    const revealObserver = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (!entry.isIntersecting) continue;
          entry.target.classList.add("is-visible");
          revealObserver.unobserve(entry.target);
        }
      },
      { rootMargin: "0px 0px -12%", threshold: 0.12 },
    );

    const revealTargets = document.querySelectorAll<HTMLElement>("[data-reveal]");
    revealTargets.forEach((target) => revealObserver.observe(target));

    const updatePointer = (event: PointerEvent) => {
      if (pointerFrame) return;
      pointerFrame = window.requestAnimationFrame(() => {
        const x = (event.clientX / window.innerWidth - 0.5) * 2;
        const y = (event.clientY / window.innerHeight - 0.5) * 2;
        root.style.setProperty("--portal-x", `${(-x * 12).toFixed(2)}px`);
        root.style.setProperty("--portal-y", `${(-y * 8).toFixed(2)}px`);
        root.style.setProperty("--content-x", `${(x * 4).toFixed(2)}px`);
        pointerFrame = 0;
      });
    };

    const updateScroll = () => {
      if (scrollFrame) return;
      scrollFrame = window.requestAnimationFrame(() => {
        const viewport = Math.max(window.innerHeight, 1);
        const heroProgress = Math.min(window.scrollY / viewport, 1);
        root.style.setProperty("--hero-shift", `${(heroProgress * 44).toFixed(2)}px`);
        root.style.setProperty("--hero-fade", (1 - heroProgress * 0.7).toFixed(3));
        scrollFrame = 0;
      });
    };

    window.addEventListener("pointermove", updatePointer, { passive: true });
    window.addEventListener("scroll", updateScroll, { passive: true });
    updateScroll();

    const readyFrame = window.requestAnimationFrame(() => root.classList.add("motion-ready"));

    return () => {
      revealObserver.disconnect();
      window.removeEventListener("pointermove", updatePointer);
      window.removeEventListener("scroll", updateScroll);
      window.cancelAnimationFrame(readyFrame);
      if (pointerFrame) window.cancelAnimationFrame(pointerFrame);
      if (scrollFrame) window.cancelAnimationFrame(scrollFrame);
      root.classList.remove("motion-ready", "reduced-motion");
      root.style.removeProperty("--portal-x");
      root.style.removeProperty("--portal-y");
      root.style.removeProperty("--content-x");
      root.style.removeProperty("--hero-shift");
      root.style.removeProperty("--hero-fade");
    };
  }, []);

  return null;
}
