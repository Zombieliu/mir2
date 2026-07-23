export const CRYSTAL_CHAT_WIDTH_PX = 614;
export const CRYSTAL_CHAT_LINE_COUNT = 4;

export const CrystalChatType = Object.freeze({
  Normal: 0,
  Shout: 1,
  System: 2,
  Hint: 3,
  Announcement: 4,
  Group: 5,
  WhisperIn: 6,
  WhisperOut: 7,
  Guild: 8,
  Trainer: 9,
  LevelUp: 10,
  System2: 11,
  Relationship: 12,
  Mentor: 13,
  Shout2: 14,
  Shout3: 15,
  LineMessage: 16,
} as const);

export type CrystalChatType = (typeof CrystalChatType)[keyof typeof CrystalChatType];
export type CrystalArgb = `#${string}`;
export type CrystalTextMeasure = (text: string) => number;

export type CrystalUiLogChannel =
  | "normal"
  | "shout"
  | "whisper"
  | "group"
  | "guild"
  | "mentor"
  | "relationship"
  | "system"
  | "hint"
  | "line"
  | "announcement";

export type CrystalChatStyle = Readonly<{
  ForeColour: CrystalArgb;
  BackColour: CrystalArgb;
  Channel: CrystalUiLogChannel;
}>;

function chatStyle(
  ForeColour: CrystalArgb,
  BackColour: CrystalArgb,
  Channel: CrystalUiLogChannel,
): CrystalChatStyle {
  return Object.freeze({ ForeColour, BackColour, Channel });
}

const NORMAL_STYLE = chatStyle("#FF000000", "#FFFFFFFF", "normal");

export const CRYSTAL_CHAT_STYLES = Object.freeze({
  [CrystalChatType.Normal]: NORMAL_STYLE,
  [CrystalChatType.Shout]: chatStyle("#FF000000", "#FFFFFF00", "shout"),
  [CrystalChatType.System]: chatStyle("#FFFFFFFF", "#FFFF0000", "system"),
  [CrystalChatType.Hint]: chatStyle("#FF006400", "#FFFFFFFF", "hint"),
  [CrystalChatType.Announcement]: chatStyle("#FFFFFFFF", "#FF0000FF", "announcement"),
  [CrystalChatType.Group]: chatStyle("#FFA52A2A", "#FFFFFFFF", "group"),
  [CrystalChatType.WhisperIn]: chatStyle("#FF00008B", "#FFFFFFFF", "whisper"),
  [CrystalChatType.WhisperOut]: chatStyle("#FF6495ED", "#FFFFFFFF", "whisper"),
  [CrystalChatType.Guild]: chatStyle("#FF008000", "#FFFFFFFF", "guild"),
  [CrystalChatType.Trainer]: NORMAL_STYLE,
  [CrystalChatType.LevelUp]: chatStyle("#FF0000FF", "#FFE1B9FA", "announcement"),
  [CrystalChatType.System2]: chatStyle("#FFFFFFFF", "#FF8B0000", "system"),
  [CrystalChatType.Relationship]: chatStyle("#FFFF69B4", "#00000000", "relationship"),
  [CrystalChatType.Mentor]: chatStyle("#FF800080", "#FFFFFFFF", "mentor"),
  [CrystalChatType.Shout2]: chatStyle("#FFFFFFFF", "#FF008000", "shout"),
  [CrystalChatType.Shout3]: chatStyle("#FFFFFFFF", "#FF800080", "shout"),
  [CrystalChatType.LineMessage]: chatStyle("#FFFFFFFF", "#FF0000FF", "line"),
} satisfies Record<CrystalChatType, CrystalChatStyle>);

export type CrystalChatFilterState = Readonly<{
  FilterNormalChat: boolean;
  FilterWhisperChat: boolean;
  FilterShoutChat: boolean;
  FilterSystemChat: boolean;
  FilterGroupChat: boolean;
  FilterGuildChat: boolean;
}>;

export const DEFAULT_CRYSTAL_CHAT_FILTERS: CrystalChatFilterState = Object.freeze({
  FilterNormalChat: false,
  FilterWhisperChat: false,
  FilterShoutChat: false,
  FilterSystemChat: false,
  FilterGroupChat: false,
  FilterGuildChat: false,
});

export type CrystalChatLine = Readonly<{
  Text: string;
  Type: CrystalChatType;
  ForeColour: CrystalArgb;
  BackColour: CrystalArgb;
  Channel: CrystalUiLogChannel;
}>;

type ItemLinkMatch = Readonly<{
  index: number;
  length: number;
}>;

function csharpSubstring(text: string, start: number, length: number): string {
  if (
    !Number.isInteger(start) ||
    !Number.isInteger(length) ||
    start < 0 ||
    length < 0 ||
    start + length > text.length
  ) {
    throw new RangeError(`Invalid C# Substring range: start=${start}, length=${length}`);
  }

  return text.slice(start, start + length);
}

function itemLinkMatches(text: string): ItemLinkMatch[] {
  const matches: ItemLinkMatch[] = [];
  const pattern = /<(.*?\/.*?)>/g;

  for (const match of text.matchAll(pattern)) {
    matches.push({ index: match.index, length: match[0].length });
  }

  return matches;
}

function measuredWidth(measure: CrystalTextMeasure, text: string): number {
  const width = measure(text);
  if (!Number.isFinite(width) || width < 0) {
    throw new RangeError(`Crystal text measure must return a finite non-negative width, got ${width}`);
  }

  return width;
}

export function wrapCrystalChatText(text: string, measure: CrystalTextMeasure): string[] {
  if (typeof measure !== "function") {
    throw new TypeError("Crystal chat wrapping requires an injected text measure function");
  }

  const chat: string[] = [];
  let index = 0;

  // Keep the original offset/newIndex behavior, including its relative item-link index semantics.
  for (let i = 1; i < text.length; i += 1) {
    if (i - index < 0) continue;

    const candidate = csharpSubstring(text, index, i - index);
    if (measuredWidth(measure, candidate) <= CRYSTAL_CHAT_WIDTH_PX) continue;

    let offset = i - index;
    let newIndex = i - 1;
    const overlappingLinks = itemLinkMatches(csharpSubstring(text, index, text.length - index)).filter(
      (match) => match.index < i - index && match.index + match.length > offset - 1,
    );

    if (overlappingLinks.length > 1) {
      throw new Error("Crystal ChatItemLinks SingleOrDefault matched more than one item link");
    }

    const overlappingLink = overlappingLinks[0];
    if (overlappingLink) {
      offset = overlappingLink.index;
      newIndex = overlappingLink.index;
    }

    chat.push(csharpSubstring(text, index, offset - 1));
    index = newIndex;
  }

  chat.push(csharpSubstring(text, index, text.length - index));
  return chat;
}

export function crystalChatStyle(type: CrystalChatType): CrystalChatStyle {
  return CRYSTAL_CHAT_STYLES[type] ?? NORMAL_STYLE;
}

function filteredByCrystalSettings(type: CrystalChatType, filters: CrystalChatFilterState): boolean {
  switch (type) {
    case CrystalChatType.Normal:
    case CrystalChatType.LineMessage:
      return filters.FilterNormalChat;
    case CrystalChatType.WhisperIn:
    case CrystalChatType.WhisperOut:
      return filters.FilterWhisperChat;
    case CrystalChatType.Shout:
    case CrystalChatType.Shout2:
    case CrystalChatType.Shout3:
      return filters.FilterShoutChat;
    case CrystalChatType.System:
    case CrystalChatType.System2:
      return filters.FilterSystemChat;
    case CrystalChatType.Group:
      return filters.FilterGroupChat;
    case CrystalChatType.Guild:
      return filters.FilterGuildChat;
    default:
      return false;
  }
}

export class CrystalChatHistory {
  readonly LineCount = CRYSTAL_CHAT_LINE_COUNT;
  FullHistory: CrystalChatLine[] = [];
  History: CrystalChatLine[] = [];
  StartIndex = 0;

  private filters: CrystalChatFilterState;

  constructor(
    private readonly measure: CrystalTextMeasure,
    filters: Partial<CrystalChatFilterState> = {},
  ) {
    if (typeof measure !== "function") {
      throw new TypeError("CrystalChatHistory requires an injected text measure function");
    }

    this.filters = Object.freeze({ ...DEFAULT_CRYSTAL_CHAT_FILTERS, ...filters });
  }

  get Filters(): CrystalChatFilterState {
    return this.filters;
  }

  get VisibleHistory(): CrystalChatLine[] {
    return this.History.slice(this.StartIndex, this.StartIndex + this.LineCount);
  }

  receiveChat(text: string, type: CrystalChatType): CrystalChatLine[] {
    const wrappedText = wrapCrystalChatText(text, this.measure);
    const style = crystalChatStyle(type);

    if (this.StartIndex === this.History.length - this.LineCount) {
      this.StartIndex += wrappedText.length;
    }

    const addedLines = wrappedText.map((Text) =>
      Object.freeze({
        Text,
        Type: type,
        ForeColour: style.ForeColour,
        BackColour: style.BackColour,
        Channel: style.Channel,
      }),
    );

    this.FullHistory.push(...addedLines);
    this.update();
    return addedLines;
  }

  setFilters(filters: Partial<CrystalChatFilterState>): void {
    this.filters = Object.freeze({ ...this.filters, ...filters });
    this.update();
  }

  update(): void {
    this.History = this.FullHistory.filter((line) => !filteredByCrystalSettings(line.Type, this.filters));

    if (this.StartIndex >= this.History.length) this.StartIndex = this.History.length - 1;
    if (this.StartIndex < 0) this.StartIndex = 0;
  }

  home(): void {
    if (this.StartIndex === 0) return;
    this.StartIndex = 0;
    this.update();
  }

  up(): void {
    if (this.StartIndex === 0) return;
    this.StartIndex -= 1;
    this.update();
  }

  down(): void {
    if (this.StartIndex === this.History.length - 1) return;
    this.StartIndex += 1;
    this.update();
  }

  end(): void {
    if (this.StartIndex === this.History.length - 1) return;
    this.StartIndex = this.History.length - 1;
    this.update();
  }
}
