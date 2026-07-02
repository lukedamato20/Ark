import { invoke } from "@tauri-apps/api/core";
import type {
  AppBootstrap,
  BranchAlternative,
  BuiltInRuntimeStatus,
  Conversation,
  DiagnosticsResult,
  Message,
  ProviderConfig,
  RefreshModelsResult,
  SendChatResult,
  WorkspaceInfo,
} from "../types/ark";

export function getAppBootstrap() {
  return invoke<AppBootstrap>("get_app_bootstrap");
}

export function createConversation(title?: string) {
  return invoke<Conversation>("create_conversation", { title });
}

export function listConversations() {
  return invoke<Conversation[]>("list_conversations");
}

export function renameConversation(id: string, title: string) {
  return invoke<Conversation>("rename_conversation", { request: { id, title } });
}

export function setThemePreference(theme: "dark" | "light") {
  return invoke<"dark" | "light">("set_theme", { request: { theme } });
}

export function setWorkspace(rootPath: string) {
  return invoke<WorkspaceInfo>("set_workspace", { request: { rootPath } });
}

export function resetWorkspace() {
  return invoke<WorkspaceInfo>("reset_workspace");
}

export function deleteConversation(id: string) {
  return invoke<void>("delete_conversation", { id });
}

export function getConversationMessages(conversationId: string) {
  return invoke<Message[]>("get_conversation_messages", { conversationId });
}

export function getAssistantAlternatives(conversationId: string, messageId: string) {
  return invoke<BranchAlternative[]>("get_assistant_alternatives", {
    request: { conversationId, messageId },
  });
}

export function switchActiveBranch(conversationId: string, messageId: string) {
  return invoke<Message[]>("switch_active_branch", {
    request: { conversationId, messageId },
  });
}

export function sendChatMessage(input: {
  conversationId: string;
  content: string;
  providerId: string;
  model: string;
  temperature?: number | null;
  maxTokens?: number | null;
}) {
  return invoke<SendChatResult>("send_chat_message", {
    request: {
      conversationId: input.conversationId,
      content: input.content,
      providerId: input.providerId,
      model: input.model,
      temperature: input.temperature ?? undefined,
      maxTokens: input.maxTokens ?? undefined,
    },
  });
}

export function editUserMessage(input: {
  conversationId: string;
  messageId: string;
  content: string;
  providerId: string;
  model: string;
  temperature?: number | null;
  maxTokens?: number | null;
}) {
  return invoke<SendChatResult>("edit_user_message", {
    request: {
      conversationId: input.conversationId,
      messageId: input.messageId,
      content: input.content,
      providerId: input.providerId,
      model: input.model,
      temperature: input.temperature ?? undefined,
      maxTokens: input.maxTokens ?? undefined,
    },
  });
}

export function regenerateAssistantMessage(input: {
  conversationId: string;
  messageId: string;
  providerId: string;
  model: string;
  temperature?: number | null;
  maxTokens?: number | null;
}) {
  return invoke<SendChatResult>("regenerate_assistant_message", {
    request: {
      conversationId: input.conversationId,
      messageId: input.messageId,
      providerId: input.providerId,
      model: input.model,
      temperature: input.temperature ?? undefined,
      maxTokens: input.maxTokens ?? undefined,
    },
  });
}

export function cancelStream(messageId: string) {
  return invoke<void>("cancel_stream", { messageId });
}

export function refreshModels(providerId: string) {
  return invoke<RefreshModelsResult>("refresh_models", { providerId });
}

export function updateProvider(input: {
  providerId: string;
  baseUrl: string;
  defaultModelId?: string | null;
  temperature?: number | null;
  maxTokens?: number | null;
  streamingEnabled: boolean;
}) {
  return invoke<ProviderConfig>("update_provider", {
    request: {
      providerId: input.providerId,
      baseUrl: input.baseUrl,
      defaultModelId: input.defaultModelId ?? null,
      temperature: input.temperature ?? null,
      maxTokens: input.maxTokens ?? null,
      streamingEnabled: input.streamingEnabled,
    },
  });
}

export function runDiagnostics(providerId: string, model?: string | null) {
  return invoke<DiagnosticsResult>("run_diagnostics", {
    providerId,
    model: model ?? null,
  });
}

export function exportConversationMarkdown(conversationId: string) {
  return invoke<string>("export_conversation_markdown", { conversationId });
}

export function exportConversationJson(conversationId: string) {
  return invoke<string>("export_conversation_json", { conversationId });
}

export function importConversationJson(json: string) {
  return invoke<Conversation>("import_conversation_json", { json });
}

export function getBuiltInRuntimeStatus() {
  return invoke<BuiltInRuntimeStatus>("get_built_in_runtime_status");
}

export function startBuiltInRuntime(modelPath: string) {
  return invoke<BuiltInRuntimeStatus>("start_built_in_runtime", { modelPath });
}

export function stopBuiltInRuntime() {
  return invoke<void>("stop_built_in_runtime");
}

export function getErrorMessage(error: unknown) {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object" && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }

  return "Unexpected Ark error.";
}
