"use client";

import { usePathname } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";

type BeforeInstallPromptEvent = Event & {
  prompt: () => Promise<void>;
  userChoice: Promise<{ outcome: "accepted" | "dismissed"; platform: string }>;
};

type PwaCopy = {
  install: string;
  fullscreen: string;
  title: string;
  body: string;
  iosSteps: string;
  androidSteps: string;
  close: string;
  unavailable: string;
};

const INSTALL_HINT_DISMISSED_KEY = "mir2.pwa.installHintDismissed.v1";

const COPY: Record<"en" | "pt" | "zh", PwaCopy> = {
  en: {
    install: "Install game",
    fullscreen: "Full screen",
    title: "Play without browser bars",
    body: "Install Mir 2 on your Home Screen for a stable landscape, app-like game window.",
    iosSteps: "Tap Share, then Add to Home Screen. Launch Mir 2 from its new Home Screen icon.",
    androidSteps: "Choose Install game. If no prompt appears, open the browser menu and tap Install app.",
    close: "Not now",
    unavailable: "Full screen is unavailable here. Install the game for the cleanest view.",
  },
  pt: {
    install: "Instalar jogo",
    fullscreen: "Tela cheia",
    title: "Jogue sem as barras do navegador",
    body: "Instale o Mir 2 na tela inicial para jogar em uma janela estável e horizontal.",
    iosSteps: "Toque em Compartilhar e em Adicionar à Tela de Início. Abra o Mir 2 pelo novo ícone.",
    androidSteps: "Escolha Instalar jogo. Se nada aparecer, abra o menu do navegador e toque em Instalar app.",
    close: "Agora não",
    unavailable: "A tela cheia não está disponível aqui. Instale o jogo para obter a melhor visualização.",
  },
  zh: {
    install: "安装游戏",
    fullscreen: "进入全屏",
    title: "隐藏浏览器导航栏",
    body: "将 Mir 2 添加到主屏幕，即可使用稳定横屏的独立游戏窗口。",
    iosSteps: "点击浏览器的分享按钮，再选“添加到主屏幕”，之后从桌面上的 Mir 2 图标启动。",
    androidSteps: "点击“安装游戏”；若未出现系统提示，请打开浏览器菜单并选择“安装应用”。",
    close: "暂不安装",
    unavailable: "当前浏览器不能直接全屏，请安装到主屏幕以获得完整游戏画面。",
  },
};

export function PwaGameShell() {
  const pathname = usePathname();
  const [installPrompt, setInstallPrompt] = useState<BeforeInstallPromptEvent | null>(null);
  const [mobileBrowser, setMobileBrowser] = useState(false);
  const [standalone, setStandalone] = useState(false);
  const [guideOpen, setGuideOpen] = useState(false);
  const [fullscreenActive, setFullscreenActive] = useState(false);
  const [ios, setIos] = useState(false);
  const [status, setStatus] = useState("");

  const copy = useMemo(() => COPY[preferredLanguage()], []);

  const refreshDisplayMode = useCallback(() => {
    const navigatorWithStandalone = navigator as Navigator & { standalone?: boolean };
    const nextStandalone =
      window.matchMedia("(display-mode: standalone)").matches ||
      window.matchMedia("(display-mode: fullscreen)").matches ||
      navigatorWithStandalone.standalone === true;
    setStandalone(nextStandalone);
    setFullscreenActive(Boolean(document.fullscreenElement));
    document.documentElement.dataset.displayMode = nextStandalone ? "standalone" : "browser";
  }, []);

  useEffect(() => {
    if (pathname !== "/") return;

    const detectedIos = isIosDevice();
    const coarsePointer = window.matchMedia("(pointer: coarse)");
    const standaloneQuery = window.matchMedia("(display-mode: standalone)");
    const fullscreenQuery = window.matchMedia("(display-mode: fullscreen)");
    const handleInstallPrompt = (event: Event) => {
      event.preventDefault();
      setInstallPrompt(event as BeforeInstallPromptEvent);
    };
    const handleInstalled = () => {
      setInstallPrompt(null);
      setGuideOpen(false);
      refreshDisplayMode();
    };
    const updateMobile = () => setMobileBrowser(coarsePointer.matches || detectedIos);

    setIos(detectedIos);
    updateMobile();
    refreshDisplayMode();
    window.addEventListener("beforeinstallprompt", handleInstallPrompt);
    window.addEventListener("appinstalled", handleInstalled);
    document.addEventListener("fullscreenchange", refreshDisplayMode);
    coarsePointer.addEventListener("change", updateMobile);
    standaloneQuery.addEventListener("change", refreshDisplayMode);
    fullscreenQuery.addEventListener("change", refreshDisplayMode);

    const hintTimer = window.setTimeout(() => {
      if (!isStandaloneDisplay() && !installHintWasDismissed()) {
        setGuideOpen(true);
      }
    }, 5000);

    return () => {
      window.clearTimeout(hintTimer);
      window.removeEventListener("beforeinstallprompt", handleInstallPrompt);
      window.removeEventListener("appinstalled", handleInstalled);
      document.removeEventListener("fullscreenchange", refreshDisplayMode);
      coarsePointer.removeEventListener("change", updateMobile);
      standaloneQuery.removeEventListener("change", refreshDisplayMode);
      fullscreenQuery.removeEventListener("change", refreshDisplayMode);
    };
  }, [pathname, refreshDisplayMode]);

  const install = useCallback(async () => {
    setStatus("");
    if (!installPrompt) {
      setGuideOpen(true);
      return;
    }
    await installPrompt.prompt();
    const choice = await installPrompt.userChoice;
    setInstallPrompt(null);
    if (choice.outcome === "accepted") {
      setGuideOpen(false);
    }
  }, [installPrompt]);

  const enterFullscreen = useCallback(async () => {
    setStatus("");
    try {
      if (!document.fullscreenElement && document.documentElement.requestFullscreen) {
        await document.documentElement.requestFullscreen({ navigationUI: "hide" });
      } else if (!document.fullscreenElement) {
        setGuideOpen(true);
        setStatus(copy.unavailable);
        return;
      }
      await lockLandscapeWhenSupported();
      setGuideOpen(false);
      refreshDisplayMode();
    } catch {
      setGuideOpen(true);
      setStatus(copy.unavailable);
    }
  }, [copy.unavailable, refreshDisplayMode]);

  const dismissGuide = useCallback(() => {
    rememberInstallHintDismissal();
    setGuideOpen(false);
    setStatus("");
  }, []);

  if (pathname !== "/" || !mobileBrowser || standalone || fullscreenActive) return null;

  return (
    <aside className="mir-pwa-shell" data-ios={ios ? "true" : "false"} data-ui-interactive="true">
      <div className="mir-pwa-actions" aria-label={copy.title}>
        <button type="button" className="mir-pwa-action install" onClick={() => void install()}>
          <span className="mir-pwa-action-mark" aria-hidden="true">+</span>
          {copy.install}
        </button>
        {!ios && !fullscreenActive ? (
          <button type="button" className="mir-pwa-action fullscreen" onClick={() => void enterFullscreen()}>
            <span className="mir-pwa-action-mark" aria-hidden="true">[]</span>
            {copy.fullscreen}
          </button>
        ) : null}
      </div>

      {guideOpen ? (
        <div className="mir-pwa-guide" role="dialog" aria-modal="false" aria-labelledby="mir-pwa-guide-title">
          <div className="mir-pwa-guide-kicker">MIR 2 / WEB APP</div>
          <strong id="mir-pwa-guide-title">{copy.title}</strong>
          <p>{copy.body}</p>
          <p className="mir-pwa-guide-steps">{ios ? copy.iosSteps : copy.androidSteps}</p>
          {status ? <p className="mir-pwa-guide-status" role="status">{status}</p> : null}
          <div className="mir-pwa-guide-buttons">
            <button type="button" className="mir-pwa-guide-primary" onClick={() => void install()}>
              {copy.install}
            </button>
            <button type="button" className="mir-pwa-guide-dismiss" onClick={dismissGuide}>
              {copy.close}
            </button>
          </div>
        </div>
      ) : null}
    </aside>
  );
}

function preferredLanguage(): keyof typeof COPY {
  if (typeof navigator === "undefined") return "en";
  const language = navigator.language.toLowerCase();
  if (language.startsWith("zh")) return "zh";
  if (language.startsWith("pt")) return "pt";
  return "en";
}

function isIosDevice(): boolean {
  const platform = navigator.platform || "";
  return /iPad|iPhone|iPod/.test(navigator.userAgent) || (platform === "MacIntel" && navigator.maxTouchPoints > 1);
}

function isStandaloneDisplay(): boolean {
  const navigatorWithStandalone = navigator as Navigator & { standalone?: boolean };
  return (
    window.matchMedia("(display-mode: standalone)").matches ||
    window.matchMedia("(display-mode: fullscreen)").matches ||
    navigatorWithStandalone.standalone === true
  );
}

function installHintWasDismissed(): boolean {
  try {
    return localStorage.getItem(INSTALL_HINT_DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

function rememberInstallHintDismissal() {
  try {
    localStorage.setItem(INSTALL_HINT_DISMISSED_KEY, "1");
  } catch {
    // Private browsing may deny storage; dismissing still works for this page lifetime.
  }
}

async function lockLandscapeWhenSupported() {
  const orientation = screen.orientation as ScreenOrientation & {
    lock?: (orientation: "landscape") => Promise<void>;
  };
  if (!orientation?.lock) return;
  try {
    await orientation.lock("landscape");
  } catch {
    // Orientation locking is advisory and commonly denied outside installed/fullscreen mode.
  }
}
