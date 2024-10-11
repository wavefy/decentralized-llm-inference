import ChatPage from "@/components/chat/chat-page";
import { controlBasePath } from "@/lib/utils";
import { createLazyFileRoute } from "@tanstack/react-router";
import { useState } from "react";

export const Route = createLazyFileRoute("/")({
  component: Index,
});

function Index() {
  const [chatId, setChatId] = useState<string>("");
  if (!controlBasePath) {
    throw new Error("VLLM_CONTROLS_URL is not defined");
  }

  return <ChatPage chatId={chatId} setChatId={setChatId} />;
}
