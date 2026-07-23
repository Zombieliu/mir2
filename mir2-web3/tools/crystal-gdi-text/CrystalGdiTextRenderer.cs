using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Windows.Forms;

namespace CrystalGdiText
{
    public static class StrictJson
    {
        public static void ValidateNoDuplicateProperties(string json)
        {
            if (json == null)
                throw new ArgumentNullException("json");

            new Parser(json).ParseDocument();
        }

        private sealed class Parser
        {
            private const int MaximumDepth = 128;
            private readonly string text;
            private int position;

            public Parser(string text)
            {
                this.text = text;
            }

            public void ParseDocument()
            {
                SkipWhitespace();
                ParseValue(0);
                SkipWhitespace();
                if (position != text.Length)
                    Fail("Unexpected content after the root value.");
            }

            private void ParseValue(int depth)
            {
                if (depth > MaximumDepth)
                    Fail("Maximum nesting depth exceeded.");
                if (position >= text.Length)
                    Fail("Expected a JSON value.");

                switch (text[position])
                {
                    case '{':
                        ParseObject(depth + 1);
                        return;
                    case '[':
                        ParseArray(depth + 1);
                        return;
                    case '"':
                        ParseString();
                        return;
                    case 't':
                        ParseLiteral("true");
                        return;
                    case 'f':
                        ParseLiteral("false");
                        return;
                    case 'n':
                        ParseLiteral("null");
                        return;
                    default:
                        ParseNumber();
                        return;
                }
            }

            private void ParseObject(int depth)
            {
                Expect('{');
                SkipWhitespace();
                if (TryConsume('}'))
                    return;

                HashSet<string> names = new HashSet<string>(StringComparer.Ordinal);
                while (true)
                {
                    if (position >= text.Length || text[position] != '"')
                        Fail("Expected an object property name.");

                    int propertyPosition = position;
                    string name = ParseString();
                    if (!names.Add(name))
                        throw new FormatException(
                            "Duplicate JSON property at index " + propertyPosition + ".");

                    SkipWhitespace();
                    Expect(':');
                    SkipWhitespace();
                    ParseValue(depth);
                    SkipWhitespace();

                    if (TryConsume('}'))
                        return;
                    Expect(',');
                    SkipWhitespace();
                }
            }

            private void ParseArray(int depth)
            {
                Expect('[');
                SkipWhitespace();
                if (TryConsume(']'))
                    return;

                while (true)
                {
                    ParseValue(depth);
                    SkipWhitespace();
                    if (TryConsume(']'))
                        return;
                    Expect(',');
                    SkipWhitespace();
                }
            }

            private string ParseString()
            {
                Expect('"');
                StringBuilder result = new StringBuilder();

                while (position < text.Length)
                {
                    char character = text[position++];
                    if (character == '"')
                        return result.ToString();
                    if (character < 0x20)
                        Fail("Unescaped control character in string.");

                    if (character == '\\')
                    {
                        if (position >= text.Length)
                            Fail("Incomplete string escape.");

                        char escaped = text[position++];
                        switch (escaped)
                        {
                            case '"': result.Append('"'); break;
                            case '\\': result.Append('\\'); break;
                            case '/': result.Append('/'); break;
                            case 'b': result.Append('\b'); break;
                            case 'f': result.Append('\f'); break;
                            case 'n': result.Append('\n'); break;
                            case 'r': result.Append('\r'); break;
                            case 't': result.Append('\t'); break;
                            case 'u': AppendEscapedUnicode(result); break;
                            default: Fail("Unsupported string escape."); break;
                        }
                        continue;
                    }

                    if (Char.IsHighSurrogate(character))
                    {
                        if (position >= text.Length || !Char.IsLowSurrogate(text[position]))
                            Fail("Unpaired high surrogate in string.");
                        result.Append(character);
                        result.Append(text[position++]);
                    }
                    else if (Char.IsLowSurrogate(character))
                    {
                        Fail("Unpaired low surrogate in string.");
                    }
                    else
                    {
                        result.Append(character);
                    }
                }

                Fail("Unterminated string.");
                return null;
            }

            private void AppendEscapedUnicode(StringBuilder result)
            {
                char first = (char)ParseHexQuad();
                if (Char.IsHighSurrogate(first))
                {
                    if (position + 2 > text.Length || text[position] != '\\' || text[position + 1] != 'u')
                        Fail("Escaped high surrogate is not followed by an escaped low surrogate.");
                    position += 2;
                    char second = (char)ParseHexQuad();
                    if (!Char.IsLowSurrogate(second))
                        Fail("Escaped high surrogate is not followed by an escaped low surrogate.");
                    result.Append(first);
                    result.Append(second);
                }
                else if (Char.IsLowSurrogate(first))
                {
                    Fail("Unpaired escaped low surrogate in string.");
                }
                else
                {
                    result.Append(first);
                }
            }

            private int ParseHexQuad()
            {
                if (position + 4 > text.Length)
                    Fail("Incomplete Unicode escape.");

                int value = 0;
                for (int index = 0; index < 4; index++)
                {
                    char character = text[position++];
                    value <<= 4;
                    if (character >= '0' && character <= '9')
                        value += character - '0';
                    else if (character >= 'a' && character <= 'f')
                        value += character - 'a' + 10;
                    else if (character >= 'A' && character <= 'F')
                        value += character - 'A' + 10;
                    else
                        Fail("Invalid Unicode escape.");
                }
                return value;
            }

            private void ParseNumber()
            {
                int start = position;
                TryConsume('-');
                if (TryConsume('0'))
                {
                    if (position < text.Length && IsDigit(text[position]))
                        Fail("Leading zero in number.");
                }
                else
                {
                    if (position >= text.Length || text[position] < '1' || text[position] > '9')
                        Fail("Invalid number.");
                    while (position < text.Length && IsDigit(text[position]))
                        position++;
                }

                if (TryConsume('.'))
                {
                    if (position >= text.Length || !IsDigit(text[position]))
                        Fail("Fraction requires at least one digit.");
                    while (position < text.Length && IsDigit(text[position]))
                        position++;
                }

                if (position < text.Length && (text[position] == 'e' || text[position] == 'E'))
                {
                    position++;
                    if (position < text.Length && (text[position] == '+' || text[position] == '-'))
                        position++;
                    if (position >= text.Length || !IsDigit(text[position]))
                        Fail("Exponent requires at least one digit.");
                    while (position < text.Length && IsDigit(text[position]))
                        position++;
                }

                if (position == start)
                    Fail("Expected a JSON value.");
            }

            private void ParseLiteral(string literal)
            {
                if (position + literal.Length > text.Length ||
                    !String.Equals(text.Substring(position, literal.Length), literal, StringComparison.Ordinal))
                    Fail("Invalid JSON literal.");
                position += literal.Length;
            }

            private void SkipWhitespace()
            {
                while (position < text.Length)
                {
                    char character = text[position];
                    if (character != ' ' && character != '\t' && character != '\r' && character != '\n')
                        return;
                    position++;
                }
            }

            private void Expect(char expected)
            {
                if (!TryConsume(expected))
                    Fail("Expected '" + expected + "'.");
            }

            private bool TryConsume(char expected)
            {
                if (position >= text.Length || text[position] != expected)
                    return false;
                position++;
                return true;
            }

            private static bool IsDigit(char character)
            {
                return character >= '0' && character <= '9';
            }

            private void Fail(string message)
            {
                throw new FormatException("Invalid JSON at index " + position + ": " + message);
            }
        }
    }

    public sealed class PixelSummary
    {
        public string ArgbSha256 { get; set; }
        public long TransparentPixels { get; set; }
        public long TranslucentPixels { get; set; }
        public long OpaquePixels { get; set; }
        public int MinAlpha { get; set; }
        public int MaxAlpha { get; set; }
    }

    public sealed class RenderResult
    {
        public int MeasuredWidth { get; set; }
        public int MeasuredHeight { get; set; }
        public int OutputWidth { get; set; }
        public int OutputHeight { get; set; }
        public string PixelFormat { get; set; }
        public float DpiX { get; set; }
        public float DpiY { get; set; }
        public string PngSha256 { get; set; }
        public PixelSummary Pixels { get; set; }
    }

    public sealed class PngInspection
    {
        public int Width { get; set; }
        public int Height { get; set; }
        public string PixelFormat { get; set; }
        public float DpiX { get; set; }
        public float DpiY { get; set; }
        public string PngSha256 { get; set; }
        public PixelSummary Pixels { get; set; }
    }

    public static class Renderer
    {
        public const string FontFamilyName = "Arial";
        public const float FontSizePoints = 8.0F;
        public const float TargetDpi = 96.0F;
        public const int MaximumDimension = 16384;
        public const long MaximumPixels = 67108864L;

        public static string ResolveFontFamily()
        {
            using (Font font = CreateFont())
            {
                return font.Name;
            }
        }

        public static RenderResult Render(
            string text,
            string foregroundArgb,
            string backgroundArgb,
            bool outline,
            int drawFormatValue,
            bool autoSize,
            int requestedWidth,
            int requestedHeight,
            string outputPath)
        {
            if (String.IsNullOrEmpty(text))
                throw new ArgumentException("Text must not be null or empty.", "text");
            if (String.IsNullOrEmpty(outputPath))
                throw new ArgumentException("Output path must not be null or empty.", "outputPath");

            Color foreground = ParseArgb(foregroundArgb, "foregroundArgb");
            Color background = ParseArgb(backgroundArgb, "backgroundArgb");
            Size measured;

            using (Bitmap measuringBitmap = CreateBitmap(1, 1))
            using (Graphics measuringGraphics = Graphics.FromImage(measuringBitmap))
            using (Font font = CreateFont())
            {
                measured = TextRenderer.MeasureText(measuringGraphics, text, font);
            }

            int width = autoSize ? checked(measured.Width + (outline ? 2 : 0)) : requestedWidth;
            int height = autoSize ? checked(measured.Height + (outline ? 2 : 0)) : requestedHeight;
            ValidateDimensions(width, height);

            string parent = Path.GetDirectoryName(Path.GetFullPath(outputPath));
            if (String.IsNullOrEmpty(parent) || !Directory.Exists(parent))
                throw new DirectoryNotFoundException("The output parent directory must already exist.");
            if (File.Exists(outputPath))
                throw new IOException("Refusing to overwrite an existing PNG: " + outputPath);

            PixelSummary pixels;
            string pixelFormat;
            float dpiX;
            float dpiY;

            using (Bitmap bitmap = CreateBitmap(width, height))
            {
                using (Graphics graphics = Graphics.FromImage(bitmap))
                using (Font font = CreateFont())
                {
                    ConfigureCrystalGraphics(graphics);
                    graphics.Clear(background);

                    Rectangle bounds = new Rectangle(0, 0, width, height);
                    TextFormatFlags flags = (TextFormatFlags)drawFormatValue;
                    if (outline)
                    {
                        Color outlineColor = Color.Black;
                        TextRenderer.DrawText(graphics, text, font, Offset(bounds, 1, 0), outlineColor, flags);
                        TextRenderer.DrawText(graphics, text, font, Offset(bounds, 0, 1), outlineColor, flags);
                        TextRenderer.DrawText(graphics, text, font, Offset(bounds, 2, 1), outlineColor, flags);
                        TextRenderer.DrawText(graphics, text, font, Offset(bounds, 1, 2), outlineColor, flags);
                        TextRenderer.DrawText(graphics, text, font, Offset(bounds, 1, 1), foreground, flags);
                    }
                    else
                    {
                        TextRenderer.DrawText(graphics, text, font, Offset(bounds, 1, 0), foreground, flags);
                    }

                    graphics.Flush(FlushIntention.Sync);
                }

                pixelFormat = bitmap.PixelFormat.ToString();
                dpiX = bitmap.HorizontalResolution;
                dpiY = bitmap.VerticalResolution;
                pixels = SummarizePixels(bitmap);
                bitmap.Save(outputPath, ImageFormat.Png);
            }

            return new RenderResult
            {
                MeasuredWidth = measured.Width,
                MeasuredHeight = measured.Height,
                OutputWidth = width,
                OutputHeight = height,
                PixelFormat = pixelFormat,
                DpiX = dpiX,
                DpiY = dpiY,
                PngSha256 = HashFile(outputPath),
                Pixels = pixels
            };
        }

        public static PngInspection InspectPng(string path)
        {
            if (String.IsNullOrEmpty(path) || !File.Exists(path))
                throw new FileNotFoundException("PNG does not exist.", path);

            using (Bitmap source = new Bitmap(path))
            using (Bitmap normalized = CreateBitmap(source.Width, source.Height))
            {
                using (Graphics graphics = Graphics.FromImage(normalized))
                {
                    graphics.CompositingMode = CompositingMode.SourceCopy;
                    graphics.DrawImageUnscaled(source, 0, 0);
                    graphics.Flush(FlushIntention.Sync);
                }

                return new PngInspection
                {
                    Width = source.Width,
                    Height = source.Height,
                    PixelFormat = source.PixelFormat.ToString(),
                    DpiX = source.HorizontalResolution,
                    DpiY = source.VerticalResolution,
                    PngSha256 = HashFile(path),
                    Pixels = SummarizePixels(normalized)
                };
            }
        }

        private static Font CreateFont()
        {
            Font font = new Font(FontFamilyName, FontSizePoints, FontStyle.Regular, GraphicsUnit.Point);
            if (!String.Equals(font.Name, FontFamilyName, StringComparison.OrdinalIgnoreCase))
            {
                string resolved = font.Name;
                font.Dispose();
                throw new InvalidOperationException(
                    "Arial is unavailable; System.Drawing substituted '" + resolved + "'. Refusing non-Crystal output.");
            }
            return font;
        }

        private static Bitmap CreateBitmap(int width, int height)
        {
            Bitmap bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb);
            bitmap.SetResolution(TargetDpi, TargetDpi);
            return bitmap;
        }

        private static void ConfigureCrystalGraphics(Graphics graphics)
        {
            graphics.SmoothingMode = SmoothingMode.AntiAlias;
            graphics.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
            graphics.CompositingQuality = CompositingQuality.HighQuality;
            graphics.InterpolationMode = InterpolationMode.NearestNeighbor;
            graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
            graphics.TextContrast = 0;
        }

        private static Rectangle Offset(Rectangle bounds, int x, int y)
        {
            return new Rectangle(x, y, bounds.Width, bounds.Height);
        }

        private static Color ParseArgb(string value, string parameterName)
        {
            if (value == null || value.Length != 9 || value[0] != '#')
                throw new ArgumentException("Colour must use #AARRGGBB.", parameterName);

            uint argb;
            if (!UInt32.TryParse(
                value.Substring(1),
                System.Globalization.NumberStyles.AllowHexSpecifier,
                System.Globalization.CultureInfo.InvariantCulture,
                out argb))
                throw new ArgumentException("Colour must use #AARRGGBB.", parameterName);

            return Color.FromArgb(unchecked((int)argb));
        }

        private static void ValidateDimensions(int width, int height)
        {
            if (width <= 0 || height <= 0)
                throw new ArgumentOutOfRangeException("size", "Output dimensions must be positive.");
            if (width > MaximumDimension || height > MaximumDimension)
                throw new ArgumentOutOfRangeException("size", "Output dimensions exceed the safety limit.");
            if ((long)width * (long)height > MaximumPixels)
                throw new ArgumentOutOfRangeException("size", "Output pixel count exceeds the safety limit.");
        }

        private static PixelSummary SummarizePixels(Bitmap bitmap)
        {
            Rectangle rectangle = new Rectangle(0, 0, bitmap.Width, bitmap.Height);
            BitmapData data = bitmap.LockBits(rectangle, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            long transparent = 0;
            long translucent = 0;
            long opaque = 0;
            int minAlpha = 255;
            int maxAlpha = 0;

            try
            {
                int absoluteStride = Math.Abs(data.Stride);
                byte[] sourceRow = new byte[absoluteStride];
                byte[] argbRow = new byte[checked(bitmap.Width * 4)];

                using (SHA256 sha = SHA256.Create())
                {
                    for (int y = 0; y < bitmap.Height; y++)
                    {
                        IntPtr rowPointer = IntPtr.Add(data.Scan0, checked(y * data.Stride));
                        Marshal.Copy(rowPointer, sourceRow, 0, absoluteStride);

                        for (int x = 0; x < bitmap.Width; x++)
                        {
                            int sourceIndex = x * 4;
                            int destinationIndex = sourceIndex;
                            byte blue = sourceRow[sourceIndex];
                            byte green = sourceRow[sourceIndex + 1];
                            byte red = sourceRow[sourceIndex + 2];
                            byte alpha = sourceRow[sourceIndex + 3];

                            argbRow[destinationIndex] = alpha;
                            argbRow[destinationIndex + 1] = red;
                            argbRow[destinationIndex + 2] = green;
                            argbRow[destinationIndex + 3] = blue;

                            if (alpha == 0)
                                transparent++;
                            else if (alpha == 255)
                                opaque++;
                            else
                                translucent++;

                            if (alpha < minAlpha)
                                minAlpha = alpha;
                            if (alpha > maxAlpha)
                                maxAlpha = alpha;
                        }

                        sha.TransformBlock(argbRow, 0, argbRow.Length, null, 0);
                    }

                    sha.TransformFinalBlock(new byte[0], 0, 0);
                    return new PixelSummary
                    {
                        ArgbSha256 = ToHex(sha.Hash),
                        TransparentPixels = transparent,
                        TranslucentPixels = translucent,
                        OpaquePixels = opaque,
                        MinAlpha = minAlpha,
                        MaxAlpha = maxAlpha
                    };
                }
            }
            finally
            {
                bitmap.UnlockBits(data);
            }
        }

        private static string HashFile(string path)
        {
            using (FileStream stream = File.OpenRead(path))
            using (SHA256 sha = SHA256.Create())
            {
                return ToHex(sha.ComputeHash(stream));
            }
        }

        private static string ToHex(byte[] bytes)
        {
            char[] result = new char[bytes.Length * 2];
            const string alphabet = "0123456789abcdef";
            for (int i = 0; i < bytes.Length; i++)
            {
                result[i * 2] = alphabet[bytes[i] >> 4];
                result[i * 2 + 1] = alphabet[bytes[i] & 0x0F];
            }
            return new string(result);
        }
    }
}
