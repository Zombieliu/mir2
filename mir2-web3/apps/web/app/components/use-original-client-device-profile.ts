"use client";

import { useEffect, useState } from "react";

import {
  readMir2ClientEnvironment,
  resolveMir2ClientProfile,
  type Mir2ClientProfile,
  type Mir2InputProfile,
} from "./original-client-device-profile";

const DEFAULT_PROFILE: Mir2ClientProfile = {
  layout: "desktop",
  input: "keyboardMouse",
  layoutForced: false,
  inputForced: false,
};

export function useOriginalClientDeviceProfile() {
  const [profile, setProfile] = useState<Mir2ClientProfile>(DEFAULT_PROFILE);

  useEffect(() => {
    const resolve = () => setProfile(resolveMir2ClientProfile(readMir2ClientEnvironment()));
    const setActiveInput = (input: Mir2InputProfile) => {
      setProfile((current) => {
        if (current.inputForced || current.input === input) return current;
        if (input === "touch" && current.layout !== "touch") return current;
        return { ...current, input };
      });
    };
    const coarsePointer = window.matchMedia("(pointer: coarse)");
    const onPointerDown = (event: PointerEvent) => {
      setActiveInput(event.pointerType === "touch" ? "touch" : "keyboardMouse");
    };
    const onKeyDown = () => setActiveInput("keyboardMouse");
    const onGamepadConnected = () => setActiveInput("gamepad");

    resolve();
    window.addEventListener("resize", resolve);
    window.addEventListener("gamepadconnected", onGamepadConnected);
    window.addEventListener("gamepaddisconnected", resolve);
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown, true);
    coarsePointer.addEventListener?.("change", resolve);

    return () => {
      window.removeEventListener("resize", resolve);
      window.removeEventListener("gamepadconnected", onGamepadConnected);
      window.removeEventListener("gamepaddisconnected", resolve);
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown, true);
      coarsePointer.removeEventListener?.("change", resolve);
    };
  }, []);

  return profile;
}
