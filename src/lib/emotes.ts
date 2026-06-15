// Auto-generated emote hash map
// Each hash maps to a filename in /emotes/
export const EMOTE_MAP: Record<string, string> = {
  "a06e4cf039e8e97b6474ba0720859501660d99950f432c46dbef78c46a42d5ad": "snake_blushing.png",
  "c8e473645fb1dd7d5ef5f76f9ccc7c4bda1f0c30f5f9b86d712e77d56c415061": "snake_calm.png",
  "2bf5f216c4754f476ff2b7781e07d0372664841bed7ec0296b354c9fdb1fd50a": "snake_cool.png",
  "4efba75bfe6f79ede876f458924c9b98bfd5d150ad034498dbf2beaab15791ee": "snake_crying.png",
  "8e13d69d41a7c87f400cad28112333637bdd10b956d3b0b15b14686351eaac25": "snake_dizzy.png",
  "5bef7090d831c0db163a28e562879067d4a517727b6f4c23e5c3d38dd3d3a693": "snake_embarrassed.png",
  "60eb6b00caf2e63bcbf01d9703385354a0d28c3ea5010efaead3d67b2fab9f90": "snake_evil.png",
  "83c480e7692b83cefb6a4ad1563ee4ee1972ab452427b2bc5297941b9e4ea29a": "snake_excited.png",
  "05d5ef81f91ddd6af3630d4e58372d7e1c1c0169b43b260c1af1e0f4544f4d58": "snake_grinning.png",
  "2e4d0ea1e69b605e122edfca0169ca304436c65171b3de48ead78a19f39d44e6": "snake_happy.png",
  "5cab0b8ccf4e2280d1586b54531e5fe95b43734d8dfc917c510cb9a907f2b962": "snake_hiding.png",
  "c2347be1ac9d5ebc7d9d1f9767e322b90a762b322f9b8943fe0ac966d99481c8": "snake_hurt.png",
  "ca07436755d8ce7cc86cedd7ce4138914429ba86ee8d8dbe739d02cb99182339": "snake_idea.png",
  "de84e48b7b24a4a660e50f76089c2fad26e02d0b58f8a43d76e1cf31c442f17c": "snake_laughing.png",
  "dbae14c73e8600504e352734719b3b65f0a3be5eae6636028ab1975d0e73d688": "snake_pointing.png",
  "6d73513880fccb4111c709302f904cbd1d83701e6dccb24f990560865e62bf06": "snake_sad.png",
  "eba6291b012aba53c6b41fc6085797a278d7ba9ff3b997c57b5f69a064a56082": "snake_scared.png",
  "1d2f0f622e4bb23a76d84a95bf5837ffccf1204bba51c293375bcf77874a65b2": "snake_shocked.png",
  "c89cecf71804aa4b98c32e5ba622cac747b359ef25b3b8f9e2232cba5297e1cd": "snake_shy.png",
  "803f999c317be8d6e6a23426ab04480955b0e313cb78f9bd56b9623b9c28ccd4": "snake_smiling.png",
  "64eaf3b144864fd1bdef04c6250c1267f2628694dc1286d451d375047ed4c7e6": "snake_sparkling.png",
  "4ba4b3b3dfe11034579adc6d294330be36fdeee83e6f39676e391f03af1e32c8": "snake_suspicious.png",
  "30e2b75c5b33556dfcf3284742fc33477b6a9cac5795324d7c28fa8859cfdbb1": "snake_tears_of_joy.png",
  "aa1215d06117e6bebcebbaa3819fb4ce88e452e73f881cc5af1f72cf36273a08": "snake_thinking.png",
  "e263e151fb259c1f3e6b6406bcdaf6bcd5602fece9c14efc02cf086e47db6cf0": "snake_upset.png",
  "68999fe682893a81217ef20b0195cb5200144a116979ca39b673b6379e6c109c": "snake_waving.png",
};

// Regex to detect [IMAGE]:hash tags in message content
export const IMAGE_TAG_REGEX = /\[IMAGE\]:([a-f0-9]{64})/g;

export function resolveEmoteUrl(hash: string): string | null {
  const filename = EMOTE_MAP[hash];
  return filename ? `/emotes/${filename}` : null;
}

// Parse message content into segments: text or image
export type MessageSegment =
  | { type: "text"; content: string }
  | { type: "image"; hash: string; url: string };

export function parseMessageContent(content: string): MessageSegment[] {
  const segments: MessageSegment[] = [];
  let lastIndex = 0;
  const regex = /\[IMAGE\]:([a-f0-9]{64})/g;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(content)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ type: "text", content: content.slice(lastIndex, match.index) });
    }
    const hash = match[1];
    const url = resolveEmoteUrl(hash);
    if (url) {
      segments.push({ type: "image", hash, url });
    } else {
      // Unknown hash, render as raw text
      segments.push({ type: "text", content: match[0] });
    }
    lastIndex = regex.lastIndex;
  }

  if (lastIndex < content.length) {
    segments.push({ type: "text", content: content.slice(lastIndex) });
  }

  return segments;
}
