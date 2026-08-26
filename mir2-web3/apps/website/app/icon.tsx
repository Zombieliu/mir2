import { ImageResponse } from "next/og";

export const size = { width: 64, height: 64 };
export const contentType = "image/png";

export default function Icon() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "#f0cc7f",
          background: "#080807",
          border: "2px solid #c9a35e",
          fontSize: 30,
          fontWeight: 700,
          letterSpacing: "0",
        }}
      >
        N
      </div>
    ),
    size,
  );
}
