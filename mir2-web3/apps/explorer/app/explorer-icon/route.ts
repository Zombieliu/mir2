import { ImageResponse } from "next/og";
import { createElement } from "react";

export function GET() {
  return new ImageResponse(
    createElement(
      "div",
      {
        style: {
          width: "100%",
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "#d4ad62",
          background: "#080b0c",
          border: "2px solid #67c9c2",
          fontSize: 30,
          fontFamily: "serif",
        },
      },
      "A",
    ),
    { width: 64, height: 64 },
  );
}
