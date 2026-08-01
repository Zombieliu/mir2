"use client";

import { crystalWeatherLayers, crystalWeatherTexturePath } from "./original-client-weather";

export function OriginalClientWeatherLayer({ weatherParticles }: { weatherParticles?: number }) {
  const layers = crystalWeatherLayers(weatherParticles);
  if (layers.length === 0) return null;

  return (
    <div
      aria-hidden="true"
      className="viewport-crystal-weather-overlay"
      data-weather-particles={weatherParticles ?? 0}
    >
      {layers.map((layer) => (
        <div
          key={layer.key}
          className={`viewport-crystal-weather-layer ${layer.className}`}
          style={{ backgroundImage: `url(${crystalWeatherTexturePath(layer.frame)})` }}
        />
      ))}
    </div>
  );
}
