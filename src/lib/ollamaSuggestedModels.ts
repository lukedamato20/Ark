/**
 * UX-011: a small, hand-picked, bundled, offline list of well-known Ollama tags — not a live
 * remote catalogue fetch. Ark's model picker filters this list as the user types, with a
 * free-text fallback for any tag not covered here (Ollama supports far more than any curated
 * list could track). Sizes are approximate published download sizes for the tag's default
 * quantization and are used only for the disk-space warning heuristic, never as an exact figure
 * shown as fact.
 */
export interface SuggestedOllamaModel {
  /** The exact tag passed to `ollama pull`, e.g. "llama3.2:3b". */
  name: string;
  label: string;
  description: string;
  approxSizeGb: number;
}

export const SUGGESTED_OLLAMA_MODELS: SuggestedOllamaModel[] = [
  {
    name: "llama3.2:1b",
    label: "Llama 3.2 1B",
    description: "Meta's smallest current model — fastest, lowest memory, least capable.",
    approxSizeGb: 1.3,
  },
  {
    name: "llama3.2:3b",
    label: "Llama 3.2 3B",
    description: "A good balance of speed and quality for everyday chat on modest hardware.",
    approxSizeGb: 2.0,
  },
  {
    name: "llama3.1:8b",
    label: "Llama 3.1 8B",
    description: "Meta's general-purpose mid-size model — stronger reasoning than the 3B tier.",
    approxSizeGb: 4.7,
  },
  {
    name: "mistral:7b",
    label: "Mistral 7B",
    description: "A well-rounded general-purpose model from Mistral AI.",
    approxSizeGb: 4.1,
  },
  {
    name: "gemma2:2b",
    label: "Gemma 2 2B",
    description: "Google's small, efficient model — quick responses, light footprint.",
    approxSizeGb: 1.6,
  },
  {
    name: "gemma2:9b",
    label: "Gemma 2 9B",
    description: "Google's mid-size model, stronger quality at a larger download.",
    approxSizeGb: 5.4,
  },
  {
    name: "phi3:mini",
    label: "Phi-3 Mini",
    description: "Microsoft's compact model, tuned for efficient reasoning on limited hardware.",
    approxSizeGb: 2.2,
  },
  {
    name: "qwen2.5:0.5b",
    label: "Qwen 2.5 0.5B",
    description: "A very small model for quick, low-resource tasks.",
    approxSizeGb: 0.4,
  },
  {
    name: "qwen2.5:7b",
    label: "Qwen 2.5 7B",
    description: "Alibaba's general-purpose mid-size model with strong multilingual support.",
    approxSizeGb: 4.7,
  },
  {
    name: "codellama:7b",
    label: "Code Llama 7B",
    description: "Meta's model specialized for code generation and completion.",
    approxSizeGb: 3.8,
  },
  {
    name: "deepseek-coder-v2:16b",
    label: "DeepSeek Coder V2 16B",
    description: "A larger model specialized for coding tasks — higher quality, larger download.",
    approxSizeGb: 8.9,
  },
  {
    name: "llava:7b",
    label: "LLaVA 7B",
    description: "A vision-capable model that can describe and answer questions about images.",
    approxSizeGb: 4.7,
  },
  {
    name: "nomic-embed-text",
    label: "Nomic Embed Text",
    description: "A compact text-embedding model, not a chat model.",
    approxSizeGb: 0.27,
  },
  {
    name: "tinyllama",
    label: "TinyLlama",
    description: "An extremely small model for testing on very limited hardware.",
    approxSizeGb: 0.6,
  },
];
